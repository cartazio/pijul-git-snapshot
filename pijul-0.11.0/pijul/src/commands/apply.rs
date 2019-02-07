use super::{get_current_branch, validate_base58, BasicOptions};
use clap::{Arg, ArgMatches, SubCommand};
use commands::{default_explain, StaticSubcommand};
use error::Error;
use libpijul::patch::Patch;
use libpijul::{apply_resize, apply_resize_no_output, Hash};
use std::collections::HashSet;
use std::fs::File;
use std::io::{stdin, Read, Write};

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("apply")
        .about("Apply a patch")
        .arg(
            Arg::with_name("patch")
                .help(
                    "Hash of the patch to apply, in base58. If no patch is given, patches are \
                     read from the standard input.",
                )
                .takes_value(true)
                .multiple(true)
                .validator(validate_base58),
        )
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .help(
                    "Path to the repository where the patches will be applied. Defaults to the \
                     repository containing the current directory.",
                )
                .takes_value(true),
        )
        .arg(
            Arg::with_name("json")
                .long("json")
                .help(
                    "Accept patch in JSON format (for debugging only).",
                )
        )
        .arg(
            Arg::with_name("branch")
                .long("branch")
                .help(
                    "The branch to which the patches will be applied. Defaults to the current \
                     branch.",
                )
                .takes_value(true),
        )
        .arg(
            Arg::with_name("no-output")
                .long("no-output")
                .help("Only apply the patch, don't output it to the repository."),
        );
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    let opts = BasicOptions::from_args(args)?;
    debug!("applying");
    let mut remote = HashSet::new();

    // let remote: HashSet<Hash> =
    let mut has_patches = false;
    if let Some(hashes) = args.values_of("patch") {
        remote.extend(hashes.map(|h| Hash::from_base58(&h).unwrap()));
        has_patches = true
    }

    if !has_patches {
        // Read patches in gz format from stdin.
        let mut buf = Vec::new();
        stdin().read_to_end(&mut buf)?;

        let mut buf_ = &buf[..];
        let mut i = 0;
        if args.is_present("json") {
            let patch:Patch = serde_json::from_reader(&buf[..]).unwrap();
            let path = opts.patches_dir();
            let h = patch.save(&path, None)?;
            remote.insert(h);
        } else {
            while let Ok((h, _, patch)) = Patch::from_reader_compressed(&mut buf_) {
                debug!("{:?}", patch);

                {
                    let mut path = opts.patches_dir();
                    path.push(h.to_base58());
                    path.set_extension("gz");
                    let mut f = File::create(&path)?;
                    f.write_all(&buf[i..(buf.len() - buf_.len())])?;
                    i = buf.len() - buf_.len();
                }

                remote.insert(h);
            }
        }
    }

    debug!("remote={:?}", remote);
    let is_current_branch = if let Ok(br) = get_current_branch(&opts.repo_root) {
        br == opts.branch()
    } else {
        false
    };
    loop {
        let result = if args.is_present("no-output") || !is_current_branch {
            apply_resize_no_output(&opts.repo_root, &opts.branch(), remote.iter(), |_, _| ())
        } else {
            apply_resize(
                &opts.repo_root,
                &opts.branch(),
                remote.iter(),
                &[] as &[&str],
                |_, _| {},
            )
        };
        match result {
            Err(ref e) if e.lacks_space() => {}
            Ok(()) => return Ok(()),
            Err(e) => return Err(From::from(e)),
        }
    }
}

pub fn explain(res: Result<(), Error>) {
    default_explain(res)
}
