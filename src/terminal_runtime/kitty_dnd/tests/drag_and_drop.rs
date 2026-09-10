use super::*;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn unsupported_drop_scheme_status_names_one_or_many_schemes() {
    assert_eq!(
        unsupported_drop_scheme_status(&["trash".to_string()]),
        "Unsupported drop URI scheme: trash"
    );
    assert_eq!(
        unsupported_drop_scheme_status(&["trash".to_string(), "smb".to_string()]),
        "Unsupported drop URI schemes: trash, smb"
    );
}

#[test]
fn drag_icon_label_uses_name_for_single_item_and_count_for_many() {
    assert_eq!(
        drag_icon_label_with(&[PathBuf::from("/tmp/report.pdf")], |_| (
            "󰈙".to_string(),
            ratatui::style::Color::White,
        ))
        .as_text(),
        "󰈙 report.pdf"
    );
    assert_eq!(
        drag_icon_label_with(&[PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")], |_| (
            "󰈔".to_string(),
            ratatui::style::Color::White,
        ))
        .as_text(),
        "󰈔 2 items"
    );
}

#[test]
fn drag_icon_label_uses_multi_item_icon_for_mixed_selection() {
    let paths = [PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")];

    assert_eq!(
        drag_icon_label_with(&paths, |path| (
            if path.ends_with("a") { "󰉋" } else { "󰈔" }.to_string(),
            ratatui::style::Color::White,
        ))
        .as_text(),
        " 2 items"
    );
}

#[test]
fn drag_icon_label_uses_multi_folder_icon_for_different_folder_icons() {
    let root = std::env::temp_dir().join(format!(
        "elio-drag-icons-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let first = root.join("src");
    let second = root.join("docs");
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();

    let label = drag_icon_label_with(&[first, second], |path| {
        let icon = if path.ends_with("src") {
            "󰉋"
        } else {
            "󰉓"
        };
        (icon.to_string(), ratatui::style::Color::White)
    })
    .as_text();
    fs::remove_dir_all(root).unwrap();

    assert_eq!(label, "󰉓 2 items");
}

#[test]
fn drag_icon_label_truncates_long_names() {
    assert_eq!(
        drag_icon_label_with(
            &[PathBuf::from(
                "/tmp/abcdefghijklmnopqrstuvwxyz0123456789.txt"
            )],
            |_| ("󰈔".to_string(), ratatui::style::Color::White)
        )
        .as_text(),
        "󰈔 abcdefghijklmnopqrstuvwxyz012..."
    );
}
