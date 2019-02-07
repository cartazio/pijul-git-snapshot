use clap::{Arg, ArgMatches, SubCommand};

use super::{ask, default_explain, get_current_branch, BasicOptions, StaticSubcommand,
            validate_base58};
use meta::{load_global_or_local_signing_key, Global, Meta};
use std::collections::HashSet;
use std::path::Path;

use chrono;
use libpijul::fs_representation::{patch_file_name, patches_dir};
use libpijul::patch::{Patch, PatchFlags};
use libpijul::{apply_resize, apply_resize_no_output, Hash, HashRef, PatchId};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::iter;
use std::mem::drop;
use std::str::FromStr;

use commands::record::{decide_authors, decide_patch_message, record_args};
use error::Error;

pub fn invocation() -> StaticSubcommand {
    record_args(
        SubCommand::with_name("rollback").arg(
            Arg::with_name("patch")
                .help("Patch to roll back.")
                .takes_value(true)
                .multiple(true)
                .validator(validate_base58),
        ),
    )
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    let opts = BasicOptions::from_args(args)?;
    let patches: Option<HashSet<Hash>> = args.values_of("patch")
        .map(|ps| ps.map(|x| Hash::from_base58(x).unwrap()).collect());

    let mut increase = 409600;
    let repo = opts.open_and_grow_repo(increase)?;
    let branch_name = opts.branch();

    let mut patches: HashMap<_, _> = if let Some(ref patches) = patches {
        let txn = repo.txn_begin()?;
        if let Some(branch) = txn.get_branch(&branch_name) {
            let mut patches_ = HashMap::new();
            for h in patches.iter() {
                debug!("unrecording {:?}", h);

                if let Some(internal) = txn.get_internal(h.as_ref()) {
                    if txn.get_patch(&branch.patches, internal).is_some() {
                        let patch = load_patch(&opts.repo_root, h.as_ref());
                        patches_.insert(h.to_owned(), patch);
                        continue;
                    }
                }
                return Err(Error::BranchDoesNotHavePatch {
                    branch_name: branch.name.as_str().to_string(),
                    patch: h.to_owned(),
                });
            }
            patches_
        } else {
            HashMap::new()
        }
    } else {
        let mut patches: Vec<_> = {
            let txn = repo.txn_begin()?;
            if let Some(branch) = txn.get_branch(&branch_name) {
                txn.rev_iter_applied(&branch, None)
                    .map(|(t, h)| {
                        let ext = txn.get_external(h).unwrap();
                        let patch = load_patch(&opts.repo_root, ext);
                        (ext.to_owned(), Some(h.to_owned()), patch, t)
                    })
                    .collect()
            } else {
                Vec::new()
            }
        };
        patches.sort_by(|&(_, _, _, a), &(_, _, _, b)| b.cmp(&a));
        let patches: Vec<(Hash, Option<PatchId>, Patch)> =
            patches.into_iter().map(|(a, b, c, _)| (a, b, c)).collect();
        // debug!("patches: {:?}", patches);
        let to_unrecord = ask::ask_patches(ask::Command::Unrecord, &patches).unwrap();
        debug!("to_unrecord: {:?}", to_unrecord);
        let patches: HashMap<_, _> = patches
            .into_iter()
            .filter(|&(ref k, _, _)| to_unrecord.contains(&k))
            .map(|(k, _, p)| (k, p))
            .collect();
        patches
    };

    let mut selected = Vec::new();
    loop {
        let hash = if let Some((hash, patch)) = patches.iter().next() {
            increase += patch.size_upper_bound() as u64;
            hash.to_owned()
        } else {
            break;
        };
        deps_dfs(&mut selected, &mut patches, &hash)
    }

    // Create the inverse changes.
    let mut changes = Vec::new();
    for &(ref hash, ref patch) in selected.iter() {
        debug!("inverting {:?}", patch);
        patch.inverse(hash, &mut changes)
    }

    let meta = Meta::load(&opts.repo_root).unwrap_or_else(|_| Meta::new());
    let global = Global::load().unwrap_or_else(|_| Global::new());

    // Create the inverse patch, and save it.
    let patch = {
        let authors_arg = args.values_of("author").map(|x| x.collect::<Vec<_>>());
        let patch_name_arg = args.value_of("message");
        let patch_descr_arg = args.value_of("description");

        let txn = repo.txn_begin()?;
        let authors = decide_authors(authors_arg, &meta, &global)?;

        let patch_date = args.value_of("date").map_or(Ok(chrono::Utc::now()), |x| {
            chrono::DateTime::from_str(x).map_err(|_| Error::InvalidDate { date: String::from(x) })
        })?;

        let (name, description) = decide_patch_message(
            patch_name_arg,
            patch_descr_arg,
            String::from(""),
            !args.is_present("no-editor"),
            &opts.repo_root,
            &meta,
            &global,
        )?;

        if let Some(branch) = txn.get_branch(&branch_name) {
            txn.new_patch(
                &branch,
                authors,
                name,
                description,
                patch_date,
                changes,
                iter::empty(),
                PatchFlags::empty(),
            )
        } else {
            unimplemented!()
        }
    };
    let patches_dir = patches_dir(&opts.repo_root);
    let dot_pijul = opts.repo_dir();
    let key = if let Ok(Some(key)) = meta.signing_key() {
        Some(key)
    } else {
        load_global_or_local_signing_key(Some(&dot_pijul)).ok()
    };
    let hash = patch.save(&patches_dir, key.as_ref())?;
    drop(repo);
    println!("Recorded patch {}", hash.to_base58());

    let is_current_branch = if let Ok(br) = get_current_branch(&opts.repo_root) {
        br == opts.branch()
    } else {
        false
    };

    // Apply the inverse patch.
    loop {
        let app = if !is_current_branch {
            apply_resize_no_output(
                &opts.repo_root,
                &opts.branch(),
                iter::once(&hash),
                |_, _| (),
            )
        } else {
            apply_resize(
                &opts.repo_root,
                &opts.branch(),
                iter::once(&hash),
                &[] as &[&str],
                |_, _| (),
            )
        };
        match app {
            Err(ref e) if e.lacks_space() => {}
            Ok(()) => return Ok(()),
            Err(e) => return Err(From::from(e)),
        }
    }
}

fn load_patch(repo_root: &Path, ext: HashRef) -> Patch {
    let base = patch_file_name(ext);
    let filename = patches_dir(repo_root).join(&base);
    debug!("filename: {:?}", filename);
    let file = File::open(&filename).unwrap();
    let mut file = BufReader::new(file);
    let (_, _, patch) = Patch::from_reader_compressed(&mut file).unwrap();
    patch
}

fn deps_dfs(selected: &mut Vec<(Hash, Patch)>, patches: &mut HashMap<Hash, Patch>, current: &Hash) {
    if let Some(patch) = patches.remove(current) {
        for dep in patch.dependencies().iter() {
            deps_dfs(selected, patches, dep)
        }

        selected.push((current.to_owned(), patch))
    }
}

pub fn explain(res: Result<(), Error>) {
    default_explain(res)
}
