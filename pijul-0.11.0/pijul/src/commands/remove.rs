use clap::{Arg, ArgMatches, SubCommand};
use commands::fs_operation;
use commands::fs_operation::Operation;
use commands::{default_explain, StaticSubcommand};
use error;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("remove")
        .about("Remove file from the repository")
        .arg(
            Arg::with_name("files")
                .multiple(true)
                .help("Files to remove from the repository.")
                .required(true),
        )
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .help("Repository to remove files from.")
                .takes_value(true),
        );
}

pub fn run(args: &ArgMatches) -> Result<(), error::Error> {
    fs_operation::run(args, Operation::Remove)
}

pub fn explain(res: Result<(), error::Error>) {
    default_explain(res)
}
