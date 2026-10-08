use super::super::{
    command_destination::{command_start_error, parse_command_output},
    resolve_command_destination,
};
use std::io;

const INVALID_DIRECTORY: &str = "command did not return a valid directory";

#[test]
fn parses_a_single_absolute_directory_path() {
    let path = std::env::temp_dir();
    let output = format!("{}\n", path.display());

    assert_eq!(parse_command_output(output.as_bytes()), Ok(path));
}

#[test]
fn rejects_invalid_command_output() {
    let missing = std::env::temp_dir().join("elio-goto-command-missing-directory");
    let missing_output = format!("{}\n", missing.display());

    assert_eq!(parse_command_output(b""), Err(INVALID_DIRECTORY));
    assert_eq!(parse_command_output(b"relative\n"), Err(INVALID_DIRECTORY));
    assert_eq!(
        parse_command_output(b"/tmp\n/var\n"),
        Err(INVALID_DIRECTORY)
    );
    assert_eq!(
        parse_command_output(missing_output.as_bytes()),
        Err(INVALID_DIRECTORY)
    );
    assert_eq!(parse_command_output(&[0xff]), Err(INVALID_DIRECTORY));
}

#[test]
fn describes_common_command_start_failures() {
    assert_eq!(
        command_start_error(&io::Error::from(io::ErrorKind::NotFound)),
        "command not found"
    );
    assert_eq!(
        command_start_error(&io::Error::from(io::ErrorKind::PermissionDenied)),
        "command permission denied"
    );
}

#[cfg(any(unix, windows))]
#[test]
fn command_uses_the_displayed_directory_as_its_working_directory() {
    let cwd = std::env::temp_dir();
    #[cfg(unix)]
    let command = "pwd";
    #[cfg(windows)]
    let command = "cmd /C cd";

    let destination = resolve_command_destination(command, &cwd)
        .expect("command should resolve the displayed directory");
    assert_eq!(
        destination
            .canonicalize()
            .expect("destination should canonicalize"),
        cwd.canonicalize()
            .expect("working directory should canonicalize")
    );
}
