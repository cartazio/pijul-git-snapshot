use clap::{Arg, ArgMatches, SubCommand};
use commands::{default_explain, BasicOptions, StaticSubcommand};
use error::Error;
use libpijul::fs_representation::untracked_files;
use libpijul::patch::Record;
use libpijul::{MutTxn, RecordState};
use rand;
use relativize::relativize;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

const UNRECORDED_FILES: &'static str = r#"
Changes not yet recorded:
  (use "pijul record ..." to record a new patch)
"#;

const UNTRACKED_FILES: &'static str = r#"
Untracked files:
  (use "pijul add <file>..." to track them)
"#;

const CONFLICTED_FILES: &'static str = r#"
Unresolved conflicts:
  (fix conflicts and record the resolution with "pijul record ...")
"#;

pub fn invocation() -> StaticSubcommand {
    SubCommand::with_name("status")
        .about("Show working tree status")
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .takes_value(true)
                .help("Local repository."),
        )
        .arg(
            Arg::with_name("short")
                .long("short")
                .short("s")
                .help("Output in short format"),
        )
}

pub fn explain(r: Result<(), Error>) {
    default_explain(r)
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    let opts = BasicOptions::from_args(args)?;
    let current_branch = opts.branch();
    let repo = opts.open_and_grow_repo(409600)?;
    let short = args.is_present("short");

    let (unrecorded, untracked, conflicts) = {
        let mut txn = repo.mut_txn_begin(rand::thread_rng())?;
        let unrecorded = unrecorded_changes(&mut txn, &opts.repo_root, &current_branch)?;

        let untracked = untracked_files(&txn, &opts.repo_root);
        let conflicts = txn.list_conflict_files(&current_branch, &[])?;
        (unrecorded, untracked, conflicts)
    };

    if short {
        print_shortstatus(&opts.cwd, &opts.repo_root, unrecorded, untracked, conflicts);
    } else {
        print_longstatus(
            &current_branch,
            &opts.repo_root,
            &opts.cwd,
            unrecorded,
            untracked,
            conflicts,
        );
    }
    Ok(())
}

fn print_longstatus(
    branch: &str,
    repo_root: &PathBuf,
    cwd: &Path,
    changed: Vec<(Rc<PathBuf>, ChangeType)>,
    untracked: HashSet<PathBuf>,
    conflicts: Vec<PathBuf>,
) {
    println!("On branch {}", branch);
    if changed.is_empty() && untracked.is_empty() && conflicts.is_empty() {
        println!("Nothing to record, working tree clean");
    }

    if !conflicts.is_empty() {
        println!("{}", CONFLICTED_FILES);
        for f in conflicts {
            println!(
                "        {}",
                relativize(&cwd, &repo_root.as_path().join(f.as_path())).display()
            );
        }
    }

    if !changed.is_empty() {
        println!("{}", UNRECORDED_FILES);
        for (f, t) in changed {
            println!(
                "        {:10} {}",
                t.long(),
                relativize(&cwd, f.as_path()).display()
            );
        }
    }

    if !untracked.is_empty() {
        println!("{}", UNTRACKED_FILES);
        for f in untracked {
            println!("        {}", relativize(&cwd, f.as_path()).display());
        }
    }
}

fn print_shortstatus(
    cwd: &Path,
    repo_root: &PathBuf,
    changed: Vec<(Rc<PathBuf>, ChangeType)>,
    untracked: HashSet<PathBuf>,
    conflicts: Vec<PathBuf>,
) {
    for f in conflicts {
        println!(
            "C {}",
            relativize(&cwd, &repo_root.as_path().join(f.as_path())).display()
        );
    }
    for (f, t) in changed {
        println!("{} {}", t.short(), relativize(&cwd, f.as_path()).display());
    }
    for f in untracked {
        println!("? {}", relativize(&cwd, f.as_path()).display());
    }
}

#[derive(Debug)]
enum ChangeType {
    Modified,
    New,
    Del,
    Move(Rc<PathBuf>),
}

impl ChangeType {
    fn short(&self) -> &str {
        match *self {
            ChangeType::Modified => "M",
            ChangeType::New => "A",
            ChangeType::Del => "D",
            ChangeType::Move(_) => "→",
        }
    }

    fn long(&self) -> &str {
        match *self {
            ChangeType::Modified => "modified:",
            ChangeType::New => "new file:",
            ChangeType::Del => "deleted:",
            ChangeType::Move(_) => "moved:",
        }
    }
}

fn unrecorded_changes<T: rand::Rng>(
    txn: &mut MutTxn<T>,
    repo_root: &PathBuf,
    branch: &String,
) -> Result<Vec<(Rc<PathBuf>, ChangeType)>, Error> {
    let mut record = RecordState::new();
    let branch = txn.open_branch(branch)?;
    txn.record(&mut record, &branch, repo_root, None)?;
    txn.commit_branch(branch)?;
    let (changes, _) = record.finish();

    let mut ret = vec![];
    let mut current_file = None;

    for change in changes.iter() {
        match *change {
            Record::Change { ref file, .. } | Record::Replace { ref file, .. } => {
                if current_file.clone().map_or(true, |f| &f != file) {
                    ret.push((file.clone(), ChangeType::Modified));
                    current_file = Some(file.clone());
                }
            }
            Record::FileAdd { ref name, .. } => {
                let file = Rc::new(PathBuf::from(name.clone()));
                current_file = Some(file.clone());
                ret.push((file.clone(), ChangeType::New));
            }
            Record::FileDel { ref name, .. } => {
                let file = Rc::new(PathBuf::from(name.clone()));
                current_file = Some(file.clone());
                ret.push((file.clone(), ChangeType::Del));
            }
            Record::FileMove { ref new_name, .. } => {
                let file = Rc::new(PathBuf::from(new_name.clone()));
                current_file = Some(file.clone());
                ret.push((file.clone(), ChangeType::Move(file)));
            }
        }
    }
    Ok(ret)
}
