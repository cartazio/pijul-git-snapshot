use clap::{Arg, ArgMatches, SubCommand};
use std::collections::HashSet;
use std::mem;
use std::path::Path;
use std::string::String;

use libpijul::fs_representation::{find_repo_root, pristine_dir, read_patch};
use libpijul::{Hash, Repository, DEFAULT_BRANCH};
use error::Error;
use super::{default_explain, get_current_branch, get_wd, StaticSubcommand, validate_base58};

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("show-dependencies")
        .about("Print the patch dependencies using the DOT syntax in stdout")
        .arg(
            Arg::with_name("hash")
                .help("Hash of a patch.")
                .takes_value(true)
                .required(false)
                .multiple(true)
                .validator(validate_base58),
        )
        .arg(
            Arg::with_name("depth")
                .long("depth")
                .help("The depth of the dependencies graph")
                .takes_value(true)
                .required(false)
                .validator(|x| {
                    if let Ok(x) = x.parse::<usize>() {
                        if x >= 1 {
                            return Ok(());
                        }
                    }
                    Err("The depth argument must be an integer, and at least 1".to_owned())
                }),
        )
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .help("Local repository.")
                .multiple(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("branch")
                .long("branch")
                .help("Branch.")
                .takes_value(true)
                .required(false),
        );
}

enum Target<'a> {
    Branch(Option<&'a str>),
    Hash(Vec<&'a str>, usize),
}

pub struct Params<'a> {
    pub repository: Option<&'a Path>,
    target: Target<'a>,
}

pub fn parse_args<'a>(args: &'a ArgMatches) -> Result<Params<'a>, Error> {
    let target = if let Some(hash) = args.values_of("hash") {
        let depth = args.value_of("depth")
            .unwrap_or("1")
            .parse::<usize>()
            .unwrap();

        Target::Hash(hash.collect(), depth)
    } else {
        Target::Branch(args.value_of("branch"))
    };

    Ok(Params {
        repository: args.value_of("repository").map(|x| Path::new(x)),
        target: target,
    })
}

fn label_sanitize(str: String) -> String {
    // First, we escape the quotes, because otherwise it may interfere with dot
    // notation.
    let label = str.replace("\"", "\\\"");

    // Then, to get a more readable graph, we add line breaks every five words,
    // in order to avoid very width nodes.
    let mut words = label.split_whitespace();

    let mut nth = 0;
    let mut res = String::from("");

    if let Some(first_word) = words.next() {
        res.push_str(first_word);

        for word in words {
            if nth >= 5 {
                res.push_str("\\n");
                nth = 0;
            } else {
                res.push_str(" ");
                nth += 1;
            }

            res.push_str(word);
        }
    }

    res
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    let args = parse_args(args)?;
    let wd = get_wd(args.repository)?;
    let target = if let Some(r) = find_repo_root(&wd) {
        r
    } else {
        return Err(Error::NotInARepository);
    };
    let repo_dir = pristine_dir(&target);
    let repo = Repository::open(&repo_dir, None)?;
    let txn = repo.txn_begin()?;

    match args.target {
        Target::Branch(branch_arg) => {
            let branch_name = if let Some(b) = branch_arg {
                b.to_string()
            } else if let Ok(b) = get_current_branch(&target) {
                b
            } else {
                DEFAULT_BRANCH.to_string()
            };

            if let Some(branch) = txn.get_branch(&branch_name) {
                println!("digraph dependencies {{");
                println!("  graph [rankdir=LR];");

                for (_, hash) in txn.rev_iter_applied(&branch, None) {
                    let hash_ext = txn.get_external(hash).unwrap();
                    let patch = read_patch(&target, hash_ext)?;

                    patch_node(
                        hash_ext.to_base58(),
                        patch.header().name.clone(),
                        patch.is_tag(),
                    );

                    let deps = txn.minimize_deps(patch.dependencies());
                    for hash_dep in deps {
                        println!("  N{} -> N{}", hash_ext.to_base58(), hash_dep.to_base58());
                    }
                }
                println!("}}");
            }
        }
        Target::Hash(hashes, depth) => {
            let mut seen = HashSet::new();
            let mut vec: Vec<_> = hashes
                .iter()
                .map(|h| Hash::from_base58(h).unwrap())
                .collect();
            let mut next = Vec::new();

            println!("digraph dependencies {{");
            println!("  graph [rankdir=LR];");

            for _ in 0..depth {
                for hash in vec.drain(..) {
                    debug!("hash: {:?}", hash);
                    seen.insert(hash.clone());
                    let hash_ext = hash.as_ref();
                    let patch = read_patch(&target, hash_ext)?;

                    patch_node(
                        hash_ext.to_base58(),
                        patch.header().name.clone(),
                        patch.is_tag(),
                    );

                    let deps = txn.minimize_deps(patch.dependencies());
                    for hash_dep in deps.iter() {
                        debug!("dep: {:?}", hash_dep);
                        println!("  N{} -> N{}", hash_ext.to_base58(), hash_dep.to_base58());

                        let h = hash_dep.to_owned();

                        if !seen.contains(&h) {
                            seen.insert(h.clone());
                            next.push(h);
                        }
                    }
                }

                // vec should be empty, has it has been consumed by drain
                // on the other hand, next contains all the
                // dependencies to walk into in the next loop
                // iteration
                mem::swap(&mut next, &mut vec);
            }

            // lets have a last for to get the name of the last dependencies
            for hash in vec.drain(..) {
                let hash_ext = hash.as_ref();
                let patch = read_patch(&target, hash_ext)?;

                patch_node(
                    hash_ext.to_base58(),
                    patch.header().name.clone(),
                    patch.is_tag(),
                );
            }

            // and we are done
            println!("}}");
        }
    }

    Ok(())
}

fn patch_node(hash: String, name: String, is_tag: bool) {
    if is_tag {
        println!(
            "  N{} [label=\"TAG: {}\", shape=box]",
            hash,
            label_sanitize(name)
        );
    } else {
        println!("  N{} [label=\"{}\"]", hash, label_sanitize(name));
    }
}

pub fn explain(res: Result<(), Error>) {
    default_explain(res)
}
