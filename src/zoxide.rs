use std::{
    env, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

pub(crate) enum QueryResult {
    Selected(PathBuf),
    Cancelled,
    NotFound,
    PickerNotFound,
    Empty,
    OnlyCurrentDirectory,
    LaunchFailed,
}

pub(crate) fn preflight(cwd: &Path) -> Option<QueryResult> {
    match has_match_excluding(cwd) {
        Ok(true) => None,
        Ok(false) => match has_any_match() {
            Ok(true) => Some(QueryResult::OnlyCurrentDirectory),
            Ok(false) => Some(QueryResult::Empty),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Some(QueryResult::NotFound),
            Err(_) => None,
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => Some(QueryResult::NotFound),
        Err(_) => None,
    }
}

pub(crate) fn run_query_in_terminal(cwd: &Path) -> QueryResult {
    let output = match Command::new("zoxide")
        .args(["query", "-i", "--exclude"])
        .arg(cwd)
        .env("SHELL", "sh")
        .env("CLICOLOR", "1")
        .env("CLICOLOR_FORCE", "1")
        .env("_ZO_FZF_OPTS", fzf_options())
        .stdin(Stdio::inherit())
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return QueryResult::NotFound,
        Err(_) => return QueryResult::LaunchFailed,
    };

    if !output.status.success() {
        if stderr_mentions_missing_fzf(&output.stderr) {
            return QueryResult::PickerNotFound;
        }
        return QueryResult::Cancelled;
    }

    match path_from_command_stdout(output.stdout) {
        Some(path) => QueryResult::Selected(path),
        None => QueryResult::Cancelled,
    }
}

fn has_match_excluding(cwd: &Path) -> io::Result<bool> {
    let output = Command::new("zoxide")
        .args(["query", "-l", "--exclude"])
        .arg(cwd)
        .output()?;
    Ok(!output.stdout.is_empty())
}

fn has_any_match() -> io::Result<bool> {
    let output = Command::new("zoxide").args(["query", "-l"]).output()?;
    Ok(!output.stdout.is_empty())
}

fn stderr_mentions_missing_fzf(stderr: &[u8]) -> bool {
    // zoxide reports a missing interactive picker through stderr; keep this
    // narrow so ordinary cancellations remain silent.
    let stderr = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    stderr.contains("fzf")
        && (stderr.contains("not found")
            || stderr.contains("not installed")
            || stderr.contains("no such file")
            || stderr.contains("could not find"))
}

fn fzf_options() -> String {
    let mut defaults = vec![
        "--exact",
        "--no-sort",
        "--bind=ctrl-z:ignore,btab:up,tab:down",
        "--cycle",
        "--keep-right",
        "--layout=reverse",
        "--height=100%",
        "--border",
        "--info=inline",
        "--tabstop=1",
        "--exit-0",
    ];

    #[cfg(target_os = "linux")]
    {
        defaults.push("--preview-window=down,30%,sharp");
        defaults
            .push("--preview='\\command -p ls -Cp --color=always --group-directories-first {2..}'");
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        defaults.push("--preview-window=down,30%,sharp");
        defaults.push("--preview='\\command -p ls -Cp {2..}'");
    }

    let defaults = defaults.join(" ");

    match (env::var("FZF_DEFAULT_OPTS"), env::var("ELIO_ZOXIDE_OPTS")) {
        (Ok(base), Ok(extra)) => format!("{base} {defaults} {extra}"),
        (Ok(base), Err(_)) => format!("{base} {defaults}"),
        (Err(_), Ok(extra)) => format!("{defaults} {extra}"),
        (Err(_), Err(_)) => defaults,
    }
}

fn path_from_command_stdout(mut stdout: Vec<u8>) -> Option<PathBuf> {
    while matches!(stdout.last(), Some(b'\n' | b'\r')) {
        stdout.pop();
    }
    if stdout.is_empty() {
        return None;
    }

    #[cfg(unix)]
    {
        Some(PathBuf::from(OsString::from_vec(stdout)))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(stdout).ok().map(PathBuf::from)
    }
}
