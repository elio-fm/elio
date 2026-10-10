use super::*;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn opening_overlay_cancels_active_drag_immediately() {
    let mut app = App::new_at(std::env::temp_dir()).expect("app should initialize");
    app.overlays.help = true;
    let mut pending_drag_out = PendingDragOut {
        active: true,
        uri_list: b"file:///tmp/item".to_vec(),
    };
    let mut output = Vec::new();

    assert!(
        cancel_active_drag_for_overlay(&mut output, &mut app, &mut pending_drag_out)
            .expect("overlay should cancel the active drag")
    );
    assert_eq!(output, cancel_drag_sequence().as_bytes());
    assert!(!pending_drag_out.active);
    assert!(pending_drag_out.uri_list.is_empty());
}

#[test]
fn overlays_block_drag_and_drop() {
    let mut app = App::new_at(std::env::temp_dir()).expect("app should initialize");
    assert!(!app.blocks_file_drag_and_drop());

    app.overlays.help = true;
    assert!(app.blocks_file_drag_and_drop());
    app.overlays.help = false;

    app.open_create_prompt();
    assert!(app.blocks_file_drag_and_drop());
}

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
fn drag_card_cell_height_uses_real_terminal_pixels() {
    assert_eq!(drag_card_cell_height_from_dimensions(960, 40), Some(24.0));
    assert_eq!(drag_card_cell_height_from_dimensions(0, 40), None);
    assert_eq!(drag_card_cell_height_from_dimensions(960, 0), None);
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
