use std::{
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_COMMAND_OUTPUT_BYTES: u64 = 8 * 1024;
const COMMAND_FAILED: &str = "command failed";
const COMMAND_COULD_NOT_START: &str = "command could not start";
const COMMAND_TIMED_OUT: &str = "command timed out";
const INVALID_DIRECTORY: &str = "command did not return a valid directory";

pub(crate) fn resolve_command_destination(
    command: &str,
    cwd: &Path,
) -> Result<PathBuf, &'static str> {
    let tokens = crate::opening::tokenize_command(command);
    let Some((program, args)) = tokens.split_first() else {
        return Err(COMMAND_COULD_NOT_START);
    };

    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| command_start_error(&error))?;
    let stdout = child
        .stdout
        .take()
        .expect("stdout is piped when starting a Go To command");
    let reader = thread::spawn(move || {
        let mut output = Vec::new();
        stdout
            .take(MAX_COMMAND_OUTPUT_BYTES + 1)
            .read_to_end(&mut output)
            .map(|_| output)
    });

    let started_at = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|_| COMMAND_FAILED)? {
            break status;
        }
        if started_at.elapsed() >= COMMAND_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(COMMAND_TIMED_OUT);
        }
        thread::sleep(Duration::from_millis(10));
    };
    let output = reader
        .join()
        .map_err(|_| INVALID_DIRECTORY)?
        .map_err(|_| INVALID_DIRECTORY)?;

    if !status.success() {
        return Err(COMMAND_FAILED);
    }
    if output.len() as u64 > MAX_COMMAND_OUTPUT_BYTES {
        return Err(INVALID_DIRECTORY);
    }
    parse_command_output(&output)
}

pub(in crate::goto_menu) fn command_start_error(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::NotFound => "command not found",
        io::ErrorKind::PermissionDenied => "command permission denied",
        _ => COMMAND_COULD_NOT_START,
    }
}

pub(in crate::goto_menu) fn parse_command_output(output: &[u8]) -> Result<PathBuf, &'static str> {
    let output = std::str::from_utf8(output).map_err(|_| INVALID_DIRECTORY)?;
    let output = output.strip_suffix('\n').unwrap_or(output);
    let output = output.strip_suffix('\r').unwrap_or(output);
    if output.is_empty() {
        return Err(INVALID_DIRECTORY);
    }
    if output.contains(['\n', '\r', '\0']) {
        return Err(INVALID_DIRECTORY);
    }

    let path = PathBuf::from(output);
    if !path.is_absolute() {
        return Err(INVALID_DIRECTORY);
    }
    if !path.is_dir() {
        return Err(INVALID_DIRECTORY);
    }
    Ok(path)
}
