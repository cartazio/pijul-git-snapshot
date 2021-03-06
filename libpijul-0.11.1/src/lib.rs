//! This crate contains the core API to access Pijul repositories.
//!
//! The key object is a `Repository`, on which `Txn` (immutable
//! transactions) and `MutTxn` (mutable transactions) can be started,
//! to perform a variety of operations.
//!
//! Another important object is a `Patch`, which encodes two different pieces of information:
//!
//! - Information about deleted and inserted lines between two versions of a file.
//!
//! - Information about file moves, additions and deletions.
//!
//! The standard layout of a repository is defined in module
//! `fs_representation`, and mainly consists of a directory called
//! `.pijul` at the root of the repository, containing:
//!
//! - a directory called `pristine`, containing a Sanakirja database
//! storing most of the repository information.
//!
//! - a directory called `patches`, actually containing the patches,
//! where each patch is a gzipped compression of the bincode encoding
//! of the `patch::Patch` type.
//!
//! At the moment, users of this library, such as the Pijul
//! command-line tool, may use other files in the `.pijul` directory,
//! such as user preferences, or information about remote branches and
//! repositories.
#![recursion_limit = "128"]
#[macro_use]
extern crate bitflags;
extern crate chrono;
#[macro_use]
extern crate log;

extern crate base64;
extern crate bincode;
extern crate bs58;
extern crate byteorder;
extern crate flate2;
extern crate hex;
extern crate ignore;
extern crate openssl;
extern crate rand;
extern crate sanakirja;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tempdir;
extern crate thrussh_keys;
extern crate toml;

pub use sanakirja::Transaction;
use std::collections::HashSet;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Sanakirja(sanakirja::Error),
    Bincode(bincode::Error),
    Utf8(std::str::Utf8Error),
    Serde(serde_json::Error),
    OpenSSL(openssl::error::Error),
    OpenSSLStack(openssl::error::ErrorStack),
    Keys(thrussh_keys::Error),
    Base58Decode(bs58::decode::DecodeError),
    AlreadyAdded,
    FileNotInRepo(PathBuf),
    NoDb(backend::Root),
    WrongHash,
    EOF,
    WrongPatchSignature,
    BranchNameAlreadyExists(String),
    WrongFileHeader(Key<PatchId>),
    FileNameCount(Key<PatchId>),
}

impl std::convert::From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IO(e)
    }
}

impl std::convert::From<sanakirja::Error> for Error {
    fn from(e: sanakirja::Error) -> Self {
        Error::Sanakirja(e)
    }
}

impl std::convert::From<bincode::Error> for Error {
    fn from(e: bincode::Error) -> Self {
        Error::Bincode(e)
    }
}

impl std::convert::From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Serde(e)
    }
}

impl std::convert::From<std::str::Utf8Error> for Error {
    fn from(e: std::str::Utf8Error) -> Self {
        Error::Utf8(e)
    }
}

impl std::convert::From<thrussh_keys::Error> for Error {
    fn from(e: thrussh_keys::Error) -> Self {
        Error::Keys(e)
    }
}

impl std::convert::From<openssl::error::ErrorStack> for Error {
    fn from(e: openssl::error::ErrorStack) -> Self {
        Error::OpenSSLStack(e)
    }
}

impl std::convert::From<bs58::decode::DecodeError> for Error {
    fn from(e: bs58::decode::DecodeError) -> Self {
        Error::Base58Decode(e)
    }
}

pub type Result<A> = std::result::Result<A, Error>;

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Error::IO(ref e) => e.fmt(fmt),
            Error::Sanakirja(ref e) => e.fmt(fmt),
            Error::Bincode(ref e) => e.fmt(fmt),
            Error::Utf8(ref e) => e.fmt(fmt),
            Error::Serde(ref e) => e.fmt(fmt),
            Error::OpenSSL(ref e) => e.fmt(fmt),
            Error::OpenSSLStack(ref e) => e.fmt(fmt),
            Error::Keys(ref e) => e.fmt(fmt),
            Error::Base58Decode(ref e) => e.fmt(fmt),
            Error::AlreadyAdded => write!(fmt, "Already added"),
            Error::FileNotInRepo(ref file) => write!(fmt, "File {:?} not tracked", file),
            Error::NoDb(ref e) => write!(fmt, "Table missing: {:?}", e),
            Error::WrongHash => write!(fmt, "Wrong hash"),
            Error::EOF => write!(fmt, "EOF"),
            Error::WrongPatchSignature => write!(fmt, "Wrong patch signature"),
            Error::BranchNameAlreadyExists(ref name) => write!(fmt, "Branch {:?} already exists", name),
            Error::WrongFileHeader(ref h) => write!(fmt, "Wrong file header (possible branch corruption): {:?}", h),
            Error::FileNameCount(ref f) => write!(fmt, "Name {:?} doesn't have exactly one child", f),
        }
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::IO(ref e) => e.description(),
            Error::Sanakirja(ref e) => e.description(),
            Error::Bincode(ref e) => e.description(),
            Error::Utf8(ref e) => e.description(),
            Error::Serde(ref e) => e.description(),
            Error::OpenSSL(ref e) => e.description(),
            Error::OpenSSLStack(ref e) => e.description(),
            Error::Keys(ref e) => e.description(),
            Error::Base58Decode(ref e) => e.description(),
            Error::AlreadyAdded => "Already added",
            Error::FileNotInRepo(_) => "File not tracked",
            Error::NoDb(_) => "One of the tables is missing",
            Error::WrongHash => "Wrong hash",
            Error::EOF => "EOF",
            Error::WrongPatchSignature => "Wrong patch signature",
            Error::BranchNameAlreadyExists(_) => "Branch name already exists",
            Error::WrongFileHeader(_) => "Wrong file header (possible branch corruption)",
            Error::FileNameCount(_) => "A file name doesn't have exactly one child",
        }
    }
}

impl Error {
    pub fn lacks_space(&self) -> bool {
        match *self {
            Error::Sanakirja(sanakirja::Error::NotEnoughSpace) => true,
            _ => false,
        }
    }
}

#[macro_use]
mod backend;
mod file_operations;
pub mod fs_representation;

pub mod patch;

pub mod apply;
mod conflict;
pub mod graph;
mod optimal_diff;
mod output;
mod record;
mod unrecord;

pub use backend::{ApplyTimestamp, Branch, Edge, EdgeFlags, FileId, FileMetadata, FileStatus,
                  GenericTxn, Hash, HashRef, Inode, Key, LineId, MutTxn, OwnedFileId, PatchId,
                  Repository, SmallStr, SmallString, Txn, DEFAULT_BRANCH, ROOT_INODE, ROOT_KEY};

use fs_representation::ID_LENGTH;
pub use output::{Prefixes, ToPrefixes};
pub use patch::{Patch, PatchHeader};
use rand::Rng;
use rand::distributions::Alphanumeric;
pub use record::{InodeUpdate, RecordState};
pub use sanakirja::value::Value;
use std::io::Read;

impl<'env, T: rand::Rng> backend::MutTxn<'env, T> {
    pub fn output_changes_file<P: AsRef<Path>>(&mut self, branch: &Branch, path: P) -> Result<()> {
        let changes_file =
            fs_representation::branch_changes_file(path.as_ref(), branch.name.as_str());
        let mut branch_id: Vec<u8> = vec![b'\n'; ID_LENGTH + 1];
        {
            if let Ok(mut file) = std::fs::File::open(&changes_file) {
                file.read_exact(&mut branch_id)?;
            }
        }
        let mut branch_id = if let Ok(s) = String::from_utf8(branch_id) {
            s
        } else {
            "\n".to_string()
        };
        if branch_id.as_bytes()[0] == b'\n' {
            branch_id.truncate(0);
            let mut rng = rand::thread_rng();
            branch_id.extend(rng.sample_iter(&Alphanumeric).take(ID_LENGTH));
            branch_id.push('\n');
        }

        let mut file = std::fs::File::create(&changes_file)?;
        file.write_all(&branch_id.as_bytes())?;
        for (s, hash) in self.iter_applied(&branch, None) {
            let hash_ext = self.get_external(hash).unwrap();
            writeln!(file, "{}:{}", hash_ext.to_base58(), s)?
        }
        Ok(())
    }

    pub fn branch_patches(&mut self, branch: &Branch) -> HashSet<(backend::Hash, ApplyTimestamp)> {
        self.iter_patches(branch, None)
            .map(|(patch, time)| (self.external_hash(patch).to_owned(), time))
            .collect()
    }

    pub fn fork(&mut self, branch: &Branch, new_name: &str) -> Result<Branch> {
        if branch.name.as_str() == new_name {
            Err(Error::BranchNameAlreadyExists(new_name.to_string()))
        } else {
            Ok(Branch {
                db: self.txn.fork(&mut self.rng, &branch.db)?,
                patches: self.txn.fork(&mut self.rng, &branch.patches)?,
                revpatches: self.txn.fork(&mut self.rng, &branch.revpatches)?,
                name: SmallString::from_str(new_name),
                apply_counter: branch.apply_counter,
            })
        }
    }
    pub fn add_file<P: AsRef<Path>>(&mut self, path: P, is_dir: bool) -> Result<()> {
        self.add_inode(None, path.as_ref(), is_dir)
    }

    fn file_nodes_fold_<A, F: FnMut(A, Key<PatchId>) -> A>(
        &self,
        branch: &Branch,
        root: Key<PatchId>,
        level: usize,
        mut init: A,
        f: &mut F,
    ) -> Result<A> {
        for v in self.iter_adjacent(&branch, root, EdgeFlags::empty(), EdgeFlags::all())
            .take_while(|v| { v.flag.contains(EdgeFlags::FOLDER_EDGE)
                              && !v.flag.contains(EdgeFlags::PARENT_EDGE)
            }) {
            debug!("file_nodes_fold_: {:?} {:?}", root, v);
            if level & 1 == 0 && level > 0 {
                init = f(init, root)
            }
            init = self.file_nodes_fold_(branch, v.dest, level + 1, init, f)?
        }
        Ok(init)
    }

    pub fn file_nodes_fold<A, F: FnMut(A, Key<PatchId>) -> A>(
        &self,
        branch: &Branch,
        init: A,
        mut f: F,
    ) -> Result<A> {
        self.file_nodes_fold_(branch, ROOT_KEY, 0, init, &mut f)
    }
}

impl<T: Transaction, R> backend::GenericTxn<T, R> {
    /// Tells whether a `key` is alive in `branch`, i.e. is either the
    /// root, or all its ingoing edges are alive.
    pub fn is_alive(&self, branch: &Branch, key: Key<PatchId>) -> bool {
        debug!("is_alive {:?}?", key);
        let mut alive = key.is_root();
        let e = Edge::zero(EdgeFlags::PARENT_EDGE);
        for (k, v) in self.iter_nodes(&branch, Some((key, Some(e)))) {
            if k != key {
                break;
            }
            alive = alive
                || (!v.flag.contains(EdgeFlags::DELETED_EDGE)
                    && !v.flag.contains(EdgeFlags::PSEUDO_EDGE))
        }
        alive
    }

    /// Tells whether a `key` is alive or zombie in `branch`, i.e. is
    /// either the root, or has at least one of its incoming alive
    /// edge is alive.
    pub fn is_alive_or_zombie(&self, branch: &Branch, key: Key<PatchId>) -> bool {
        debug!("is_alive_or_zombie {:?}?", key);
        if key == ROOT_KEY {
            return true;
        }
        let e = Edge::zero(EdgeFlags::PARENT_EDGE);
        for (k, v) in self.iter_nodes(&branch, Some((key, Some(e)))) {
            if k != key {
                break;
            }
            debug!("{:?}", v);
            if v.flag.contains(EdgeFlags::PARENT_EDGE) && !v.flag.contains(EdgeFlags::DELETED_EDGE)
            {
                return true;
            }
        }
        false
    }

    /// Test whether `key` has a neighbor with flag `flag0`. If
    /// `include_pseudo`, this includes pseudo-neighbors.
    pub fn has_edge(
        &self,
        branch: &Branch,
        key: Key<PatchId>,
        min: EdgeFlags,
        max: EdgeFlags,
    ) -> bool {
        let e = Edge::zero(min);
        if let Some((k, v)) = self.iter_nodes(&branch, Some((key, Some(e)))).next() {
            debug!("has_edge {:?}", v.flag);
            k == key && (v.flag <= max)
        } else {
            false
        }
    }

    /// Tells which paths (of folder nodes) a key is in.
    pub fn get_file<'a>(&'a self, branch: &Branch, key: Key<PatchId>) -> Vec<Key<PatchId>> {
        let mut stack = vec![key.to_owned()];
        let mut seen = HashSet::new();
        let mut names = Vec::new();
        loop {
            match stack.pop() {
                None => break,
                Some(key) if !seen.contains(&key) => {
                    debug!("key {:?}, None", key);
                    seen.insert(key.clone());

                    for v in self.iter_adjacent(branch, key, EdgeFlags::empty(), EdgeFlags::all())
                    {
                        debug!("all_edges: {:?}", v);
                    }
                    for v in self.iter_adjacent(branch, key, EdgeFlags::PARENT_EDGE, EdgeFlags::all())
                    {
                        debug!("get_file {:?}", v);
                        if v.flag | EdgeFlags::PSEUDO_EDGE
                            == EdgeFlags::PARENT_EDGE | EdgeFlags::PSEUDO_EDGE
                        {
                            debug!("push!");
                            stack.push(v.dest.clone())
                        } else if v.flag
                            .contains(EdgeFlags::PARENT_EDGE | EdgeFlags::FOLDER_EDGE)
                        {
                            names.push(key);
                        }
                    }
                }
                _ => {}
            }
        }
        debug!("get_file returning {:?}", names);
        names
    }

    pub fn get_file_names<'a>(
        &'a self,
        branch: &Branch,
        key: Key<PatchId>,
    ) -> Vec<(Key<PatchId>, Vec<&'a str>)> {
        let mut names = vec![(key, Vec::new())];
        debug!("inode: {:?}", names);
        // Go back to the root.
        let mut next_names = Vec::new();
        let mut only_roots = false;
        let mut inodes = HashSet::new();
        while !only_roots {
            next_names.clear();
            only_roots = true;
            for (inode, names) in names.drain(..) {
                if !inodes.contains(&inode) {
                    inodes.insert(inode.clone());

                    if inode != ROOT_KEY {
                        only_roots = false;
                    }
                    let names_ = self.file_names(branch, inode);
                    if names_.is_empty() {
                        next_names.push((inode, names));
                        break;
                    } else {
                        debug!("names_ = {:?}", names_);
                        for (inode_, _, base) in names_ {
                            let mut names = names.clone();
                            names.push(base);
                            next_names.push((inode_, names))
                        }
                    }
                }
            }
            std::mem::swap(&mut names, &mut next_names)
        }
        debug!("end: {:?}", names);
        for &mut (_, ref mut name) in names.iter_mut() {
            name.reverse()
        }
        names
    }
}

fn make_remote<'a, I: Iterator<Item = &'a Hash>>(
    target: &Path,
    remote: I,
) -> Result<(Vec<(Hash, Patch)>, usize)> {
    use fs_representation::*;
    use std::fs::File;
    use std::io::BufReader;
    let mut patches = Vec::new();
    let mut patches_dir = patches_dir(target).to_path_buf();
    let mut size_increase = 0;

    for h in remote {
        patches_dir.push(&patch_file_name(h.as_ref()));

        debug!("opening {:?}", patches_dir);
        let file = try!(File::open(&patches_dir));
        let mut file = BufReader::new(file);
        let (h, _, patch) = Patch::from_reader_compressed(&mut file)?;

        size_increase += patch.size_upper_bound();
        patches.push((h.clone(), patch));

        patches_dir.pop();
    }
    Ok((patches, size_increase))
}

/// Apply a number of patches, guessing the new repository size.  If
/// this fails, the repository size is guaranteed to have been
/// increased by at least some pages, and it is safe to call this
/// function again.
///
/// Also, this function takes a file lock on the repository.
pub fn apply_resize<'a, I, F, P: output::ToPrefixes>(
    target: &Path,
    branch_name: &str,
    remote: I,
    partial_paths: P,
    apply_cb: F,
) -> Result<()>
where
    I: Iterator<Item = &'a Hash>,
    F: FnMut(usize, &Hash),
{
    let (patches, size_increase) = make_remote(target, remote)?;
    apply_resize_patches(
        target,
        branch_name,
        &patches,
        size_increase,
        partial_paths,
        apply_cb,
    )
}

/// A version of `apply_resize` with the patches list already loaded.
pub fn apply_resize_patches<'a, F, P: output::ToPrefixes>(
    target: &Path,
    branch_name: &str,
    patches: &[(Hash, Patch)],
    size_increase: usize,
    partial_paths: P,
    apply_cb: F,
) -> Result<()>
where
    F: FnMut(usize, &Hash),
{
    use fs_representation::*;
    info!("applying patches with size_increase {:?}", size_increase);
    let pristine_dir = pristine_dir(target).to_path_buf();
    let repo = Repository::open(pristine_dir, Some(size_increase as u64))?;
    let mut txn = repo.mut_txn_begin(rand::thread_rng())?;
    let mut branch = txn.open_branch(branch_name)?;
    txn.apply_patches(&mut branch, target, &patches, partial_paths, apply_cb)?;
    txn.commit_branch(branch)?;
    txn.commit().map_err(Into::into)
}

/// Apply a number of patches, guessing the new repository size.  If
/// this fails, the repository size is guaranteed to have been
/// increased by at least some pages, and it is safe to call this
/// function again.
///
/// Also, this function takes a file lock on the repository.
pub fn apply_resize_no_output<'a, F, I>(
    target: &Path,
    branch_name: &str,
    remote: I,
    apply_cb: F,
) -> Result<()>
where
    I: Iterator<Item = &'a Hash>,
    F: FnMut(usize, &Hash),
{
    let (patches, size_increase) = make_remote(target, remote)?;
    apply_resize_patches_no_output(target, branch_name, &patches, size_increase, apply_cb)
}

pub fn apply_resize_patches_no_output<'a, F>(
    target: &Path,
    branch_name: &str,
    patches: &[(Hash, Patch)],
    size_increase: usize,
    mut apply_cb: F,
) -> Result<()>
where
    F: FnMut(usize, &Hash),
{
    use fs_representation::*;
    debug!("apply_resize_no_output: patches = {:?}", patches);
    let pristine_dir = pristine_dir(target).to_path_buf();
    let repo = try!(Repository::open(pristine_dir, Some(size_increase as u64)));
    let mut txn = try!(repo.mut_txn_begin(rand::thread_rng()));
    let mut branch = txn.open_branch(branch_name)?;
    let mut new_patches_count = 0;
    for &(ref p, ref patch) in patches.iter() {
        debug!("apply_patches: {:?}", p);
        txn.apply_patches_rec(&mut branch, &patches, p, patch, &mut new_patches_count)?;
        apply_cb(new_patches_count, p);
    }
    info!("branch: {:?}", branch);
    txn.commit_branch(branch)?;
    txn.commit()?;
    Ok(())
}

/// Open the repository, and unrecord the patch, without increasing
/// the size. If this fails, the repository file is guaranteed to have
/// been increased by `increase` bytes.
pub fn unrecord_no_resize(
    repo_dir: &Path,
    repo_root: &Path,
    branch_name: &str,
    selected: &mut Vec<(Hash, Patch)>,
    increase: u64,
) -> Result<()> {
    debug!("unrecord_no_resize: {:?}", repo_dir);
    let repo = try!(Repository::open(repo_dir, Some(increase)));

    let mut txn = try!(repo.mut_txn_begin(rand::thread_rng()));
    let mut branch = txn.open_branch(branch_name)?;
    let mut timestamps = Vec::new();
    while let Some((hash, patch)) = selected.pop() {
        let internal = txn.get_internal(hash.as_ref()).unwrap().to_owned();
        debug!("Unrecording {:?}", hash);
        if let Some(ts) = txn.get_patch(&branch.patches, internal) {
            timestamps.push(ts);
        }
        txn.unrecord(&mut branch, internal, &patch)?;
        debug!("Done unrecording {:?}", hash);
    }

    if let Err(e) = txn.output_changes_file(&branch, repo_root) {
        error!("no changes file: {:?}", e)
    }
    try!(txn.commit_branch(branch));
    try!(txn.commit());
    Ok(())
}
