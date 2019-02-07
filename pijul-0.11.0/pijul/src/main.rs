#![recursion_limit = "256"]
#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;
extern crate chrono;
extern crate cryptovec;
extern crate dirs;
extern crate env_logger;
extern crate futures;
extern crate getch;
extern crate ignore;
extern crate libpijul;
extern crate rand;
extern crate regex;
extern crate reqwest;
extern crate shell_escape;
extern crate term;
extern crate thrussh;
extern crate thrussh_config;
extern crate thrussh_keys;
extern crate tokio;
extern crate toml;
extern crate username;

#[cfg(unix)]
extern crate tokio_uds;

extern crate flate2;
extern crate tar;
#[macro_use]
extern crate serde_derive;
extern crate hex;
extern crate bincode;
extern crate progrs;
extern crate tempdir;

extern crate isatty;
#[cfg(unix)]
extern crate pager;

extern crate base64;
extern crate line;
extern crate rpassword;
extern crate serde_json;

mod cli;
mod commands;
mod error;
mod meta;
mod relativize;

macro_rules! pijul_subcommand_dispatch {
    ($default:expr, $p:expr => $($subcommand_name:expr => $subcommand:ident),*) => {{
        match $p {
            $(($subcommand_name, Some(args)) =>
             {
                 let res = commands::$subcommand::run(&args);
                 commands::$subcommand::explain(res)
             }
              ),*
                ("", None) => { $default; println!(""); },
            _ => panic!("Incorrect subcommand name")
        }
    }}
}

fn main() {
    env_logger::init();
    let time0 = chrono::Local::now();
    let app = cli::build_cli();
    let mut app_help = app.clone();

    let args = app.get_matches();
    pijul_subcommand_dispatch!(app_help.print_help().unwrap(), args.subcommand() =>
                               "info" => info,
                               "generate-completions" => generate_completions,
                               "log" => log,
                               "patch" => patch,
                               "init" => init,
                               "add" => add,
                               "record" => record,
                               "pull" => pull,
                               "push" => push,
                               "apply" => apply,
                               "clone" => clone,
                               "remove" => remove,
                               "mv" => mv,
                               "ls" => ls,
                               "revert" => revert,
                               "unrecord" => unrecord,
                               "fork" => fork,
                               "branches" => branches,
                               "delete-branch" => delete_branch,
                               "checkout" => checkout,
                               "diff" => diff,
                               "credit" => credit,
                               "dist" => dist,
                               "key" => key,
                               "rollback" => rollback,
                               "status" => status,
                               "show-dependencies" => show_dependencies,
                               "tag" => tag,
                               "sign" => sign,
                               "challenge" => challenge
                               );
    let time1 = chrono::Local::now();
    info!("The command took: {:?}", time1.signed_duration_since(time0));
}
