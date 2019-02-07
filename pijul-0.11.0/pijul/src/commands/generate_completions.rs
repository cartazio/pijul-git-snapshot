use clap::{Arg, ArgGroup, ArgMatches, Shell, SubCommand};
use cli;
use commands::{default_explain, StaticSubcommand};
use error::Error;
use std::io;

pub fn invocation() -> StaticSubcommand {
    return SubCommand::with_name("generate-completions")
        .about("Generate shell completions for pijul to stdout")
        .group(
            ArgGroup::with_name("shells")
                .args(&["bash", "fish", "zsh", "powershell"])
                .required(true),
        )
        .arg(
            Arg::with_name("bash")
                .long("bash")
                .help("Completions for Bash"),
        )
        .arg(
            Arg::with_name("zsh")
                .long("zsh")
                .help("Completions for Zsh"),
        )
        .arg(
            Arg::with_name("fish")
                .long("fish")
                .help("Completions for Fish"),
        )
        .arg(
            Arg::with_name("powershell")
                .long("powershell")
                .help("Completions for Powershell"),
        );
}

pub fn run(args: &ArgMatches) -> Result<(), Error> {
    if args.is_present("bash") {
        cli::build_cli().gen_completions_to("pijul", Shell::Bash, &mut io::stdout());
        Ok(())
    } else if args.is_present("zsh") {
        cli::build_cli().gen_completions_to("pijul", Shell::Zsh, &mut io::stdout());
        Ok(())
    } else if args.is_present("fish") {
        cli::build_cli().gen_completions_to("pijul", Shell::Fish, &mut io::stdout());
        Ok(())
    } else if args.is_present("powershell") {
        cli::build_cli().gen_completions_to("pijul", Shell::PowerShell, &mut io::stdout());
        Ok(())
    } else {
        Ok(()) // should never happen anyway thanks to clap's groups
    }
}

pub fn explain(res: Result<(), Error>) {
    default_explain(res)
}
