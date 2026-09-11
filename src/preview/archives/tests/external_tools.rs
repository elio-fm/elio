use super::{parse_7z_listing, parse_unrar_bare_listing, run_archive_listing_command};
#[cfg(unix)]
use std::time::{Duration, Instant};
use std::{fs, path::PathBuf};

#[test]
fn parse_7z_listing_collects_external_fallback_metadata_and_entries() {
    let output = r#"
Path = app.AppImage
Type = SquashFS
Physical Size = 12345
Comment = portable build

----------
Path = AppRun
Folder = -
Size = 12
Packed Size = 10

Path = usr/bin/elio
Folder = -
Size = 52
Packed Size = 20

Path = usr/share/icons
Folder = +
Size = 0
Packed Size = 0
"#;

    let (metadata, entries) =
        parse_7z_listing(output).expect("7z listing should parse archive metadata");

    assert_eq!(metadata.format_label.as_deref(), Some("SquashFS"));
    assert_eq!(metadata.physical_size, Some(12_345));
    assert_eq!(metadata.comment.as_deref(), Some("portable build"));
    assert_eq!(metadata.unpacked_size, Some(64));
    assert_eq!(metadata.compressed_size, Some(30));
    assert_eq!(entries.len(), 3);
    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "AppRun" && !entry.is_dir)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "usr/bin/elio" && !entry.is_dir)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "usr/share/icons" && entry.is_dir)
    );
}

#[test]
fn parse_unrar_bare_listing_normalizes_nested_entries() {
    let output = r#"
./docs/readme.txt
src\main.rs
../ignored.txt
images/
"#;

    let entries = parse_unrar_bare_listing(output);

    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "docs/readme.txt" && !entry.is_dir)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "src/main.rs" && !entry.is_dir)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.path == "images" && entry.is_dir)
    );
    assert!(!entries.iter().any(|entry| entry.path.contains("ignored")));
}

#[cfg(unix)]
#[test]
fn external_archive_command_observes_cancellation_while_running() {
    let started_at = Instant::now();
    let output = run_archive_listing_command(
        "sh",
        &["-c", "sleep 5", "sh"],
        std::path::Path::new("ignored.zip"),
        &|| started_at.elapsed() >= Duration::from_millis(40),
    );

    assert!(output.is_none());
    assert!(
        started_at.elapsed() < Duration::from_secs(1),
        "canceled archive command should not wait for the child process to finish"
    );
}

#[cfg(unix)]
#[test]
fn external_archive_command_does_not_inherit_stdin() {
    let started_at = Instant::now();
    let output = run_archive_listing_command(
        "sh",
        &["-c", "read ignored || exit 7", "sh"],
        std::path::Path::new("ignored.rar"),
        &|| false,
    );

    assert!(output.is_none());
    assert!(
        started_at.elapsed() < Duration::from_secs(1),
        "archive commands must not block the UI waiting for interactive password input"
    );
}

#[test]
fn external_archive_command_cleans_temp_file_when_spawn_fails() {
    let program = "elio-definitely-missing-archive-tool-for-cleanup-test";
    remove_archive_temp_outputs(program);

    let output =
        run_archive_listing_command(program, &[], std::path::Path::new("ignored.zip"), &|| false);

    assert!(output.is_none());
    assert!(
        archive_temp_outputs(program).is_empty(),
        "failed spawn should clean up its temp output file"
    );
}

fn archive_temp_outputs(program: &str) -> Vec<PathBuf> {
    let prefix = format!("elio-archive-{program}-{}-", std::process::id());
    fs::read_dir(std::env::temp_dir())
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".out"))
        })
        .collect()
}

fn remove_archive_temp_outputs(program: &str) {
    for path in archive_temp_outputs(program) {
        let _ = fs::remove_file(path);
    }
}
