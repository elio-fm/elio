use super::*;

fn test_user() -> InvokingUser {
    InvokingUser {
        uid: 1000,
        gid: 1000,
        name: OsString::from("paco"),
        home: PathBuf::from("/home/paco"),
        shell: OsString::from("/bin/sh"),
        groups: vec![1000],
        session_environment: vec![(OsString::from("EDITOR"), OsString::from("nvim"))],
        xdg_config_home: None,
        xdg_data_home: None,
    }
}

fn assert_elevated_user(context: InvocationContext, expected: &InvokingUser) {
    let InvocationContext::Elevated(actual) = context else {
        panic!("user should resolve as invoking user");
    };
    assert_eq!(actual.uid, expected.uid);
    assert_eq!(actual.gid, expected.gid);
    assert_eq!(actual.home, expected.home);
    assert_eq!(actual.shell, expected.shell);
    assert!(actual.groups.contains(&actual.gid));
}

#[test]
fn uid_parser_accepts_root_and_non_root_numeric_ids() {
    assert_eq!(parse_uid(Some(OsStr::new("1000"))), Some(1000));
    assert_eq!(parse_uid(Some(OsStr::new("0"))), Some(0));
    assert_eq!(parse_uid(Some(OsStr::new("paco"))), None);
    assert_eq!(parse_uid(None), None);
}

#[test]
fn invoking_user_xdg_home_rejects_untrusted_paths() {
    let user = test_user();
    assert_eq!(
        validated_xdg_home(Some(OsStr::new("relative/config")), &user),
        None
    );
    assert_eq!(
        validated_xdg_home(Some(OsStr::new("/root/custom")), &user),
        None
    );
}

#[test]
fn invoking_user_xdg_home_accepts_user_owned_absolute_path() {
    let uid = unsafe { libc::geteuid() };
    let user = passwd_by_uid(uid).expect("current user should have a passwd record");
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should follow the Unix epoch")
        .as_nanos();
    let base = std::env::temp_dir().join(format!("elio-xdg-home-{}-{unique}", std::process::id()));
    std::fs::create_dir(&base).expect("failed to create user-owned XDG test directory");
    let candidate = base.join("config");

    let actual = validated_xdg_home(Some(candidate.as_os_str()), &user);

    std::fs::remove_dir(base).expect("failed to remove XDG test directory");
    assert_eq!(actual, Some(candidate));
}

#[test]
fn non_root_process_ignores_elevation_metadata() {
    assert!(matches!(
        invocation_context(
            1000,
            Some(OsStr::new("invalid")),
            Some(OsStr::new("unknown")),
            true,
            Some(OsStr::new("unknown")),
        ),
        InvocationContext::Normal
    ));
}

#[test]
fn direct_and_coherent_root_claims_are_root_sessions() {
    let root = passwd_by_uid(0).expect("root should have a passwd record");
    for context in [
        invocation_context(0, None, None, false, None),
        invocation_context(0, Some(OsStr::new("0")), Some(&root.name), false, None),
        invocation_context(0, None, None, false, Some(&root.name)),
        invocation_context(
            0,
            Some(OsStr::new("0")),
            Some(&root.name),
            false,
            Some(&root.name),
        ),
    ] {
        assert!(matches!(context, InvocationContext::RootSession));
    }
}

#[test]
fn coherent_non_root_claims_resolve_the_invoking_user() {
    let uid = unsafe { libc::getuid() };
    if uid == 0 {
        return;
    }
    let expected = passwd_by_uid(uid).expect("current user should have a passwd record");
    let uid = uid.to_string();
    for context in [
        invocation_context(0, Some(OsStr::new(&uid)), Some(&expected.name), false, None),
        invocation_context(0, None, None, false, Some(&expected.name)),
        invocation_context(
            0,
            Some(OsStr::new(&uid)),
            Some(&expected.name),
            false,
            Some(&expected.name),
        ),
    ] {
        assert_elevated_user(context, &expected);
    }
}

#[test]
fn sudo_claim_requires_matching_uid_and_user() {
    let root = passwd_by_uid(0).expect("root should have a passwd record");
    for (uid, user, other_metadata, expected) in [
        (
            Some(OsStr::new("0")),
            Some(root.name.as_os_str()),
            false,
            IdentityClaim::Root,
        ),
        (
            Some(OsStr::new("1")),
            Some(root.name.as_os_str()),
            false,
            IdentityClaim::Invalid,
        ),
        (
            Some(OsStr::new("invalid")),
            Some(root.name.as_os_str()),
            false,
            IdentityClaim::Invalid,
        ),
        (Some(OsStr::new("0")), None, false, IdentityClaim::Invalid),
        (
            None,
            Some(root.name.as_os_str()),
            false,
            IdentityClaim::Invalid,
        ),
        (None, None, true, IdentityClaim::Invalid),
        (None, None, false, IdentityClaim::Absent),
    ] {
        assert_eq!(sudo_identity_claim(uid, user, other_metadata), expected);
    }
}

#[test]
fn doas_claim_resolves_users_by_uid() {
    let root = passwd_by_uid(0).expect("root should have a passwd record");
    assert_eq!(doas_identity_claim(Some(&root.name)), IdentityClaim::Root);
    assert_eq!(doas_identity_claim(None), IdentityClaim::Absent);
    assert_eq!(
        doas_identity_claim(Some(OsStr::new("elio-user-that-does-not-exist"))),
        IdentityClaim::Invalid
    );
}

#[test]
fn identity_claims_must_be_absent_or_coherent() {
    use IdentityClaim::{Absent, Invalid, Root, User};

    for (sudo, doas, expected) in [
        (Absent, Absent, Ok(None)),
        (Root, Absent, Ok(None)),
        (Absent, Root, Ok(None)),
        (Root, Root, Ok(None)),
        (User(1000), Absent, Ok(Some(1000))),
        (Absent, User(1000), Ok(Some(1000))),
        (User(1000), User(1000), Ok(Some(1000))),
        (Root, User(1000), Err(())),
        (User(1000), Root, Err(())),
        (User(1000), User(1001), Err(())),
        (Invalid, User(1000), Err(())),
        (User(1000), Invalid, Err(())),
        (Invalid, Absent, Err(())),
        (Absent, Invalid, Err(())),
    ] {
        assert_eq!(agreed_invoking_uid(sudo, doas), expected);
    }
}

#[test]
fn elevated_environment_uses_invoking_user_value_not_root_value() {
    assert_eq!(
        env_var_for_context(
            &InvocationContext::Elevated(test_user()),
            "EDITOR",
            Some(OsString::from("root-editor")),
        ),
        Some(OsString::from("nvim"))
    );
    assert_eq!(
        env_var_for_context(
            &InvocationContext::Elevated(test_user()),
            "DISPLAY",
            Some(OsString::from(":root")),
        ),
        None
    );
}

#[test]
fn normal_environment_remains_unchanged() {
    assert_eq!(
        env_var_for_context(
            &InvocationContext::Normal,
            "EDITOR",
            Some(OsString::from("nvim")),
        ),
        Some(OsString::from("nvim"))
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_environment_parser_keeps_only_allowlisted_values() {
    let environment = parse_allowlisted_environment(
        b"HOME=/root\0DISPLAY=:0\0EDITOR=nvim --clean\0ELIO_ZOXIDE_OPTS=--no-mouse\0FZF_DEFAULT_OPTS=--height=40%\0SUDO_UID=1000\0",
    );

    assert_eq!(
        environment,
        vec![
            (OsString::from("DISPLAY"), OsString::from(":0")),
            (OsString::from("EDITOR"), OsString::from("nvim --clean")),
            (
                OsString::from("ELIO_ZOXIDE_OPTS"),
                OsString::from("--no-mouse"),
            ),
            (
                OsString::from("FZF_DEFAULT_OPTS"),
                OsString::from("--height=40%"),
            ),
        ]
    );
}

#[test]
fn zoxide_environment_values_are_allowlisted() {
    for name in [
        "_ZO_DATA_DIR",
        "_ZO_ECHO",
        "_ZO_EXCLUDE_DIRS",
        "_ZO_MAXAGE",
        "_ZO_RESOLVE_SYMLINKS",
    ] {
        assert!(
            SESSION_ENVIRONMENT_KEYS.contains(&name),
            "missing zoxide environment value: {name}"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linux_ancestor_environment_reads_current_user_parent() {
    let uid = unsafe { libc::getuid() };
    if uid == 0 {
        return;
    }
    let environment = linux_ancestor_environment(uid)
        .expect("non-root test process should have a same-user ancestor");
    assert!(
        environment.iter().any(|(name, _)| name == "PATH"),
        "allowlisted parent environment should contain PATH"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn elevation_environment_overrides_stale_shell_values() {
    let merged = merge_linux_environments(
        vec![
            (OsString::from("DISPLAY"), OsString::from(":0")),
            (OsString::from("EDITOR"), OsString::from("vi")),
        ],
        vec![(OsString::from("EDITOR"), OsString::from("nvim"))],
    );

    assert_eq!(
        merged,
        vec![
            (OsString::from("DISPLAY"), OsString::from(":0")),
            (OsString::from("EDITOR"), OsString::from("nvim")),
        ]
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_status_parser_reads_parent_and_all_uids() {
    assert_eq!(
        parse_linux_process_identity(b"Name:\ttest\nPPid:\t42\nUid:\t1000\t0\t0\t0\n"),
        Some((42, [1000, 0, 0, 0]))
    );
    assert_eq!(parse_linux_process_identity(b"Name:\ttest\n"), None);
}

#[test]
fn unresolved_elevated_context_has_no_trash_home() {
    assert_eq!(
        trash_home_for_context(&InvocationContext::ElevatedUnresolved),
        None
    );
}

#[test]
fn elevated_context_uses_invoking_user_trash_home() {
    assert_eq!(
        trash_home_for_context(&InvocationContext::Elevated(test_user())),
        Some(PathBuf::from("/home/paco"))
    );
}

#[test]
fn process_context_is_resolved_once() {
    assert!(std::ptr::eq(context(), context()));
}
