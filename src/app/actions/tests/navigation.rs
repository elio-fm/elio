use super::*;
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
    std::env::temp_dir().join(format!("elio-navigation-{label}-{unique}"))
}

#[cfg(windows)]
fn successful_detached_command() -> (String, Vec<String>) {
    (
        "cmd.exe".to_string(),
        vec!["/C".to_string(), "exit 0".to_string()],
    )
}

#[cfg(not(windows))]
fn successful_detached_command() -> (String, Vec<String>) {
    (
        "/bin/sh".to_string(),
        vec!["-c".to_string(), "true".to_string()],
    )
}

#[test]
fn detached_open_rule_reports_command_launch_failure() {
    let root = temp_path("detached-open-rule-failure");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("failed to create app");

    app.run_open_plans(vec![crate::opening::OpenPlan::Detached {
        program: "definitely-not-real-elio-command".to_string(),
        args: Vec::new(),
    }])
    .expect("open plan should be handled");

    assert!(
        app.status_message()
            .contains("definitely-not-real-elio-command"),
        "status should report detached command failure, got: {}",
        app.status_message()
    );

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn detached_open_rule_uses_opening_status_after_successful_launch() {
    let root = temp_path("detached-open-rule-opening");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let mut app = App::new_at(root.clone()).expect("failed to create app");

    let (program, args) = successful_detached_command();
    app.run_open_plans(vec![crate::opening::OpenPlan::Detached { program, args }])
        .expect("open plan should be handled");

    assert_eq!(app.status_message(), "Opening item");

    fs::remove_dir_all(root).expect("temp directory should be removed");
}

#[test]
fn cold_rar_preview_navigation_defers_preview_job_until_idle() {
    let root = temp_path("rar-deferred-preview");
    fs::create_dir_all(&root).expect("failed to create temp root");
    fs::write(root.join("a.txt"), "ready").expect("failed to write text fixture");
    let rar = root.join("b.rar");
    fs::write(&rar, b"not-a-real-rar").expect("failed to write rar fixture");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    let before = app.scheduler_metrics();
    let rar_index = app
        .file_browser
        .entries
        .iter()
        .position(|entry| entry.path == rar)
        .expect("rar fixture should be visible");

    app.set_selected(rar_index);
    let after = app.scheduler_metrics();

    assert!(app.preview.state.deferred_refresh_at.is_some());
    assert_eq!(
        after.preview_jobs_submitted_high, before.preview_jobs_submitted_high,
        "cold RAR selection should wait for the deferred refresh instead of immediately queuing heavy preview work"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
