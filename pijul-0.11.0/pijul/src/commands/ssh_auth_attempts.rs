use dirs;
use futures::future::{ok, Either, FutureResult};
use futures::{Async, Future, Poll};
use meta;
use rpassword;
use std;
use std::path::{Path, PathBuf};
use thrussh::client::{Authenticate, Connection, Handler};
use thrussh::{HandlerError, Tcp};
use thrussh_keys;
use tokio::io::{AsyncRead, AsyncWrite};
use std::sync::Arc;
use error::Error;

pub enum AuthAttempt {
    Agent(thrussh_keys::key::PublicKey),
    Key(Arc<thrussh_keys::key::KeyPair>),
    Password(String),
}

#[derive(Debug, Copy, Clone)]
enum AuthState {
    Agent(KeyPath),
    Key(KeyPath),
    Password,
}

#[derive(Debug)]
pub struct AuthAttempts {
    state: AuthState,
    local_repo_root: Option<PathBuf>,
    user: String,
    server_name: String,
}

impl AuthAttempts {
    pub fn new(server_name: String, user: String, local_repo_root: Option<PathBuf>, use_agent: bool) -> Self {
        AuthAttempts {
            state: if use_agent {
                AuthState::Agent(KeyPath::first())
            } else {
                AuthState::Key(KeyPath::first())
            },
            local_repo_root,
            user,
            server_name,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum KeyLocation {
    Local,
    Pijul,
    Ssh,
}

#[derive(Debug, Clone, Copy)]
enum KeyType {
    Ed25519,
    Rsa,
}

#[derive(Debug, Clone, Copy)]
struct KeyPath {
    location: KeyLocation,
    typ: KeyType,
}

impl KeyPath {
    fn first() -> Self {
        KeyPath {
            location: KeyLocation::Local,
            typ: KeyType::Ed25519,
        }
    }
    fn next(&self) -> Option<KeyPath> {
        match self.typ {
            KeyType::Ed25519 => Some(KeyPath {
                location: self.location,
                typ: KeyType::Rsa,
            }),
            KeyType::Rsa => Some(KeyPath {
                location: match self.location {
                    KeyLocation::Local => KeyLocation::Pijul,
                    KeyLocation::Pijul => KeyLocation::Ssh,
                    KeyLocation::Ssh => return None,
                },
                typ: KeyType::Ed25519,
            }),
        }
    }
}

impl AuthAttempts {
    fn key_dir(&self, key: &KeyPath) -> Option<PathBuf> {
        match key.location {
            KeyLocation::Local => self.local_repo_root.clone(),
            KeyLocation::Pijul => meta::global_path().ok(),
            KeyLocation::Ssh => {
                if let Some(mut path) = dirs::home_dir() {
                    path.push(".ssh");
                    Some(path)
                } else {
                    None
                }
            }
        }
    }

    fn key(&self, key: &KeyPath) -> Option<PathBuf> {
        self.key_dir(key).map(|mut p| {
            p.push(match key.typ {
                KeyType::Ed25519 => "id_ed25519",
                KeyType::Rsa => "id_rsa",
            });
            p
        })
    }

    fn public_key(&self, key: &KeyPath) -> Option<PathBuf> {
        self.key(key).map(|mut p| {
            p.set_extension("pub");
            p
        })
    }
}

impl Iterator for AuthAttempts {
    type Item = AuthAttempt;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            debug!("state = {:?}", self.state);
            match self.state {
                AuthState::Agent(key_path) => {
                    let path = self.public_key(&key_path);
                    if let Some(key_path) = key_path.next() {
                        self.state = AuthState::Agent(key_path)
                    } else {
                        self.state = AuthState::Key(KeyPath::first())
                    }

                    if let Some(path) = path {
                        if let Ok(key) = thrussh_keys::load_public_key(&path) {
                            return Some(AuthAttempt::Agent(key));
                        }
                    }
                }
                AuthState::Key(key_path) => {
                    let path = self.key(&key_path);
                    if let Some(key_path) = key_path.next() {
                        self.state = AuthState::Key(key_path)
                    } else {
                        self.state = AuthState::Password
                    }
                    if let Some(path) = path {
                        if let Ok(key) = load_key_or_ask(&path) {
                            return Some(AuthAttempt::Key(Arc::new(key)));
                        }
                    }
                }
                AuthState::Password => {
                    let password = rpassword::prompt_password_stdout(&format!("Password for {:?}: ", self.server_name));
                    if let Ok(password) = password {
                        return Some(AuthAttempt::Password(password));
                    }
                }
            }
        }
    }
}

pub struct AuthAttemptFuture<
    R: Tcp + AsyncRead + AsyncWrite,
    H: Handler,
    I: Iterator<Item = AuthAttempt>,
> {
    auth:
        Option<Either<Authenticate<R, H>, FutureResult<Connection<R, H>, HandlerError<H::Error>>>>,
    it: I,
    user: String,
}

impl<R: AsyncRead + AsyncWrite + Tcp, H: Handler, I: Iterator<Item = AuthAttempt>>
    AuthAttemptFuture<R, H, I>
{
    pub fn new(session: Connection<R, H>, mut it: I, user: String) -> Self {
        debug!("AuthAttemptFuture::new");
        let auth = if let Some(next) = it.next() {
            Either::A(next_auth(session, &user, next))
        } else {
            Either::B(ok(session))
        };
        AuthAttemptFuture {
            auth: Some(auth),
            it,
            user,
        }
    }
}

impl<R: AsyncRead + AsyncWrite + Tcp, H: Handler, I: Iterator<Item = AuthAttempt>> Future
    for AuthAttemptFuture<R, H, I>
{
    type Item = Connection<R, H>;
    type Error = HandlerError<H::Error>;
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            debug!("authattemptfuture");
            let mut auth = self.auth.take().expect("future polled after completion");
            if let Async::Ready(session) = auth.poll()? {
                if session.is_authenticated() {
                    debug!("is_authenticated!");
                    return Ok(Async::Ready(session));
                } else if let Some(next) = self.it.next() {
                    self.auth = Some(Either::A(next_auth(session, &self.user, next)))
                } else {
                    return Ok(Async::Ready(session));
                }
            } else {
                self.auth = Some(auth);
                return Ok(Async::NotReady);
            }
        }
    }
}

fn next_auth<R: AsyncRead + AsyncWrite + Tcp, H: Handler>(
    session: Connection<R, H>,
    user: &str,
    next: AuthAttempt,
) -> Authenticate<R, H> {
    debug!("next_auth");
    match next {
        AuthAttempt::Agent(pk) => session.authenticate_key_future(user, pk),
        AuthAttempt::Key(k) => session.authenticate_key(user, k),
        AuthAttempt::Password(pass) => session.authenticate_password(user, pass),
    }
}

pub fn load_key_or_ask(path_sec: &Path) -> Result<thrussh_keys::key::KeyPair, Error> {
    debug!("path_sec {:?}", path_sec);
    match thrussh_keys::load_secret_key(path_sec.to_str().unwrap(), None) {
        Ok(key) => Ok(key),
        Err(e) => {
            match e {
                thrussh_keys::Error::KeyIsEncrypted => {
                    let password = rpassword::prompt_password_stdout(&format!("Password for key {:?}: ", path_sec))?;
                    return Ok(thrussh_keys::load_secret_key(
                        path_sec.to_str().unwrap(),
                        Some(password.as_bytes()),
                    )?);
                }

                thrussh_keys::Error::IO(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Err(Error::SshKeyNotFound { path: path_sec.to_path_buf() })
                }

                _ => {}
            }
            return Err(From::from(e));
        }
    }
}
