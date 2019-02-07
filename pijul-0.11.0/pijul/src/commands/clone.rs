use clap::{Arg, ArgMatches, SubCommand};

use commands::remote::{parse_remote, Remote};
use commands::{assert_no_containing_repo, create_repo, default_explain, StaticSubcommand};
use error::Error;
use libpijul::fs_representation::set_current_branch;
use libpijul::DEFAULT_BRANCH;
use regex::Regex;
use std::io::{stderr, Write};
use std::process::exit;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("clone")
        .about("Clone a remote branch")
        .arg(
            Arg::with_name("from")
                .help("Repository to clone.")
                .required(true),
        )
        .arg(
            Arg::with_name("from_branch")
                .long("from-branch")
                .help("The branch to pull from")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("to_branch")
                .long("to-branch")
                .help("The branch to pull into")
                .takes_value(true),
        )
        .arg(Arg::with_name("to").help("Target."))
        .arg(
            Arg::with_name("from_path")
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
    pub from: Remote<'a>,
    pub from_branch: &'a str,
    pub from_path: Vec<&'a str>,
    pub to: Remote<'a>,
    pub to_branch: &'a str,
}

pub fn parse_args<'a>(args: &'a ArgMatches) -> Params<'a> {
    // At least one must not use its "port" argument
    let from = parse_remote(
        args.value_of("from").unwrap(),
        args.value_of("port").and_then(|x| Some(x.parse().unwrap())),
        None,
        None,
    );
    let to = if let Some(to) = args.value_of("to") {
        parse_remote(
            to,
            args.value_of("port").and_then(|x| Some(x.parse().unwrap())),
            None,
            None,
        )
    } else {
        let basename = Regex::new(r"([^/:]*)").unwrap();
        let from = args.value_of("from").unwrap();
        if let Some(to) = basename.captures_iter(from).last().and_then(|to| to.get(1)) {
            parse_remote(
                to.as_str(),
                args.value_of("port").and_then(|x| Some(x.parse().unwrap())),
                None,
                None,
            )
        } else {
            panic!("Could not parse target")
        }
    };
    let from_branch = args.value_of("from_branch").unwrap_or(DEFAULT_BRANCH);
    let from_path = args
        .values_of("from_path")
        .map(|x| x.collect())
        .unwrap_or(Vec::new());
    let to_branch = args.value_of("to_branch").unwrap_or(from_branch);
    Params {
        from,
        from_branch,
        from_path,
        to,
        to_branch,
    }
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    let args = parse_args(args);
    debug!("{:?}", args);
    match args.to {
        Remote::Local { ref path } => {
            // This is "darcs get"
            assert_no_containing_repo(path)?;
            create_repo(path)?;
            let mut session = args.from.session()?;
            let mut pullable: Vec<_> = session
                .pullable_patches(args.from_branch, args.to_branch, path, &args.from_path)?
                .iter()
                .collect();
            session.pull(path, args.to_branch, &mut pullable, &args.from_path, true)?;
            set_current_branch(path, args.to_branch).map_err(|x| x.into())
        }
        _ => {
            // Clone between remote repositories.
            match args.from {
                Remote::Local { ref path } => {
                    let mut to_session = args.to.session()?;
                    debug!("remote init");
                    to_session.remote_init()?;
                    debug!("pushable?");
                    let pushable = to_session.pushable_patches(
                        args.from_branch,
                        args.to_branch,
                        path,
                        &args.from_path,
                    )?;
                    debug!("pushable = {:?}", pushable);
                    let pushable = pushable.into_iter().map(|(h, _, _)| h).collect();
                    to_session.push(path, args.to_branch, pushable)?;
                    set_current_branch(path, args.to_branch).map_err(|x| x.into())
                }
                _ => unimplemented!(),
            }
        }
    }
}

pub fn explain(res: Result<(), Error>) {
    if let Err(Error::InARepository { ref path }) = res {
        writeln!(
            stderr(),
            "error: Cannot clone onto / into existing repository {:?}",
            path
        ).unwrap();
        exit(1)
    }
    default_explain(res)
}
