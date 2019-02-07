use clap::{Arg, ArgMatches, SubCommand};
use commands::{BasicOptions, StaticSubcommand};
use error::Error;
use libpijul::RecordState;
use rand;
use std::fs::canonicalize;
use std::io::{stderr, Write};
use std::process::exit;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("diff")
        .about("Show what would be recorded if record were called")
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .help("The repository to show, defaults to the current directory.")
                .takes_value(true)
                .required(false),
        )
        .arg(
            Arg::with_name("branch")
                .long("branch")
                .help("The branch to show, defaults to the current branch.")
                .takes_value(true)
                .required(false),
        )
        .arg(
            Arg::with_name("prefix")
                .help("Prefix to start from")
                .takes_value(true)
                .multiple(true),
        );
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    super::setup_pager();

    let opts = BasicOptions::from_args(args)?;
    let repo = opts.open_repo()?;
    let mut txn = repo.mut_txn_begin(rand::thread_rng())?;
    let prefix = if let Some(prefix) = args.value_of("prefix") {
        let p = canonicalize(opts.cwd.join(prefix))?;
        if let Ok(file) = p.strip_prefix(&opts.repo_root) {
            Some(file.to_path_buf())
        } else {
            None
        }
    } else {
        None
    };
    let prefix = if let Some(ref prefix) = prefix {
        Some(prefix.as_path())
    } else {
        None
    };
    let mut record = RecordState::new();
    let branch = txn.open_branch(&opts.branch())?;
    txn.record(&mut record, &branch, &opts.repo_root, prefix)?;
    txn.commit_branch(branch)?;
    let (changes, _) = record.finish();
    let changes: Vec<_> = changes
        .into_iter()
        .map(|x| txn.globalize_record(x))
        .collect();
    super::ask::print_status(&txn, &opts.cwd, &changes)?;
    Ok(())
}

pub fn explain(res: Result<(), Error>) {
    match res {
        Ok(_) => (),
        Err(e) => {
            write!(stderr(), "error: {}", e).unwrap();
            exit(1)
        }
    }
}
