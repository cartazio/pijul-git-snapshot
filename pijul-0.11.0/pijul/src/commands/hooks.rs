use std::path::Path;
use std::process::Command;
use error::Error;
use libpijul::fs_representation::PIJUL_DIR_NAME;

pub fn run_hook(
    repo_root: &Path,
    hook: &'static str,
    additional_arg: Option<&String>,
) -> Result<(), Error> {
    let mut cmd = repo_root.to_path_buf();
    cmd.push(PIJUL_DIR_NAME);
    cmd.push("hooks");
    cmd.push(hook);

    if cmd.is_file() {
        println!("Running hook: {}", hook);

        let arg = match additional_arg {
            Some(ref arg) => vec![*arg],
            None => vec![],
        };

        let output = Command::new(cmd.as_path())
            .args(arg)
            .current_dir(repo_root)
            .output()?;

        if !output.status.success() {
            if let Ok(err) = String::from_utf8(output.stderr) {
                print!("{}", err);
            }
            return Err(Error::HookFailed { cmd: String::from(hook) });
        }
    }

    Ok(())
}
