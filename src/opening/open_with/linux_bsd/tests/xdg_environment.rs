use super::*;

fn test_user() -> crate::elevated_session::InvokingUser {
    crate::elevated_session::InvokingUser {
        uid: 1000,
        gid: 1000,
        name: OsString::from("paco"),
        home: PathBuf::from("/home/paco"),
        shell: OsString::from("/bin/sh"),
        groups: vec![1000],
        session_environment: Vec::new(),
        xdg_config_home: Some(PathBuf::from("/home/paco/config")),
        xdg_data_home: Some(PathBuf::from("/home/paco/data")),
    }
}

#[test]
fn elevated_discovery_uses_invoking_user_directories() {
    let context = crate::elevated_session::InvocationContext::Elevated(test_user());

    assert_eq!(
        invoking_home_dir_for_context(&context),
        Some(PathBuf::from("/home/paco"))
    );
    assert_eq!(
        data_home_for_context(&context, Some(OsString::from("/root/data"))),
        Some(PathBuf::from("/home/paco/data"))
    );
    assert_eq!(
        config_home_for_context(&context, Some(OsString::from("/root/config"))),
        Some(PathBuf::from("/home/paco/config"))
    );
}

#[test]
fn unresolved_elevated_discovery_omits_user_directories() {
    let context = crate::elevated_session::InvocationContext::ElevatedUnresolved;

    assert_eq!(invoking_home_dir_for_context(&context), None);
    assert_eq!(
        data_home_for_context(&context, Some(OsString::from("/root/data"))),
        None
    );
    assert_eq!(
        config_home_for_context(&context, Some(OsString::from("/root/config"))),
        None
    );
}
