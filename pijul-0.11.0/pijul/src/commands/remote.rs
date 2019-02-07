use libpijul::fs_representation::{
    branch_changes_base_path, patch_file_name, patches_dir, pristine_dir, PIJUL_DIR_NAME,
};
use libpijul::patch::read_changes;
use libpijul::{
    apply_resize, apply_resize_no_output, apply_resize_patches, apply_resize_patches_no_output,
    ApplyTimestamp, Hash, Patch, PatchId, Repository,
};
use regex::Regex;
use reqwest;
use reqwest::async as reqwest_async;

use error::Error;
use std;
use std::collections::hash_set::HashSet;
use std::collections::HashMap;
use std::fs::{copy, hard_link, metadata, rename, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use commands::{ask, assert_no_containing_repo, create_repo};
use cryptovec;
use dirs;
use futures;
use futures::{Async, Future, Poll, Stream};
use progrs;
use shell_escape::unix::escape;
use std::borrow::Cow;
use std::io::prelude::*;
use std::io::BufReader;
use std::net::ToSocketAddrs;
use tempdir::TempDir;
use thrussh;
use thrussh_config;
use thrussh_keys;
use tokio;
use username;

use super::get_current_branch;

#[derive(Debug)]
pub enum Remote<'a> {
    Ssh {
        user: Option<&'a str>,
        host: &'a str,
        port: Option<u16>,
        path: &'a str,
        id: &'a str,
        local_repo_root: Option<&'a Path>,
        pijul_cmd: Cow<'static, str>,
    },
    Uri {
        uri: &'a str,
    },
    Local {
        path: PathBuf,
    },
}

pub enum Session<'a> {
    Ssh(SshSession<'a>),
    Uri(UriSession<'a>),
    Local(LocalSession<'a>),
}

pub struct SshSession<'a> {
    l: tokio::runtime::Runtime,
    path: &'a str,
    pijul_cmd: &'a str,
    session: Option<thrussh::client::Connection<thrussh_config::Stream, Client>>,
}

pub struct UriSession<'a> {
    l: tokio::runtime::Runtime,
    uri: &'a str,
    client: reqwest_async::Client,
}

pub struct LocalSession<'a> {
    path: &'a Path,
}

impl<'a> Drop for SshSession<'a> {
    fn drop(&mut self) {
        if let Some(mut session) = self.session.take() {
            debug!("disconnecting");
            session.disconnect(thrussh::Disconnect::ByApplication, "finished", "EN");
            if let Err(e) = self.l.block_on(session) {
                error!("While dropping SSH Session: {:?}", e);
            }
        }
    }
}

#[cfg(unix)]
use thrussh_keys::agent::client::AgentClient;
#[cfg(unix)]
use tokio_uds::UnixStream;

pub struct Client {
    exit_status: HashMap<thrussh::ChannelId, u32>,
    state: State,
    host: String,
    port: u16,
    channel: Option<thrussh::ChannelId>,
    #[cfg(unix)]
    agent: Option<AgentClient<UnixStream>>,
    #[cfg(windows)]
    agent: Option<()>,
}

impl Client {
    #[cfg(unix)]
    fn new(port: Option<u16>, host: &str, l: &mut tokio::runtime::Runtime) -> Self {
        let agent = if let Ok(path) = std::env::var("SSH_AUTH_SOCK") {
            l.block_on(
                UnixStream::connect(path).map(thrussh_keys::agent::client::AgentClient::connect),
            ).ok()
        } else {
            None
        };
        debug!("Client::new(), agent: {:?}", agent.is_some());
        Client {
            exit_status: HashMap::new(),
            state: State::None,
            port: port.unwrap_or(22),
            host: host.to_string(),
            channel: None,
            agent,
        }
    }

    #[cfg(windows)]
    fn new(port: Option<u16>, host: &str, _: &mut tokio::runtime::Runtime) -> Self {
        Client {
            exit_status: HashMap::new(),
            state: State::None,
            port: port.unwrap_or(22),
            host: host.to_string(),
            channel: None,
            agent: None,
        }
    }
}

enum State {
    None,
    Changes {
        changes: HashMap<Hash, ApplyTimestamp>,
    },
    DownloadPatch {
        file: File,
    },
}

enum SendFileState {
    Read(thrussh::client::Connection<thrussh_config::Stream, Client>),
    Wait(thrussh::client::Data<thrussh_config::Stream, Client, Vec<u8>>),
}

struct SendFile {
    f: File,
    buf: Option<Vec<u8>>,
    chan: thrussh::ChannelId,
    state: Option<SendFileState>,
}

impl Future for SendFile {
    type Item = (
        thrussh::client::Connection<thrussh_config::Stream, Client>,
        Vec<u8>,
    );
    type Error = Error;
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        debug!("SendFile loop starting");
        loop {
            debug!("sendfile loop");
            match self.state.take() {
                Some(SendFileState::Read(c)) => {
                    debug!("read");
                    let mut buf = self.buf.take().unwrap();
                    buf.resize(BUFFER_SIZE, 0);
                    let len = self.f.read(&mut buf)?;
                    if len == 0 {
                        // If nothing has been read, return.
                        return Ok(Async::Ready((c, buf)));
                    }
                    buf.truncate(len);
                    debug!("sending {:?} bytes, {:?}", len, buf.len());
                    self.state = Some(SendFileState::Wait(c.data(self.chan, None, buf)));
                }
                Some(SendFileState::Wait(mut c)) => {
                    debug!("wait");
                    match c.poll()? {
                        Async::Ready((c, buf)) => {
                            self.buf = Some(buf);
                            self.state = Some(SendFileState::Read(c))
                        }
                        Async::NotReady => {
                            self.state = Some(SendFileState::Wait(c));
                            return Ok(Async::NotReady);
                        }
                    }
                }
                None => unreachable!(),
            }
        }
    }
}

impl thrussh::client::Handler for Client {
    type Error = Error;
    type FutureUnit = futures::Finished<Client, Error>;
    type SessionUnit = futures::Finished<(Client, thrussh::client::Session), Error>;
    type FutureBool = futures::future::FutureResult<(Client, bool), Error>;
    type FutureSign =
        Box<futures::Future<Item = (Self, cryptovec::CryptoVec), Error = Self::Error>>;

    #[cfg(unix)]
    fn auth_publickey_sign(
        mut self,
        key: &thrussh_keys::key::PublicKey,
        mut to_sign: cryptovec::CryptoVec,
    ) -> Self::FutureSign {
        debug!("auth_publickey_sign");
        if let Some(agent) = self.agent.take() {
            use thrussh_keys::encoding::Encoding;
            debug!("using agent");
            Box::new(
                agent
                    .sign_request(key, &to_sign)
                    .and_then(move |(client, sig)| {
                        debug!("sig = {:?}", sig);
                        if let Some(sig) = sig {
                            to_sign.extend_ssh_string(&sig[..]);
                        }
                        self.agent = Some(client);
                        futures::finished((self, to_sign))
                    }).from_err(),
            )
        } else {
            debug!("no agent");
            Box::new(futures::finished((self, to_sign)))
        }
    }

    fn data(
        mut self,
        channel: thrussh::ChannelId,
        stream: Option<u32>,
        data: &[u8],
        session: thrussh::client::Session,
    ) -> Self::SessionUnit {
        debug!(
            "data ({:?}): {:?}",
            channel,
            &data[..std::cmp::min(data.len(), 100)]
        );
        if stream == Some(1) {
            std::io::stderr().write(data).unwrap();
        } else if stream == None {
            match self.state {
                State::None => {
                    std::io::stdout().write(data).unwrap();
                }
                State::Changes { ref mut changes } => {
                    let data = std::str::from_utf8(data).unwrap();
                    for l in data.lines() {
                        let mut spl = l.split(':');
                        if let (Some(h), Some(s)) = (spl.next(), spl.next()) {
                            if let (Some(h), Ok(s)) = (Hash::from_base58(h), s.parse()) {
                                changes.insert(h, s);
                            }
                        }
                    }
                }
                State::DownloadPatch { ref mut file, .. } => {
                    file.write_all(data).unwrap();
                }
            }
        } else {
            debug!(
                "SSH data received on channel {:?}: {:?} {:?}",
                channel, stream, data
            );
        }
        futures::finished((self, session))
    }
    fn exit_status(
        mut self,
        channel: thrussh::ChannelId,
        exit_status: u32,
        session: thrussh::client::Session,
    ) -> Self::SessionUnit {
        debug!(
            "exit_status received on channel {:?}: {:?}:",
            channel, exit_status
        );
        debug!("self.channel = {:?}", self.channel);
        if let Some(c) = self.channel {
            if channel == c {
                self.exit_status.insert(channel, exit_status);
            }
        }
        debug!("self.exit_status = {:?}", self.exit_status);
        futures::finished((self, session))
    }

    fn check_server_key(
        self,
        server_public_key: &thrussh_keys::key::PublicKey,
    ) -> Self::FutureBool {
        let path = dirs::home_dir().unwrap().join(".ssh").join("known_hosts");
        match thrussh_keys::check_known_hosts_path(&self.host, self.port, server_public_key, &path)
        {
            Ok(true) => futures::done(Ok((self, true))),
            Ok(false) => {
                if let Ok(false) = ask::ask_learn_ssh(&self.host, self.port, "") {
                    // TODO
                    // &server_public_key.fingerprint()) {

                    futures::done(Ok((self, false)))
                } else {
                    thrussh_keys::learn_known_hosts_path(
                        &self.host,
                        self.port,
                        server_public_key,
                        &path,
                    ).unwrap();
                    futures::done(Ok((self, true)))
                }
            }
            Err(e) => {
                if let thrussh_keys::Error::KeyChanged(line) = e {
                    println!(
                        "Host key changed! Someone might be eavesdropping this communication, \
                         refusing to continue. Previous key found line {}",
                        line
                    );
                    futures::done(Ok((self, false)))
                } else {
                    futures::done(Err(From::from(e)))
                }
            }
        }
    }
}

const BUFFER_SIZE: usize = 1 << 14; // 16 kb.

impl<'a> SshSession<'a> {
    pub fn changes(
        &mut self,
        branch: &str,
        path: &[&str],
    ) -> Result<HashMap<Hash, ApplyTimestamp>, Error> {
        let esc_path = escape(Cow::Borrowed(self.path));
        let mut cmd = format!(
            "{} log --repository {} --branch {:?} --hash-only",
            self.pijul_cmd, esc_path, branch
        );
        for p in path {
            cmd.push_str(&format!(" --path {}", p))
        }

        if let Some(ref mut session) = self.session {
            session.handler_mut().state = State::Changes {
                changes: HashMap::new(),
            }
        }
        let mut channel = None;
        self.session = Some(
            self.l
                .block_on(
                    self.session
                        .take()
                        .unwrap()
                        .channel_open_session()
                        .and_then(move |(mut connection, chan)| {
                            debug!("exec: {:?}", cmd);
                            channel = Some(chan);
                            connection.handler_mut().exit_status.remove(&chan);
                            connection.handler_mut().channel = Some(chan);
                            connection.exec(chan, false, &cmd);
                            connection.channel_eof(chan);
                            // Wait until channel close.
                            debug!("waiting channel close");
                            connection
                                .wait(move |session| {
                                    session.handler().exit_status.get(&chan).is_some()
                                }).and_then(move |mut session| {
                                    if session.is_channel_open(chan) {
                                        session.channel_close(chan);
                                    }
                                    session.wait(move |session| !session.is_channel_open(chan))
                                })
                        }),
                ).unwrap(),
        );

        if let Some(ref session) = self.session {
            if let Some(channel) = channel {
                if let Some(&exit_code) = session.handler().exit_status.get(&channel) {
                    debug!("exit_code = {:?}", exit_code);
                    if exit_code != 0 {
                        return Ok(HashMap::new());
                    }
                }
            }
        }
        if let Some(ref mut session) = self.session {
            match std::mem::replace(&mut session.handler_mut().state, State::None) {
                State::Changes { changes } => {
                    debug!("changes: {:?}", changes);
                    Ok(changes)
                }
                _ => unreachable!(),
            }
        } else {
            unreachable!()
        }
    }

    pub fn fetch_patch(
        &mut self,
        patch_hash: &Hash,
        local_file: PathBuf,
        local_tmp_file: PathBuf,
    ) -> Result<PathBuf, Error> {
        let esc_path = escape(Cow::Borrowed(self.path));
        let cmd = format!(
            "{} patch --repository {} --bin {}",
            self.pijul_cmd,
            esc_path,
            patch_hash.to_base58()
        );
        debug!("cmd {:?} {:?}", cmd, local_file);
        if let Some(ref mut session) = self.session {
            session.handler_mut().state = State::DownloadPatch {
                file: File::create(&local_tmp_file)?,
            };
            session.handler_mut().channel = None;
        }
        self.session = Some(
            self.l
                .block_on(
                    self.session
                        .take()
                        .unwrap()
                        .channel_open_session()
                        .and_then(move |(mut connection, chan)| {
                            connection.handler_mut().exit_status.remove(&chan);
                            connection.handler_mut().channel = Some(chan);
                            connection.exec(chan, false, &cmd);
                            connection.channel_eof(chan);
                            connection
                                .wait(move |session| {
                                    session.handler().exit_status.get(&chan).is_some()
                                }).and_then(move |mut session| {
                                    if session.is_channel_open(chan) {
                                        session.channel_close(chan);
                                    }
                                    session.wait(move |session| !session.is_channel_open(chan))
                                })
                        }),
                ).unwrap(),
        );

        if let Some(ref mut session) = self.session {
            if let State::DownloadPatch { mut file, .. } =
                std::mem::replace(&mut session.handler_mut().state, State::None)
            {
                file.flush()?;
                rename(&local_tmp_file, &local_file)?;
            }
        }
        Ok(local_file)
    }

    pub fn remote_apply(
        &mut self,
        repo_root: &Path,
        remote_branch: &str,
        patch_hashes: HashSet<Hash>,
    ) -> Result<(), Error> {
        let pdir = patches_dir(repo_root);
        let mut exit_status = None;
        let esc_path = escape(Cow::Borrowed(&self.path));
        let apply_cmd = format!(
            "{} apply --repository {} --branch {:?}",
            self.pijul_cmd, esc_path, remote_branch
        );
        let sign_cmd = format!(
            "{} sign --repository {}",
            self.pijul_cmd, esc_path
        );

        let session = self.session.take().unwrap();
        self.session = Some(self.l.block_on(
            session.channel_open_session()
                .and_then(move |(session, chan0)| session.channel_open_session().and_then(move |(mut session, chan1)| {
                    session.handler_mut().exit_status.remove(&chan0);
                    session.handler_mut().channel = Some(chan0);
                    debug!("exec {:?}", apply_cmd);
                    session.exec(chan0, false, &apply_cmd);
                    debug!("exec {:?}", sign_cmd);
                    session.exec(chan1, false, &sign_cmd);
                    futures::stream::iter_ok(patch_hashes.into_iter())
                        .fold((session, Vec::new()), move |(session, buf), hash| {
                            let mut pdir = pdir.clone();
                            pdir.push(hash.to_base58());
                            pdir.set_extension("gz");
                            let f = std::fs::File::open(&pdir).unwrap();
                            pdir.set_extension("sig");
                            if let Ok(sig) = std::fs::File::open(&pdir) {
                                futures::future::Either::A((SendFile {
                                    f: f,
                                    buf: Some(buf),
                                    chan: chan0,
                                    state: Some(SendFileState::Read(session)),
                                }).and_then(move |(session, mut buf)| {
                                    buf.clear();
                                    SendFile {
                                        f: sig,
                                        buf: Some(buf),
                                        chan: chan1,
                                        state: Some(SendFileState::Read(session)),
                                    }
                                }))
                            } else {
                                futures::future::Either::B(SendFile {
                                    f: f,
                                    buf: Some(buf),
                                    chan: chan0,
                                    state: Some(SendFileState::Read(session)),
                                })
                            }
                        }).and_then(move |(mut session, _)| {
                            session.channel_eof(chan0);
                            session
                                .wait(move |session| {
                                    session.handler().exit_status.get(&chan0).is_some()
                                }).map(
                                    move |mut session| {
                                        exit_status = session
                                            .handler()
                                            .exit_status
                                            .get(&chan0)
                                            .map(|x| *x);
                                        session.channel_close(chan0);
                                        session
                                    },
                                )
                        }).map_err(From::from)
                })),
        ).unwrap());

        if let Some(ref session) = self.session {
            debug!("exit status = {:?}", session.handler().exit_status);
        }
        Ok(())
    }

    pub fn remote_init(&mut self) -> Result<(), Error> {
        let esc_path = escape(Cow::Borrowed(self.path));
        let cmd = format!("{} init {}", self.pijul_cmd, esc_path);
        debug!("command line:{:?}", cmd);

        self.session = Some(
            self.l
                .block_on(
                    self.session
                        .take()
                        .unwrap()
                        .channel_open_session()
                        .and_then(move |(mut session, chan)| {
                            debug!("chan = {:?}", chan);
                            session.handler_mut().exit_status.remove(&chan);
                            session.handler_mut().channel = Some(chan);
                            session.exec(chan, false, &cmd);
                            session.channel_eof(chan);
                            // Wait until channel close.
                            session
                                .wait(move |session| {
                                    session.handler().exit_status.get(&chan).is_some()
                                }).and_then(move |mut session| {
                                    if session.is_channel_open(chan) {
                                        session.channel_close(chan);
                                    }
                                    session.wait(move |session| !session.is_channel_open(chan))
                                })
                        }),
                ).unwrap(),
        );
        Ok(())
    }
}

impl<'a> UriSession<'a> {
    pub fn changes(
        &mut self,
        branch: &str,
        path: &[&str],
    ) -> Result<HashMap<Hash, ApplyTimestamp>, Error> {
        if !path.is_empty() {
            return Err(Error::PartialPullOverHttp);
        }
        let mut uri = self.uri.to_string();
        uri = uri + "/" + PIJUL_DIR_NAME + "/" + &branch_changes_base_path(branch);
        let mut req = reqwest_async::Request::new(reqwest::Method::GET, uri.parse().unwrap());
        req.headers_mut().insert(
            reqwest::header::CONNECTION,
            reqwest::header::HeaderValue::from_static("close"),
        );
        let res: Vec<u8> = self.l.block_on(self.client.execute(req).and_then(
            |resp: reqwest_async::Response| {
                let res = Vec::new();
                let body = resp.into_body();
                body.fold(res, |mut res, x| {
                    res.extend(x.iter());
                    futures::finished::<_, reqwest::Error>(res)
                })
            },
        ))?;
        let changes = read_changes(&mut &res[..]).unwrap_or(HashMap::new());
        debug!("http: {:?}", changes);
        Ok(changes)
    }

    pub fn fetch_patch(
        &mut self,
        patch_hash: &Hash,
        local_file: PathBuf,
        local_tmp_file: PathBuf,
    ) -> Result<PathBuf, Error> {
        let ref mut l = self.l;
        let ref mut client = self.client;
        let uri =
            self.uri.to_string() + "/" + PIJUL_DIR_NAME + "/patches/" + &patch_hash.to_base58() + ".gz";
        debug!("downloading uri {:?}", uri);

        let mut req = reqwest_async::Request::new(reqwest::Method::GET, uri.parse().unwrap());
        req.headers_mut().insert(
            reqwest::header::CONNECTION,
            reqwest::header::HeaderValue::from_static("close"),
        );

        let uri_sig =
            self.uri.to_string() + "/" + PIJUL_DIR_NAME + "/patches/" + &patch_hash.to_base58() + ".sig";
        debug!("{:?}", uri_sig);
        let req_sig = reqwest_async::Request::new(reqwest::Method::GET, uri_sig.parse().unwrap());
        req.headers_mut().insert(
            reqwest::header::CONNECTION,
            reqwest::header::HeaderValue::from_static("close"),
        );
        let mut local_sig_file = local_file.clone();
        let mut local_tmp_sig_file = local_tmp_file.clone();
        local_sig_file.set_extension("sig");
        local_tmp_sig_file.set_extension("sig");

        let res = l
            .block_on(client.execute(req).and_then(move |resp| {
                if resp.status() == reqwest::StatusCode::OK {
                    let res = Vec::new();
                    futures::future::Either::A(
                        resp.into_body()
                            .fold(res, |mut res, x| {
                                res.extend(x.iter());
                                futures::finished::<_, reqwest::Error>(res)
                            }).map(|body| {
                                // debug!("response={:?}", body);
                                let mut f = File::create(&local_tmp_file).unwrap();
                                f.write_all(&body).unwrap();
                                // debug!("patch downloaded through http: {:?}", body);
                                Some((local_tmp_file, local_file))
                            }),
                    )
                } else {
                    futures::future::Either::B(futures::finished(None))
                }
            }).join(client.execute(req_sig).and_then(move |resp| {
                debug!("sig status {:?}", resp.status());
                if resp.status() == reqwest::StatusCode::OK {
                    let res = Vec::new();
                    futures::future::Either::A(
                        resp.into_body()
                            .fold(res, |mut res, x| {
                                res.extend(x.iter());
                                futures::finished::<_, reqwest::Error>(res)
                            }).map(|body| {
                                // debug!("response={:?}", body);
                                let mut f = File::create(&local_tmp_sig_file).unwrap();
                                f.write_all(&body).unwrap();
                                // debug!("patch downloaded through http: {:?}", body);
                                Some((local_tmp_sig_file, local_sig_file))
                            }),
                    )
                } else {
                    futures::future::Either::B(futures::finished(None))
                }
            }))).unwrap();
        if let Some((local_tmp_file, local_file)) = res.0 {
            debug!("renaming {:?} to {:?}", local_tmp_file, local_file);
            rename(&local_tmp_file, &local_file)?;
            if let Some((local_tmp_sig_file, local_sig_file)) = res.1 {
                debug!("renaming {:?} to {:?}", local_tmp_sig_file, local_sig_file);
                rename(&local_tmp_sig_file, &local_sig_file).unwrap_or(());
            }
            Ok(local_file)
        } else {
            Err(Error::PatchNotFound {
                repo_root: self.uri.into(),
                patch_hash: patch_hash.to_owned(),
            })
        }
    }
}

impl<'a> LocalSession<'a> {
    pub fn changes(
        &mut self,
        branch: &str,
        path: &[&str],
    ) -> Result<HashMap<Hash, ApplyTimestamp>, Error> {
        let repo_dir = pristine_dir(&self.path);
        let repo = Repository::open(&repo_dir, None)?;
        let txn = repo.txn_begin()?;
        if let Some(branch) = txn.get_branch(&branch) {
            if !path.is_empty() {
                let mut patches = HashMap::new();
                for (hash, s) in txn.iter_patches(&branch, None) {
                    for path in path {
                        let inode = txn.find_inode(Path::new(path)).unwrap();
                        let key = txn.get_inodes(inode).unwrap().key;
                        if txn.get_touched(key, hash) {
                            patches.insert(txn.get_external(hash).unwrap().to_owned(), s);
                            break;
                        }
                    }
                }
                Ok(patches)
            } else {
                Ok(txn
                    .iter_patches(&branch, None)
                    .map(|(hash, s)| (txn.get_external(hash).unwrap().to_owned(), s))
                    .collect())
            }
        } else {
            Ok(HashMap::new())
        }
    }

    pub fn fetch_patch(&mut self, patch_hash: &Hash, local_file: PathBuf) -> Result<PathBuf, Error> {
        debug!("local downloading {:?}", patch_hash);
        let remote_file = patches_dir(self.path).join(&patch_file_name(patch_hash.as_ref()));
        debug!("hard linking {:?} to {:?}", remote_file, local_file);
        if hard_link(&remote_file, &local_file).is_err() {
            copy(&remote_file, &local_file)?;
        }
        Ok(local_file)
    }

    pub fn remote_apply(
        &mut self,
        repo_root: &Path,
        remote_branch: &str,
        patch_hashes: &HashSet<Hash>,
    ) -> Result<(), Error> {
        let mut remote_path = patches_dir(self.path);
        let mut local_path = patches_dir(repo_root);
        let remote_current_branch = get_current_branch(&self.path)?;

        for hash in patch_hashes {
            remote_path.push(&hash.to_base58());
            remote_path.set_extension("gz");

            local_path.push(&hash.to_base58());
            local_path.set_extension("gz");

            debug!("hard linking {:?} to {:?}", local_path, remote_path);
            if metadata(&remote_path).is_err() {
                if hard_link(&local_path, &remote_path).is_err() {
                    copy(&local_path, &remote_path)?;
                }
            }

            remote_path.set_extension("sig");
            local_path.set_extension("sig");

            if metadata(&remote_path).is_err() && metadata(&local_path).is_ok() {
                if hard_link(&local_path, &remote_path).is_err() {
                    copy(&local_path, &remote_path)?;
                }
            }

            local_path.pop();
            remote_path.pop();
        }

        loop {
            let app = if remote_current_branch != remote_branch {
                apply_resize_no_output(&self.path, &remote_branch, patch_hashes.iter(), |_, _| {})
            } else {
                apply_resize(
                    &self.path,
                    &remote_branch,
                    patch_hashes.iter(),
                    &[] as &[&str],
                    |_, _| {},
                )
            };
            match app {
                Err(ref e) if e.lacks_space() => debug!("lacks space"),
                Ok(()) => return Ok(()),
                Err(e) => return Err(From::from(e)),
            }
        }
    }
}

impl<'a> Session<'a> {
    pub fn changes(
        &mut self,
        branch: &str,
        remote_path: &[&str],
    ) -> Result<HashMap<Hash, ApplyTimestamp>, Error> {
        match *self {
            Session::Ssh(ref mut ssh_session) => ssh_session.changes(branch, remote_path),
            Session::Local(ref mut local_session) => local_session.changes(branch, remote_path),
            Session::Uri(ref mut uri_session) => uri_session.changes(branch, remote_path),
        }
    }
    pub fn download_patch(&mut self, repo_root: &Path, patch_hash: &Hash) -> Result<PathBuf, Error> {
        let patches_dir_ = patches_dir(repo_root);
        let local_file = patches_dir_.join(&patch_file_name(patch_hash.as_ref()));

        if !metadata(&local_file).is_ok() {
            match *self {
                Session::Local(ref mut local_session) => {
                    local_session.fetch_patch(patch_hash, local_file)
                }
                Session::Ssh(ref mut ssh_session) => {
                    let tmp_dir = TempDir::new_in(&patches_dir_, "pijul_patch")?;
                    let local_tmp_file = tmp_dir.path().join("patch");
                    ssh_session.fetch_patch(patch_hash, local_file, local_tmp_file)
                }
                Session::Uri(ref mut uri_session) => {
                    let tmp_dir = TempDir::new_in(&patches_dir_, "pijul_patch")?;
                    let local_tmp_file = tmp_dir.path().join("patch");
                    uri_session.fetch_patch(patch_hash, local_file, local_tmp_file)
                }
            }
        } else {
            Ok(local_file)
        }
    }

    fn remote_apply(
        &mut self,
        repo_root: &Path,
        remote_branch: &str,
        patch_hashes: HashSet<Hash>,
    ) -> Result<(), Error> {
        match *self {
            Session::Ssh(ref mut ssh_session) => {
                ssh_session.remote_apply(repo_root, remote_branch, patch_hashes)
            }

            Session::Local(ref mut local_session) => {
                local_session.remote_apply(repo_root, remote_branch, &patch_hashes)
            }

            _ => panic!("upload to URI impossible"),
        }
    }

    pub fn remote_init(&mut self) -> Result<(), Error> {
        match *self {
            Session::Ssh(ref mut ssh_session) => ssh_session.remote_init(),
            Session::Local(ref mut local_session) => {
                assert_no_containing_repo(local_session.path)?;
                create_repo(local_session.path)
            }
            _ => panic!("remote init not possible"),
        }
    }

    pub fn pullable_patches(
        &mut self,
        remote_branch: &str,
        local_branch: &str,
        target: &Path,
        remote_path: &[&str],
    ) -> Result<Pullable, Error> {
        let mut remote_patches: Vec<(Hash, ApplyTimestamp)> = self
            .changes(remote_branch, remote_path)?
            .into_iter()
            .map(|(h, s)| (h.to_owned(), s))
            .collect();
        remote_patches.sort_by(|&(_, ref a), &(_, ref b)| a.cmp(&b));
        let local_patches: HashMap<Hash, ApplyTimestamp> = {
            let repo_dir = pristine_dir(&target);
            let repo = Repository::open(&repo_dir, None)?;
            let txn = repo.txn_begin()?;
            if let Some(branch) = txn.get_branch(&local_branch) {
                txn.iter_patches(&branch, None)
                    .map(|(hash, s)| (txn.get_external(hash).unwrap().to_owned(), s))
                    .collect()
            } else {
                HashMap::new()
            }
        };
        debug!("pullable done: {:?}", remote_patches);
        Ok(Pullable {
            local: local_patches.iter().map(|(h, _)| h.to_owned()).collect(),
            remote: remote_patches.into_iter().collect(),
        })
    }

    pub fn pull(
        &mut self,
        target: &Path,
        to_branch: &str,
        pullable: &mut Vec<(Hash, ApplyTimestamp)>,
        partial_paths: &[&str],
        display_progress: bool,
    ) -> Result<(), Error> {
        let mut p = if display_progress && !pullable.is_empty() {
            Some((progrs::start("Pulling patches", pullable.len() as u64), 0))
        } else {
            None
        };
        let mut pullable_plus_deps = Vec::new();
        let mut pulled = HashSet::new();

        while let Some((hash, _)) = pullable.pop() {
            if pulled.contains(&hash) {
                continue;
            }
            debug!("hash = {:?}", hash);
            let path = self.download_patch(&target, &hash)?;

            let patch = {
                let file = File::open(&path)?;
                let mut file = BufReader::new(file);
                Patch::from_reader_compressed(&mut file)?.2
            };
            pulled.insert(hash.clone());

            // If the apply is partial, we might not have all the
            // dependencies. Add them to this list.
            if !partial_paths.is_empty() {
                for dep in patch.dependencies() {
                    if !pulled.contains(dep) {
                        pullable.push((dep.to_owned(), 0));
                    }
                }
            }

            pullable_plus_deps.push((hash.to_owned(), patch));

            p.as_mut().map(|&mut (ref mut p, ref mut n)| {
                p.display({
                    *n = *n + 1;
                    *n
                })
            });
        }
        p.map(|(p, _)| p.stop("done"));
        debug!("patches downloaded");

        let p = std::cell::RefCell::new(progrs::start(
            "Applying patches",
            pullable_plus_deps.len() as u64,
        ));
        let mut size_increase = 4096;
        let current_branch = get_current_branch(target)?;
        loop {
            let app = if current_branch != to_branch {
                apply_resize_patches_no_output(
                    target,
                    &to_branch,
                    &pullable_plus_deps,
                    size_increase,
                    |c, _| p.borrow_mut().display(c as u64),
                )
            } else {
                apply_resize_patches(
                    target,
                    &to_branch,
                    &pullable_plus_deps,
                    size_increase,
                    partial_paths,
                    |c, _| p.borrow_mut().display(c as u64),
                )
            };
            match app {
                Ok(()) => break,
                Err(ref e) if e.lacks_space() => size_increase *= 2,
                Err(e) => return Err(e.into()),
            }
        }
        p.into_inner().stop("done");
        Ok(())
    }

    pub fn pushable_patches(
        &mut self,
        from_branch: &str,
        to_branch: &str,
        source: &Path,
        remote_paths: &[&str],
    ) -> Result<Vec<(Hash, Option<PatchId>, ApplyTimestamp)>, Error> {
        debug!("source: {:?}", source);
        let to_changes = self.changes(to_branch, remote_paths)?;
        let from_changes: Vec<_> = {
            let repo_dir = pristine_dir(&source);
            let repo = Repository::open(&repo_dir, None)?;
            let txn = repo.txn_begin()?;
            if let Some(branch) = txn.get_branch(&from_branch) {
                txn.iter_patches(&branch, None)
                    .map(|(hash, s)| {
                        (
                            txn.get_external(hash).unwrap().to_owned(),
                            Some(hash.to_owned()),
                            s,
                        )
                    }).filter(|&(ref hash, _, _)| to_changes.get(hash).is_none())
                    .collect()
            } else {
                Vec::new()
            }
        };
        debug!("pushing: {:?}", from_changes);
        let to_changes: HashSet<Hash> = to_changes.into_iter().map(|(h, _)| h).collect();
        debug!("to_changes: {:?}", to_changes);

        Ok(from_changes
            .into_iter()
            .filter(|&(ref h, _, _)| !to_changes.contains(h))
            .collect())
    }

    pub fn push(
        &mut self,
        source: &Path,
        remote_branch: &str,
        pushable: HashSet<Hash>,
    ) -> Result<(), Error> {
        debug!("push, remote_applying");
        debug!("pushable: {:?}", pushable);
        if pushable.len() > 0 {
            self.remote_apply(source, remote_branch, pushable)?;
        }
        Ok(())
    }
}

pub fn ssh_connect(user: &Option<&str>, host: &str, port: Option<u16>) -> Result<(thrussh_config::Config, thrussh_config::ConnectFuture), Error> {

    let mut ssh_config = thrussh_config::parse_home(host).unwrap_or(thrussh_config::Config::default());
    debug!("ssh_config = {:?}", ssh_config);

    if ssh_config.host_name.is_none() {
        ssh_config.host_name = Some(host.to_string())
    }

    if let Some(port) = port {
        ssh_config.port = Some(port)
    } else if ssh_config.port.is_none() {
        ssh_config.port = Some(22)
    }

    if let Some(ref user) = *user {
        ssh_config.user = Some(user.to_string())
    } else if ssh_config.user.is_none() {
        ssh_config.user = Some(username::get_user_name().unwrap())
    }

    ssh_config.update_proxy_command();
    let stream = if let Some(ref proxycmd) = ssh_config.proxy_command {
        debug!("{:?}", proxycmd);
        thrussh_config::Stream::proxy_command("sh", &["-c", proxycmd.as_str()])
    } else {
        let addr =
            if let Some(addrs) = (ssh_config.host_name.as_ref().unwrap().as_str(),
                                  ssh_config.port.unwrap()).to_socket_addrs()?.next() {
                addrs
            } else {
                return Err(Error::UnknownHost { host: host.to_string() });
            };
        debug!("addr = {:?}", addr);
        thrussh_config::Stream::tcp_connect(&addr)
    };
    Ok((ssh_config, stream))
}

impl<'a> Remote<'a> {
    pub fn session(&'a self) -> Result<Session<'a>, Error> {
        match *self {
            Remote::Local { ref path } => Ok(Session::Local(LocalSession {
                path: path.as_path(),
            })),
            Remote::Uri { uri } => {
                let l = tokio::runtime::Runtime::new().unwrap();
                let proxy_url = std::env::var("http_proxy");
                let c = match proxy_url {
                    Err(std::env::VarError::NotPresent) => reqwest_async::Client::new(),
                    Ok(p_url) => reqwest_async::Client::builder()
                        .proxy(reqwest::Proxy::all(reqwest::Url::parse(&p_url).unwrap())?)
                        .build()?,
                    Err(std::env::VarError::NotUnicode(s)) => {
                        panic!("invalid http_proxy value: {:?}", s)
                    }
                };
                Ok(Session::Uri(UriSession {
                    l,
                    uri: uri,
                    client: c,
                }))
            }
            Remote::Ssh {
                ref user,
                ref host,
                port,
                ref path,
                ref local_repo_root,
                ref pijul_cmd,
                ..
            } => {
                let mut l = tokio::runtime::Runtime::new().unwrap();

                let (ssh_config, stream) = ssh_connect(user, host, port)?;
                let config = Arc::new(thrussh::client::Config::default());
                let handler = Client::new(ssh_config.port,
                                          ssh_config.host_name.as_ref().unwrap().as_str(), &mut l);

                let local_repo_root = local_repo_root.map(|x| x.to_path_buf());
                let host = host.to_string();
                let session: thrussh::client::Connection<_, _> = l.block_on(
                    stream.map_err(Error::from)
                        .and_then(move |socket| {
                            let use_agent = handler.agent.is_some();
                            let connection = thrussh::client::Connection::new(
                                config.clone(),
                                socket,
                                handler,
                                None,
                            )?;
                            debug!("connection done");
                            use super::ssh_auth_attempts::{AuthAttemptFuture, AuthAttempts};
                            let user = ssh_config.user.unwrap();
                            Ok(AuthAttemptFuture::new(
                                connection,
                                AuthAttempts::new(host, user.clone(), local_repo_root, use_agent),
                                user,
                            ))
                        }).flatten(),
                )?;
                debug!("session ready");
                Ok(Session::Ssh(SshSession {
                    l,
                    session: Some(session),
                    path,
                    pijul_cmd: &pijul_cmd,
                }))
            }
        }
    }
}

pub fn parse_remote<'a>(
    remote_id: &'a str,
    port: Option<u16>,
    base_path: Option<&'a Path>,
    local_repo_root: Option<&'a Path>,
) -> Remote<'a> {
    let pijul_cmd = super::remote_pijul_cmd();
    let ssh = Regex::new(r"^([^:]*):(.*)$").unwrap();
    let uri = Regex::new(r"^([a-zA-Z]*)://(.*)$").unwrap();
    if uri.is_match(remote_id) {
        let cap = uri.captures(remote_id).unwrap();
        if &cap[1] == "file" {
            if let Some(a) = base_path {
                let path = a.join(&cap[2]);
                Remote::Local { path: path }
            } else {
                let path = Path::new(&cap[2]).to_path_buf();
                Remote::Local { path: path }
            }
        } else {
            Remote::Uri { uri: remote_id }
        }
    } else if ssh.is_match(remote_id) {
        let cap = ssh.captures(remote_id).unwrap();
        let user_host = cap.get(1).unwrap().as_str();

        let (user, host) = {
            let ssh_user_host = Regex::new(r"^([^@]*)@(.*)$").unwrap();
            if ssh_user_host.is_match(user_host) {
                let cap = ssh_user_host.captures(user_host).unwrap();
                (
                    Some(cap.get(1).unwrap().as_str()),
                    cap.get(2).unwrap().as_str(),
                )
            } else {
                (None, user_host)
            }
        };
        Remote::Ssh {
            user: user,
            host: host,
            port: port,
            path: cap.get(2).unwrap().as_str(),
            id: remote_id,
            local_repo_root,
            pijul_cmd,
        }
    } else {
        if let Some(a) = base_path {
            let path = a.join(remote_id);
            Remote::Local { path: path }
        } else {
            let path = Path::new(remote_id).to_path_buf();
            Remote::Local { path: path }
        }
    }
}

#[derive(Debug)]
pub struct Pullable {
    pub local: HashSet<Hash>,
    pub remote: Vec<(Hash, ApplyTimestamp)>,
}

pub struct PullableIterator<'a> {
    remote: std::slice::Iter<'a, (Hash, ApplyTimestamp)>,
    local: &'a HashSet<Hash>,
}

impl Pullable {
    pub fn iter(&self) -> PullableIterator {
        PullableIterator {
            local: &self.local,
            remote: self.remote.iter(),
        }
    }
}

impl<'a> Iterator for PullableIterator<'a> {
    type Item = (Hash, ApplyTimestamp);
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(&(ref h, t)) = self.remote.next() {
            if !self.local.contains(h) {
                return Some((h.to_owned(), t));
            }
        }
        None
    }
}
