use clap::{Arg, ArgMatches, SubCommand};

use commands::{default_explain, BasicOptions, StaticSubcommand};
use error::Error;
use std::fs::File;
use std::path::Path;

use commands::ask::{ask_patches, Command};
use commands::remote;
use libpijul::patch::Patch;
use libpijul::{ApplyTimestamp, Hash, PatchId, DEFAULT_BRANCH};
use meta::{Meta, Repository, DEFAULT_REMOTE};
use progrs;
use rand;
use std::env::current_dir;
use std::io::BufReader;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("pull")
        .about("Pull from a remote repository")
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .help("Repository to list.")
                .takes_value(true),
        )
        .arg(Arg::with_name("remote").help("Repository from which to pull."))
        .arg(
            Arg::with_name("remote_branch")
                .long("from-branch")
                .help("The branch to pull from. Defaults to master.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("local_branch")
                .long("to-branch")
                .help("The branch to pull into. Defaults to the current branch.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("all")
                .short("a")
                .long("all")
                .help("Answer 'y' to all questions")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("set-default")
                .long("set-default")
                .help("Used with --set-remote, sets this remote as the default pull remote."),
        )
        .arg(
            Arg::with_name("set-remote")
                .long("set-remote")
                .help("Name this remote destination.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("remote_path")
                .long("path")
                .help("Only pull patches relative to that patch.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .help("Port of the remote ssh server.")
                .takes_value(true)
                .validator(|val| {
                    let x: Result<u16, _> = val.parse();
                    match x {
                        Ok(_) => Ok(()),
                        Err(_) => Err(val),
                    }
                }),
        );
}

#[derive(Debug)]
pub struct Params<'a> {
    pub remote_id: Option<&'a str>,
    pub set_remote: Option<&'a str>,
    pub yes_to_all: bool,
    pub set_default: bool,
    pub port: Option<u16>,
    pub local_branch: Option<&'a str>,
    pub remote_branch: &'a str,
    pub remote_paths: Vec<&'a str>,
}

fn parse_args<'a>(args: &'a ArgMatches) -> Params<'a> {
    Params {
        remote_id: args.value_of("remote"),
        set_remote: args.value_of("set-remote"),
        yes_to_all: args.is_present("all"),
        set_default: args.is_present("set-default"),
        port: args.value_of("port").and_then(|x| Some(x.parse().unwrap())),
        local_branch: args.value_of("local_branch"),
        remote_branch: args.value_of("remote_branch").unwrap_or(DEFAULT_BRANCH),
        remote_paths: if let Some(rem) = args.values_of("remote_path") {
            rem.collect()
        } else {
            Vec::new()
        },
    }
}

fn fetch_pullable_patches(
    session: &mut remote::Session,
    pullable: &[(Hash, ApplyTimestamp)],
    r: &Path,
) -> Result<Vec<(Hash, Option<PatchId>, Patch)>, Error> {
    let mut patches = Vec::new();

    let (mut p, mut n) = (progrs::start("Pulling patches", pullable.len() as u64), 0);
    for &(ref i, _) in pullable {
        let (hash, _, patch) = {
            let filename = session.download_patch(r, i)?;
            debug!("filename {:?}", filename);
            let file = File::open(&filename)?;
            let mut file = BufReader::new(file);
            Patch::from_reader_compressed(&mut file)?
        };
        p.display({
            n += 1;
            n
        });
        assert_eq!(&hash, i);
        patches.push((hash, None, patch));
    }
    p.stop("done");
    Ok(patches)
}

pub fn select_patches(
    interactive: bool,
    session: &mut remote::Session,
    remote_branch: &str,
    local_branch: &str,
    r: &Path,
    remote_paths: &[&str],
) -> Result<Vec<(Hash, ApplyTimestamp)>, Error> {
    let pullable = session.pullable_patches(remote_branch, local_branch, r, remote_paths)?;
    let mut pullable: Vec<_> = pullable.iter().collect();
    pullable.sort_by(|&(_, a), &(_, b)| a.cmp(&b));
    if interactive && !pullable.is_empty() {
        let selected = {
            let patches = fetch_pullable_patches(session, &pullable, r)?;
            ask_patches(Command::Pull, &patches[..])?
        };
        Ok(pullable
            .into_iter()
            .filter(|&(ref h, _)| selected.contains(h))
            .collect())
    } else {
        Ok(pullable)
    }
}

pub fn run(arg_matches: &ArgMatches) -> Result<(), Error> {
    let opts = BasicOptions::from_args(arg_matches)?;
    let args = parse_args(arg_matches);
    debug!("pull args {:?}", args);
    let mut meta = Meta::load(&opts.repo_root).unwrap_or(Meta::new());
    let cwd = current_dir()?;
    let local_branch = if let Some(b) = args.local_branch {
        b.to_string()
    } else {
        opts.branch()
    };
    let repo_root = opts.repo_root();
    {
        let remote = meta.pull(args.remote_id, args.port, Some(&cwd), Some(&repo_root))?;
        let mut session = remote.session()?;
        let mut pullable = select_patches(
            !args.yes_to_all,
            &mut session,
            args.remote_branch,
            &local_branch,
            &opts.repo_root,
            &args.remote_paths,
        )?;

        // Pulling and applying
        info!("Pulling patch {:?}", pullable);
        if !pullable.is_empty() {
            session.pull(
                &opts.repo_root,
                &local_branch,
                &mut pullable,
                &args.remote_paths,
                false,
            )?;
        } else {
            println!("No new patches to pull.");
        }
    }

    info!("Saving meta");
    let set_remote = if args.set_default && args.set_remote.is_none() {
        Some(DEFAULT_REMOTE)
    } else {
        args.set_remote
    };
    if let (Some(set_remote), Some(remote_id)) = (set_remote, args.remote_id) {
        let mut repo = Repository::default();
        repo.address = remote_id.to_string();
        repo.port = args.port;

        meta.remote.insert(set_remote.to_string(), repo);
        if args.set_default {
            meta.pull = Some(set_remote.to_string());
        }
        meta.save(&opts.repo_root)?;
    }
    let repo = opts.open_repo()?;
    let mut txn = repo.mut_txn_begin(rand::thread_rng())?;
    let conflicts = txn.list_conflict_files(&local_branch, &args.remote_paths)?;
    if !conflicts.is_empty() {
        println!("There are pending conflicts waiting to be solved:");
        for f in conflicts {
            println!("    {}", f.to_str().unwrap_or("(invalid path)"));
        }
    }

    Ok(())
}

pub fn explain(res: Result<(), Error>) {
    default_explain(res)
}
