use clap::{Arg, ArgMatches, SubCommand};

use super::ask;
use commands::{default_explain, BasicOptions, StaticSubcommand};
use error::Error;
use libpijul::fs_representation::read_patch;
use libpijul::DEFAULT_BRANCH;
use meta::{Meta, Repository, DEFAULT_REMOTE};
use std::env::current_dir;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("push")
        .about("Push to a remote repository")
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .help("Repository to list.")
                .takes_value(true),
        )
        .arg(Arg::with_name("remote").help("Repository to push to."))
        .arg(
            Arg::with_name("local_branch")
                .long("from-branch")
                .help("The branch to push from")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("remote_branch")
                .long("to-branch")
                .help("The branch to push into. Defaults to the current branch.")
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
                .help("Used with --set-remote, sets this remote as the default push target."),
        )
        .arg(
            Arg::with_name("set-remote")
                .long("set-remote")
                .help("Set the name of this remote")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("remote_path")
                .long("path")
                .help("Only pull patches relative to that patch.")
                .takes_value(true)
                .multiple(true),
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
    pub remote_path: Vec<&'a str>,
}

pub fn parse_args<'a>(args: &'a ArgMatches) -> Params<'a> {
    Params {
        remote_id: args.value_of("remote"),
        set_remote: args.value_of("set-remote"),
        yes_to_all: args.is_present("all"),
        set_default: args.is_present("set-default"),
        port: args.value_of("port").and_then(|x| Some(x.parse().unwrap())),
        local_branch: args.value_of("local_branch"),
        remote_branch: args.value_of("remote_branch").unwrap_or(DEFAULT_BRANCH),
        remote_path: args
            .values_of("remote_path")
            .map(|x| x.collect())
            .unwrap_or(Vec::new()),
    }
}

pub fn run(arg_matches: &ArgMatches) -> Result<(), Error> {
    let opts = BasicOptions::from_args(arg_matches)?;
    let args = parse_args(arg_matches);
    let mut meta = Meta::load(&opts.repo_root).unwrap_or(Meta::new());
    let local_branch = if let Some(b) = args.local_branch {
        b.to_string()
    } else {
        opts.branch()
    };
    let cwd = current_dir()?;
    let repo_root = opts.repo_root();
    {
        let remote = meta.push(args.remote_id, args.port, Some(&cwd), Some(&repo_root))?;
        debug!("remote: {:?}", remote);
        let mut session = remote.session()?;
        let pushable = session.pushable_patches(
            &local_branch,
            args.remote_branch,
            &opts.repo_root,
            &args.remote_path,
        )?;
        let pushable = if !args.yes_to_all {
            let mut patches = Vec::new();
            let mut pushable: Vec<_> = pushable.into_iter().collect();
            pushable.sort_by(|&(_, _, a), &(_, _, b)| a.cmp(&b));
            for &(ref i, ref internal, _) in pushable.iter() {
                patches.push((
                    i.clone(),
                    internal.clone(),
                    read_patch(&opts.repo_root, i.as_ref())?,
                ))
            }
            ask::ask_patches(ask::Command::Push, &patches)?
        } else {
            pushable.into_iter().map(|(h, _, _)| h).collect()
        };
        if !pushable.is_empty() {
            session.push(&opts.repo_root, args.remote_branch, pushable)?;
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
            meta.push = Some(set_remote.to_string());
        }
        meta.save(&opts.repo_root)?;
    }
    Ok(())
}

pub fn explain(res: Result<(), Error>) {
    default_explain(res)
}
