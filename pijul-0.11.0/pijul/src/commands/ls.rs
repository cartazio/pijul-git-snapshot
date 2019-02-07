use clap::{Arg, ArgMatches, SubCommand};
use commands::{default_explain, BasicOptions, StaticSubcommand};
use error;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("ls")
        .about("List tracked files")
        .arg(
            Arg::with_name("dir")
                .multiple(true)
                .help("Prefix of the list"),
        )
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .help("Repository to list.")
                .takes_value(true),
        );
}

pub fn run(args: &ArgMatches) -> Result<(), error::Error> {
    let opts = BasicOptions::from_args(args)?;
    let repo = opts.open_repo()?;
    let txn = repo.txn_begin()?;
    let files = txn.list_files(opts.dir_inode(&txn)?)?;
    for f in files {
        println!("{}", f.display())
    }
    Ok(())
}

pub fn explain(res: Result<(), error::Error>) {
    default_explain(res)
}
