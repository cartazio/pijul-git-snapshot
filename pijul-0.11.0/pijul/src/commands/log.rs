use clap::{Arg, ArgMatches, SubCommand};
use commands::{ask, default_explain, BasicOptions, StaticSubcommand};
use error::Error;
use libpijul::fs_representation::{id_file, read_patch_nochanges};
use libpijul::{HashRef, PatchId};
use regex::Regex;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use term;

pub fn invocation() -> StaticSubcommand {
    SubCommand::with_name("log")
        .about("List the patches applied to the given branch")
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .help("Path to the repository to list.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("branch")
                .long("branch")
                .help("The branch to list.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("internal-id")
                .long("internal-id")
                .help("Display only patches with these internal identifiers.")
                .multiple(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("hash-only")
                .long("hash-only")
                .help("Only display the hash of each path."),
        )
        .arg(
            Arg::with_name("path")
                .long("path")
                .multiple(true)
                .takes_value(true)
                .help("Only display patches that touch the given path."),
        )
        .arg(
            Arg::with_name("grep")
                .long("grep")
                .multiple(true)
                .takes_value(true)
                .help("Search patch name and description with a regular expression."),
        )
}

struct Pager {
    is_setup: bool,
}

impl Pager {
    fn setup(&mut self) {
        if !self.is_setup {
            super::setup_pager();
            self.is_setup = true;
        }
    }

    fn new() -> Self {
        Pager { is_setup: true }
    }
}

struct Settings<'a> {
    hash_only: bool,
    regex: Vec<Regex>,
    opts: BasicOptions<'a>,
    path: Vec<&'a str>,
}

impl<'a> Settings<'a> {
    fn parse(args: &'a ArgMatches) -> Result<Self, Error> {
        let basic_opts = BasicOptions::from_args(args)?;
        let hash_only = args.is_present("hash-only");
        let mut regex = Vec::new();
        if let Some(regex_args) = args.values_of("grep") {
            for r in regex_args {
                debug!("regex: {:?}", r);
                regex.push(Regex::new(r)?)
            }
        }
        let path = args.values_of("path")
            .map(|x| x.collect())
            .unwrap_or(Vec::new());
        Ok(Settings {
            hash_only,
            regex,
            opts: basic_opts,
            path,
        })
    }
}

fn display_patch(
    pager: &mut Pager,
    settings: &Settings,
    nth: usize,
    patchid: PatchId,
    hash_ext: HashRef,
) -> Result<(), Error> {
    let (matches_regex, o_patch) = if settings.regex.is_empty() {
        (true, None)
    } else {
        let patch = read_patch_nochanges(&settings.opts.repo_root, hash_ext)?;
        let does_match = {
            let descr = match patch.description {
                Some(ref d) => d,
                None => "",
            };
            settings
                .regex
                .iter()
                .any(|ref r| r.is_match(&patch.name) || r.is_match(descr))
        };
        (does_match, Some(patch))
    };
    if !matches_regex {
        return Ok(());
    };

    pager.setup();

    if settings.hash_only {
        println!("{}:{}", hash_ext.to_base58(), nth);
        Ok(())
    } else {
        let patch = match o_patch {
            None => read_patch_nochanges(&settings.opts.repo_root, hash_ext)?,
            Some(patch) => patch,
        };
        let mut term = term::stdout();
        ask::print_patch_descr(&mut term, &hash_ext.to_owned(), Some(patchid), &patch);
        Ok(())
    }
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    super::setup_pager();

    let settings = Settings::parse(args)?;

    if settings.hash_only {
        // If in binary form, start with this repository's id.
        let id_file = id_file(&settings.opts.repo_root);
        let mut f = File::open(&id_file)?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        println!("{}", s.trim());
    };

    let mut pager = Pager::new();

    let repo = settings.opts.open_repo()?;
    let txn = repo.txn_begin()?;
    let branch = match txn.get_branch(&settings.opts.branch()) {
        Some(b) => b,
        None => return Err(Error::NoSuchBranch),
    };
    if let Some(v) = args.values_of("internal-id") {
        for (n, patchid) in v.filter_map(|x| PatchId::from_base58(x)).enumerate() {
            let hash_ext = txn.get_external(patchid).unwrap();
            display_patch(&mut pager, &settings, n, patchid, hash_ext)?;
        }
    } else if !settings.path.is_empty() {
        for (n, (applied, patchid)) in txn.rev_iter_applied(&branch, None).enumerate() {
            for path in settings.path.iter() {
                let inode = txn.find_inode(Path::new(path))?;
                let key = txn.get_inodes(inode).unwrap().key;

                if txn.get_touched(key, patchid) {
                    debug!("applied: {:?}", applied);
                    let hash_ext = txn.get_external(patchid).unwrap();
                    debug!("hash: {:?}", hash_ext.to_base58());
                    display_patch(&mut pager, &settings, n, patchid, hash_ext)?;
                    break;
                }
            }
        }
    } else {
        for (n, (applied, patchid)) in txn.rev_iter_applied(&branch, None).enumerate() {
            debug!("applied: {:?} {:?}", applied, patchid);
            let hash_ext = txn.get_external(patchid).unwrap();
            debug!("hash: {:?}", hash_ext.to_base58());
            display_patch(&mut pager, &settings, n, patchid, hash_ext)?;
        }
    }
    Ok(())
}

pub fn explain(r: Result<(), Error>) {
    default_explain(r)
}
