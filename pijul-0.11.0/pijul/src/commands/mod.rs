use clap;
use clap::ArgMatches;
pub type StaticSubcommand = clap::App<'static, 'static>;

mod ask;
mod fs_operation;
pub mod remote;
mod ssh_auth_attempts;

pub mod add;
pub mod apply;
pub mod branches;
pub mod challenge;
pub mod checkout;
pub mod clone;
pub mod credit;
pub mod delete_branch;
pub mod diff;
pub mod dist;
pub mod fork;
pub mod generate_completions;
pub mod hooks;
pub mod info;
pub mod init;
pub mod key;
pub mod log;
pub mod ls;
pub mod mv;
pub mod patch;
pub mod pull;
pub mod push;
pub mod record;
pub mod remove;
pub mod revert;
pub mod rollback;
pub mod show_dependencies;
pub mod sign;
pub mod status;
pub mod tag;
pub mod unrecord;

#[cfg(unix)]
use pager::Pager;
#[cfg(unix)]
use std::env;

use libpijul::Hash;
use error::Error;
use libpijul::fs_representation::get_current_branch;
use libpijul::{fs_representation, Inode, Repository, Txn, DEFAULT_BRANCH};
use rand;
use std::borrow::Cow;
use std::env::current_dir;
use std::env::var;
use std::fs::{canonicalize, create_dir, metadata};
use std::io::{stderr, Write};
use std::path::{Path, PathBuf};
use std::process::exit;

pub fn all_command_invocations() -> Vec<StaticSubcommand> {
    return vec![
        log::invocation(),
        info::invocation(),
        init::invocation(),
        record::invocation(),
        unrecord::invocation(),
        add::invocation(),
        pull::invocation(),
        push::invocation(),
        apply::invocation(),
        clone::invocation(),
        remove::invocation(),
        mv::invocation(),
        ls::invocation(),
        revert::invocation(),
        patch::invocation(),
        fork::invocation(),
        branches::invocation(),
        delete_branch::invocation(),
        checkout::invocation(),
        diff::invocation(),
        credit::invocation(),
        dist::invocation(),
        key::invocation(),
        rollback::invocation(),
        status::invocation(),
        show_dependencies::invocation(),
        tag::invocation(),
        sign::invocation(),
        challenge::invocation(),
        generate_completions::invocation(),
    ];
}

pub fn get_wd(repository_path: Option<&Path>) -> Result<PathBuf, Error> {
    debug!("get_wd: {:?}", repository_path);
    match repository_path {
        None => Ok(canonicalize(current_dir()?)?),
        Some(a) if a.is_relative() => Ok(canonicalize(current_dir()?.join(a))?),
        Some(a) => Ok(canonicalize(a)?),
    }
}

/// Returns an error if the `dir` is contained in a repository.
pub fn assert_no_containing_repo(dir: &Path) -> Result<(), Error> {
    if metadata(dir).is_ok() {
        if fs_representation::find_repo_root(&canonicalize(dir)?).is_some() {
            return Err(Error::InARepository { path: dir.to_owned() });
        }
    }
    Ok(())
}

/// Creates an empty pijul repository in the given directory.
pub fn create_repo(dir: &Path) -> Result<(), Error> {
    // Check that a repository does not already exist.
    if metadata(dir).is_err() {
        create_dir(dir)?;
    }
    let dir = canonicalize(dir)?;
    if fs_representation::find_repo_root(&dir).is_some() {
        return Err(Error::InARepository { path: dir.to_owned() });
    }

    fs_representation::create(&dir, rand::thread_rng())?;
    let pristine_dir = fs_representation::pristine_dir(&dir);
    let repo = Repository::open(&pristine_dir, None)?;
    repo.mut_txn_begin(rand::thread_rng())?.commit()?;
    Ok(())
}

fn default_explain<R>(command_result: Result<R, Error>) {
    debug!("default_explain");
    match command_result {
        Ok(_) => (),
        Err(e) => {
            writeln!(stderr(), "error: {}", e).unwrap();
            exit(1)
        }
    }
}

fn validate_base58(x: String) -> ::std::result::Result<(), String> {
    if Hash::from_base58(&x).is_some() {
        Ok(())
    } else {
        Err(format!("\"{}\" is invalid base58", x))
    }
}

/// Almost all commands want to know the current directory and the repository root.  This struct
/// fills that need, and also provides methods for other commonly-used tasks.
pub struct BasicOptions<'a> {
    /// This isn't 100% the same as the actual current working directory, so pay attention: this
    /// will be the current directory, unless the user specifies `--repository`, in which case
    /// `cwd` will actually be the path of the repository root. In other words, specifying
    /// `--repository` has the same effect as changing directory to the repository root before
    /// running `pijul`.
    pub cwd: PathBuf,
    pub repo_root: PathBuf,
    args: &'a ArgMatches<'a>,
}

pub enum ScanScope {
    FromRoot,
    WithPrefix(PathBuf, String),
}

impl<'a> BasicOptions<'a> {
    /// Reads the options from command line arguments.
    pub fn from_args(args: &'a ArgMatches<'a>) -> Result<BasicOptions<'a>, Error> {
        let wd = get_wd(args.value_of("repository").map(Path::new))?;
        let repo_root = if let Some(r) = fs_representation::find_repo_root(&canonicalize(&wd)?) {
            r
        } else {
            return Err(Error::NotInARepository);
        };
        Ok(BasicOptions {
            cwd: wd,
            repo_root: repo_root,
            args: args,
        })
    }

    /// Gets the name of the desired branch.
    pub fn branch(&self) -> String {
        if let Some(b) = self.args.value_of("branch") {
            b.to_string()
        } else if let Ok(b) = get_current_branch(&self.repo_root) {
            b
        } else {
            DEFAULT_BRANCH.to_string()
        }
    }

    pub fn repo_dir(&self) -> PathBuf {
        fs_representation::repo_dir(&self.repo_root)
    }

    pub fn repo_root(&self) -> PathBuf {
        self.repo_root.clone()
    }

    pub fn open_repo(&self) -> Result<Repository, Error> {
        Repository::open(self.pristine_dir(), None).map_err(|e| e.into())
    }

    pub fn open_and_grow_repo(&self, increase: u64) -> Result<Repository, Error> {
        Repository::open(self.pristine_dir(), Some(increase)).map_err(|e| e.into())
    }

    pub fn pristine_dir(&self) -> PathBuf {
        fs_representation::pristine_dir(&self.repo_root)
    }

    pub fn patches_dir(&self) -> PathBuf {
        fs_representation::patches_dir(&self.repo_root)
    }

    pub fn scan_scope(&self) -> Result<ScanScope, Error> {
        if let Some(prefix) = self.args.value_of("dir") {
            let root = self.args
                .value_of("repository")
                .map(|root| Path::new(root).to_path_buf())
                .unwrap_or(current_dir()?);

            Ok(ScanScope::WithPrefix(
                relative_repo_path(&self.repo_root, &root, prefix)?,
                prefix.into(),
            ))
        } else {
            Ok(ScanScope::FromRoot)
        }
    }

    fn dir_inode(&self, txn: &Txn) -> Result<Inode, Error> {
        use libpijul::ROOT_INODE;
        if let Some(dir) = self.args.value_of("dir") {
            let dir = if Path::new(dir).is_relative() {
                let root = if let Some(root) = self.args.value_of("repository") {
                    Path::new(root).to_path_buf()
                } else {
                    current_dir()?
                };
                root.join(&dir).canonicalize()?
            } else {
                Path::new(dir).canonicalize()?
            };
            let prefix = self.repo_root();
            let dir = dir.strip_prefix(&prefix)?;
            debug!("{:?}", dir);
            let inode = txn.find_inode(&dir)?;
            debug!("{:?}", inode);
            Ok(inode)
        } else {
            Ok(ROOT_INODE)
        }
    }
}

fn remote_pijul_cmd() -> Cow<'static, str> {
    if let Ok(cmd) = var("REMOTE_PIJUL") {
        Cow::Owned(cmd)
    } else {
        Cow::Borrowed("pijul")
    }
}

#[cfg(unix)]
fn setup_pager() {
    if env::var_os("NOPAGER").is_none() {
        Pager::with_pager("less -r").setup()
    }
}

#[cfg(not(unix))]
fn setup_pager() {}

pub fn relative_repo_path(repo_root: &PathBuf, base: &PathBuf, dir: &str) -> Result<PathBuf, Error> {
    let dir = if Path::new(dir).is_relative() {
        base.join(&dir).canonicalize()?
    } else {
        Path::new(dir).canonicalize()?
    };

    Ok(dir.strip_prefix(&repo_root)?.to_owned())
}
