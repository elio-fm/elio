use super::Theme;
use crate::config;
use std::{fs, io, path::PathBuf};

pub(super) fn load_theme_from_disk() -> Theme {
    let Some(path) = theme_path() else {
        return Theme::default_theme();
    };
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Theme::default_theme(),
        Err(error) => {
            eprintln!(
                "elio: failed to read theme from {}: {error}",
                path.display()
            );
            return Theme::default_theme();
        }
    };

    match Theme::from_config_str(&contents) {
        Ok(theme) => theme,
        Err(error) => {
            eprintln!(
                "elio: failed to load theme from {}: {error}",
                path.display()
            );
            Theme::default_theme()
        }
    }
}

fn theme_path() -> Option<PathBuf> {
    config::config_dir().map(|dir| dir.join("theme.toml"))
}
