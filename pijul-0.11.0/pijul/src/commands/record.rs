use super::ask::{ask_changes, ChangesDirection};
use super::default_explain;
use chrono;
use clap::{Arg, ArgMatches, SubCommand};
use commands::hooks::run_hook;
use commands::{ask, BasicOptions, StaticSubcommand};
use libpijul;
use libpijul::fs_representation::{ignore_file, patches_dir, untracked_files};
use libpijul::patch::{PatchFlags, Record};
use libpijul::{Hash, InodeUpdate, Key, MutTxn, Patch, PatchId, RecordState, Repository};
use meta::{load_global_or_local_signing_key, Global, Meta};
use rand;
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs::canonicalize;
use std::fs::{metadata, OpenOptions};
use std::io::Write;
use std::mem::drop;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;
use error::Error;

pub fn record_args(sub: StaticSubcommand) -> StaticSubcommand {
    sub.arg(Arg::with_name("repository")
            .long("repository")
            .help("The repository where to record, defaults to the current directory.")
            .takes_value(true)
            .required(false))
        .arg(Arg::with_name("branch")
             .long("branch")
             .help("The branch where to record, defaults to the current branch.")
             .takes_value(true)
             .required(false))
        .arg(Arg::with_name("date")
             .long("date")
             .help("The date to use to record the patch, default is now.")
             .takes_value(true)
             .required(false))
        .arg(Arg::with_name("message")
             .short("m")
             .long("message")
             .help("The name of the patch to record")
             .takes_value(true))
        .arg(Arg::with_name("description")
             .short("d")
             .long("description")
             .help("The description of the patch to record")
             .takes_value(true))
        .arg(Arg::with_name("no-editor")
             .long("no-editor")
             .help("Do not use an editor to write the patch name and description, even if the variable is set in the configuration file")
             .takes_value(false))
        .arg(Arg::with_name("author")
             .short("A")
             .long("author")
             .help("Author of this patch (multiple occurrences allowed)")
             .takes_value(true))
}

pub fn invocation() -> StaticSubcommand {
    return record_args(
        SubCommand::with_name("record")
            .about("Record changes in the repository")
            .arg(
                Arg::with_name("all")
                    .short("a")
                    .long("all")
                    .help("Answer 'y' to all questions")
                    .takes_value(false),
            )
            .arg(
                Arg::with_name("add-new-files")
                    .short("n")
                    .long("add-new-files")
                    .help("Offer to add files that have been created since the last record")
                    .takes_value(false),
            )
            .arg(
                Arg::with_name("depends-on")
                    .help("Add a dependency to this patch (internal id or hash accepted)")
                    .long("depends-on")
                    .takes_value(true)
                    .multiple(true),
            )
            .arg(
                Arg::with_name("prefix")
                    .help("Prefix to start from")
                    .takes_value(true)
                    .multiple(true),
            ),
    );
}

fn add_untracked_files<T: rand::Rng>(
    txn: &mut MutTxn<T>,
    repo_root: &Path,
) -> Result<HashSet<PathBuf>, Error> {
    let untracked = untracked_files(txn, repo_root);
    debug!("adding untracked_files at record time: {:?}", &untracked);
    for file in untracked.iter() {
        let m = metadata(&file)?;
        let file = file.strip_prefix(&repo_root)?;
        if let Err(e) = txn.add_file(&file, m.is_dir()) {
            if let libpijul::Error::AlreadyAdded = e {
            } else {
                return Err(e.into());
            }
        }
    }
    Ok(untracked)
}

fn append_to_ignore_file(repo_root: &Path, lines: &Vec<String>) -> Result<(), Error> {
    let ignore_file = ignore_file(repo_root);
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(ignore_file)?;
    for line in lines {
        file.write_all(line.as_ref())?;
        file.write_all(b"\n")?
    }
    Ok(())
}

fn select_changes(
    opts: &BasicOptions,
    add_new_files: bool,
    branch_name: &str,
    yes_to_all: bool,
    prefix: Option<Vec<PathBuf>>,
) -> Result<(Vec<Record<Vec<Key<Option<Hash>>>>>, HashSet<InodeUpdate>), Error> {
    // Increase by 100 pages. The most things record can write is one
    // write in the branches table, affecting at most O(log n) blocks.
    let repo = opts.open_and_grow_repo(409600)?;
    let mut txn = repo.mut_txn_begin(rand::thread_rng())?;
    let mut to_unadd = if add_new_files {
        add_untracked_files(&mut txn, &opts.repo_root)?
    } else {
        HashSet::<PathBuf>::new()
    };
    let (changes, syncs) =
        changes_from_prefixes(&opts.repo_root, &mut txn, &branch_name, prefix.as_ref())?;
    let changes: Vec<_> = changes
        .into_iter()
        .map(|x| txn.globalize_record(x))
        .collect();
    if !yes_to_all {
        let (c, i) = ask_changes(
            &txn,
            &opts.repo_root,
            &opts.cwd,
            &changes,
            ChangesDirection::Record,
            &mut to_unadd,
        )?;
        let selected = changes
            .into_iter()
            .enumerate()
            .filter(|&(i, _)| *(c.get(&i).unwrap_or(&false)))
            .map(|(_, x)| x)
            .collect();
        for file in to_unadd {
            txn.remove_file(&file)?
        }
        txn.commit()?;
        append_to_ignore_file(&opts.repo_root, &i)?;
        Ok((selected, syncs))
    } else {
        txn.commit()?;
        Ok((changes, syncs))
    }
}

pub fn run(args: &ArgMatches) -> Result<Option<Hash>, Error> {
    let opts = BasicOptions::from_args(args)?;
    let yes_to_all = args.is_present("all");
    let patch_name_arg = args.value_of("message");
    let patch_descr_arg = args.value_of("description");
    let authors_arg = args.values_of("author").map(|x| x.collect::<Vec<_>>());
    let branch_name = opts.branch();
    let add_new_files = args.is_present("add-new-files");

    let patch_date = args.value_of("date").map_or(Ok(chrono::Utc::now()), |x| {
        chrono::DateTime::from_str(x).map_err(|_| Error::InvalidDate { date: String::from(x) })
    })?;

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

    run_hook(&opts.repo_root, "pre-record", None)?;

    debug!("prefix {:?}", args.value_of("prefix"));
    let prefix = prefix(args, &opts)?;

    let (changes, syncs) = select_changes(&opts, add_new_files, &branch_name, yes_to_all, prefix)?;

    if changes.is_empty() {
        println!("Nothing to record");
        Ok(None)
    } else {
        let template = prepare_changes_template(&opts.repo_root, patch_name_arg.unwrap_or(""), &changes);

        let repo = opts.open_repo()?;
        let patch = {
            let txn = repo.txn_begin()?;
            debug!("meta:{:?}", meta);

            let authors = decide_authors(authors_arg, &meta, &global)?;

            if authors.is_empty() {
                return Err(Error::NoAuthor)
            }

            if meta.authors.is_empty() {
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
                template,
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
                global.save().unwrap_or(())
            }
            debug!("new");
            let changes = changes.into_iter().flat_map(|x| x.into_iter()).collect();
            let branch = txn.get_branch(&branch_name).unwrap();

            let mut extra_deps = Vec::new();
            if let Some(deps) = args.values_of("depends-on") {
                for dep in deps {
                    if let Some(hash) = Hash::from_base58(dep) {
                        if let Some(internal) = txn.get_internal(hash.as_ref()) {
                            if txn.get_patch(&branch.patches, internal).is_some() {
                                extra_deps.push(hash)
                            } else {
                                return Err(Error::ExtraDepNotOnBranch { hash });
                            }
                        } else {
                            return Err(Error::PatchNotFound {
                                repo_root: opts.repo_root().to_string_lossy().into_owned(),
                                patch_hash: hash,
                            });
                        }
                    } else if let Some(internal) = PatchId::from_base58(dep) {
                        if let Some(hash) = txn.get_external(internal) {
                            if txn.get_patch(&branch.patches, internal).is_some() {
                                extra_deps.push(hash.to_owned())
                            } else {
                                return Err(Error::ExtraDepNotOnBranch { hash: hash.to_owned() });
                            }
                        }
                    } else {
                        return Err(Error::WrongHash);
                    }
                }
            }
            txn.new_patch(
                &branch,
                authors,
                patch_name,
                description,
                patch_date,
                changes,
                extra_deps.into_iter(),
                PatchFlags::empty(),
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
        let mut increase = 409600;
        let res = loop {
            match record_no_resize(
                &pristine_dir,
                &opts.repo_root,
                &branch_name,
                &hash,
                &patch,
                &syncs,
                increase,
            ) {
                Err(ref e) if e.lacks_space() => increase *= 2,
                e => break e,
            }
        };

        run_hook(&opts.repo_root, "post-record", None)?;

        res
    }
}

pub fn record_no_resize(
    pristine_dir: &Path,
    r: &Path,
    branch_name: &str,
    hash: &Hash,
    patch: &Patch,
    syncs: &HashSet<InodeUpdate>,
    increase: u64,
) -> Result<Option<Hash>, Error> {
    let size_increase = increase + patch.size_upper_bound() as u64;
    let repo = match Repository::open(&pristine_dir, Some(size_increase)) {
        Ok(repo) => repo,
        Err(x) => return Err(Error::Repository(x)),
    };
    let mut txn = repo.mut_txn_begin(rand::thread_rng())?;
    // save patch
    debug!("syncs: {:?}", syncs);
    let mut branch = txn.open_branch(branch_name)?;
    txn.apply_local_patch(&mut branch, r, &hash, &patch, &syncs, false)?;
    txn.commit_branch(branch)?;
    txn.commit()?;
    println!("Recorded patch {}", hash.to_base58());
    Ok(Some(hash.clone()))
}

pub fn explain(res: Result<Option<Hash>, Error>) {
    default_explain(res)
}

pub fn changes_from_prefixes<T: rand::Rng, P: AsRef<Path>>(
    repo_root: &Path,
    txn: &mut MutTxn<T>,
    branch_name: &str,
    prefix: Option<&Vec<P>>,
) -> Result<(
    Vec<libpijul::patch::Record<Rc<RefCell<libpijul::patch::ChangeContext<PatchId>>>>>,
    HashSet<libpijul::InodeUpdate>,
), Error> {
    let mut record = RecordState::new();
    let branch = txn.open_branch(branch_name)?;
    if let Some(prefixes) = prefix {
        for prefix in prefixes {
            txn.record(&mut record, &branch, repo_root, Some(prefix.as_ref()))?;
        }
    } else {
        txn.record(&mut record, &branch, repo_root, None)?;
    }
    txn.commit_branch(branch)?;
    let (changes, updates) = record.finish();
    // let changes = changes.into_iter().map(|x| txn.globalize_change(x)).collect();
    Ok((changes, updates))
}

pub fn prefix(args: &ArgMatches, opts: &BasicOptions) -> Result<Option<Vec<PathBuf>>, Error> {
    if let Some(prefixes) = args.values_of("prefix") {
        let prefixes: Result<Vec<_>, Error> = prefixes
            .map(|prefix| {
                let p = opts.cwd.join(prefix);
                let p = if let Ok(p) = canonicalize(&p) { p } else { p };
                let file = p.strip_prefix(&opts.repo_root)?;
                debug!("prefix: {:?}", file);
                Ok(file.to_path_buf())
            })
            .collect();
        Ok(Some(prefixes?))
    } else {
        Ok(None)
    }
}

pub fn decide_authors(
    authors_args: Option<Vec<&str>>,
    meta: &Meta,
    global: &Global,
) -> Result<Vec<String>, Error> {
    Ok(match authors_args {
        Some(authors) => authors.iter().map(|x| x.to_string()).collect(),
        _ => {
            if meta.authors.len() > 0 {
                meta.authors.clone()
            } else if global.author.len() > 0 {
                vec![global.author.clone()]
            } else {
                ask::ask_authors()?
            }
        }
    })
}

pub fn decide_patch_message(
    name_arg: Option<&str>,
    descr_arg: Option<&str>,
    template: String,
    use_editor: bool,
    repo_root: &PathBuf,
    meta: &Meta,
    global: &Global,
) -> Result<(String, Option<String>), Error> {
    Ok(match name_arg {
        Some(m) => (m.to_string(), descr_arg.map(|x| String::from(x.trim()))),
        _ => {
            let maybe_editor = if use_editor {
                if meta.editor.is_some() {
                    meta.editor.as_ref()
                } else {
                    global.editor.as_ref()
                }
            } else {
                None
            };

            ask::ask_patch_name(repo_root, maybe_editor, template)?
        }
    })
}

fn prepare_changes_template(
    repo_root: &Path,
    descr: &str,
    changes: &[Record<Vec<Key<Option<Hash>>>>],
) -> String {
    let mut res = format!(r#"
{}
# Please enter a patch title, and consider writing a description too. Lines
# starting with '#' will be ignored. Besides, an empty patch title aborts the
# patch recording.
#
# Here is a summary of the changes you are about to record:
#"#, descr);
    let mut known_files = Vec::new();

    for change in changes.iter() {
        match *change {
            Record::Change { ref file, .. }
            | Record::Replace { ref file, .. } => {
                let filename = file
                    .strip_prefix(repo_root)
                    .ok()
                    .and_then(|x| x.to_str())
                    .map(|x| x.to_owned())
                    .unwrap_or(String::from("(invalid finemane)"));

                if !known_files.contains(&filename) {
                    res = format!("{}\n#\tmodified:  {}", res, filename);
                    known_files.push(filename);
                }
            }
            Record::FileAdd { ref name, .. } => {
                let filename = PathBuf::from(name)
                    .strip_prefix(repo_root)
                    .ok()
                    .and_then(|x| x.to_str())
                    .map(|x| x.to_owned())
                    .unwrap_or(String::from("(invalid finemane)"));

                res = format!("{}\n#\tnew file:  {}", res, filename);
            }
            Record::FileDel { ref name, .. } => {
                let filename = PathBuf::from(name)
                    .strip_prefix(repo_root)
                    .ok()
                    .and_then(|x| x.to_str())
                    .map(|x| x.to_owned())
                    .unwrap_or(String::from("(invalid finemane)"));

                res = format!("{}\n#\t deleted:  {}", res, filename);
            }
            Record::FileMove { ref new_name, .. } => {
                let filename = PathBuf::from(new_name)
                    .strip_prefix(repo_root)
                    .ok()
                    .and_then(|x| x.to_str())
                    .map(|x| x.to_owned())
                    .unwrap_or(String::from("(invalid finemane)"));

                res = format!("{}\n#\t   moved:  to {}", res, filename);
            }
        }
    }

    return res;
}
