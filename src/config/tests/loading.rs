use super::*;
#[cfg(unix)]
use crate::elevated_session::InvokingUser;
#[cfg(unix)]
use std::ffi::OsString;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
fn test_user(xdg_config_home: Option<&str>) -> InvokingUser {
    InvokingUser {
        uid: 1000,
        gid: 1000,
        name: OsString::from("paco"),
        home: PathBuf::from("/home/paco"),
        shell: OsString::from("/bin/sh"),
        groups: vec![1000],
        session_environment: Vec::new(),
        xdg_config_home: xdg_config_home.map(PathBuf::from),
        xdg_data_home: None,
    }
}

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-config-{label}-{unique}"))
}

#[cfg(unix)]
#[test]
fn config_home_follows_invocation_context() {
    let root_default = if cfg!(target_os = "macos") {
        Path::new("/root/Library/Application Support")
    } else {
        Path::new("/root/.config")
    };
    let user_default = if cfg!(target_os = "macos") {
        Path::new("/home/paco/Library/Application Support")
    } else {
        Path::new("/home/paco/.config")
    };
    let process_xdg = Some(Path::new("/root/custom"));
    let cases = [
        (
            InvocationContext::Normal,
            process_xdg,
            Some(Path::new("/root/custom")),
        ),
        (
            InvocationContext::RootSession,
            process_xdg,
            Some(Path::new("/root/custom")),
        ),
        (InvocationContext::Normal, None, Some(root_default)),
        (InvocationContext::RootSession, None, Some(root_default)),
        (
            InvocationContext::Elevated(test_user(Some("/home/paco/custom-config"))),
            process_xdg,
            Some(Path::new("/home/paco/custom-config")),
        ),
        (
            InvocationContext::Elevated(test_user(None)),
            process_xdg,
            Some(user_default),
        ),
        (InvocationContext::ElevatedUnresolved, process_xdg, None),
    ];

    for (context, process_xdg, expected) in cases {
        let actual = config_home_for_context(&context, process_xdg, Some(Path::new("/root")));
        assert_eq!(actual.as_deref(), expected, "context: {context:?}");
    }
}

#[test]
fn explicit_config_path_is_loaded() {
    let root = temp_path("explicit");
    let path = root.join("custom-settings.toml");
    fs::create_dir_all(&root).expect("config directory should be created");
    fs::write(&path, "[ui]\nshow_hidden = true\n").expect("explicit config should be written");

    let config = load_config_from_disk(Some(&path)).expect("explicit config should load");

    assert!(config.ui.show_hidden);
    fs::remove_dir_all(root).expect("config directory should be removed");
}

#[test]
fn missing_explicit_config_path_is_an_error() {
    let path = temp_path("missing").join("config.toml");

    let error = load_config_from_disk(Some(&path))
        .err()
        .expect("missing explicit config should fail");

    assert!(error.to_string().contains(&format!(
        "elio: failed to read config from {}",
        path.display()
    )));
}

#[test]
fn unreadable_explicit_config_path_is_an_error() {
    let root = temp_path("unreadable");
    let path = root.join("config.toml");
    fs::create_dir_all(&root).expect("config directory should be created");
    fs::write(&path, [0xff]).expect("invalid UTF-8 config should be written");

    let error = load_config_from_disk(Some(&path))
        .err()
        .expect("unreadable explicit config should fail");

    assert!(error.to_string().contains(&format!(
        "elio: failed to read config from {}",
        path.display()
    )));
    fs::remove_dir_all(root).expect("config directory should be removed");
}

#[test]
fn invalid_explicit_config_falls_back_to_defaults() {
    let root = temp_path("invalid");
    let path = root.join("config.toml");
    fs::create_dir_all(&root).expect("config directory should be created");
    fs::write(&path, "[ui\nshow_hidden = true\n").expect("invalid config should be written");

    let config =
        load_config_from_disk(Some(&path)).expect("invalid explicit config should fall back");

    assert!(!config.ui.show_hidden);
    fs::remove_dir_all(root).expect("config directory should be removed");
}
