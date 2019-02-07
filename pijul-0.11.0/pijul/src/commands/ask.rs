use getch;
use libpijul::patch::{Change, ChangeContext, Patch, PatchHeader, Record};
use std::io::prelude::*;

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::io::stdout;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use regex::Regex;

use libpijul::fs_representation::PIJUL_DIR_NAME;

use error::Error;
use isatty::stdout_isatty;
use libpijul::{EdgeFlags, Hash, LineId, MutTxn, PatchId};
use rand;
use std;
use std::char::from_u32;
use std::fs::{remove_file, File};
use std::process;
use std::str;
use term;
use term::{Attr, StdoutTerminal};

use ignore::gitignore::GitignoreBuilder;
use line;
use relativize::relativize;

const BINARY_CONTENTS: &'static str = "<binary contents>";
#[derive(Clone, Copy)]
pub enum Command {
    Pull,
    Push,
    Unrecord,
}

impl Command {
    fn verb(&self) -> &'static str {
        match *self {
            Command::Push => "push",
            Command::Pull => "pull",
            Command::Unrecord => "unrecord",
        }
    }
}

fn print_section(term: &mut Option<Box<StdoutTerminal>>, title: &str, contents: &str) {
    if let Some(ref mut term) = *term {
        term.attr(Attr::Bold).unwrap_or(());
    }
    let mut stdout = std::io::stdout();
    write!(stdout, "{}", title).unwrap_or(());
    if let Some(ref mut term) = *term {
        term.reset().unwrap_or(());
    }
    writeln!(stdout, "{}", contents).unwrap_or(());
}

pub fn print_patch_descr(
    term: &mut Option<Box<StdoutTerminal>>,
    hash: &Hash,
    internal: Option<PatchId>,
    patch: &PatchHeader,
) {
    print_section(term, "Hash:", &format!(" {}", &hash.to_base58()));
    if let Some(internal) = internal {
        print_section(term, "Internal id:", &format!(" {}", &internal.to_base58()));
    }

    print_section(term, "Authors:", &format!(" {}", patch.authors.join(", ")));
    print_section(term, "Timestamp:", &format!(" {}", patch.timestamp));

    let is_tag = if !patch.flag.is_empty() { "TAG: " } else { "" };

    let mut stdout = std::io::stdout();
    writeln!(stdout, "\n    {}{}", is_tag, patch.name).unwrap_or(());
    if let Some(ref d) = patch.description {
        writeln!(stdout, "").unwrap_or(());
        for descr_line in d.lines() {
            writeln!(stdout, "    {}", descr_line).unwrap_or(());
        }
    }
    writeln!(stdout, "").unwrap_or(());
}

fn check_forced_decision(
    command: Command,
    choices: &HashMap<&Hash, bool>,
    rev_dependencies: &HashMap<&Hash, Vec<&Hash>>,
    a: &Hash,
    b: &Patch,
) -> Option<bool> {
    let covariant = match command {
        Command::Pull | Command::Push => true,
        Command::Unrecord => false,
    };
    // If we've selected patches that depend on a, and this is a pull
    // or a push, select a.
    if let Some(x) = rev_dependencies.get(a) {
        for y in x {
            // Here, y depends on a.
            //
            // If this command is covariant, and we've selected y, select a.
            // If this command is covariant, and we've unselected y, don't do anything.
            //
            // If this command is contravariant, and we've selected y, don't do anything.
            // If this command is contravariant, and we've unselected y, unselect a.
            if let Some(&choice) = choices.get(y) {
                if choice == covariant {
                    return Some(covariant);
                }
            }
        }
    };

    // If we've unselected dependencies of a, unselect a.
    for y in b.dependencies().iter() {
        // Here, a depends on y.
        //
        // If this command is covariant, and we've selected y, don't do anything.
        // If this command is covariant, and we've unselected y, unselect a.
        //
        // If this command is contravariant, and we've selected y, select a.
        // If this command is contravariant, and we've unselected y, don't do anything.

        if let Some(&choice) = choices.get(&y) {
            if choice != covariant {
                return Some(!covariant);
            }
        }
    }

    None
}

fn interactive_ask(
    getch: &getch::Getch,
    a: &Hash,
    patchid: Option<PatchId>,
    b: &Patch,
    command_name: Command,
    show_help: bool,
) -> Result<(char, Option<bool>), Error> {
    let mut term = if stdout_isatty() {
        term::stdout()
    } else {
        None
    };
    print_patch_descr(&mut term, a, patchid, b);

    if show_help {
        display_help(command_name);
        print!("Shall I {} this patch? ", command_name.verb());
    } else {
        print!("Shall I {} this patch? [ynkad?] ", command_name.verb());
    }

    stdout().flush()?;
    match getch.getch().ok().and_then(|x| from_u32(x as u32)) {
        Some(e) => {
            println!("{}", e);
            let e = e.to_uppercase().next().unwrap_or('\0');
            match e {
                'A' => Ok(('Y', Some(true))),
                'D' => Ok(('N', Some(false))),
                e => Ok((e, None)),
            }
        }
        _ => Ok(('\0', None)),
    }
}

fn display_help(c: Command) {
    println!("Available options: ynkad?");
    println!("y: {} this patch", c.verb());
    println!("n: don't {} this patch", c.verb());
    println!("k: go bacK to the previous patch");
    println!("a: {} all remaining patches", c.verb());
    println!("d: finish, skipping all remaining patches");
    println!("")
}

/// Patches might have a dummy "changes" field here.
pub fn ask_patches(
    command: Command,
    patches: &[(Hash, Option<PatchId>, Patch)],
) -> Result<HashSet<Hash>, Error> {
    let getch = getch::Getch::new();
    let mut i = 0;

    // Record of the user's choices.
    let mut choices: HashMap<&Hash, bool> = HashMap::new();

    // For each patch, the list of patches that depend on it.
    let mut rev_dependencies: HashMap<&Hash, Vec<&Hash>> = HashMap::new();

    // Decision for the remaining patches ('a' or 'd'), if any.
    let mut final_decision = None;
    let mut show_help = false;

    while i < patches.len() {
        let (ref a, patchid, ref b) = patches[i];
        let forced_decision = check_forced_decision(command, &choices, &rev_dependencies, a, b);

        // Is the decision already forced by a previous choice?
        let e = match final_decision.or(forced_decision) {
            Some(true) => 'Y',
            Some(false) => 'N',
            None => {
                debug!("decision not forced");
                let (current, remaining) =
                    interactive_ask(&getch, a, patchid, b, command, show_help)?;
                final_decision = remaining;
                current
            }
        };

        show_help = false;

        debug!("decision: {:?}", e);
        match e {
            'Y' => {
                choices.insert(a, true);
                match command {
                    Command::Pull | Command::Push => for ref dep in b.dependencies().iter() {
                        let d = rev_dependencies.entry(dep).or_insert(vec![]);
                        d.push(a)
                    },
                    Command::Unrecord => {}
                }
                i += 1
            }
            'N' => {
                choices.insert(a, false);
                match command {
                    Command::Unrecord => for ref dep in b.dependencies().iter() {
                        let d = rev_dependencies.entry(dep).or_insert(vec![]);
                        d.push(a)
                    },
                    Command::Pull | Command::Push => {}
                }
                i += 1
            }
            'K' if i > 0 => {
                let (ref a, _, _) = patches[i];
                choices.remove(a);
                i -= 1
            }
            '?' => {
                show_help = true;
            }
            _ => {}
        }
    }
    Ok(choices
        .into_iter()
        .filter(|&(_, selected)| selected)
        .map(|(x, _)| x.to_owned())
        .collect())
}

/// Compute the dependencies of this change.
fn change_deps(
    id: usize,
    c: &Record<ChangeContext<Hash>>,
    provided_by: &mut HashMap<LineId, usize>,
) -> HashSet<LineId> {
    let mut s = HashSet::new();
    for c in c.iter() {
        match *c {
            Change::NewNodes {
                ref up_context,
                ref down_context,
                ref line_num,
                ref nodes,
                ..
            } => {
                for cont in up_context.iter().chain(down_context) {
                    if cont.patch.is_none() && !cont.line.is_root() {
                        s.insert(cont.line.clone());
                    }
                }
                for i in 0..nodes.len() {
                    provided_by.insert(*line_num + i, id);
                }
            }
            Change::NewEdges { ref edges, .. } => for e in edges {
                if e.from.patch.is_none() && !e.from.line.is_root() {
                    s.insert(e.from.line.clone());
                }
                if e.to.patch.is_none() && !e.from.line.is_root() {
                    s.insert(e.to.line.clone());
                }
            },
        }
    }
    s
}

fn print_change<T: rand::Rng>(
    term: &mut Option<Box<StdoutTerminal>>,
    cwd: &Path,
    repo: &MutTxn<T>,
    current_file: &mut Option<Rc<PathBuf>>,
    c: &Record<ChangeContext<Hash>>,
) -> Result<(), Error> {
    match *c {
        Record::FileAdd { ref name, .. } => {
            if let Some(ref mut term) = *term {
                term.fg(term::color::CYAN).unwrap_or(());
            }
            print!("added file ");
            if let Some(ref mut term) = *term {
                term.reset().unwrap_or(());
            }
            println!("{}", relativize(cwd, Path::new(&name)).display());
            Ok(())
        }
        Record::FileDel { ref name, .. } => {
            if let Some(ref mut term) = *term {
                term.fg(term::color::MAGENTA).unwrap_or(());
            }
            print!("deleted file: ");
            if let Some(ref mut term) = *term {
                term.reset().unwrap_or(());
            }
            println!("{}", relativize(cwd, Path::new(&name)).display());
            Ok(())
        }
        Record::FileMove { ref new_name, .. } => {
            if let Some(ref mut term) = *term {
                term.fg(term::color::YELLOW).unwrap_or(());
            }
            print!("file moved to: ");
            if let Some(ref mut term) = *term {
                term.reset().unwrap_or(());
            }
            println!("{}", relativize(cwd, Path::new(new_name)).display());
            Ok(())
        }
        Record::Replace {
            ref adds,
            ref dels,
            ref file,
            ..
        } => {
            let r = Record::Change {
                change: dels.clone(),
                file: file.clone(),
                conflict_reordering: Vec::new(),
            };
            print_change(term, cwd, repo, current_file, &r)?;
            let r = Record::Change {
                change: adds.clone(),
                file: file.clone(),
                conflict_reordering: Vec::new(),
            };
            print_change(term, cwd, repo, current_file, &r)
        }
        Record::Change {
            ref change,
            ref file,
            ..
        } => {
            match *change {
                Change::NewNodes {
                    // ref up_context,ref down_context,ref line_num,
                    ref flag,
                    ref nodes,
                    ..
                } => {
                    for n in nodes {
                        if flag.contains(EdgeFlags::FOLDER_EDGE) {
                            if n.len() >= 2 {
                                if let Some(ref mut term) = *term {
                                    term.fg(term::color::CYAN).unwrap_or(());
                                }
                                print!("new file ");
                                if let Some(ref mut term) = *term {
                                    term.reset().unwrap_or(());
                                }
                                println!("{}", str::from_utf8(&n[2..]).unwrap_or(""));
                            }
                        } else {
                            let s = str::from_utf8(n).unwrap_or(BINARY_CONTENTS);
                            let mut file_changed = true;
                            if let Some(ref cur_file) = *current_file {
                                if file == cur_file {
                                    file_changed = false;
                                }
                            }
                            if file_changed {
                                if let Some(ref mut term) = *term {
                                    term.attr(Attr::Bold).unwrap_or(());
                                    term.attr(Attr::Underline(true)).unwrap_or(());
                                }
                                println!("In file {:?}\n", relativize(cwd, file.as_path()));
                                if let Some(ref mut term) = *term {
                                    term.reset().unwrap_or(());
                                }
                                *current_file = Some(file.clone())
                            }
                            if let Some(ref mut term) = *term {
                                term.fg(term::color::GREEN).unwrap_or(());
                            }
                            print!("+ ");
                            if let Some(ref mut term) = *term {
                                term.reset().unwrap_or(());
                            }
                            if s.ends_with("\n") {
                                print!("{}", s);
                            } else {
                                println!("{}", s);
                            }
                        }
                    }
                    Ok(())
                }
                Change::NewEdges {
                    ref edges, flag, ..
                } => {
                    let mut h_targets = HashSet::with_capacity(edges.len());
                    for e in edges {
                        let (target, flag) = if !flag.contains(EdgeFlags::PARENT_EDGE) {
                            if h_targets.insert(&e.to) {
                                (Some(&e.to), flag)
                            } else {
                                (None, flag)
                            }
                        } else {
                            if h_targets.insert(&e.from) {
                                (Some(&e.from), flag)
                            } else {
                                (None, flag)
                            }
                        };
                        if let Some(target) = target {
                            let internal = repo.internal_key_unwrap(target);
                            let l = repo.get_contents(internal).unwrap();
                            let l = l.into_cow();
                            let s = str::from_utf8(&l).unwrap_or(BINARY_CONTENTS);

                            let mut file_changed = true;
                            if let Some(ref cur_file) = *current_file {
                                if file == cur_file {
                                    file_changed = false;
                                }
                            }
                            if file_changed {
                                if let Some(ref mut term) = *term {
                                    term.attr(Attr::Bold).unwrap_or(());
                                    term.attr(Attr::Underline(true)).unwrap_or(());
                                }
                                println!(
                                    "In file {:?}\n",
                                    relativize(cwd, file.as_path()).display()
                                );
                                if let Some(ref mut term) = *term {
                                    term.reset().unwrap_or(());
                                }
                                *current_file = Some(file.clone())
                            }

                            if flag.contains(EdgeFlags::DELETED_EDGE) {
                                if let Some(ref mut term) = *term {
                                    term.fg(term::color::RED).unwrap_or(());
                                }
                                print!("- ");
                            } else {
                                if let Some(ref mut term) = *term {
                                    term.fg(term::color::GREEN).unwrap_or(());
                                }
                                print!("+ ");
                            }
                            if let Some(ref mut term) = *term {
                                term.reset().unwrap_or(());
                            }
                            if s.ends_with("\n") {
                                print!("{}", s)
                            } else {
                                println!("{}", s)
                            }
                        }
                    }
                    Ok(())
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ChangesDirection {
    Record,
    Revert,
}

impl ChangesDirection {
    fn is_record(&self) -> bool {
        match *self {
            ChangesDirection::Record => true,
            _ => false,
        }
    }
    fn verb(&self) -> &str {
        match *self {
            ChangesDirection::Record => "record",
            ChangesDirection::Revert => "revert",
        }
    }
}

fn display_help_changes(potential_new_ignore: Option<&str>, direction: ChangesDirection) {
    println!("Available options:");
    println!("y: {} this change", direction.verb());
    println!("n: don't {} this change", direction.verb());
    println!("k: go back to the previous change");
    println!("a: {} all remaining changes", direction.verb());
    println!("d: skip all remaining changes");
    match potential_new_ignore {
        Some(filename) => println!("i: ignore file {}", filename),
        None => (),
    }
    println!("")
}

fn prompt_one_change<T: rand::Rng>(
    repository: &MutTxn<T>,
    change: &Record<ChangeContext<Hash>>,
    current_file: &mut Option<Rc<PathBuf>>,
    n_changes: usize,
    i: usize,
    direction: ChangesDirection,
    potential_new_ignore: Option<&str>,
    terminal: &mut Option<Box<StdoutTerminal>>,
    getch: &getch::Getch,
    cwd: &Path,
    show_help: bool,
) -> Result<(char, Option<char>), Error> {
    debug!("changes: {:?}", change);
    print_change(terminal, cwd, repository, current_file, &change)?;
    println!("");
    let choices = if potential_new_ignore.is_some() {
        "[ynkadi?]"
    } else {
        "[ynkad?]"
    };
    if show_help {
        display_help_changes(potential_new_ignore, direction);
        print!(
            "Shall I {} this change? ({}/{}) ",
            direction.verb(),
            i + 1,
            n_changes
        );
    } else {
        print!(
            "Shall I {} this change? ({}/{}) {} ",
            direction.verb(),
            i + 1,
            n_changes,
            choices
        );
    }
    stdout().flush()?;
    match getch.getch().ok().and_then(|x| from_u32(x as u32)) {
        Some(e) => {
            println!("{}\n", e);
            let e = e.to_uppercase().next().unwrap_or('\0');
            match e {
                'A' => Ok(('Y', Some('Y'))),
                'D' => Ok(('N', Some('N'))),
                e => Ok((e, None)),
            }
        }
        _ => Ok(('\0', None)),
    }
}

fn add_to_ignore_file(
    file: &str,
    repo_root: &Path,
    new_ignored_patterns: &mut Vec<String>,
    new_ignore_builder: &mut GitignoreBuilder,
) {
    loop {
        let file = relativize(repo_root, Path::new(file));
        let pat = read_line_with_suggestion(
            "Pattern to add to ignore file (relative to repository root, empty to add nothing)? ",
            &file.to_string_lossy(),
        );
        if pat.is_empty() {
            return;
        };

        let mut ignore_builder = GitignoreBuilder::new(repo_root);
        let add_ok = match ignore_builder.add_line(None, &pat) {
            Ok(i) => match i.build() {
                Ok(i) => i.matched_path_or_any_parents(&file, false).is_ignore(),
                Err(e) => {
                    println!("could not match pattern {}: {}", &pat, e);
                    false
                }
            },
            Err(e) => {
                println!("did not understand pattern {}: {}", &pat, e);
                false
            }
        };
        if add_ok {
            new_ignore_builder.add_line(None, &pat).unwrap();
            new_ignored_patterns.push(pat);
            return;
        }
        println!(
            "pattern {} is incorrect or does not match {}",
            pat,
            &file.display()
        );
    }
}

pub fn ask_changes<T: rand::Rng>(
    repository: &MutTxn<T>,
    repo_root: &Path,
    cwd: &Path,
    changes: &[Record<ChangeContext<Hash>>],
    direction: ChangesDirection,
    to_unadd: &mut HashSet<PathBuf>,
) -> Result<(HashMap<usize, bool>, Vec<String>), Error> {
    debug!("changes: {:?}", changes);
    let mut terminal = if stdout_isatty() {
        term::stdout()
    } else {
        None
    };
    let getch = getch::Getch::new();
    let mut i = 0;
    let mut choices: HashMap<usize, bool> = HashMap::new();
    let mut new_ignored_patterns: Vec<String> = Vec::new();
    let mut new_ignore_builder = GitignoreBuilder::new(repo_root);
    let mut final_decision = None;
    let mut provided_by = HashMap::new();
    let mut line_deps = Vec::with_capacity(changes.len());
    for i in 0..changes.len() {
        line_deps.push(change_deps(i, &changes[i], &mut provided_by));
    }
    let mut deps: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut rev_deps: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..changes.len() {
        for dep in line_deps[i].iter() {
            debug!("provided: i {}, dep {:?}", i, dep);
            let p = provided_by.get(dep).unwrap();
            debug!("provided: p= {}", p);

            let e = deps.entry(i).or_insert(Vec::new());
            e.push(*p);

            let e = rev_deps.entry(*p).or_insert(Vec::new());
            e.push(i);
        }
    }

    let empty_deps = Vec::new();
    let mut current_file = None;
    let mut show_help = false;

    while i < changes.len() {
        let decision=
            // If one of our dependencies has been unselected (with "n")
            if deps.get(&i)
            .unwrap_or(&empty_deps)
            .iter()
            .any(|x| { ! *(choices.get(x).unwrap_or(&true)) }) {
                Some(false)
            } else if rev_deps.get(&i).unwrap_or(&empty_deps)
            .iter().any(|x| { *(choices.get(x).unwrap_or(&false)) }) {
                // If we are a dependency of someone selected (with "y").
                Some(true)
            } else {
                None
            };

        let decision = match changes[i] {
            Record::FileAdd { ref name, .. } => {
                let i = new_ignore_builder.build().unwrap();
                if i.matched_path_or_any_parents(&name, false).is_ignore() {
                    Some(false)
                } else {
                    None
                }
            }
            _ => decision,
        };
        let potential_new_ignore: Option<&str> = match direction {
            ChangesDirection::Revert => None,
            ChangesDirection::Record => match changes[i] {
                Record::FileAdd { ref name, .. } => Some(&name),
                _ => None,
            },
        };
        let (e, f) = match decision {
            Some(true) => ('Y', final_decision),
            Some(false) => ('N', final_decision),
            None => {
                if let Some(d) = final_decision {
                    (d, Some(d))
                } else {
                    prompt_one_change(
                        repository,
                        &changes[i],
                        &mut current_file,
                        changes.len(),
                        i,
                        direction,
                        potential_new_ignore,
                        &mut terminal,
                        &getch,
                        cwd,
                        show_help,
                    )?
                }
            }
        };

        show_help = false;

        final_decision = f;
        match e {
            'Y' => {
                choices.insert(i, direction.is_record());
                match changes[i] {
                    Record::FileAdd { ref name, .. } => {
                        let name: PathBuf = name.into();
                        to_unadd.remove(&name);
                    }
                    _ => (),
                }
                i += 1
            }
            'N' => {
                choices.insert(i, !direction.is_record());
                i += 1
            }
            'K' if i > 0 => {
                choices.remove(&i);
                i -= 1
            }
            'I' => match potential_new_ignore {
                Some(file) => {
                    add_to_ignore_file(
                        file,
                        repo_root,
                        &mut new_ignored_patterns,
                        &mut new_ignore_builder,
                    );
                    choices.insert(i, !direction.is_record());
                    i += 1;
                }
                _ => {}
            },
            '?' => {
                show_help = true;
            }
            _ => {}
        }
    }
    Ok((choices, new_ignored_patterns))
}

fn read_line(s: &str) -> String {
    print!("{}", s);
    if let Some(mut term) = line::Terminal::new() {
        term.read_line().unwrap()
    } else {
        let stdin = std::io::stdin();
        let mut stdin = stdin.lock().lines();
        if let Some(Ok(x)) = stdin.next() {
            x
        } else {
            String::new()
        }
    }
}

pub fn read_line_with_suggestion(prompt: &str, _suggestion: &str) -> String {
    read_line(prompt)
}

pub fn ask_authors() -> Result<Vec<String>, Error> {
    std::io::stdout().flush()?;
    Ok(vec![read_line("What is your name <and email address>? ")])
}

pub fn ask_patch_name(
    repo_root: &Path,
    maybe_editor: Option<&String>,
    template: String,
) -> Result<(String, Option<String>), Error> {
    if let Some(editor) = maybe_editor {
        let mut patch_name_file = repo_root.to_path_buf();
        patch_name_file.push(PIJUL_DIR_NAME);
        patch_name_file.push("PATCH_NAME");

        debug!("patch name file: {:?}", patch_name_file);

        // Initialize the PATCH_NAME file with the template given as argument of
        // this function. `File::create` truncate the file if it already exists.
        // FIXME: should we ask users if they want to use the previous content
        // instead?
        let _ = File::create(patch_name_file.as_path())?
            .write_all(template.into_bytes().as_slice())?;

        let mut editor_cmd = editor
            .trim()
            .split(" ")
            .map(OsString::from)
            .collect::<Vec<_>>();

        editor_cmd.push(patch_name_file.clone().into_os_string());

        process::Command::new(&editor_cmd[0])
            .args(&editor_cmd[1..])
            .current_dir(repo_root)
            .status()
            .map_err(|e| Error::CannotSpawnEditor { editor: editor.to_owned(), cause: e.to_string() })?;
        // if we are here, it means the editor must have stopped and we can read
        // the content of PATCH_NAME.

        // in case of error, we consider it is because the file has not been
        // created and we consider it as empty
        let mut patch_name =
            File::open(patch_name_file.as_path()).map_err(|_| Error::EmptyPatchName)?;
        let mut patch_name_content = String::new();
        patch_name.read_to_string(&mut patch_name_content)?;

        // we are done with PATCH_NAME, so delete it
        remove_file(patch_name_file)?;

        // Now, we parse the file. About `(?s:.)`, it is the syntax of the regex
        // crate for `.` to also match `\n`. So `.(?s:.)` means we want at least
        // one character that is not a newline, then the rest.
        let re_with_desc = Regex::new(r"^([^\n]+)\n\s*(.(?s:.)*)$").unwrap();
        let re_without_desc = Regex::new(r"^([^\n]+)\s*$").unwrap();

        if let Some(capt) = re_without_desc.captures(patch_name_content.as_ref()) {
            debug!("patch name without description");
            Ok((String::from(&capt[1]), None))
        } else if let Some(capt) = re_with_desc.captures(patch_name_content.as_ref()) {
            debug!("patch name with description");

            // In the description, we ignore the line starting with `#`, and we
            // remove trailing and leading space.  The `map()` call is necessary
            // because `lines()` elements does not contain the newline
            // character, therefore `collect` returns a String with a single
            // line.
            //
            // Note that, in the current implementation, it remains possible to
            // start the patch name with `#`.
            let descr: String = capt[2]
                .lines()
                .filter(|l| !l.starts_with("#"))
                .map(|x| format!("{}\n", x))
                .collect::<String>()
                .trim()
                .into();

            // If, once cleaned up, the description is empty, then we prefer
            // using `None` rather than `Some("")`.
            if descr.is_empty() {
                Ok((String::from(&capt[1]), None))
            } else {
                Ok((String::from(&capt[1]), Some(String::from(descr.trim()))))
            }
        } else {
            debug!("couldn't get a valid patch name");
            debug!("the content was:");
            debug!("=======================");
            debug!("{}", patch_name_content);
            debug!("=======================");
            Err(Error::EmptyPatchName)
        }
    } else {
        std::io::stdout().flush()?;

        let res = read_line("What is the name of this patch? ");
        debug!("res = {:?}", res);
        if res.trim().is_empty() {
            Err(Error::EmptyPatchName)
        } else {
            Ok((res, None))
        }
    }
}

pub fn ask_learn_ssh(host: &str, port: u16, fingerprint: &str) -> Result<bool, Error> {
    std::io::stdout().flush()?;
    print!(
        "The authenticity of host {:?}:{} cannot be established.\nThe fingerprint is {:?}.",
        host, port, fingerprint
    );

    let input = read_line("Are you sure you want to continue (yes/no)? ");
    let input = input.trim().to_uppercase();
    Ok(input == "YES")
}

pub fn print_status<T: rand::Rng>(
    repository: &MutTxn<T>,
    cwd: &Path,
    changes: &[Record<ChangeContext<Hash>>],
) -> Result<(), Error> {
    debug!("changes: {:?}", changes);
    let mut terminal = if stdout_isatty() {
        term::stdout()
    } else {
        None
    };
    let mut i = 0;
    let mut current_file = None;
    while i < changes.len() {
        debug!("changes: {:?}", changes[i]);
        print_change(
            &mut terminal,
            cwd,
            repository,
            &mut current_file,
            &changes[i],
        )?;
        println!("");
        i += 1
    }
    Ok(())
}
