use anyhow::Result;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::ExitCode,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();

    match args.as_slice() {
        [] => elio::run(),
        [arg] if arg == "--version" || arg == "-V" => {
            print_version();
            Ok(())
        }
        [arg] if arg == "--help" || arg == "-h" => {
            print_help();
            Ok(())
        }
        [arg, unexpected, ..] if arg == "--version" || arg == "-V" => {
            Err(anyhow::anyhow!(unknown_argument_message(unexpected)))
        }
        [arg, unexpected, ..] if arg == "--help" || arg == "-h" => {
            Err(anyhow::anyhow!(unknown_argument_message(unexpected)))
        }
        [arg] if arg.starts_with('-') => Err(anyhow::anyhow!(unknown_argument_message(arg))),
        [arg] => elio::run_at(resolve_startup_directory(arg)?),
        [arg, ..] => Err(anyhow::anyhow!(unknown_argument_message(arg))),
    }
}

fn print_version() {
    println!("elio {}", env!("CARGO_PKG_VERSION"));
}

fn print_help() {
    println!("elio {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Usage: elio [OPTIONS] [DIRECTORY]");
    println!();
    println!("Arguments:");
    println!("  [DIRECTORY]  Start elio in this directory");
    println!();
    println!("Options:");
    println!("  -h, --help     Print help");
    println!("  -V, --version  Print version");
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
        std::env::temp_dir().join(format!("elio-main-{label}-{unique}"))
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
