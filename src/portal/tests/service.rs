use super::{
    ChooserRequest, ChooserRequestMode, Completion, OPEN_OPTIONS, SAVE_OPTIONS, SelectionKind,
    accepted, chooser_executable_with_fallback, file_uri, has_only_options, invocation_launcher,
    option_bool, option_path, save_startup, valid_ignored_options,
};
use std::collections::HashMap;
use zbus::zvariant::OwnedValue;

type Filter = (String, Vec<(u32, String)>);

#[test]
fn invocation_launcher_keeps_an_absolute_wrapper_path() {
    use std::{fs, os::unix::fs::PermissionsExt, process};

    let directory = std::env::temp_dir().join(format!("elio-portal-service-{}", process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir(&directory).expect("create test directory");
    let launcher = directory.join("elio");
    fs::write(&launcher, "#!/bin/sh\nexec elio \"$@\"\n").expect("create wrapper-style launcher");
    let mut permissions = fs::metadata(&launcher)
        .expect("read wrapper permissions")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&launcher, permissions).expect("make wrapper executable");

    assert_eq!(
        invocation_launcher(Some(launcher.clone().into_os_string())),
        Some(launcher.clone())
    );
    assert_eq!(
        chooser_executable_with_fallback(Some(launcher.clone().into_os_string()), || {
            panic!("valid wrapper must not use fallback")
        })
        .expect("select wrapper launcher"),
        launcher
    );
    assert_ne!(
        fs::canonicalize(&launcher).expect("canonicalize launcher"),
        std::env::current_exe().expect("find test executable")
    );
    assert_eq!(invocation_launcher(Some("elio".into())), None);
    assert_eq!(
        invocation_launcher(Some(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).into_os_string()
        )),
        None
    );
    let fallback = std::path::PathBuf::from("/fallback/elio");
    assert_eq!(
        chooser_executable_with_fallback(Some("elio".into()), || Ok(fallback.clone()))
            .expect("fall back from relative launcher"),
        fallback
    );

    fs::remove_dir_all(directory).expect("remove test directory");
}

fn zen_filters() -> Vec<Filter> {
    vec![
        (
            "Images".into(),
            vec![(1, "image/jpeg".into()), (1, "image/png".into())],
        ),
        ("All files".into(), vec![(0, "*".into())]),
    ]
}

fn zen_open_options() -> HashMap<String, OwnedValue> {
    let filters = zen_filters();
    let current_filter = filters[0].clone();
    HashMap::from([
        ("modal".into(), OwnedValue::from(true)),
        ("multiple".into(), OwnedValue::from(true)),
        (
            "filters".into(),
            OwnedValue::try_from(zbus::zvariant::Value::from(filters))
                .expect("Zen filter list is a valid D-Bus value"),
        ),
        (
            "current_filter".into(),
            OwnedValue::try_from(zbus::zvariant::Value::from(current_filter))
                .expect("Zen current filter is a valid D-Bus value"),
        ),
    ])
}

fn zen_save_options() -> HashMap<String, OwnedValue> {
    let filters = zen_filters();
    let current_filter = filters[0].clone();
    HashMap::from([
        ("modal".into(), OwnedValue::from(true)),
        (
            "accept_label".into(),
            OwnedValue::try_from(zbus::zvariant::Value::from("Save"))
                .expect("Zen accept label is a valid D-Bus value"),
        ),
        (
            "filters".into(),
            OwnedValue::try_from(zbus::zvariant::Value::from(filters))
                .expect("Zen filter list is a valid D-Bus value"),
        ),
        (
            "current_filter".into(),
            OwnedValue::try_from(zbus::zvariant::Value::from(current_filter))
                .expect("Zen current filter is a valid D-Bus value"),
        ),
        (
            "current_folder".into(),
            OwnedValue::try_from(zbus::zvariant::Value::from(b"/tmp\0".to_vec()))
                .expect("Zen current folder is a valid D-Bus value"),
        ),
        (
            "current_name".into(),
            OwnedValue::try_from(zbus::zvariant::Value::from("download.png"))
                .expect("Zen current name is a valid D-Bus value"),
        ),
    ])
}

#[test]
fn file_uri_percent_encodes_raw_unix_bytes() {
    assert_eq!(
        file_uri(b"/tmp/a b#%\xff"),
        Some("file:///tmp/a%20b%23%25%FF".into())
    );
}

#[test]
fn file_uri_requires_an_absolute_non_nul_path() {
    assert_eq!(file_uri(b"relative"), None);
    assert_eq!(file_uri(b"/tmp/a\0b"), None);
}

#[test]
fn zen_open_and_save_option_shapes_are_allowed_and_validated() {
    let open = zen_open_options();
    let save = zen_save_options();
    assert!(has_only_options(&open, OPEN_OPTIONS));
    assert!(has_only_options(&save, SAVE_OPTIONS));
    assert!(valid_ignored_options(&open));
    assert!(valid_ignored_options(&save));
}

#[test]
fn ignored_options_require_documented_types_and_bounds() {
    let mut options = zen_open_options();
    options.insert("filters".into(), OwnedValue::from(true));
    assert!(!valid_ignored_options(&options));

    let mut options = zen_save_options();
    options.insert(
        "accept_label".into(),
        OwnedValue::try_from(zbus::zvariant::Value::from("x".repeat(4097)))
            .expect("a string is a valid D-Bus value"),
    );
    assert!(!valid_ignored_options(&options));
}

#[test]
fn current_file_supplies_save_startup_when_more_specific_hints_are_absent() {
    let mut options = HashMap::new();
    options.insert(
        "current_file".into(),
        OwnedValue::try_from(zbus::zvariant::Value::from(
            b"/tmp/downloads/report.png\0".to_vec(),
        ))
        .expect("current file is a valid D-Bus value"),
    );
    assert_eq!(
        save_startup(&options),
        Some((Some(b"/tmp/downloads".to_vec()), "report.png".into()))
    );

    options.insert(
        "current_folder".into(),
        OwnedValue::try_from(zbus::zvariant::Value::from(b"/tmp/elsewhere\0".to_vec()))
            .expect("current folder is a valid D-Bus value"),
    );
    options.insert(
        "current_name".into(),
        OwnedValue::try_from(zbus::zvariant::Value::from("renamed.png"))
            .expect("current name is a valid D-Bus value"),
    );
    assert_eq!(
        save_startup(&options),
        Some((Some(b"/tmp/elsewhere".to_vec()), "renamed.png".into()))
    );
}

#[test]
fn choices_remain_rejected() {
    let mut options = HashMap::new();
    options.insert("choices".to_owned(), OwnedValue::from(true));
    assert!(!has_only_options(&options, OPEN_OPTIONS));
}

#[test]
fn open_multiple_returns_every_valid_selected_file() {
    let root = env!("CARGO_MANIFEST_DIR").as_bytes().to_vec();
    let paths = [
        [root.clone(), b"/Cargo.toml".to_vec()].concat(),
        [root, b"/src/portal/service.rs".to_vec()].concat(),
    ];
    let multiple = ChooserRequest {
        mode: ChooserRequestMode::Open {
            kind: SelectionKind::File,
            multiple: true,
        },
        initial_path: None,
    };
    assert_eq!(accepted(paths.to_vec(), &multiple).0, 0);
    let single = ChooserRequest {
        mode: ChooserRequestMode::Open {
            kind: SelectionKind::File,
            multiple: false,
        },
        initial_path: None,
    };
    assert_eq!(accepted(paths.to_vec(), &single).0, 2);
}

#[test]
fn typed_boolean_options_do_not_silently_default() {
    let mut options = HashMap::new();
    options.insert(
        "multiple".into(),
        OwnedValue::try_from(zbus::zvariant::Value::from("yes"))
            .expect("a string is a valid D-Bus value"),
    );
    assert_eq!(option_bool(&options, "multiple"), None);
    assert_eq!(option_bool(&HashMap::new(), "multiple"), Some(false));
}

#[test]
fn current_folder_requires_an_absolute_nonempty_byte_path() {
    let mut options = HashMap::new();
    options.insert(
        "current_folder".into(),
        OwnedValue::try_from(zbus::zvariant::Value::from(vec![b'/', b't', b'm', b'p', 0]))
            .expect("a byte array is a valid D-Bus value"),
    );
    assert_eq!(
        option_path(&options, "current_folder"),
        Some(Some(b"/tmp".to_vec()))
    );
    options.insert(
        "current_folder".into(),
        OwnedValue::try_from(zbus::zvariant::Value::from(Vec::<u8>::new()))
            .expect("a byte array is a valid D-Bus value"),
    );
    assert_eq!(option_path(&options, "current_folder"), None);
}

#[test]
fn first_terminal_completion_wins_close_result_races() {
    let completion = Completion::default();
    completion.complete((1, HashMap::new()));
    completion.complete((2, HashMap::new()));
    assert_eq!(
        completion
            .state
            .lock()
            .expect("completion lock poisoned")
            .value
            .take()
            .map(|(response, _)| response),
        Some(1)
    );
}
