use crate::shell_integration::{self, Shell};
use anyhow::Result;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

pub(crate) fn run() -> Result<()> {
    match parse_args(env::args().skip(1))? {
        Command::Run(options) => elio::run_with_options(options),
        Command::PrintVersion => {
            print_version();
            Ok(())
        }
        Command::PrintHelp => {
            print_help();
            Ok(())
        }
        Command::PrintShellInit(shell) => {
            let executable = env::current_exe()?;
            let invocation = env::args().next();
            let binary = shell_integration::binary_command(invocation.as_deref(), &executable);
            print!("{}", shell_integration::init_script(shell, &binary));
            Ok(())
        }
        Command::InstallShellIntegration(shell) => {
            let executable = env::current_exe()?;
            let invocation = env::args().next();
            let binary = shell_integration::binary_command(invocation.as_deref(), &executable);
            let shell = match shell {
                Some(shell) => shell,
                None => shell_integration::detect_shell()?,
            };
            let report = shell_integration::install(shell, &binary)?;
            println!(
                "Installed elio shell integration for {}.",
                report.shell.name()
            );
            println!();
            println!("Wrote: {}", report.path.display());
            println!();
            println!("Restart your shell, or run:");
            println!("  {}", report.reload_command);
            println!();
            println!("From now on, `elio` will change your shell directory on quit.");
            Ok(())
        }
        Command::UninstallShellIntegration(shell) => {
            let shell = match shell {
                Some(shell) => shell,
                None => shell_integration::detect_shell()?,
            };
            let report = shell_integration::uninstall(shell)?;
            println!(
                "Uninstalled elio shell integration for {}.",
                report.shell.name()
            );
            println!();
            if report.changed {
                if report.removed_file {
                    println!("Removed: {}", report.path.display());
                } else {
                    println!("Updated: {}", report.path.display());
                }
            } else {
                println!("No integration found at: {}", report.path.display());
            }
            println!();
            println!("Restart your shell, or run:");
            println!("  {}", report.reload_command);
            println!();
            println!("From now on, `elio` will leave your shell directory unchanged.");
            Ok(())
        }
    }
}

#[derive(Debug)]
enum Command {
    Run(elio::RunOptions),
    PrintVersion,
    PrintHelp,
    PrintShellInit(Shell),
    InstallShellIntegration(Option<Shell>),
    UninstallShellIntegration(Option<Shell>),
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command> {
    let args = args.into_iter().collect::<Vec<_>>();

    if args.is_empty() {
        return Ok(Command::Run(elio::RunOptions::default()));
    }

    match args.as_slice() {
        [arg] if arg == "--version" || arg == "-V" => return Ok(Command::PrintVersion),
        [arg] if arg == "--help" || arg == "-h" => return Ok(Command::PrintHelp),
        [arg, unexpected, ..] if arg == "--version" || arg == "-V" => {
            return Err(anyhow::anyhow!(unknown_argument_message(unexpected)));
        }
        [arg, unexpected, ..] if arg == "--help" || arg == "-h" => {
            return Err(anyhow::anyhow!(unknown_argument_message(unexpected)));
        }
        [command, subcommand, shell] if command == "shell" && subcommand == "init" => {
            return Shell::parse(shell)
                .map(Command::PrintShellInit)
                .map_err(anyhow::Error::msg);
        }
        [command, subcommand] if command == "shell" && subcommand == "install" => {
            return Ok(Command::InstallShellIntegration(None));
        }
        [command, subcommand, shell] if command == "shell" && subcommand == "install" => {
            return Shell::parse(shell)
                .map(|shell| Command::InstallShellIntegration(Some(shell)))
                .map_err(anyhow::Error::msg);
        }
        [command, subcommand] if command == "shell" && subcommand == "uninstall" => {
            return Ok(Command::UninstallShellIntegration(None));
        }
        [command, subcommand, shell] if command == "shell" && subcommand == "uninstall" => {
            return Shell::parse(shell)
                .map(|shell| Command::UninstallShellIntegration(Some(shell)))
                .map_err(anyhow::Error::msg);
        }
        [command, subcommand, _shell, unexpected, ..]
            if command == "shell" && subcommand == "install" =>
        {
            return Err(anyhow::anyhow!(
                unknown_argument_message(unexpected).replace(
                    "Usage: elio [OPTIONS] [DIRECTORY]",
                    "Usage: elio shell install [SHELL]",
                )
            ));
        }
        [command, subcommand, _shell, unexpected, ..]
            if command == "shell" && subcommand == "uninstall" =>
        {
            return Err(anyhow::anyhow!(
                unknown_argument_message(unexpected).replace(
                    "Usage: elio [OPTIONS] [DIRECTORY]",
                    "Usage: elio shell uninstall [SHELL]",
                )
            ));
        }
        [command, subcommand, _shell, unexpected, ..]
            if command == "shell" && subcommand == "init" =>
        {
            return Err(anyhow::anyhow!(
                unknown_argument_message(unexpected).replace(
                    "Usage: elio [OPTIONS] [DIRECTORY]",
                    "Usage: elio shell init <SHELL>",
                )
            ));
        }
        [command, subcommand] if command == "shell" && subcommand == "init" => {
            return Err(anyhow::anyhow!(
                "error: expected a shell after 'elio shell init'\n\nsupported shells: bash, zsh, fish"
            ));
        }
        [command, ..] if command == "shell" => {
            return Err(anyhow::anyhow!(
                "error: expected subcommand 'init', 'install', or 'uninstall' after 'elio shell'\n\nUsage: elio shell init <SHELL>\n       elio shell install [SHELL]\n       elio shell uninstall [SHELL]"
            ));
        }
        _ => {}
    }

    parse_run_args(args)
}

fn parse_run_args(args: Vec<String>) -> Result<Command> {
    let mut start_dir = None;
    let mut cwd_file = None;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        if let Some(file) = arg.strip_prefix("--cwd-file=") {
            if cwd_file.is_some() {
                return Err(anyhow::anyhow!(
                    "error: '--cwd-file' cannot be used more than once\n\nUsage: elio [OPTIONS] [DIRECTORY]"
                ));
            }
            if file.is_empty() {
                return Err(anyhow::anyhow!(
                    "error: expected a file path after '--cwd-file'\n\nUsage: elio [OPTIONS] [DIRECTORY]"
                ));
            }
            cwd_file = Some(PathBuf::from(file));
            index += 1;
            continue;
        }

        if arg == "--cwd-file" {
            if cwd_file.is_some() {
                return Err(anyhow::anyhow!(
                    "error: '--cwd-file' cannot be used more than once\n\nUsage: elio [OPTIONS] [DIRECTORY]"
                ));
            }
            let Some(file) = args.get(index + 1) else {
                return Err(anyhow::anyhow!(
                    "error: expected a file path after '--cwd-file'\n\nUsage: elio [OPTIONS] [DIRECTORY]"
                ));
            };
            cwd_file = Some(PathBuf::from(file));
            index += 2;
            continue;
        }

        if arg.starts_with('-') {
            return Err(anyhow::anyhow!(unknown_argument_message(arg)));
        }

        if start_dir.is_some() {
            return Err(anyhow::anyhow!(unknown_argument_message(arg)));
        }
        start_dir = Some(resolve_startup_directory(arg)?);
        index += 1;
    }

    Ok(Command::Run(elio::RunOptions {
        start_dir,
        cwd_file,
    }))
}

fn print_version() {
    println!("elio {}", env!("CARGO_PKG_VERSION"));
}

fn print_help() {
    println!("elio {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Usage: elio [OPTIONS] [DIRECTORY]");
    println!("       elio shell init <SHELL>");
    println!("       elio shell install [SHELL]");
    println!("       elio shell uninstall [SHELL]");
    println!();
    println!("Arguments:");
    println!("  [DIRECTORY]          Start elio in this directory");
    println!();
    println!("Options:");
    println!("      --cwd-file FILE  Write the final current directory to FILE on exit");
    println!("  -h, --help           Print help");
    println!("  -V, --version        Print version");
    println!();
    println!("Commands:");
    println!("  shell init <SHELL>        Print shell integration for bash, zsh, or fish");
    println!("  shell install [SHELL]    Install shell integration for bash, zsh, or fish");
    println!("  shell uninstall [SHELL]  Remove shell integration for bash, zsh, or fish");
}

fn resolve_startup_directory(arg: &str) -> Result<PathBuf> {
    let path = PathBuf::from(arg);
    let metadata = fs::metadata(&path).map_err(|error| startup_path_error(&path, &error))?;
    if !metadata.is_dir() {
        return Err(anyhow::anyhow!(
            "Cannot open \"{}\": not a directory",
            path.display()
        ));
    }
    Ok(path.canonicalize().unwrap_or(path))
}

fn startup_path_error(path: &Path, error: &io::Error) -> anyhow::Error {
    let detail = match error.kind() {
        io::ErrorKind::NotFound => "no such file or directory".to_string(),
        io::ErrorKind::PermissionDenied => "permission denied".to_string(),
        _ => error.to_string(),
    };
    anyhow::anyhow!("Cannot open \"{}\": {detail}", path.display())
}

fn unknown_argument_message(arg: &str) -> String {
    let mut message = format!("error: unexpected argument '{arg}' found");

    if arg != "--version" && arg != "-V" && ("--version".starts_with(arg) || "-V".starts_with(arg))
    {
        message.push_str("\n\n  tip: a similar argument exists: '--version'");
    } else if arg != "--help" && arg != "-h" && ("--help".starts_with(arg) || "-h".starts_with(arg))
    {
        message.push_str("\n\n  tip: a similar argument exists: '--help'");
    } else if arg != "--cwd-file" && "--cwd-file".starts_with(arg) {
        message.push_str("\n\n  tip: a similar argument exists: '--cwd-file'");
    }

    message.push_str("\n\nUsage: elio [OPTIONS] [DIRECTORY]");
    message.push_str("\n\nFor more information, try '--help'.");
    message
}

#[cfg(test)]
mod tests {
    use super::resolve_startup_directory;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-cli-{label}-{unique}"))
    }

    #[test]
    fn resolve_startup_directory_accepts_existing_directory() {
        let root = temp_path("directory");
        fs::create_dir_all(&root).expect("temp directory should be created");

        let resolved = resolve_startup_directory(root.to_str().expect("temp path should be utf-8"))
            .expect("existing directory should resolve");

        assert_eq!(
            resolved,
            root.canonicalize()
                .expect("temp directory should canonicalize successfully")
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn resolve_startup_directory_rejects_missing_path() {
        let missing = temp_path("missing");

        let error =
            resolve_startup_directory(missing.to_str().expect("temp path should be valid utf-8"))
                .expect_err("missing path should return an error");

        assert_eq!(
            error.to_string(),
            format!(
                "Cannot open \"{}\": no such file or directory",
                missing.display()
            )
        );
    }

    #[test]
    fn resolve_startup_directory_rejects_files() {
        let root = temp_path("file");
        fs::create_dir_all(&root).expect("temp directory should be created");
        let file = root.join("notes.txt");
        fs::write(&file, "hello").expect("temp file should be created");

        let error =
            resolve_startup_directory(file.to_str().expect("temp path should be valid utf-8"))
                .expect_err("file path should return an error");

        assert_eq!(
            error.to_string(),
            format!("Cannot open \"{}\": not a directory", file.display())
        );

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }
}
