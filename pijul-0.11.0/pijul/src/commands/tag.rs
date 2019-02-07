use chrono;
use clap::{ArgMatches, SubCommand};
use commands::hooks::run_hook;
use commands::record::{decide_authors, decide_patch_message, record_args};
use commands::{BasicOptions, StaticSubcommand};
use libpijul::fs_representation::patches_dir;
use libpijul::patch::PatchFlags;
use libpijul::Hash;
use std::collections::HashSet;
use std::mem::drop;
use error::Error;
use super::default_explain;
use super::record;
use meta::{load_global_or_local_signing_key, Global, Meta};

pub fn invocation() -> StaticSubcommand {
    record_args(SubCommand::with_name("tag")
        .about("Create a patch (a \"tag\") with no changes, and all currently applied patches as dependencies"))
}

pub fn run(args: &ArgMatches) -> Result<Option<Hash>, Error> {
    let opts = BasicOptions::from_args(args)?;
    let patch_name_arg = args.value_of("message");
    let patch_descr_arg = args.value_of("description");
    let authors_arg = args.values_of("author").map(|x| x.collect::<Vec<_>>());
    let branch_name = opts.branch();

    let mut save_meta = false;
    let mut save_global = false;

    let mut global = Global::load().unwrap_or_else(|e| {
        info!("loading global key, error {:?}", e);
        save_global = true;
        Global::new()
    });

    let mut meta = match Meta::load(&opts.repo_root) {
        Ok(m) => m,
        Err(_) => {
            save_meta = true;
            Meta::new()
        }
    };

    let repo = opts.open_repo()?;
    let patch = {
        let txn = repo.txn_begin()?;
        debug!("meta:{:?}", meta);

        let authors = decide_authors(authors_arg, &meta, &global)?;

        if meta.authors.len() == 0 {
            meta.authors = authors.clone();
            save_meta = true;
        }

        if global.author.len() == 0 {
            global.author = authors[0].clone();
            save_global = true;
        }

        debug!("authors:{:?}", authors);

        let (patch_name, description) = decide_patch_message(
            patch_name_arg,
            patch_descr_arg,
            String::from(""),
            !args.is_present("no-editor"),
            &opts.repo_root,
            &meta,
            &global,
        )?;

        run_hook(&opts.repo_root, "patch-name", Some(&patch_name))?;

        debug!("patch_name:{:?}", patch_name);
        if save_meta {
            meta.save(&opts.repo_root)?
        }
        if save_global {
            global.save()?
        }
        debug!("new");
        let branch = txn.get_branch(&branch_name).unwrap();

        let mut included = HashSet::new();
        let mut patches = Vec::new();
        for (_, patch) in txn.rev_iter_applied(&branch, None) {
            // `patch` is already implied if a patch on the branch
            // depends on `patch`. Let's look at all patches known to
            // the repository that depend on `patch`, and see if a
            // patch on the branch (i.e. all patches in `included`,
            // since we're considering patches in reverse order of
            // application) depends on `patch`.
            let mut already_in = false;
            for (p, revdep) in txn.iter_revdep(Some((patch, None))) {
                if p == patch {
                    if included.contains(&revdep) {
                        already_in = true
                    }
                } else {
                    break;
                }
            }
            if !already_in {
                let patch = txn.get_external(patch).unwrap();
                patches.push(patch.to_owned());
            }
            included.insert(patch.to_owned());
        }
        txn.new_patch(
            &branch,
            authors,
            patch_name,
            description,
            chrono::Utc::now(),
            Vec::new(),
            patches.into_iter(),
            PatchFlags::TAG,
        )
    };
    drop(repo);

    let dot_pijul = opts.repo_dir();
    let key = if let Ok(Some(key)) = meta.signing_key() {
        Some(key)
    } else {
        load_global_or_local_signing_key(Some(&dot_pijul)).ok()
    };
    debug!("key.is_some(): {:?}", key.is_some());
    let patches_dir = patches_dir(&opts.repo_root);
    let hash = patch.save(&patches_dir, key.as_ref())?;

    let pristine_dir = opts.pristine_dir();
    let mut increase = 40960;
    loop {
        match record::record_no_resize(
            &pristine_dir,
            &opts.repo_root,
            &branch_name,
            &hash,
            &patch,
            &HashSet::new(),
            increase,
        ) {
            Err(ref e) if e.lacks_space() => increase *= 2,
            _ => break,
        }
    }
    Ok(Some(hash))
}

pub fn explain(res: Result<Option<Hash>, Error>) {
    default_explain(res)
}
