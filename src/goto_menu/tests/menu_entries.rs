use super::super::*;
use crate::{
    config::GotoEntrySpec,
    places::{PlaceItem, PlaceKind, PlaceRow},
};
use std::{fs, path::PathBuf, time::SystemTime};

fn temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time should be after the Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-goto-menu-{label}-{unique}"))
}

#[test]
fn configured_entries_resolve_available_and_missing_paths() {
    let root = temp_dir("configured-entries");
    let available = root.join("available");
    let missing = root.join("missing");
    fs::create_dir_all(&available).expect("available destination should be created");

    let configured = vec![
        GotoEntrySpec::Custom {
            title: "Available".to_string(),
            path: available.clone(),
            key: 'a',
        },
        GotoEntrySpec::Custom {
            title: "Missing".to_string(),
            path: missing,
            key: 'm',
        },
    ];
    let menu = build_goto_menu(&configured, &[]);

    assert_eq!(menu.title(), "Go to");
    assert_eq!(menu.len(), 2);
    assert_eq!(menu.index_for_shortcut('a'), Some(0));
    assert_eq!(menu.destination(0), Some(GotoDestination::Path(available)));
    assert_eq!(
        menu.destination(1),
        Some(GotoDestination::Missing(
            "Missing not available".to_string()
        ))
    );

    fs::remove_dir_all(root).expect("temporary directory should be removed");
}

#[test]
fn downloads_destination_uses_the_places_list() {
    let root = temp_dir("downloads");
    let downloads = root.join("Downloads");
    fs::create_dir_all(&downloads).expect("downloads destination should be created");
    let places = vec![PlaceRow::Item(PlaceItem::new(
        PlaceKind::Downloads,
        "Downloads",
        "",
        downloads.clone(),
        downloads.clone(),
    ))];
    let configured = vec![GotoEntrySpec::Builtin {
        destination: crate::config::BuiltinGoto::Downloads,
        key: 'd',
    }];

    let menu = build_goto_menu(&configured, &places);

    assert_eq!(menu.destination(0), Some(GotoDestination::Path(downloads)));

    fs::remove_dir_all(root).expect("temporary directory should be removed");
}
