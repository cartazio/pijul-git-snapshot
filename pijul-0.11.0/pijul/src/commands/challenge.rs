use clap::{AppSettings, ArgMatches, SubCommand};
use commands::{default_explain, StaticSubcommand};
use error::Error;
use rand;
use rand::Rng;
use rand::distributions::Alphanumeric;
use std::io::Read;
use std::io::stdin;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("challenge")
        .setting(AppSettings::Hidden)
        .about("Prove ownership of a signature key");
}

pub fn run(_: &ArgMatches) -> Result<(), Error> {
    let challenge: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(80)
        .collect::<String>();
    println!("{}", challenge);
    let mut v = Vec::new();
    stdin().read_to_end(&mut v)?;
    Ok(())
}

pub fn explain(res: Result<(), Error>) {
    default_explain(res)
}
