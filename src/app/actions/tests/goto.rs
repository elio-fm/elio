use super::helpers::temp_path;
use crate::{
    app::App,
    background_jobs::job_results::GotoCommandBuild,
    goto_menu::{GotoDestination, GotoMenu, GotoMenuEntry},
};
use std::fs;
#[cfg(unix)]
use std::time::Duration;

#[test]
fn failed_goto_command_keeps_the_menu_open_with_a_concise_status() {
    let root = temp_path("goto-command-failure");
    fs::create_dir_all(&root).expect("temporary root should be created");
    let mut app = App::new_at(root.clone()).expect("app should be created");
    app.overlays.goto = Some(GotoMenu::new(vec![GotoMenuEntry::new(
        'r',
        "Project root",
        GotoDestination::Top,
    )]));
    app.pending_goto_command = Some(4);

    assert!(app.apply_goto_command_job_result(GotoCommandBuild {
        token: 4,
        title: "Project root".to_string(),
        result: Err("command failed"),
    }));

    assert!(app.goto_is_open());
    assert_eq!(app.status, "Project root: command failed");

    fs::remove_dir_all(root).expect("temporary root should be removed");
}

#[test]
fn stale_goto_command_results_are_ignored() {
    let root = temp_path("goto-command-stale-result");
    fs::create_dir_all(&root).expect("temporary root should be created");
    let mut app = App::new_at(root.clone()).expect("app should be created");
    app.pending_goto_command = Some(5);

    assert!(!app.apply_goto_command_job_result(GotoCommandBuild {
        token: 4,
        title: "Project root".to_string(),
        result: Err("command failed"),
    }));
    assert!(app.status.is_empty());

    fs::remove_dir_all(root).expect("temporary root should be removed");
}

#[cfg(unix)]
#[test]
fn command_goto_entries_resolve_in_the_background() {
    let root = temp_path("goto-command-background");
    fs::create_dir_all(&root).expect("temporary root should be created");
    let mut app = App::new_at(root.clone()).expect("app should be created");
    app.overlays.goto = Some(GotoMenu::new(vec![GotoMenuEntry::new(
        'r',
        "Repo root",
        GotoDestination::Command {
            title: "Repo root".to_string(),
            command: "pwd".to_string(),
        },
    )]));

    app.confirm_goto_index(0)
        .expect("command entry should be submitted");
    for _ in 0..100 {
        app.process_background_jobs();
        if !app.goto_is_open() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(!app.goto_is_open(), "command result should close Go To");

    fs::remove_dir_all(root).expect("temporary root should be removed");
}
