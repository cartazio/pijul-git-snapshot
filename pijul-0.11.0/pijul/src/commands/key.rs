use super::ask;
use bincode;
use clap::{Arg, ArgMatches, SubCommand};
use commands::{BasicOptions, StaticSubcommand};
use cryptovec;
use dirs;
use error::Error;
use futures;
use futures::Future;
use meta;
use meta::KeyType;
use regex::Regex;
#[cfg(unix)]
use std;
use std::borrow::Cow;
use std::io::{stderr, Write};
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;
use thrussh;
use thrussh::{client, ChannelId};
use thrussh_keys;
use thrussh_keys::key;
use username;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("key")
        .about("Manage signing and SSH keys")
        .subcommand(
            SubCommand::with_name("upload")
                .about("Upload keys to a remote server")
                .arg(Arg::with_name("port")
                     .long("port")
                     .short("p")
                     .help("Port of the SSH server.")
                     .takes_value(true)
                     .required(false))
                .arg(Arg::with_name("repository")
                     .long("repository")
                     .help("The repository where the signing key is, if the key was generated with --for-repository.")
                     .takes_value(true)
                     .required(false))
                .arg(Arg::with_name("local")
                     .long("local")
                     .help("Save keys for the local repository only")
                     .takes_value(false)
                     .required(false))
                .arg(Arg::with_name("address")
                     .help("Address to use, for instance pijul_org@nest.pijul.com.")
                     .takes_value(true)
                     .required(true))
        )
        .subcommand(
            SubCommand::with_name("gen")
                .about("Generate keys. If neither --ssh nor --signing is given, both key types are generated.")
                .arg(Arg::with_name("ssh")
                     .long("ssh")
                     .help("Generate an SSH key")
                     .takes_value(false))
                .arg(Arg::with_name("signing")
                     .long("signing")
                     .help("Generate a signing key")
                     .takes_value(false))
                .arg(Arg::with_name("local")
                     .long("local")
                     .help("Save keys for the local repository only")
                     .takes_value(false)
                     .required(false))
                .arg(Arg::with_name("repository")
                     .long("for-repository")
                     .help("Save keys for the given repository only")
                     .takes_value(true)
                     .required(false))
        );
}

pub enum Params<'a> {
    Upload {
        address: &'a str,
        port: u16,
        repository: Option<PathBuf>,
        remote_cmd: Cow<'static, str>,
    },
    Gen {
        signing: bool,
        ssh: bool,
        local: Option<PathBuf>,
    },
    None,
}

pub fn parse_args<'a>(args: &'a ArgMatches) -> Result<Params<'a>, Error> {
    match args.subcommand() {
        ("upload", Some(args)) => Ok(Params::Upload {
            address: args.value_of("address").unwrap(),
            port: args.value_of("port")
                .and_then(|x| x.parse().ok())
                .unwrap_or(22),
            repository: if args.is_present("repository") || args.is_present("local") {
                Some(BasicOptions::from_args(args)?.repo_dir())
            } else {
                None
            },
            remote_cmd: super::remote_pijul_cmd(),
        }),
        ("gen", Some(args)) => Ok(Params::Gen {
            signing: args.is_present("signing") || !args.is_present("ssh"),
            ssh: args.is_present("ssh") || !args.is_present("signing"),
            local: if args.is_present("repository") || args.is_present("local") {
                Some(BasicOptions::from_args(args)?.repo_dir())
            } else {
                None
            },
        }),
        _ => Ok(Params::None),
    }
}

pub fn run(arg_matches: &ArgMatches) -> Result<(), Error> {
    match parse_args(arg_matches)? {
        Params::Upload {
            address,
            port,
            repository,
            remote_cmd,
        } => match meta::load_global_or_local_signing_key(repository.as_ref()) {
            Ok(key) => {

                let config = Arc::new(thrussh::client::Config::default());
                let ssh_user_host = Regex::new(r"^([^@]*)@(.*)$").unwrap();
                let (user, server) = if let Some(cap) = ssh_user_host.captures(&address) {
                    (cap[1].to_string(), cap[2].to_string())
                } else {
                    (username::get_user_name().unwrap(), address.to_string())
                };

                let mut l = tokio::runtime::Runtime::new()?;
                let client = SshClient::new(port, &server, key, &mut l);

                use super::ssh_auth_attempts::{AuthAttemptFuture, AuthAttempts};
                let use_agent = client.agent.is_some();
                let server_ = server.to_string();
                l.block_on(thrussh::client::connect_future(
                    (server.as_str(), port),
                    config,
                    None,
                    client,
                    move |connection| {
                        AuthAttemptFuture::new(
                            connection,
                            AuthAttempts::new(server_, user.to_string(), repository, use_agent),
                            user,
                        ).and_then(move |session| {
                            session.channel_open_session().and_then(
                                move |(mut session, channelid)| {
                                    session.exec(
                                        channelid,
                                        false,
                                        &format!("{} challenge", remote_cmd),
                                    );
                                    session.flush().unwrap();
                                    session.wait(move |session| {
                                        session.handler().exit_status.is_some()
                                    })
                                },
                            )
                        })
                            .from_err()
                    },
                )?)?;
            }
            Err(e) => return Err(e),
        },
        Params::Gen {
            signing,
            ssh,
            local,
        } => {
            if let Some(ref dot_pijul) = local {
                if ssh {
                    meta::generate_key(dot_pijul, None, KeyType::SSH)?
                }
                if signing {
                    meta::generate_key(dot_pijul, None, KeyType::Signing)?
                }
            } else {
                if ssh {
                    meta::generate_global_key(KeyType::SSH)?
                }
                if signing {
                    meta::generate_global_key(KeyType::Signing)?
                }
            }
        }
        Params::None => {}
    }
    Ok(())
}

pub fn explain(r: Result<(), Error>) {
    if let Err(e) = r {
        if let Error::InARepository { path } = e {
            writeln!(stderr(), "Repository {:?} already exists", path).unwrap();
        } else {
            writeln!(stderr(), "error: {}", e).unwrap();
        }
        exit(1)
    }
}

#[cfg(unix)]
use thrussh_keys::agent::client::AgentClient;
#[cfg(unix)]
use tokio_uds::UnixStream;

use thrussh_keys::key::KeyPair;
use tokio;

struct SshClient {
    exit_status: Option<u32>,
    key_pair: KeyPair,
    host: String,
    port: u16,
    #[cfg(unix)]
    agent: Option<AgentClient<UnixStream>>,
    #[cfg(windows)]
    agent: Option<()>,
}

impl SshClient {
    #[cfg(unix)]
    fn new(port: u16, host: &str, key_pair: KeyPair, l: &mut tokio::runtime::Runtime) -> Self {
        let agent = if let Ok(path) = std::env::var("SSH_AUTH_SOCK") {
            l.block_on(UnixStream::connect(path)
                       .map(thrussh_keys::agent::client::AgentClient::connect))
                .ok()
        } else {
            None
        };
        debug!("agent = {:?}", agent.is_some());
        SshClient {
            exit_status: None,
            host: host.to_string(),
            key_pair,
            port,
            agent,
        }
    }

    #[cfg(windows)]
    fn new(port: u16, host: &str, key_pair: KeyPair, _: &tokio::runtime::Runtime) -> Self {
        SshClient {
            exit_status: None,
            host: host.to_string(),
            key_pair,
            port,
            agent: None,
        }
    }
}

impl client::Handler for SshClient {
    type Error = Error;
    type FutureBool = futures::Finished<(Self, bool), Self::Error>;
    type FutureUnit = futures::Finished<Self, Self::Error>;
    type SessionUnit = futures::Finished<(Self, client::Session), Self::Error>;
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
                    })
                    .from_err(),
            )
        } else {
            debug!("no agent");
            Box::new(futures::finished((self, to_sign)))
        }
    }

    fn check_server_key(self, server_public_key: &key::PublicKey) -> Self::FutureBool {
        let path = dirs::home_dir()
            .unwrap()
            .join(".ssh")
            .join("known_hosts");
        match thrussh_keys::check_known_hosts_path(&self.host, self.port, &server_public_key, &path)
        {
            Ok(true) => futures::done(Ok((self, true))),
            Ok(false) => {
                if let Ok(false) = ask::ask_learn_ssh(&self.host, self.port, "") {
                    futures::done(Ok((self, false)))
                } else {
                    thrussh_keys::learn_known_hosts_path(
                        &self.host,
                        self.port,
                        &server_public_key,
                        &path,
                    ).unwrap();
                    futures::done(Ok((self, true)))
                }
            }
            Err(e) => if let thrussh_keys::Error::KeyChanged(ref line) = e {
                println!(
                    "Host key changed! Someone might be eavesdropping this communication, \
                     refusing to continue. Previous key found line {}",
                    line
                );
                futures::done(Ok((self, false)))
            } else {
                futures::done(Err(From::from(e)))
            },
        }
    }
    fn data(
        self,
        channel: ChannelId,
        _: Option<u32>,
        data: &[u8],
        mut session: client::Session,
    ) -> Self::SessionUnit {
        use thrussh_keys::PublicKeyBase64;
        let response: (String, thrussh_keys::key::Signature) = (
            self.key_pair.public_key_base64(),
            self.key_pair.sign_detached(data).unwrap(),
        );
        debug!("data = {:?}", data);
        let resp = bincode::serialize(&response).unwrap();
        debug!("resp = {:?}", resp);
        session.data(channel, None, &resp);
        session.channel_eof(channel);
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
        self.exit_status = Some(exit_status);
        debug!("self.exit_status = {:?}", self.exit_status);
        futures::finished((self, session))
    }
}
