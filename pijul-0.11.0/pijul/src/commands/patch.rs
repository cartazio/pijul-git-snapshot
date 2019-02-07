use super::validate_base58;
use clap::{Arg, ArgGroup, ArgMatches, SubCommand};
use commands::{default_explain, BasicOptions, StaticSubcommand};
use isatty::stdout_isatty;
use libpijul;
use libpijul::fs_representation::patches_dir;
use libpijul::graph::LineBuffer;
use libpijul::patch::{Change, NewEdge, Patch};
use libpijul::{Branch, EdgeFlags, Hash, Key, LineId, PatchId, Transaction, Txn, Value, ROOT_KEY};
use serde_json;
use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{copy, stdout, BufReader, Write};
use std::str::from_utf8;
use term;
use term::StdoutTerminal;
use error::Error;

pub fn invocation() -> StaticSubcommand {
    SubCommand::with_name("patch")
        .about("Output a patch")
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .help("Path to the repository where the patches will be applied.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("patch")
                .help("The hash of the patch to be printed.")
                .takes_value(true)
                .required(true)
                .validator(validate_base58),
        )
        .arg(
            Arg::with_name("bin")
                .long("bin")
                .help("Output the patch in binary."),
        )
        .arg(
            Arg::with_name("name")
                .long("name")
                .help("Output the patch name."),
        )
        .arg(
            Arg::with_name("description")
                .long("description")
                .help("Output the patch description."),
        )
        .arg(
            Arg::with_name("authors")
                .long("authors")
                .help("Output the patch authors."),
        )
        .arg(
            Arg::with_name("date")
                .long("date")
                .help("Output the patch date."),
        )
        .group(ArgGroup::with_name("details").required(false).args(&[
            "bin",
            "name",
            "description",
            "date",
            "authors",
        ]))
}

#[derive(PartialEq, Eq)]
enum View {
    Normal,
    Bin,
    NameOnly,
    DescrOnly,
    DateOnly,
    AuthorsOnly,
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    let opts = BasicOptions::from_args(args)?;
    let patch = Hash::from_base58(args.value_of("patch").unwrap()).unwrap();
    let mut patch_path = patches_dir(&opts.repo_root).join(&patch.to_base58());
    patch_path.set_extension("gz");
    let mut f = File::open(&patch_path)?;

    let v: View = match (
        args.is_present("bin"),
        args.is_present("name"),
        args.is_present("description"),
        args.is_present("date"),
        args.is_present("authors"),
    ) {
        (true, _, _, _, _) => View::Bin,
        (_, true, _, _, _) => View::NameOnly,
        (_, _, true, _, _) => View::DescrOnly,
        (_, _, _, true, _) => View::DateOnly,
        (_, _, _, _, true) => View::AuthorsOnly,
        (_, _, _, _, _) => View::Normal,
    };

    if v == View::Bin {
        let mut stdout = stdout();
        copy(&mut f, &mut stdout)?;
    } else {
        // Write the patch in text.
        let mut f = BufReader::new(f);
        let (hash, _, patch) = Patch::from_reader_compressed(&mut f)?;

        match v {
            View::AuthorsOnly => print!("{:?}", patch.authors),
            View::DescrOnly => print!("{}", patch.description.clone().unwrap_or("".into())),
            View::DateOnly => print!("{:?}", patch.timestamp),
            View::NameOnly => print!("{}", patch.name),
            _ => {
                // it cannot be View::Bin, so it has to be View::Normal
                let repo = opts.open_repo()?;
                let txn = repo.txn_begin()?;
                let branch = txn.get_branch(&opts.branch()).expect("Branch not found");
                let internal = txn.get_internal(hash.as_ref())
                    .expect("Patch not in repository")
                    .to_owned();

                {
                    let s = serde_json::to_string_pretty(&patch.to_pretty()).unwrap();
                    let mut f = File::create("patch").unwrap();
                    f.write_all(s.as_bytes()).unwrap();
                }

                let mut buf = LineNumbers {
                    n: 0, // The graph will start from a file base name.
                    patch: internal.clone(),
                    current_file: ROOT_KEY,
                    numbers: HashMap::new(),
                };

                let mut terminal = if stdout_isatty() {
                    term::stdout()
                } else {
                    None
                };
                for c in patch.changes() {
                    match *c {
                        Change::NewNodes {
                            ref up_context,
                            ref flag,
                            ref nodes,
                            ref line_num,
                            ..
                        } => {
                            if flag.contains(EdgeFlags::FOLDER_EDGE) {
                                /*render_new_folder(&txn, branch, deleted_files,
                                internal, up_context, down_context, nodes)?*/
                            } else {
                                render_new_change(
                                    &mut terminal,
                                    &txn,
                                    &branch,
                                    &mut buf,
                                    internal,
                                    up_context,
                                    line_num,
                                    nodes,
                                )?
                            }
                        }
                        Change::NewEdges {
                            ref edges, flag, ..
                        } => render_new_edges(
                            &mut terminal,
                            &txn,
                            &branch,
                            &mut buf,
                            internal,
                            edges,
                            flag,
                        )?,
                    }
                }
            }
        }
    }
    Ok(())
}

fn file_names(txn: &Txn, branch: &Branch, files: &[Key<PatchId>]) -> Result<(), Error> {
    let file_names: Vec<_> = files
        .iter()
        .flat_map(|x| {
            debug!("file_names {:?}", x);
            txn.get_file_names(branch, x.clone())
                .into_iter()
                .map(|(_, name)| name)
        })
        .collect();

    debug!("file_names = {:?}", file_names);
    // assert_eq!(file_names.len(), 1);

    print!("In \"{:?}\"", file_names[0]);
    if file_names.len() > 1 {
        print!("(also known as {:?}", file_names[1]);
        for name in file_names.iter().skip(2) {
            print!(", {:?}", name);
        }
        println!("):");
    } else {
        println!(":")
    }
    Ok(())
}

const INVALID_UTF8: &'static str = "(Invalid UTF-8)";

fn render_new_change(
    term: &mut Option<Box<StdoutTerminal>>,
    txn: &Txn,
    branch: &Branch,
    buf: &mut LineNumbers,
    internal: PatchId,
    up_context: &[Key<Option<Hash>>],
    line_num: &LineId,
    nodes: &[Vec<u8>],
) -> Result<(), Error> {
    // Find the file
    let mut find_alive = libpijul::apply::find_alive::FindAlive::new();
    let mut alive = HashSet::new();
    let files = if up_context.is_empty() {
        panic!("up context is empty")
    } else {
        let up = txn.internal_key(&up_context[0], internal);
        let mut file = None;
        txn.find_alive_nonfolder_ancestors(
            branch,
            &mut find_alive,
            &mut alive,
            &mut file,
            up.clone(),
        );
        if let Some(file) = file {
            vec![file]
        } else {
            txn.get_file(branch, *alive.iter().next().unwrap())
        }
    };
    debug!("render_new_change, files = {:?}", files);
    file_names(txn, branch, &files)?;

    let mut ret = txn.retrieve(branch, files[0]);
    let mut v = Vec::new();
    let mut key = Key {
        patch: internal.clone(),
        line: line_num.clone(),
    };
    if buf.numbers.get(&key).is_none() {
        buf.n = 0;
        txn.output_file(branch, buf, &mut ret, &mut v)?;
    }
    let mut current: isize = -1;
    debug!("numbers: {:?}", buf.numbers);
    for n in nodes.iter() {
        debug!("key: {:?}", key);
        if let Some(&(_, line_num)) = buf.numbers.get(&key) {
            if line_num != current + 1 {
                println!("From line {}:", line_num);
            }
            current = line_num as isize;
        } else {
            println!("Deleted in a subsequent patch:");
        }
        if let Some(ref mut term) = *term {
            term.fg(term::color::GREEN).unwrap_or(());
        }
        print!("+ ");
        if let Some(ref mut term) = *term {
            term.reset().unwrap_or(());
        }

        if let Ok(n) = from_utf8(&n) {
            print!("{}", n);
            if !n.ends_with("\n") {
                println!("");
            }
        } else {
            println!("{}", INVALID_UTF8)
        }
        key.line += 1
    }
    Ok(())
}

#[derive(Debug)]
struct LineNumbers {
    n: isize,
    patch: PatchId,
    current_file: Key<PatchId>,
    numbers: HashMap<Key<PatchId>, (Key<PatchId>, isize)>,
}

impl<'a, T: 'a + Transaction> LineBuffer<'a, T> for LineNumbers {
    fn output_line(&mut self, key: &Key<PatchId>, _: Value<'a, T>) -> libpijul::Result<()> {
        self.numbers
            .insert(key.clone(), (self.current_file.clone(), self.n));
        self.n += 1;
        Ok(())
    }
    fn output_conflict_marker(&mut self, _: &str) -> libpijul::Result<()> {
        self.n += 1;
        Ok(())
    }
}

fn render_new_edges(
    term: &mut Option<Box<StdoutTerminal>>,
    txn: &Txn,
    branch: &Branch,
    buf: &mut LineNumbers,
    internal: PatchId,
    edges: &[NewEdge],
    flag: EdgeFlags,
) -> Result<(), Error> {
    let mut find_alive = libpijul::apply::find_alive::FindAlive::new();
    let mut alive = HashSet::new();
    let mut redundant = Vec::new();
    if !flag.contains(EdgeFlags::DELETED_EDGE) {
        // Looks like a conflict resolution, I don't know how to print
        // those edges.
        return Ok(());
    }
    let mut fnames = Vec::new();
    let mut current_node = ROOT_KEY;
    let mut current_line_num = -1;
    for e in edges {
        let (from, to) = if flag.contains(EdgeFlags::PARENT_EDGE) {
            (
                txn.internal_key(&e.to, internal),
                txn.internal_key(&e.from, internal),
            )
        } else {
            (
                txn.internal_key(&e.from, internal),
                txn.internal_key(&e.to, internal),
            )
        };
        debug!("from {:?} to {:?}", from, to);

        // Find the last alive ancestor(s) to the deleted lines.
        let mut file = None;
        alive.clear();
        txn.find_alive_nonfolder_ancestors(
            branch,
            &mut find_alive,
            &mut alive,
            &mut file,
            to.clone(),
        );
        debug!("{:?}", alive);
        let mut last_num = -1;
        for &key in alive.iter() {
            if buf.numbers.get(&key).is_none() {
                debug!("starting key {:?}", key);
                let files = txn.get_file(branch, key);
                fnames.extend(files.iter().flat_map(|x| {
                    txn.get_file_names(branch, x.clone())
                        .into_iter()
                        .map(|(_, name)| name)
                }));
                let mut ret = txn.retrieve(branch, files[0]);
                redundant.clear();
                buf.current_file = files[0].clone();
                buf.n = 0;
                txn.output_file(branch, buf, &mut ret, &mut redundant)?;
            }
            debug!("buf {:?}", buf);
            if let Some(&(ref file, num)) = buf.numbers.get(&key) {
                last_num = max(num, last_num);
                debug!("{:?} {:?}", file, num)
            }
        }
        // Maybe new lines have been inserted by subsequent patches,
        // (for instance before their deletion by this patch, and they
        // have not been deleted).
        //
        // This can cause this hunk's line numbers to be
        // non-contiguous.
        if last_num != current_line_num {
            println!("After line {}:", last_num + 1);
            current_line_num = last_num
        }
        if to != current_node {
            if let Some(contents) = txn.get_contents(to) {
                if let Some(ref mut term) = *term {
                    term.fg(term::color::RED).unwrap_or(());
                }
                print!("- ");
                if let Some(ref mut term) = *term {
                    term.reset().unwrap_or(());
                }

                let mut is_valid = true;
                let mut cont = String::new();
                for chunk in contents {
                    let c = from_utf8(chunk);
                    if let Ok(c) = c {
                        cont.push_str(c)
                    } else {
                        is_valid = false;
                        break;
                    }
                }
                if is_valid {
                    print!("{}", cont);
                    if !cont.ends_with("\n") {
                        println!("");
                    }
                } else {
                    println!("{}", INVALID_UTF8)
                }
            }
        }
        current_node = to
    }
    Ok(())
}

pub fn explain(r: Result<(), Error>) {
    default_explain(r)
}
