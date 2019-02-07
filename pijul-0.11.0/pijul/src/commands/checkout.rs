use clap::{Arg, ArgMatches, SubCommand};

use super::{default_explain, BasicOptions, StaticSubcommand};
use libpijul::fs_representation::{get_current_branch, set_current_branch};
use libpijul::patch::UnsignedPatch;
use libpijul::{FileStatus, RecordState, ToPrefixes};
use rand;
use error::Error;
use std::collections::HashSet;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("checkout")
        .about("Change the current branch")
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .help("Local repository.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("branch")
                .help("Branch to switch to.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("path")
                .long("path")
                .help("Partial path to check out.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("force")
                .short("f")
                .long("force")
                .takes_value(false)
                .help("Only check files moves, deletions and additions (much faster)."),
        );
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    let opts = BasicOptions::from_args(args)?;
    if let Some(branch) = args.value_of("branch") {
        checkout(
            &opts,
            branch,
            args.is_present("force"),
            args.value_of("path"),
        )
    } else {
        Err(Error::NoSuchBranch)
    }
}

pub fn checkout(
    opts: &BasicOptions,
    branch: &str,
    force: bool,
    partial_path: Option<&str>,
) -> Result<(), Error> {
    let mut force = force;
    let mut provision = 409600;

    loop {
        match try_checkout(opts, branch, force, provision, partial_path) {
            Err(ref e) if e.lacks_space() => {
                provision = provision * 2;
                force = true;
            }
            x => return x,
        }
    }
}

pub fn try_checkout(
    opts: &BasicOptions,
    branch_name: &str,
    force: bool,
    provision: u64,
    partial_path: Option<&str>,
) -> Result<(), Error> {
    let repo = opts.open_and_grow_repo(provision)?;
    let mut txn = repo.mut_txn_begin(rand::thread_rng())?;
    let current_branch = get_current_branch(&opts.repo_root)?;
    // We need to check at least that there are no file
    // moves/additions/deletions, because these would be
    // overwritten by the checkout, sometimes causing Pijul to
    // panic.
    if force {
        // Check whether there are file moves.
        if txn.iter_inodes(None)
            .any(|(_, ch)| ch.status != FileStatus::Ok)
        {
            return Err(Error::PendingChanges);
        }
    } else {
        // Check whether there are more general changes.
        let mut record = RecordState::new();
        let current_branch = txn.open_branch(&current_branch)?;
        txn.record(&mut record, &current_branch, &opts.repo_root, None)?;
        txn.commit_branch(current_branch)?;
        let (changes, _) = record.finish();

        if !changes.is_empty() {
            return Err(Error::PendingChanges);
        }
    }

    debug!("output repository");

    let mut branch = if let Some(branch) = txn.get_branch(branch_name) {
        branch
    } else {
        return Err(Error::NoSuchBranch);
    };
    let pref = if let Some(partial) = partial_path {
        (&[partial][..]).to_prefixes(&txn, &branch)
    } else {
        (&[][..] as &[&str]).to_prefixes(&txn, &branch)
    };
    txn.output_repository(
        &mut branch,
        &opts.repo_root,
        &pref,
        &UnsignedPatch::empty().leave_unsigned(),
        &HashSet::new(),
    )?;
    txn.commit_branch(branch)?;

    txn.commit()?;

    set_current_branch(&opts.repo_root, branch_name)?;

    println!("Current branch: {:?}", get_current_branch(&opts.repo_root)?);
    Ok(())
}

pub fn explain(res: Result<(), Error>) {
    default_explain(res)
}
