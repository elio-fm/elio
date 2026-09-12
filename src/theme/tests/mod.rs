use super::{
    item_styling::{builtin_classify_browser_entry, builtin_classify_path},
    theme_loading::{DEFAULT_THEME_TOML, Theme, load_theme_from_disk, parse_color},
    *,
};
use crate::{
    file_classification::FileClass,
    filesystem::{Entry, EntryKind},
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

mod default_theme;
mod example_themes;
mod item_styling;
mod theme_loading;

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-theme-{label}-{unique}"))
}

fn specific_type_label(path: &Path, kind: EntryKind) -> Option<&'static str> {
    crate::file_classification::inspect_path(path, kind).specific_type_label
}

fn rgb(red: u8, green: u8, blue: u8) -> ratatui::style::Color {
    ratatui::style::Color::Rgb(red, green, blue)
}
