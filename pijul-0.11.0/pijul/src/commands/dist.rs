use clap::{Arg, ArgMatches, SubCommand};
use commands::{default_explain, BasicOptions, ScanScope, StaticSubcommand};
use error::Error;
use flate2::Compression;
use flate2::write::GzEncoder;
use libpijul::{Branch, Edge, Key, PatchId, Repository, Txn, ROOT_KEY, graph};
use std::fs::{remove_file, File};
use std::io::{stdout, Write};
use std::path::{Path, PathBuf};
use tar::{Builder, Header};

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("dist")
        .about("Produces a tar.gz archive of the repository")
        .arg(
            Arg::with_name("archive")
                .short("d")
                .takes_value(true)
                .required(true)
                .help("File name of the output archive."),
        )
        .arg(
            Arg::with_name("branch")
                .long("branch")
                .help("The branch from which to make the archive, defaults to the current branch.")
                .takes_value(true)
                .required(false),
        )
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .help("Repository where to work.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("stdout")
                .long("stdout")
                .short("s")
                .help("Prints the resulting archive to stdout")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("dir")
                .help("Directory (or file) to archive, defaults to the whole repository.")
                .takes_value(true),
        );
}

pub fn dist<W: Write>(
    repo: Repository,
    branch_name: &str,
    scope: ScanScope,
    archive_name: &str,
    encoder: GzEncoder<W>,
) -> Result<(), Error> {
    let txn = repo.txn_begin()?;
    let branch = txn.get_branch(branch_name)
        .ok_or(Error::NoSuchBranch)?;
    let mut current_path = Path::new(archive_name).to_path_buf();
    let mut archive = Builder::new(encoder);
    let mut buffer = graph::Writer::new(Vec::new());
    let mut forward = Vec::new();

    let key = match scope {
        ScanScope::FromRoot => ROOT_KEY,
        ScanScope::WithPrefix(prefix, user_input) => {
            let inode = txn.find_inode(prefix.as_ref())?;
            txn.get_inodes(inode)
                .map(|key| key.key.to_owned())
                .ok_or(Error::InvalidPath { path: user_input })?
        }
    };
    archive_rec(
        &txn,
        &branch,
        key,
        &mut archive,
        &mut buffer,
        &mut forward,
        &mut current_path,
    )?;

    archive
        .into_inner()?
        .finish()?
        .flush()
        .map_err(|x| x.into())
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    let opts = BasicOptions::from_args(args)?;

    let archive_name = args.value_of("archive").unwrap();

    let repo = opts.open_repo()?;
    let scan = opts.scan_scope()?;

    if args.is_present("stdout") {
        let encoder = GzEncoder::new(stdout(), Compression::best());

        dist(repo, &opts.branch(), scan, archive_name, encoder)
    } else {
        let archive_path = PathBuf::from(archive_name.to_string() + ".tar.gz");

        let encoder = GzEncoder::new(File::create(&archive_path)?, Compression::best());

        dist(repo, &opts.branch(), scan, archive_name, encoder).map_err(|err| {
            // The creation of the archive has failed, we should try to
            // remove it, but we ignore the error if we cannot.
            // This should not happen, because either we could not create
            // the file, or we have enough permission to do it, as we are
            // its creator.
            let _ = remove_file(archive_path);
            err
        })
    }
}

fn archive_rec<W: Write>(
    txn: &Txn,
    branch: &Branch,
    key: Key<PatchId>,
    builder: &mut Builder<W>,
    buffer: &mut graph::Writer<Vec<u8>>,
    forward: &mut Vec<(Key<PatchId>, Edge)>,
    current_path: &mut PathBuf,
) -> Result<(), Error> {
    let files = txn.list_files_under_node(branch, key);

    for (key, names) in files {
        debug!("archive_rec: {:?} {:?}", key, names);
        if names.len() > 1 {
            error!("file has several names: {:?}", names);
        }
        current_path.push(names[0].1);
        if names[0].0.is_dir() {
            archive_rec(txn, branch, key, builder, buffer, forward, current_path)?;
        } else {
            buffer.clear();
            let mut graph = txn.retrieve(&branch, key);
            txn.output_file(branch, buffer, &mut graph, forward)?;
            let mut header = Header::new_gnu();
            header.set_path(&current_path)?;
            header.set_size(buffer.len() as u64);
            header.set_mode(names[0].0.permissions() as u32);
            header.set_cksum();
            builder.append(&header, &buffer[..])?;
        }
        current_path.pop();
    }
    Ok(())
}

pub fn explain(res: Result<(), Error>) {
    default_explain(res)
}
