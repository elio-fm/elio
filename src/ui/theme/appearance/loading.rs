use super::Theme;
use crate::config;
use std::{fs, io, path::PathBuf};

pub(super) fn load_theme_from_disk() -> Theme {
    let base = Theme::selected_builtin_theme(config::theme_name());
    let Some(path) = theme_path() else {
        return base;
    };
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return base,
        Err(error) => {
            eprintln!(
                "elio: failed to read theme from {}: {error}",
                path.display()
            );
            return base;
        }
    };

    match Theme::apply_config_on(base.clone(), &contents) {
        Ok(theme) => theme,
        Err(error) => {
            eprintln!(
                "elio: failed to load theme from {}: {error}",
                path.display()
            );
            base
        }
    }
}

fn theme_path() -> Option<PathBuf> {
    config::config_dir().map(|dir| dir.join("theme.toml"))
}
