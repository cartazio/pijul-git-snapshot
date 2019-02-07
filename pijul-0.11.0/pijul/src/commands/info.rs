use clap::{AppSettings, Arg, ArgMatches, SubCommand};
use libpijul::Inode;
use std::fs::File;

use super::{default_explain, BasicOptions, StaticSubcommand};
use error::Error;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("info")
        .setting(AppSettings::Hidden)
        .about("Get information about the current repository, if any")
        .arg(
            Arg::with_name("debug")
                .long("--debug")
                .help("Pijul info will be given about this directory.")
                .required(false),
        )
        .arg(
            Arg::with_name("inode")
                .long("--from-inode")
                .help("Inode to start the graph from.")
                .takes_value(true)
                .required(false),
        )
        .arg(Arg::with_name("all").short("a"))
        .arg(Arg::with_name("exclude-parents").long("exclude-parents"))
        .arg(
            Arg::with_name("folder")
                .short("f")
                .help("show only folder edges"),
        )
        .arg(Arg::with_name("introduced_by").long("introducedby"));
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    let opts = BasicOptions::from_args(args)?;
    if args.is_present("debug") {
        let repo = opts.open_repo()?;
        let txn = repo.txn_begin()?;
        txn.dump();
        if let Some(ref inode) = args.value_of("inode") {
            // Output just the graph under `inode`.
            if let Some(inode) = Inode::from_hex(inode) {
                if let Some(node) = txn.get_inodes(inode) {
                    let node = node.key;
                    debug!("node {:?}", node);
                    for branch in txn.iter_branches(None) {
                        let ret = txn.retrieve(&branch, node);
                        let mut f = File::create(format!("debug_{}", branch.name.as_str()))?;
                        ret.debug(
                            &txn,
                            &branch,
                            args.is_present("all"),
                            args.is_present("introduced_by"),
                            &mut f,
                        )?
                    }
                }
            }
        } else {
            // Output everything.
            for branch in txn.iter_branches(None) {
                if args.is_present("folder") {
                    let mut f = File::create(format!("folders_{}", branch.name.as_str()))?;
                    txn.debug_folders(branch.name.as_str(), &mut f);
                } else {
                    let mut f = File::create(format!("debug_{}", branch.name.as_str()))?;
                    txn.debug(
                        branch.name.as_str(),
                        &mut f,
                        args.is_present("exclude-parents"),
                    );
                }
            }
        }
    }
    Ok(())
}

pub fn explain(r: Result<(), Error>) {
    default_explain(r)
}
