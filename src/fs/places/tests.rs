use super::resolution::{PlaceResolutionContext, build_sidebar_rows_with_context};
use crate::{
    config::{BuiltinPlace, PlaceEntrySpec, PlacesConfig},
    core::{SidebarItemKind, SidebarRow},
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-places-{label}-{unique}"))
}

fn context_for(root: &Path) -> PlaceResolutionContext {
    let home = root.join("home");
    let downloads = home.join("Downloads");
    let trash = root.join("trash");
    fs::create_dir_all(&downloads).expect("failed to create downloads");
    fs::create_dir_all(&trash).expect("failed to create trash");
    PlaceResolutionContext {
        home,
        desktop: None,
        documents: None,
        downloads: Some(downloads),
        pictures: None,
        music: None,
        videos: None,
        root: None,
        trash: Some(trash),
    }
}

#[test]
fn configured_places_order_and_semantic_kinds_are_preserved() {
    let root = temp_path("ordered-sidebar");
    let context = context_for(&root);
    let projects = root.join("projects");
    let places = PlacesConfig {
        show_devices: false,
        entries: vec![
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Downloads,
                icon: Some("D".to_string()),
            },
            PlaceEntrySpec::Custom {
                title: "Projects".to_string(),
                path: projects.clone(),
                icon: Some("P".to_string()),
            },
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Home,
                icon: None,
            },
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Trash,
                icon: None,
            },
        ],
    };

    let rows = build_sidebar_rows_with_context(&places, &context);
    let items = rows.iter().filter_map(SidebarRow::item).collect::<Vec<_>>();

    assert_eq!(items.len(), 4);
    assert_eq!(items[0].title, "Downloads");
    assert_eq!(items[0].kind, SidebarItemKind::Downloads);
    assert_eq!(items[0].icon, "D");
    assert_eq!(items[1].title, "Projects");
    assert_eq!(items[1].kind, SidebarItemKind::Custom);
    assert_eq!(items[1].icon, "P");
    assert_eq!(items[1].path, projects);
    assert_eq!(items[2].title, "Home");
    assert_eq!(items[2].kind, SidebarItemKind::Home);
    assert_eq!(items[3].title, "Trash");
    assert_eq!(items[3].kind, SidebarItemKind::Trash);
    assert!(rows.iter().all(|row| matches!(row, SidebarRow::Item(_))));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn missing_builtin_places_are_skipped_but_nonexistent_custom_places_stay_visible() {
    let root = temp_path("missing-builtins");
    let context = context_for(&root);
    let future_mount = root.join("mnt").join("camera");
    let places = PlacesConfig {
        show_devices: false,
        entries: vec![
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Desktop,
                icon: None,
            },
            PlaceEntrySpec::Custom {
                title: "Camera".to_string(),
                path: future_mount.clone(),
                icon: None,
            },
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Downloads,
                icon: None,
            },
        ],
    };

    let rows = build_sidebar_rows_with_context(&places, &context);
    let items = rows.iter().filter_map(SidebarRow::item).collect::<Vec<_>>();

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].title, "Camera");
    assert_eq!(items[0].kind, SidebarItemKind::Custom);
    assert_eq!(items[0].path, future_mount);
    assert_eq!(items[1].title, "Downloads");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn localized_builtin_places_show_resolved_folder_name() {
    let root = temp_path("localized-builtins");
    let home = root.join("home");
    let downloads = home.join("Descargas");
    fs::create_dir_all(&downloads).expect("failed to create downloads");
    let context = PlaceResolutionContext {
        home,
        desktop: None,
        documents: None,
        downloads: Some(downloads.clone()),
        pictures: None,
        music: None,
        videos: None,
        root: None,
        trash: None,
    };
    let places = PlacesConfig {
        show_devices: false,
        entries: vec![PlaceEntrySpec::Builtin {
            place: BuiltinPlace::Downloads,
            icon: None,
        }],
    };

    let rows = build_sidebar_rows_with_context(&places, &context);
    let items = rows.iter().filter_map(SidebarRow::item).collect::<Vec<_>>();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Descargas");
    assert_eq!(items[0].kind, SidebarItemKind::Downloads);
    assert_eq!(items[0].path, downloads);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn places_deduplicate_entries_by_resolved_path() {
    let root = temp_path("dedupe-sidebar");
    let context = context_for(&root);
    let places = PlacesConfig {
        show_devices: false,
        entries: vec![
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Home,
                icon: None,
            },
            PlaceEntrySpec::Custom {
                title: "Home 2".to_string(),
                path: context.home.clone(),
                icon: Some("H".to_string()),
            },
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Downloads,
                icon: None,
            },
            PlaceEntrySpec::Custom {
                title: "Downloads Alias".to_string(),
                path: context.home.join("Downloads").join("..").join("Downloads"),
                icon: Some("A".to_string()),
            },
        ],
    };

    let rows = build_sidebar_rows_with_context(&places, &context);
    let items = rows.iter().filter_map(SidebarRow::item).collect::<Vec<_>>();

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].title, "Home");
    assert_eq!(items[1].title, "Downloads");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
