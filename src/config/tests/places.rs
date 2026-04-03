use super::{super::*, toml_string};

#[test]
fn config_defaults_places_to_builtin_sidebar_and_devices() {
    let config = Config::default_config();
    assert!(config.places.show_devices);
    assert_eq!(
        config.places.entries,
        vec![
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Home,
                icon: None,
            },
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Desktop,
                icon: None,
            },
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Documents,
                icon: None,
            },
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Downloads,
                icon: None,
            },
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Pictures,
                icon: None,
            },
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Music,
                icon: None,
            },
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Videos,
                icon: None,
            },
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Root,
                icon: None,
            },
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Trash,
                icon: None,
            },
        ]
    );
}

#[test]
fn config_can_customize_places_entries_and_hide_devices() {
    let projects = std::env::temp_dir().join("elio-places-projects");
    let projects_toml = toml_string(&projects.display().to_string());
    let config = Config::from_str(&format!(
        r#"
[places]
show_devices = false
entries = [
  "downloads",
  {{ title = "Projects", path = {} }},
  "trash",
]
"#,
        projects_toml
    ))
    .expect("config should parse");

    assert!(!config.places.show_devices);
    assert_eq!(
        config.places.entries,
        vec![
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Downloads,
                icon: None,
            },
            PlaceEntrySpec::Custom {
                title: "Projects".to_string(),
                path: projects.clone(),
                icon: None,
            },
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Trash,
                icon: None,
            },
        ]
    );
}

#[test]
fn config_places_skips_relative_custom_paths_without_failing_parse() {
    let config = Config::from_str(
        r#"
[places]
entries = [
  { title = "Projects", path = "projects" },
  "downloads",
]
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.places.entries,
        vec![PlaceEntrySpec::Builtin {
            place: BuiltinPlace::Downloads,
            icon: None,
        }]
    );
}

#[test]
fn config_places_skips_unknown_builtin_names_without_failing_parse() {
    let config = Config::from_str(
        r#"
[places]
entries = ["downloads", "workspace", "trash"]
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.places.entries,
        vec![
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Downloads,
                icon: None,
            },
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Trash,
                icon: None,
            },
        ]
    );
}

#[test]
fn config_places_can_customize_icons_for_builtin_and_custom_entries() {
    let projects = std::env::temp_dir().join("elio-places-projects-icons");
    let projects_toml = toml_string(&projects.display().to_string());
    let config = Config::from_str(&format!(
        r#"
[places]
entries = [
  {{ builtin = "downloads", icon = "D" }},
  {{ title = "Projects", path = {}, icon = "P" }},
]
"#,
        projects_toml
    ))
    .expect("config should parse");

    assert_eq!(
        config.places.entries,
        vec![
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Downloads,
                icon: Some("D".to_string()),
            },
            PlaceEntrySpec::Custom {
                title: "Projects".to_string(),
                path: projects.clone(),
                icon: Some("P".to_string()),
            },
        ]
    );
}

#[test]
fn config_places_accepts_builtin_object_form_without_icon() {
    let config = Config::from_str(
        r#"
[places]
entries = [
  { builtin = "downloads" },
  "trash",
]
"#,
    )
    .expect("config should parse");

    assert_eq!(
        config.places.entries,
        vec![
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Downloads,
                icon: None,
            },
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Trash,
                icon: None,
            },
        ]
    );
}

#[test]
fn config_places_ignores_invalid_icons_without_skipping_entries() {
    let projects = std::env::temp_dir().join("elio-places-invalid-icons");
    let projects_toml = toml_string(&projects.display().to_string());
    let config = Config::from_str(&format!(
        r#"
[places]
entries = [
  {{ builtin = "downloads", icon = "" }},
  {{ title = "Projects", path = {}, icon = "   " }},
]
"#,
        projects_toml
    ))
    .expect("config should parse");

    assert_eq!(
        config.places.entries,
        vec![
            PlaceEntrySpec::Builtin {
                place: BuiltinPlace::Downloads,
                icon: None,
            },
            PlaceEntrySpec::Custom {
                title: "Projects".to_string(),
                path: projects,
                icon: None,
            },
        ]
    );
}
