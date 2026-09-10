use super::*;

#[test]
fn unix_shell_uses_shell_env_then_sh_fallback() {
    assert_eq!(
        unix_shell_invocations(Some(OsString::from("/bin/bash"))),
        vec![
            ShellInvocation {
                program: OsString::from("/bin/bash"),
                args: Vec::new(),
            },
            ShellInvocation {
                program: OsString::from("/bin/sh"),
                args: Vec::new(),
            },
        ]
    );
}

#[test]
fn unix_shell_uses_only_sh_when_shell_is_empty() {
    assert_eq!(
        unix_shell_invocations(Some(OsString::from(""))),
        vec![ShellInvocation {
            program: OsString::from("/bin/sh"),
            args: Vec::new(),
        }]
    );
}

#[test]
fn unix_shell_does_not_duplicate_sh_fallback() {
    assert_eq!(
        unix_shell_invocations(Some(OsString::from("/bin/sh"))),
        vec![ShellInvocation {
            program: OsString::from("/bin/sh"),
            args: Vec::new(),
        }]
    );
}

#[cfg(unix)]
fn test_invoking_user(shell: &str) -> crate::elevated_session::InvokingUser {
    crate::elevated_session::InvokingUser {
        uid: 1000,
        gid: 1000,
        name: OsString::from("paco"),
        home: "/home/paco".into(),
        shell: OsString::from(shell),
        groups: vec![1000],
        session_environment: Vec::new(),
        xdg_config_home: None,
        xdg_data_home: None,
    }
}

#[cfg(unix)]
#[test]
fn elevated_shell_uses_passwd_shell_not_inherited_root_shell() {
    let user = test_invoking_user("/bin/fish");
    let context = crate::elevated_session::InvocationContext::Elevated(user);
    let (invocations, actual_user) =
        unix_shell_launch(&context, Some(OsString::from("/bin/root-shell"))).unwrap();

    assert_eq!(invocations[0].program, OsString::from("/bin/fish"));
    assert_eq!(invocations[1].program, OsString::from("/bin/sh"));
    assert_eq!(actual_user.unwrap().name, OsString::from("paco"));
}

#[cfg(unix)]
#[test]
fn unresolved_elevated_shell_fails_closed() {
    let error = unix_shell_launch(
        &crate::elevated_session::InvocationContext::ElevatedUnresolved,
        Some(OsString::from("/bin/root-shell")),
    )
    .unwrap_err();

    assert_eq!(
        error,
        "Could not resolve invoking user; shell was not opened"
    );
}

#[test]
fn windows_shell_uses_comspec_before_powershell_fallbacks() {
    assert_eq!(
        windows_shell_invocations(Some(OsString::from(r"C:\Windows\System32\cmd.exe"))),
        vec![
            ShellInvocation {
                program: OsString::from(r"C:\Windows\System32\cmd.exe"),
                args: Vec::new(),
            },
            ShellInvocation {
                program: OsString::from("pwsh"),
                args: vec![OsString::from("-NoLogo")],
            },
            ShellInvocation {
                program: OsString::from("powershell"),
                args: vec![OsString::from("-NoLogo")],
            },
            ShellInvocation {
                program: OsString::from("cmd"),
                args: Vec::new(),
            },
        ]
    );
}

#[test]
fn windows_shell_falls_back_when_comspec_is_empty() {
    assert_eq!(
        windows_shell_invocations(Some(OsString::from(" "))),
        vec![
            ShellInvocation {
                program: OsString::from("pwsh"),
                args: vec![OsString::from("-NoLogo")],
            },
            ShellInvocation {
                program: OsString::from("powershell"),
                args: vec![OsString::from("-NoLogo")],
            },
            ShellInvocation {
                program: OsString::from("cmd"),
                args: Vec::new(),
            },
        ]
    );
}

#[test]
fn shell_level_starts_at_one() {
    assert_eq!(next_shell_level(None), OsString::from("1"));
    assert_eq!(
        next_shell_level(Some(OsString::from(""))),
        OsString::from("1")
    );
    assert_eq!(
        next_shell_level(Some(OsString::from("not-a-number"))),
        OsString::from("1")
    );
}

#[test]
fn shell_level_increments_existing_level() {
    assert_eq!(
        next_shell_level(Some(OsString::from("1"))),
        OsString::from("2")
    );
    assert_eq!(
        next_shell_level(Some(OsString::from(" 41 "))),
        OsString::from("42")
    );
}

#[test]
fn cwd_check_reports_deleted_folder() {
    let missing = std::env::temp_dir().join(format!(
        "elio-missing-shell-cwd-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos()
    ));

    let error = ensure_cwd_exists(&missing).expect_err("missing cwd should fail");

    assert!(
        error.contains("folder no longer exists"),
        "unexpected error: {error}"
    );
}
