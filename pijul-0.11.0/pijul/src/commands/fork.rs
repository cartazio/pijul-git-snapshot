use clap::{Arg, ArgGroup, ArgMatches, SubCommand};
use commands::checkout::checkout;
use libpijul::fs_representation::{read_patch, set_current_branch};
use libpijul::{apply_resize_no_output, Hash};
use rand;
use std::mem;
use std::path::PathBuf;

use super::{default_explain, BasicOptions, StaticSubcommand};
use error::Error;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("fork")
        .about("Create a new branch")
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .help("Local repository.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("branch")
                .long("branch")
                .help("Branch.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("patch")
                .long("patch")
                .help("A patch hash, preferably a tag.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("to")
                .help("Name of the new branch.")
                .takes_value(true)
                .required(true),
        )
        .group(
            ArgGroup::with_name("source")
                .required(false)
                .args(&["branch", "patch"]),
        );
}

fn patch_dependencies(hash_str: &str, repo_root: &PathBuf) -> Result<Vec<Hash>, Error> {
    let mut deps = Vec::new();
    let mut current = vec![
        Hash::from_base58(hash_str).ok_or::<Error>(Error::WrongHash)?,
    ];
    let mut next = Vec::new();

    while !current.is_empty() {
        for hash in current.drain(..) {
            deps.push(hash.clone());
            let patch = read_patch(&repo_root, hash.as_ref())?;

            for hash_dep in patch.dependencies().iter() {
                let h = hash_dep.to_owned();

                if !deps.contains(&h) {
                    next.push(h);
                }
            }
        }

        mem::swap(&mut next, &mut current);
    }

    deps.reverse();

    Ok(deps)
}

pub fn has_branch(opts: &BasicOptions, branch_name: &str) -> Result<bool, Error> {
    let repo = opts.open_repo()?;
    let txn = repo.txn_begin()?;

    Ok(txn.has_branch(branch_name))
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    let opts = BasicOptions::from_args(args)?;
    let to = args.value_of("to").unwrap();

    if !has_branch(&opts, to)? {
        if let Some(ref hash) = args.value_of("patch") {
            debug!(
                "Creating a new branch {:?} with dependencies of {:?}",
                to, hash
            );

            let deps = patch_dependencies(hash, &opts.repo_root)?;

            apply_resize_no_output(&opts.repo_root, to, deps.iter(), |_, _| ())?;

            println!("Branch {:?} has been created.", to);

            checkout(&opts, to, false, None)
        } else {
            let repo = opts.open_repo()?;
            let mut txn = repo.mut_txn_begin(rand::thread_rng())?;

            let br = opts.branch();
            let branch = txn.open_branch(&br)?;
            let new_branch = txn.fork(&branch, to)?;

            txn.commit_branch(branch)?;
            txn.commit_branch(new_branch)?;

            let partials = txn.iter_partials(&br)
                .take_while(|&(k, _)| k.as_str() == &br)
                .map(|(_, v)| v)
                .collect::<Vec<_>>();
            for &key in partials.iter() {
                txn.put_partials(to, key)?;
            }
            txn.commit()?;

            set_current_branch(&opts.repo_root, to)?;

            Ok(())
        }
    } else {
        Err(Error::BranchAlreadyExists)
    }
}

pub fn explain(res: Result<(), Error>) {
    default_explain(res)
}
