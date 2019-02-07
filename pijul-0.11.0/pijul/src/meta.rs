use commands::remote::{parse_remote, Remote};
use dirs;
use libpijul::fs_representation::meta_file;
use std;
use std::collections::BTreeMap;
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thrussh_keys;
use thrussh_keys::key::KeyPair;
use toml;
use error::Error;

pub const DEFAULT_REMOTE: &'static str = "remote";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Repository {
    pub address: String,
    pub port: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Meta {
    #[serde(default)]
    pub authors: Vec<String>,
    pub signing_key: Option<String>,
    pub editor: Option<String>,
    pub pull: Option<String>,
    pub push: Option<String>,
    #[serde(default)]
    pub remote: BTreeMap<String, Repository>,
}

impl Meta {
    pub fn load(r: &Path) -> Result<Meta, Error> {
        let mut str = String::new();
        {
            let mut f = File::open(meta_file(r))?;
            f.read_to_string(&mut str)?;
        }
        Ok(toml::from_str(&str)?)
    }
    pub fn new() -> Meta {
        Meta {
            authors: Vec::new(),
            signing_key: None,
            editor: None,
            pull: None,
            push: None,
            remote: BTreeMap::new(),
        }
    }
    pub fn save(&self, r: &Path) -> Result<(), Error> {
        let mut f = File::create(meta_file(r))?;
        let s: String = toml::to_string(&self)?;
        f.write_all(s.as_bytes())?;
        Ok(())
    }

    fn parse_remote<'a>(
        &'a self,
        remote: &'a str,
        port: Option<u16>,
        base_path: Option<&'a Path>,
        local_repo_root: Option<&'a Path>,
    ) -> Remote<'a> {
        if let Some(repo) = self.remote.get(remote) {
            parse_remote(
                &repo.address,
                port.or(repo.port),
                base_path,
                local_repo_root,
            )
        } else {
            parse_remote(remote, port, base_path, local_repo_root)
        }
    }

    fn get_remote<'a>(
        &'a self,
        remote: Option<&'a str>,
        default_remote: Option<&'a String>,
        port: Option<u16>,
        base_path: Option<&'a Path>,
        local_repo_root: Option<&'a Path>,
    ) -> Result<Remote<'a>, Error> {
        if let Some(remote) = remote {
            Ok(self.parse_remote(remote, port, base_path, local_repo_root))
        } else if let Some(ref remote) = default_remote {
            Ok(self.parse_remote(remote, port, base_path, local_repo_root))
        } else if self.remote.len() == 1 {
            let remote = self.remote.keys().next().unwrap();
            Ok(self.parse_remote(remote, port, base_path, local_repo_root))
        } else {
            Err(Error::MissingRemoteRepository)
        }
    }

    pub fn pull<'a>(
        &'a self,
        remote: Option<&'a str>,
        port: Option<u16>,
        base_path: Option<&'a Path>,
        local_repo_root: Option<&'a Path>,
    ) -> Result<Remote<'a>, Error> {
        self.get_remote(remote, self.pull.as_ref(), port, base_path, local_repo_root)
    }

    pub fn push<'a>(
        &'a self,
        remote: Option<&'a str>,
        port: Option<u16>,
        base_path: Option<&'a Path>,
        local_repo_root: Option<&'a Path>,
    ) -> Result<Remote<'a>, Error> {
        self.get_remote(remote, self.push.as_ref(), port, base_path, local_repo_root)
    }

    pub fn signing_key(&self) -> Result<Option<KeyPair>, Error> {
        if let Some(ref path) = self.signing_key {
            Ok(Some(thrussh_keys::load_secret_key(path, None)?))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Global {
    pub author: String,
    pub editor: Option<String>,
    pub signing_key: Option<String>,
}

pub fn global_path() -> Result<PathBuf, Error> {
    if let Ok(var) = std::env::var("PIJUL_CONFIG_DIR") {
        let mut path = PathBuf::new();
        path.push(var);
        Ok(path)
    } else if let Ok(var) = std::env::var("XDG_DATA_HOME") {
        let mut path = PathBuf::new();
        path.push(var);
        path.push("pijul");
        std::fs::create_dir_all(&path)?;
        path.push("config");
        Ok(path)
    } else {
        if let Some(mut path) = dirs::home_dir() {
            path.push(".pijulconfig");
            Ok(path)
        } else {
            Err(Error::NoHomeDir)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    SSH,
    Signing,
}

pub fn generate_key<P: AsRef<Path>>(
    dot_pijul: P,
    password: Option<(u32, &[u8])>,
    keytype: KeyType,
) -> Result<(), Error> {
    use thrussh_keys::{encode_pkcs8_pem, encode_pkcs8_pem_encrypted, write_public_key_base64};
    let key = KeyPair::generate_ed25519().unwrap();
    create_dir_all(dot_pijul.as_ref())?;

    let mut f = dot_pijul.as_ref().join(match keytype {
        KeyType::SSH => "id_ed25519",
        KeyType::Signing => "sig_ed25519",
    });
    debug!("generate_key: {:?}", f);
    if std::fs::metadata(&f).is_err() {
        let mut f = File::create(&f)?;
        if let Some((rounds, pass)) = password {
            encode_pkcs8_pem_encrypted(&key, pass, rounds, &mut f)?
        } else {
            encode_pkcs8_pem(&key, &mut f)?
        }
        f.flush().unwrap();
    } else {
        return Err(Error::WillNotOverwriteKeyFile { path: f });
    }
    f.set_extension("pub");
    {
        let mut f = File::create(&f)?;
        let pk = key.clone_public_key();
        write_public_key_base64(&mut f, &pk)?;
        f.write(b"\n")?;
        f.flush()?;
    }
    Ok(())
}

pub fn load_key<P: AsRef<Path>>(dot_pijul: P, keytype: KeyType) -> Result<KeyPair, Error> {
    let f = dot_pijul.as_ref().join(match keytype {
        KeyType::SSH => "id_ed25519",
        KeyType::Signing => "sig_ed25519",
    });
    debug!("load_key: {:?}", f);
    Ok(thrussh_keys::load_secret_key(&f, None)?)
}

pub fn generate_global_key(keytype: KeyType) -> Result<(), Error> {
    generate_key(&global_path()?, None, keytype)
}

pub fn load_global_or_local_signing_key<P: AsRef<Path>>(dot_pijul: Option<P>) -> Result<KeyPair, Error> {
    if let Some(dot_pijul) = dot_pijul {
        if let Ok(key) = load_key(dot_pijul.as_ref(), KeyType::Signing) {
            return Ok(key);
        }
    }
    load_key(&global_path()?, KeyType::Signing)
}

impl Global {
    pub fn new() -> Self {
        Global {
            author: String::new(),
            editor: None,
            signing_key: None,
        }
    }

    pub fn load() -> Result<Self, Error> {
        let mut path = global_path()?;
        path.push("config.toml");
        let mut str = String::new();
        {
            let mut f = File::open(&path)?;
            f.read_to_string(&mut str)?;
        }
        Ok(toml::from_str(&str)?)
    }

    pub fn save(&self) -> Result<(), Error> {
        let mut path = global_path()?;
        create_dir_all(&path)?;
        path.push("config.toml");
        let mut f = File::create(&path)?;
        let s: String = toml::to_string(&self)?;
        f.write_all(s.as_bytes())?;
        Ok(())
    }
}
