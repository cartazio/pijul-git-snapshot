use std;
use std::path::PathBuf;
use {hex, libpijul, regex, reqwest, term, thrussh, thrussh_config, thrussh_keys, toml};

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Term(term::Error),
    Repository(libpijul::Error),
    UTF8(std::string::FromUtf8Error),
    Hex(hex::FromHexError),
    SSH(thrussh::Error),
    SSHKeys(thrussh_keys::Error),
    Reqwest(reqwest::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
    StripPrefix(std::path::StripPrefixError),
    Regex(regex::Error),
    ThrusshConfig(thrussh_config::Error),
    HookFailed { cmd: String },
    InARepository { path: std::path::PathBuf },
    NotInARepository,
    MissingRemoteRepository,
    InvalidPath { path: String },
    FileNotInRepository { path: String },
    WrongHash,
    BranchAlreadyExists,
    CannotDeleteCurrentBranch,
    NoSuchBranch,
    IsDirectory,
    CannotParseRemote,
    WillNotOverwriteKeyFile { path: std::path::PathBuf },
    BranchDoesNotHavePatch { branch_name: String, patch: libpijul::Hash },
    PatchNotFound { repo_root: String, patch_hash: libpijul::Hash },
    SshKeyNotFound { path: PathBuf },
    NoHomeDir,
    ExtraDepNotOnBranch { hash: libpijul::Hash },
    PendingChanges,
    EmptyPatchName,
    CannotSpawnEditor { editor: String, cause: String },
    InvalidDate { date:  String },
    PartialPullOverHttp,
    UnknownHost { host: String },
    NoAuthor,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Error::IO(ref e) => e.fmt(f),
            Error::Term(ref e) => e.fmt(f),
            Error::Repository(ref e) => e.fmt(f),
            Error::UTF8(ref e) => e.fmt(f),
            Error::Hex(ref e) => e.fmt(f),
            Error::SSH(ref e) => e.fmt(f),
            Error::SSHKeys(ref e) => e.fmt(f),
            Error::Reqwest(ref e) => e.fmt(f),
            Error::TomlDe(ref e) => e.fmt(f),
            Error::TomlSer(ref e) => e.fmt(f),
            Error::StripPrefix(ref e) => e.fmt(f),
            Error::Regex(ref e) => e.fmt(f),
            Error::ThrusshConfig(ref e) => e.fmt(f),
            Error::HookFailed { ref cmd } => write!(f, "Hook failed: {}", cmd),
            Error::InARepository { ref path } => write!(f, "In a repository: {:?}", path),
            Error::NotInARepository => write!(f, "Not in a repository"),
            Error::MissingRemoteRepository => write!(f, "Missing remote repository"),
            Error::InvalidPath { ref path } => write!(f, "Invalid path: {:?}", path),
            Error::FileNotInRepository { ref path } => write!(f, "File not in repository: {:?}", path),
            Error::WrongHash => write!(f, "Wrong hash"),
            Error::BranchAlreadyExists => write!(f, "Branch already exists"),
            Error::CannotDeleteCurrentBranch => write!(f, "Cannot delete current branch"),
            Error::NoSuchBranch => write!(f, "No such branch"),
            Error::IsDirectory => write!(f, "Is a directory"),
            Error::CannotParseRemote => write!(f, "Cannot parse remote address"),
            Error::WillNotOverwriteKeyFile { ref path } => write!(f, "Will not overwrite key file {:?}", path),
            Error::BranchDoesNotHavePatch { ref branch_name, ref patch } => write!(f, "Branch {:?} does not have patch {}", branch_name, patch.to_base58()),
            Error::PatchNotFound { ref repo_root, ref patch_hash } => write!(f, "Patch {} not found in repository {:?}", patch_hash.to_base58(), repo_root),
            Error::SshKeyNotFound { ref path } => write!(f, "SSH key not found in: {:?}", path),
            Error::NoHomeDir => write!(f, "No home dir"),
            Error::ExtraDepNotOnBranch { ref hash } => write!(f, "Extra dependencies can only be added if they are on the same branch as the current record: {:?}", hash),
            Error::PendingChanges => write!(f, "There are pending changes in the repository."),
            Error::EmptyPatchName => write!(f, "Empty patch name"),
            Error::CannotSpawnEditor { ref editor, ref cause } => write!(f, "Cannot start editor {:?} ({:?})", editor, cause),
            Error::InvalidDate { ref date } => write!(f, "Invalid date: {:?}", date),
            Error::PartialPullOverHttp => write!(f, "Partial pull over HTTP is not (yet) supported"),
            Error::UnknownHost { ref host } => write!(f, "Unknown host: {}", host),
            Error::NoAuthor => write!(f, "No authors were given"),
        }
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::IO(ref e) => e.description(),
            Error::Term(ref e) => e.description(),
            Error::Repository(ref e) => e.description(),
            Error::UTF8(ref e) => e.description(),
            Error::Hex(ref e) => e.description(),
            Error::SSH(ref e) => e.description(),
            Error::SSHKeys(ref e) => e.description(),
            Error::Reqwest(ref e) => e.description(),
            Error::TomlDe(ref e) => e.description(),
            Error::TomlSer(ref e) => e.description(),
            Error::StripPrefix(ref e) => e.description(),
            Error::Regex(ref e) => e.description(),
            Error::ThrusshConfig(ref e) => e.description(),
            Error::HookFailed { .. } => "Hook failed",
            Error::InARepository { .. } => "In a repository",
            Error::NotInARepository => "Not in a repository",
            Error::MissingRemoteRepository => "Missing remote repository",
            Error::InvalidPath { .. } => "Invalid path",
            Error::FileNotInRepository { .. } => "File not in repository",
            Error::WrongHash => "Wrong hash",
            Error::BranchAlreadyExists => "Branch already exists",
            Error::CannotDeleteCurrentBranch => "Cannot delete current branch",
            Error::NoSuchBranch => "No such branch",
            Error::IsDirectory => "Is a directory",
            Error::CannotParseRemote => "Cannot parse remote address",
            Error::WillNotOverwriteKeyFile { .. } => "Will not overwrite key file",
            Error::BranchDoesNotHavePatch { .. } => "Branch does not have patch",
            Error::PatchNotFound { .. } => "Patch not found in repository",
            Error::SshKeyNotFound { .. } => "SSH key not found",
            Error::NoHomeDir => "No home dir",
            Error::ExtraDepNotOnBranch { .. } => "Extra dependencies can only be added if they are on the same branch as the current record",
            Error::PendingChanges => "There are pending changes in the repository.",
            Error::EmptyPatchName => "Empty patch name",
            Error::CannotSpawnEditor { .. } => "Cannot start editor",
            Error::InvalidDate { .. } => "Invalid date",
            Error::PartialPullOverHttp => "Partial pull over HTTP is not (yet) supported",
            Error::UnknownHost { .. } => "Unknown host",
            Error::NoAuthor => "No authors were given",
        }
    }

    fn cause(&self) -> Option<&std::error::Error> {
        match *self {
            Error::IO(ref e) => Some(e),
            Error::Term(ref e) => Some(e),
            Error::Repository(ref e) => Some(e),
            Error::UTF8(ref e) => Some(e),
            Error::Hex(ref e) => Some(e),
            Error::SSH(ref e) => Some(e),
            Error::SSHKeys(ref e) => Some(e),
            Error::Reqwest(ref e) => Some(e),
            Error::TomlDe(ref e) => Some(e),
            Error::TomlSer(ref e) => Some(e),
            Error::StripPrefix(ref e) => Some(e),
            Error::Regex(ref e) => Some(e),
            Error::ThrusshConfig(ref e) => Some(e),
            _ => None
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::IO(err)
    }
}

impl From<term::Error> for Error {
    fn from(err: term::Error) -> Error {
        Error::Term(err)
    }
}

impl From<libpijul::Error> for Error {
    fn from(err: libpijul::Error) -> Error {
        Error::Repository(err)
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(err: std::string::FromUtf8Error) -> Error {
        Error::UTF8(err)
    }
}

impl From<hex::FromHexError> for Error {
    fn from(err: hex::FromHexError) -> Error {
        Error::Hex(err)
    }
}

impl From<thrussh::Error> for Error {
    fn from(err: thrussh::Error) -> Error {
        Error::SSH(err)
    }
}

impl From<thrussh_keys::Error> for Error {
    fn from(err: thrussh_keys::Error) -> Error {
        Error::SSHKeys(err)
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Error {
        Error::Reqwest(err)
    }
}

impl From<toml::de::Error> for Error {
    fn from(err: toml::de::Error) -> Error {
        Error::TomlDe(err)
    }
}

impl From<toml::ser::Error> for Error {
    fn from(err: toml::ser::Error) -> Error {
        Error::TomlSer(err)
    }
}

impl From<std::path::StripPrefixError> for Error {
    fn from(err: std::path::StripPrefixError) -> Error {
        Error::StripPrefix(err)
    }
}

impl From<regex::Error> for Error {
    fn from(err: regex::Error) -> Error {
        Error::Regex(err)
    }
}

impl From<thrussh_config::Error> for Error {
    fn from(err: thrussh_config::Error) -> Error {
        Error::ThrusshConfig(err)
    }
}

impl Error {
    pub fn lacks_space(&self) -> bool {
        match *self {
            Error::Repository(ref r) => r.lacks_space(),
            _ => false,
        }
    }
}

impl From<thrussh::HandlerError<Error>> for Error {
    fn from(err: thrussh::HandlerError<Error>) -> Error {
        match err {
            thrussh::HandlerError::Handler(e) => e,
            thrussh::HandlerError::Error(e) => Error::SSH(e),
        }
    }
}

impl From<Error> for thrussh::HandlerError<Error> {
    fn from(e: Error) -> thrussh::HandlerError<Error> {
        thrussh::HandlerError::Handler(e)
    }
}
