use super::super::super::App;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temp_root() -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let sequence = TEMP_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "elio-tabs-{}-{unique}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("temporary root should be created");
    root.canonicalize().unwrap_or(root)
}

#[test]
fn new_tab_keeps_the_current_directory_and_closing_returns_to_the_previous_tab() {
    let root = temp_root();
    let mut app = App::new_at(root.clone()).expect("app should load temporary root");
    app.file_browser.show_hidden = true;
    app.new_tab().expect("new tab should open");

    assert_eq!(app.tabs.len(), 2);
    assert_eq!(app.tabs.active_index(), 1);
    assert_eq!(app.file_browser.cwd, root);
    assert!(app.file_browser.show_hidden);
    assert!(app.status.is_empty(), "opening a tab should need no status");

    app.activate_tab(0).expect("first tab should activate");
    assert!(
        app.status.is_empty(),
        "tab activation should need no status"
    );
    app.cycle_tabs(1).expect("tabs should cycle");
    assert!(app.status.is_empty(), "tab cycling should need no status");

    app.close_tab(1).expect("active tab should close");
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.tabs.active_index(), 0);
    assert_eq!(app.file_browser.cwd, root);

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("temporary root should be removed");
}

#[test]
fn switching_to_a_missing_tab_does_nothing_by_default() {
    let root = temp_root();
    let mut app = App::new_at(root.clone()).expect("app should load temporary root");

    app.activate_tab(4)
        .expect("switching to a missing tab should be ignored");

    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.tabs.active_index(), 0);
    assert_eq!(app.file_browser.cwd, root);

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("temporary root should be removed");
}

#[test]
fn configured_switch_tab_creates_missing_tabs_using_new_tab_destination() {
    const SETTING_ENV: &str = "ELIO_TEST_SWITCH_TAB_CREATES_MISSING";
    if std::env::var_os(SETTING_ENV).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                concat!(
                    module_path!(),
                    "::configured_switch_tab_creates_missing_tabs_using_new_tab_destination"
                )
                .trim_start_matches("elio::"),
            ])
            .env(SETTING_ENV, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "configured missing-tab switch:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }

    let root = temp_root();
    let destination = root.join("destination");
    fs::create_dir_all(&destination).expect("destination directory should be created");
    let destination_toml = toml::Value::String(destination.display().to_string()).to_string();
    fs::write(
        root.join("config.toml"),
        format!(
            "[tabs]\nswitch_tab_creates_missing = true\nnew_tab_destination = {{ path = {destination_toml} }}\n"
        ),
    )
    .expect("config should be written");
    crate::config::initialize(Some(&root.join("config.toml"))).expect("config should load");

    let mut app = App::new_at(root.clone()).expect("app should load temporary root");
    app.activate_tab(4)
        .expect("switching to tab five should create the missing tabs");

    assert_eq!(app.tabs.len(), 5);
    assert_eq!(app.tabs.active_index(), 4);
    assert_eq!(app.tabs.cwd_at(&app.file_browser, 0), Some(root.as_path()));
    for index in 1..5 {
        assert_eq!(
            app.tabs.cwd_at(&app.file_browser, index),
            Some(destination.as_path())
        );
    }

    app.activate_tab(0).expect("first tab should activate");
    app.activate_tab(4).expect("fifth tab should activate");
    assert_eq!(app.tabs.len(), 5, "existing tabs should not be recreated");

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("temporary root should be removed");
}

#[cfg(unix)]
#[test]
fn configured_switch_tab_creates_missing_tabs_using_new_tab_command_destination() {
    const SETTING_ENV: &str = "ELIO_TEST_SWITCH_TAB_CREATES_MISSING_COMMAND";
    if std::env::var_os(SETTING_ENV).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                concat!(
                    module_path!(),
                    "::configured_switch_tab_creates_missing_tabs_using_new_tab_command_destination"
                )
                .trim_start_matches("elio::"),
            ])
            .env(SETTING_ENV, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "configured command missing-tab switch:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }

    let root = temp_root();
    let destination = root.join("destination");
    fs::create_dir_all(&destination).expect("destination directory should be created");
    let command = toml::Value::String(format!("printf {}", destination.display())).to_string();
    fs::write(
        root.join("config.toml"),
        format!(
            "[tabs]\nswitch_tab_creates_missing = true\nnew_tab_destination = {{ command = {command} }}\n"
        ),
    )
    .expect("config should be written");
    crate::config::initialize(Some(&root.join("config.toml"))).expect("config should load");

    let mut app = App::new_at(root.clone()).expect("app should load temporary root");
    app.activate_tab(4)
        .expect("switching to tab five should create the missing tabs");
    for _ in 0..100 {
        app.process_background_jobs();
        if app.tabs.len() == 5 && app.pending_new_tab_command.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert_eq!(app.tabs.len(), 5);
    assert_eq!(app.tabs.active_index(), 4);
    assert_eq!(app.tabs.cwd_at(&app.file_browser, 0), Some(root.as_path()));
    for index in 1..5 {
        assert_eq!(
            app.tabs.cwd_at(&app.file_browser, index),
            Some(destination.as_path())
        );
    }

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("temporary root should be removed");
}

#[test]
fn configured_new_tab_path_uses_its_fallback_when_unavailable() {
    const SETTING_ENV: &str = "ELIO_TEST_NEW_TAB_PATH_DESTINATION";
    if std::env::var_os(SETTING_ENV).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                concat!(
                    module_path!(),
                    "::configured_new_tab_path_uses_its_fallback_when_unavailable"
                )
                .trim_start_matches("elio::"),
            ])
            .env(SETTING_ENV, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "configured new-tab path:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }

    let root = temp_root();
    let initial = root.join("initial");
    let fallback = root.join("fallback");
    let unavailable = root.join("unavailable");
    fs::create_dir_all(&initial).expect("initial directory should be created");
    fs::create_dir_all(&fallback).expect("fallback directory should be created");
    let fallback_toml = toml::Value::String(fallback.display().to_string()).to_string();
    let unavailable_toml = toml::Value::String(unavailable.display().to_string()).to_string();
    fs::write(
        root.join("config.toml"),
        format!(
            "[tabs]\nnew_tab_destination = {{ path = {unavailable_toml}, fallback = {{ path = {fallback_toml} }} }}\n"
        ),
    )
    .expect("config should be written");
    crate::config::initialize(Some(&root.join("config.toml"))).expect("config should load");

    let mut app = App::new_at(initial).expect("app should load initial directory");
    app.new_tab().expect("new tab should open from fallback");
    assert_eq!(app.file_browser.cwd, fallback);

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("temporary root should be removed");
}

#[cfg(unix)]
#[test]
fn configured_new_tab_command_opens_its_reported_directory() {
    const SETTING_ENV: &str = "ELIO_TEST_NEW_TAB_COMMAND_DESTINATION";
    if std::env::var_os(SETTING_ENV).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                concat!(
                    module_path!(),
                    "::configured_new_tab_command_opens_its_reported_directory"
                )
                .trim_start_matches("elio::"),
            ])
            .env(SETTING_ENV, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "configured new-tab command:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }

    let root = temp_root();
    let initial = root.join("initial");
    let destination = root.join("destination");
    fs::create_dir_all(&initial).expect("initial directory should be created");
    fs::create_dir_all(&destination).expect("destination directory should be created");
    fs::write(
        root.join("config.toml"),
        format!(
            "[tabs]\nnew_tab_destination = {{ command = \"printf {}\" }}\n",
            destination.display()
        ),
    )
    .expect("config should be written");
    crate::config::initialize(Some(&root.join("config.toml"))).expect("config should load");

    let mut app = App::new_at(initial).expect("app should load initial directory");
    app.new_tab().expect("new tab command should be submitted");
    for _ in 0..100 {
        app.process_background_jobs();
        if app.tabs.len() == 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(app.tabs.len(), 2, "command should open a second tab");
    assert_eq!(app.file_browser.cwd, destination);

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("temporary root should be removed");
}

#[cfg(unix)]
#[test]
fn failed_new_tab_command_uses_its_path_fallback() {
    const SETTING_ENV: &str = "ELIO_TEST_NEW_TAB_COMMAND_FALLBACK";
    if std::env::var_os(SETTING_ENV).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                concat!(
                    module_path!(),
                    "::failed_new_tab_command_uses_its_path_fallback"
                )
                .trim_start_matches("elio::"),
            ])
            .env(SETTING_ENV, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "configured new-tab command fallback:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }

    let root = temp_root();
    let initial = root.join("initial");
    let fallback = root.join("fallback");
    fs::create_dir_all(&initial).expect("initial directory should be created");
    fs::create_dir_all(&fallback).expect("fallback directory should be created");
    fs::write(
        root.join("config.toml"),
        format!(
            "[tabs]\nnew_tab_destination = {{ command = \"false\", fallback = {{ path = {:?} }} }}\n",
            fallback.display().to_string()
        ),
    )
    .expect("config should be written");
    crate::config::initialize(Some(&root.join("config.toml"))).expect("config should load");

    let mut app = App::new_at(initial).expect("app should load initial directory");
    app.new_tab().expect("new tab command should be submitted");
    for _ in 0..100 {
        app.process_background_jobs();
        if app.tabs.len() == 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(app.tabs.len(), 2, "fallback should open a second tab");
    assert_eq!(app.file_browser.cwd, fallback);

    app.file_browser.directory_runtime.watch = None;
    drop(app);
    fs::remove_dir_all(root).expect("temporary root should be removed");
}
