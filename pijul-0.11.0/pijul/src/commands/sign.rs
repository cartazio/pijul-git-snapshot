use super::BasicOptions;
use clap::{Arg, ArgMatches, SubCommand};
use commands::{default_explain, StaticSubcommand};
use error::Error;
use libpijul::patch::{read_signature_file, read_signatures};
use std::fs::File;
use std::io::stdin;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("sign")
        .about("Add patch signatures")
        .arg(
            Arg::with_name("repository")
                .long("repository")
                .help(
                    "Path to the repository where the patches will be applied. Defaults to the \
                     repository containing the current directory.",
                )
                .takes_value(true),
        );
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    let opts = BasicOptions::from_args(args)?;

    let mut path = opts.patches_dir();
    for sig in read_signatures(&mut stdin()) {
        let sig = sig?;
        path.push(&sig.hash);
        path.set_extension("sig");
        let sig = if let Ok(mut f) = File::open(&path) {
            let mut previous = read_signature_file(&mut f)?;
            previous.signatures.extend(sig.signatures.into_iter());
            previous
        } else {
            sig
        };
        let mut f = File::create(&path)?;
        sig.write_signature_file(&mut f)?;
        path.pop();
    }
    Ok(())
}

pub fn explain(res: Result<(), Error>) {
    default_explain(res)
}
