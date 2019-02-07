use clap::{Arg, ArgMatches, SubCommand};
use commands::{default_explain, BasicOptions, StaticSubcommand};
use error::Error;
use libpijul::fs_representation::read_patch_nochanges;
use libpijul::graph::LineBuffer;
use libpijul::{Key, PatchId, Txn, Value};
use std::fs::canonicalize;
use std::io::{stdout, Stdout};
use std::path::Path;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("credit")
        .about("Show what patch introduced each line of a file.")
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .takes_value(true)
                .help("Local repository."),
        )
        .arg(
            Arg::with_name("branch")
                .long("branch")
                .help("The branch to annotate, defaults to the current branch.")
                .takes_value(true)
                .required(false),
        )
        .arg(
            Arg::with_name("file")
                .help("File to annotate.")
                .required(true)
                .takes_value(true),
        );
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    let opts = BasicOptions::from_args(args)?;
    let file = Path::new(args.value_of("file").unwrap());
    let p = canonicalize(opts.cwd.join(file))?;
    if let Ok(file) = p.strip_prefix(&opts.repo_root) {
        let repo = opts.open_repo()?;
        let txn = repo.txn_begin()?;
        if let Some(branch) = txn.get_branch(&opts.branch()) {
            let inode = txn.find_inode(&file)?;
            if txn.is_directory(&inode) {
                return Err(Error::IsDirectory);
            }
            let node = txn.get_inodes(inode).unwrap();
            let mut graph = txn.retrieve(&branch, node.key);
            let mut buf = OutBuffer {
                stdout: stdout(),
                txn: &txn,
                target: &opts.repo_root,
            };
            super::setup_pager();
            txn.output_file(&branch, &mut buf, &mut graph, &mut Vec::new())?;
        }
    }
    Ok(())
}

struct OutBuffer<'a> {
    stdout: Stdout,
    txn: &'a Txn<'a>,
    target: &'a Path,
}

use libpijul;
use libpijul::Transaction;
use std::io::Write;

impl<'a, T: 'a + Transaction> LineBuffer<'a, T> for OutBuffer<'a> {
    fn output_line(
        &mut self,
        key: &Key<PatchId>,
        contents: Value<'a, T>,
    ) -> Result<(), libpijul::Error> {
        let ext = self.txn.get_external(key.patch).unwrap();
        let patch = read_patch_nochanges(self.target, ext)?;
        write!(
            self.stdout,
            "{} {} {} > ",
            patch.authors[0],
            patch.timestamp.format("%F %R %Z"),
            key.patch.to_base58()
        )?;
        let mut ends_with_eol = false;
        for chunk in contents {
            self.stdout.write_all(chunk)?;
            if let Some(&c) = chunk.last() {
                ends_with_eol = c == b'\n'
            }
        }
        if !ends_with_eol {
            writeln!(self.stdout, "")?;
        }
        Ok(())
    }

    fn output_conflict_marker(&mut self, s: &'a str) -> Result<(), libpijul::Error> {
        write!(self.stdout, "{}", s)?;
        Ok(())
    }
}

pub fn explain(res: Result<(), Error>) {
    default_explain(res)
}
