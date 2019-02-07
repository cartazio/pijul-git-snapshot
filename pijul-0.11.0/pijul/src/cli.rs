use clap::App;
use commands;

pub fn build_cli() -> App<'static, 'static> {
    let version = crate_version!();
    let app = App::new("pijul")
        .version(&version[..])
        .author("Pierre-Étienne Meunier and Florent Becker")
        .about("Version Control: fast, distributed, easy to use; pick any three");
    app.subcommands(commands::all_command_invocations())
}
