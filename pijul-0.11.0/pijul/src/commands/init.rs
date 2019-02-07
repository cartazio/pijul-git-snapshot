use clap::{Arg, ArgMatches, SubCommand};
use commands::{create_repo, default_explain, StaticSubcommand};
use error::Error;
use std::env::current_dir;
use std::io::{stderr, Write};
use std::path::Path;
use std::process::exit;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("init")
        .about("Create a new repository")
        .arg(
            Arg::with_name("directory")
                .index(1)
                .help("Where to create the repository, defaults to the current directory.")
                .required(false),
        );
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    // Since the location may not exist, we can't always canonicalize,
    // which doesn't really matter since we're going to explore the
    // whole path in `find_repo_root`.
    let wd = match args.value_of("directory").map(Path::new) {
        Some(r) if r.is_relative() => current_dir()?.join(r),
        Some(r) => r.to_path_buf(),
        None => current_dir()?,
    };
    create_repo(&wd)
}

pub fn explain(r: Result<(), Error>) {
    if let Err(Error::InARepository { ref path }) = r {
        writeln!(stderr(), "Repository {:?} already exists", path).unwrap();
        exit(1)
    }
    default_explain(r)
}
