use clap::ArgMatches;
use commands::BasicOptions;
use libpijul;
use libpijul::Repository;
use rand;
use std::fs::{canonicalize, metadata, read_dir};
use std::mem::swap;
use std::path::{Path, PathBuf};
use error::Error;

#[derive(Debug, Clone, Copy)]
pub enum Operation {
    Add,
    Remove,
}

pub fn run(args: &ArgMatches, op: Operation) -> Result<(), Error> {
    debug!("fs_operation {:?}", op);
    let opts = BasicOptions::from_args(args)?;

    debug!("repo {:?}", opts.repo_root);
    let mut extra_space = 409600;
    let recursive = args.is_present("recursive");
    loop {
        let touched_files = match args.values_of("files") {
            Some(l) => l.map(|p| Path::new(p).to_owned()).collect(),
            None => vec![],
        };
        match really_run(
            &opts.pristine_dir(),
            &opts.cwd,
            &opts.repo_root,
            touched_files,
            recursive,
            op,
            extra_space,
        ) {
            Err(ref e) if e.lacks_space() => extra_space *= 2,
            e => return e,
        }
    }
}

fn really_run(
    repo_dir: &Path,
    wd: &Path,
    r: &Path,
    mut files: Vec<PathBuf>,
    recursive: bool,
    op: Operation,
    extra_space: u64,
) -> Result<(), Error> {
    debug!("files {:?}", files);
    let mut rng = rand::thread_rng();
    let repo = Repository::open(&repo_dir, Some(extra_space))?;
    let mut txn = repo.mut_txn_begin(&mut rng)?;
    let mut files_ = Vec::new();
    match op {
        Operation::Add => {
            while !files.is_empty() {
                for file_ in files.drain(..) {
                    let p = canonicalize(wd.join(&file_))?;
                    let m = metadata(&p)?;
                    if let Ok(file) = p.strip_prefix(r) {
                        match txn.add_file(&file, m.is_dir()) {
                            Ok(()) => {}
                            Err(libpijul::Error::AlreadyAdded) => {
                                eprintln!("{:?} is already in the repository", file_)
                            }
                            Err(e) => return Err(e.into()),
                        }

                        if recursive {
                            if let Ok(dir) = read_dir(&file) {
                                for file_ in dir.filter_map(|x| x.ok()) {
                                    match file_.file_type() {
                                        Ok(f) if f.is_dir() || f.is_file() => {
                                            files_.push(file_.path())
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    } else {
                        let err = file_.to_string_lossy().into_owned();
                        return Err(Error::InvalidPath { path: err });
                    }
                }
                swap(&mut files, &mut files_);
            }
        }
        Operation::Remove => {
            for file in &files[..] {
                debug!("file: {:?} {:?}", file, wd.join(file));
                let p = wd.join(file).canonicalize()?;
                debug!("p: {:?}", p);
                if let Ok(file) = p.strip_prefix(r) {
                    debug!("remove_file {:?}", file);
                    txn.remove_file(file)?
                } else {
                    let err = file.to_string_lossy().into_owned();
                    return Err(Error::InvalidPath { path: err });
                }
            }
        }
    }
    txn.commit()?;
    Ok(())
}
