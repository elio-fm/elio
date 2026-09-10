use super::*;
use std::ffi::OsString;

fn test_user() -> InvokingUser {
    InvokingUser {
        uid: 1000,
        gid: 1000,
        name: OsString::from("paco"),
        home: PathBuf::from("/home/paco"),
        shell: OsString::from("/bin/fish"),
        groups: vec![1000],
        session_environment: vec![
            (
                OsString::from("DBUS_SESSION_BUS_ADDRESS"),
                OsString::from("unix:path=/run/user/1000/bus"),
            ),
            (OsString::from("DISPLAY"), OsString::from(":1")),
            (OsString::from("EDITOR"), OsString::from("nvim")),
            (
                OsString::from("PATH"),
                OsString::from("/home/paco/.local/bin:/usr/bin"),
            ),
            (
                OsString::from("WAYLAND_DISPLAY"),
                OsString::from("wayland-1"),
            ),
        ],
        xdg_config_home: Some(PathBuf::from("/home/paco/.config")),
        xdg_data_home: Some(PathBuf::from("/home/paco/.local/share")),
    }
}

fn command_env(command: &Command, name: &str) -> Option<Option<OsString>> {
    command
        .get_envs()
        .find(|(key, _)| *key == OsStr::new(name))
        .map(|(_, value)| value.map(OsStr::to_os_string))
}

#[test]
fn user_environment_replaces_identity_and_removes_elevation_metadata() {
    let mut command = Command::new("true");
    apply_user_environment(&mut command, &test_user());

    assert_eq!(
        command_env(&command, "HOME"),
        Some(Some(OsString::from("/home/paco")))
    );
    assert_eq!(
        command_env(&command, "USER"),
        Some(Some(OsString::from("paco")))
    );
    assert_eq!(
        command_env(&command, "USERNAME"),
        Some(Some(OsString::from("paco")))
    );
    assert_eq!(
        command_env(&command, "LOGNAME"),
        Some(Some(OsString::from("paco")))
    );
    assert_eq!(
        command_env(&command, "SHELL"),
        Some(Some(OsString::from("/bin/fish")))
    );
    assert_eq!(command_env(&command, "SUDO_UID"), Some(None));
    assert_eq!(command_env(&command, "SUDO_USER"), Some(None));
    assert_eq!(command_env(&command, "DOAS_USER"), Some(None));
    assert_eq!(command_env(&command, "MAIL"), Some(None));
    assert_eq!(command_env(&command, "OLDPWD"), Some(None));
    assert_eq!(
        command_env(&command, "XDG_DATA_HOME"),
        Some(Some(OsString::from("/home/paco/.local/share")))
    );
    assert_eq!(
        command_env(&command, "XDG_CONFIG_HOME"),
        Some(Some(OsString::from("/home/paco/.config")))
    );
    assert_eq!(
        command_env(&command, "DISPLAY"),
        Some(Some(OsString::from(":1")))
    );
    assert_eq!(
        command_env(&command, "WAYLAND_DISPLAY"),
        Some(Some(OsString::from("wayland-1")))
    );
    assert_eq!(
        command_env(&command, "EDITOR"),
        Some(Some(OsString::from("nvim")))
    );
}

#[test]
fn absent_invoking_user_session_values_remove_root_values() {
    let mut user = test_user();
    user.session_environment.clear();
    let mut command = Command::new("true");
    apply_user_environment(&mut command, &user);

    assert_eq!(command_env(&command, "DISPLAY"), Some(None));
    assert_eq!(command_env(&command, "WAYLAND_DISPLAY"), Some(None));
    assert_eq!(
        command_env(&command, "DBUS_SESSION_BUS_ADDRESS"),
        Some(None)
    );
    assert_eq!(command_env(&command, "EDITOR"), Some(None));
}

#[test]
fn prepare_rejects_nul_in_working_directory_before_spawn() {
    use std::os::unix::ffi::OsStrExt;

    let mut command = Command::new("true");
    let path = Path::new(OsStr::from_bytes(b"/tmp/a\0b"));
    let error = prepare(&mut command, &test_user(), Some(path)).unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn prepare_sets_pwd_to_requested_working_directory() {
    let mut command = Command::new("true");
    prepare(
        &mut command,
        &test_user(),
        Some(Path::new("/home/paco/Documents")),
    )
    .unwrap();

    assert_eq!(
        command_env(&command, "PWD"),
        Some(Some(OsString::from("/home/paco/Documents")))
    );
}

#[test]
fn normal_external_command_is_left_unchanged() {
    let mut command = Command::new("true");
    prepare_external_for_context(
        &mut command,
        &InvocationContext::Normal,
        Some(Path::new("/ignored")),
    )
    .unwrap();

    assert_eq!(command.get_current_dir(), None);
    assert_eq!(command_env(&command, "HOME"), None);
}

#[test]
fn elevated_detached_command_uses_invoking_user_home() {
    let mut command = Command::new("true");
    prepare_external_for_context(
        &mut command,
        &InvocationContext::Elevated(test_user()),
        None,
    )
    .unwrap();

    assert_eq!(
        command_env(&command, "PWD"),
        Some(Some(OsString::from("/home/paco")))
    );
    assert_eq!(
        command_env(&command, "HOME"),
        Some(Some(OsString::from("/home/paco")))
    );
}

#[test]
fn elevated_external_command_uses_requested_working_directory() {
    let mut command = Command::new("true");
    prepare_external_for_context(
        &mut command,
        &InvocationContext::Elevated(test_user()),
        Some(Path::new("/home/paco/Documents")),
    )
    .unwrap();

    assert_eq!(
        command_env(&command, "PWD"),
        Some(Some(OsString::from("/home/paco/Documents")))
    );
}

#[test]
fn unresolved_elevated_external_command_fails_closed() {
    let mut command = Command::new("true");
    let error =
        prepare_external_for_context(&mut command, &InvocationContext::ElevatedUnresolved, None)
            .unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(error.to_string(), "could not resolve invoking user");
}

#[test]
fn runtime_dir_must_be_absolute_existing_and_user_owned() {
    assert!(!valid_runtime_dir(Some(OsStr::new("relative")), 1000));
    assert!(!valid_runtime_dir(Some(OsStr::new("/missing")), 1000));
    let temp = std::env::temp_dir().join(format!(
        "elio-runtime-dir-test-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("unnamed")
    ));
    std::fs::create_dir_all(&temp).unwrap();
    let uid = unsafe { libc::geteuid() };
    assert!(valid_runtime_dir(Some(temp.as_os_str()), uid));
    std::fs::remove_dir(&temp).unwrap();
}

#[test]
fn owned_absolute_path_accepts_missing_leaf_under_user_directory() {
    let temp = std::env::temp_dir().join(format!(
        "elio-user-env-test-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("unnamed")
    ));
    std::fs::create_dir_all(&temp).unwrap();
    let uid = unsafe { libc::geteuid() };
    assert!(valid_owned_absolute_path(&temp.join("missing"), uid));
    assert!(!valid_owned_absolute_path(Path::new("relative"), uid));
    std::fs::remove_dir(&temp).unwrap();
}
