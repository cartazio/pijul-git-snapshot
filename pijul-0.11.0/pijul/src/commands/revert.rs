use super::ask::{ask_changes, ChangesDirection};
use super::record;
use chrono;
use clap::{Arg, ArgMatches, SubCommand};
use commands::{default_explain, BasicOptions, StaticSubcommand};
use libpijul::patch::{Patch, PatchFlags, UnsignedPatch};
use libpijul::{Inode, InodeUpdate, Repository, ToPrefixes};
use rand;
use std;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use error::Error;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("revert")
        .about("Rewrite the working copy from the pristine")
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .takes_value(true)
                .help("Local repository."),
        )
        .arg(
            Arg::with_name("all")
                .short("a")
                .long("all")
                .help("Answer 'y' to all questions")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("branch")
                .help("Branch to revert to.")
                .long("branch")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("prefix")
                .help("Prefix to start from")
                .takes_value(true)
                .multiple(true),
        );
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    let opts = BasicOptions::from_args(args)?;
    let yes_to_all = args.is_present("all");
    let branch_name = opts.branch();
    let prefix = record::prefix(args, &opts)?;
    // Generate the pending patch.
    let (pending, pending_syncs): (_, HashSet<_>) = if !yes_to_all || prefix.is_some() {
        let repo = opts.open_and_grow_repo(409600)?;
        let mut txn = repo.mut_txn_begin(rand::thread_rng())?;
        let (changes, syncs) = {
            let (changes, syncs) = record::changes_from_prefixes(
                &opts.repo_root,
                &mut txn,
                &branch_name,
                prefix.as_ref(),
            )?;
            let changes: Vec<_> = changes
                .into_iter()
                .map(|x| txn.globalize_record(x))
                .collect();
            if yes_to_all {
                (Vec::new(), HashSet::new())
            } else {
                let (c, _empty_vec) = ask_changes(
                    &txn,
                    &opts.repo_root,
                    &opts.cwd,
                    &changes,
                    ChangesDirection::Revert,
                    &mut HashSet::new(),
                )?;
                let selected = changes
                    .into_iter()
                    .enumerate()
                    .filter(|&(i, _)| *(c.get(&i).unwrap_or(&false)))
                    .map(|(_, x)| x)
                    .collect();
                (selected, syncs)
            }
        };
        debug!("changes {:?}", changes);
        debug!("syncs {:?}", syncs);
        let branch = txn.get_branch(&branch_name).unwrap();
        let changes = changes.into_iter().flat_map(|x| x.into_iter()).collect();
        let patch = txn.new_patch(
            &branch,
            Vec::new(),
            String::new(),
            None,
            chrono::Utc::now(),
            changes,
            std::iter::empty(),
            PatchFlags::empty(),
        );
        txn.commit()?;
        (patch, syncs)
    } else {
        (UnsignedPatch::empty().leave_unsigned(), HashSet::new())
    };

    let mut size_increase = None;
    let pristine = opts.pristine_dir();
    loop {
        match output_repository(
            &opts.repo_root,
            &pristine,
            &branch_name,
            size_increase,
            prefix.as_ref(),
            &pending,
            &pending_syncs,
        ) {
            Err(ref e) if e.lacks_space() => {
                size_increase = Some(Repository::repository_size(&pristine).unwrap())
            }
            e => return e,
        }
    }
}

fn output_repository(
    r: &Path,
    pristine_dir: &Path,
    branch: &str,
    size_increase: Option<u64>,
    prefixes: Option<&Vec<PathBuf>>,
    pending: &Patch,
    pending_syncs: &HashSet<InodeUpdate>,
) -> Result<(), Error> {
    let repo = Repository::open(&pristine_dir, size_increase)?;
    let mut txn = repo.mut_txn_begin(rand::thread_rng())?;

    let mut inode_prefixes = Vec::new();
    if let Some(prefixes) = prefixes {
        for pref in prefixes.iter() {
            inode_prefixes.push(txn.find_inode(pref).unwrap());
        }
    }
    for (_, key) in txn.iter_partials(branch)
        .take_while(|&(k, _)| k.as_str() == branch)
    {
        debug!("extra inode prefixes: {:?}", key);
        inode_prefixes.push(txn.get_revinodes(key).unwrap())
    }

    let mut branch = txn.open_branch(branch)?;
    let pref = (&inode_prefixes as &[Inode]).to_prefixes(&txn, &branch);
    debug!("{:?}", pref);
    txn.output_repository(&mut branch, &r, &pref, pending, pending_syncs)?;
    txn.commit_branch(branch)?;
    txn.commit()?;
    Ok(())
}

pub fn explain(res: Result<(), Error>) {
    default_explain(res)
}
