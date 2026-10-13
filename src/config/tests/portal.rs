use super::super::*;
use crate::portal::terminal::TerminalAdapter;
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
    std::env::temp_dir().join(format!("elio-portal-config-{label}-{unique}"))
}

#[test]
fn portal_terminal_accepts_the_canonical_names() {
    let cases = [
        ("kitty", TerminalAdapter::Kitty),
        ("ghostty", TerminalAdapter::Ghostty),
        ("foot", TerminalAdapter::Foot),
        ("footclient", TerminalAdapter::FootClient),
        ("wezterm", TerminalAdapter::WezTerm),
        ("alacritty", TerminalAdapter::Alacritty),
        ("rio", TerminalAdapter::Rio),
        ("konsole", TerminalAdapter::Konsole),
        ("gnome-terminal", TerminalAdapter::Gnome),
        ("xterm", TerminalAdapter::Xterm),
    ];

    for (name, terminal) in cases {
        let config = Config::from_str(&format!("[portal]\nterminal = {name:?}"))
            .expect("supported portal terminal should parse");
        assert_eq!(config.portal.terminal, Some(terminal));
    }
}

#[test]
fn portal_terminal_is_optional() {
    assert_eq!(Config::from_str("").unwrap().portal.terminal, None);
    assert_eq!(
        Config::from_str("[portal]\n").unwrap().portal.terminal,
        None
    );
}

#[test]
fn portal_terminal_rejects_unknown_values() {
    let error = match Config::from_str("[portal]\nterminal = \"warp\"") {
        Ok(_) => panic!("unknown portal terminal should be rejected"),
        Err(error) => error.to_string(),
    };

    assert!(error.contains("unknown variant `warp`"));
    assert!(error.contains("kitty"));
    assert!(error.contains("gnome-terminal"));
}

#[test]
fn existing_portal_terminal_is_not_rewritten_or_detected() {
    let root = temp_path("existing");
    let path = root.join("config.toml");
    let original = "# Keep this comment.\n[portal]\nterminal = \"foot\" # User choice\n\n[ui]\nshow_hidden = true\n";
    fs::create_dir_all(&root).unwrap();
    fs::write(&path, original).unwrap();

    let result = ensure_portal_terminal(&path, || panic!("existing setting must skip detection"))
        .expect("existing setting should be retained");

    assert!(matches!(
        result,
        ConfigurePortalTerminal::Existing(TerminalAdapter::Foot)
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn empty_portal_terminal_fails_without_rewriting_the_file() {
    let root = temp_path("empty-terminal");
    let path = root.join("config.toml");
    let original = "[portal]\nterminal = \"\"\n";
    fs::create_dir_all(&root).unwrap();
    fs::write(&path, original).unwrap();

    let error = match ensure_portal_terminal(&path, || panic!("invalid config must skip detection"))
    {
        Ok(_) => panic!("an empty terminal value must be rejected"),
        Err(error) => error.to_string(),
    };

    assert!(error.contains("failed to parse config"));
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn empty_portal_section_inserts_terminal_immediately_after_its_header() {
    let root = temp_path("empty-portal");
    let path = root.join("config.toml");
    fs::create_dir_all(&root).unwrap();
    fs::write(&path, "[portal]\n").unwrap();

    ensure_portal_terminal(&path, || Some(TerminalAdapter::Kitty))
        .expect("empty portal section should be configured");

    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "[portal]\nterminal = \"kitty\"\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_portal_terminal_is_inserted_without_reformatting_other_config() {
    let root = temp_path("insert");
    let path = root.join("config.toml");
    let original = "# Keep this comment.\n[ui]\nshow_hidden = true\n\n[portal] # Keep this too.\n# Portal comment.\n\n[open]\nrules = []\n";
    let expected = "# Keep this comment.\n[ui]\nshow_hidden = true\n\n[portal] # Keep this too.\nterminal = \"kitty\"\n# Portal comment.\n\n[open]\nrules = []\n";
    fs::create_dir_all(&root).unwrap();
    fs::write(&path, original).unwrap();

    let result = ensure_portal_terminal(&path, || Some(TerminalAdapter::Kitty))
        .expect("terminal should be configured");

    assert!(matches!(
        result,
        ConfigurePortalTerminal::Configured(TerminalAdapter::Kitty)
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), expected);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn writable_symlinked_config_updates_its_target_without_replacing_the_link() {
    use std::os::unix::fs::symlink;

    let root = temp_path("symlink");
    let target = root.join("managed-config.toml");
    let path = root.join("config.toml");
    let original = "# Keep this comment.\n[ui]\nshow_hidden = true\n";
    fs::create_dir_all(&root).unwrap();
    fs::write(&target, original).unwrap();
    symlink(&target, &path).unwrap();

    ensure_portal_terminal(&path, || Some(TerminalAdapter::Kitty))
        .expect("a writable symlink target should be configured");

    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "# Keep this comment.\n[ui]\nshow_hidden = true\n\n[portal]\nterminal = \"kitty\"\n"
    );
    assert!(
        fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_portal_section_is_appended_without_reformatting_other_config() {
    let root = temp_path("append");
    let path = root.join("config.toml");
    let original = "# Keep this comment.\n[ui]\nshow_hidden = true\n";
    let expected =
        "# Keep this comment.\n[ui]\nshow_hidden = true\n\n[portal]\nterminal = \"kitty\"\n";
    fs::create_dir_all(&root).unwrap();
    fs::write(&path, original).unwrap();

    let result = ensure_portal_terminal(&path, || Some(TerminalAdapter::Kitty))
        .expect("terminal should be configured");

    assert!(matches!(
        result,
        ConfigurePortalTerminal::Configured(TerminalAdapter::Kitty)
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), expected);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_config_is_created_with_only_the_portal_terminal() {
    let root = temp_path("new");
    let path = root.join("elio/config.toml");

    let result = ensure_portal_terminal(&path, || Some(TerminalAdapter::Rio))
        .expect("terminal should create config");

    assert!(matches!(
        result,
        ConfigurePortalTerminal::Configured(TerminalAdapter::Rio)
    ));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "[portal]\nterminal = \"rio\"\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unknown_terminal_does_not_write_config() {
    let root = temp_path("unknown");
    let path = root.join("config.toml");
    let original = "# Keep this comment.\n[ui]\nshow_hidden = true\n";
    fs::create_dir_all(&root).unwrap();
    fs::write(&path, original).unwrap();

    let error = match ensure_portal_terminal(&path, || None) {
        Ok(_) => panic!("unknown terminal should require a manual setting"),
        Err(error) => error.to_string(),
    };

    assert!(error.contains("set [portal].terminal manually"));
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    fs::remove_dir_all(root).unwrap();
}
