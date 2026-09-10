use std::{ffi::OsString, path::PathBuf};

/// Returns the ordered list of XDG base data directories:
/// `XDG_DATA_HOME` first, then each entry in `XDG_DATA_DIRS`.
/// Falls back to spec defaults (`~/.local/share` and `/usr/local/share:/usr/share`)
/// when the environment variables are unset.
pub(super) fn data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let context = crate::elevated_session::context();

    if let Some(data_home) =
        data_home_for_context(context, crate::elevated_session::env_var("XDG_DATA_HOME"))
        && !data_home.as_os_str().is_empty()
    {
        dirs.push(data_home);
    }

    for entry in crate::elevated_session::env_var("XDG_DATA_DIRS")
        .and_then(|value| value.into_string().ok())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string())
        .split(':')
        .filter(|entry| !entry.is_empty())
    {
        dirs.push(PathBuf::from(entry));
    }

    dirs
}

fn data_home_for_context(
    context: &crate::elevated_session::InvocationContext,
    normal_data_home: Option<OsString>,
) -> Option<PathBuf> {
    match context {
        crate::elevated_session::InvocationContext::Normal
        | crate::elevated_session::InvocationContext::RootSession => normal_data_home
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".local/share"))),
        crate::elevated_session::InvocationContext::Elevated(user) => user
            .xdg_data_home
            .clone()
            .or_else(|| Some(user.home.join(".local/share"))),
        crate::elevated_session::InvocationContext::ElevatedUnresolved => None,
    }
}

pub(super) fn invoking_home_dir() -> Option<PathBuf> {
    invoking_home_dir_for_context(crate::elevated_session::context())
}

fn invoking_home_dir_for_context(
    context: &crate::elevated_session::InvocationContext,
) -> Option<PathBuf> {
    match context {
        crate::elevated_session::InvocationContext::Normal
        | crate::elevated_session::InvocationContext::RootSession => dirs::home_dir(),
        crate::elevated_session::InvocationContext::Elevated(user) => Some(user.home.clone()),
        crate::elevated_session::InvocationContext::ElevatedUnresolved => None,
    }
}

pub(super) fn invoking_config_home() -> Option<PathBuf> {
    config_home_for_context(
        crate::elevated_session::context(),
        crate::elevated_session::env_var("XDG_CONFIG_HOME"),
    )
}

fn config_home_for_context(
    context: &crate::elevated_session::InvocationContext,
    normal_config_home: Option<OsString>,
) -> Option<PathBuf> {
    match context {
        crate::elevated_session::InvocationContext::Normal
        | crate::elevated_session::InvocationContext::RootSession => normal_config_home
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".config"))),
        crate::elevated_session::InvocationContext::Elevated(user) => user
            .xdg_config_home
            .clone()
            .or_else(|| Some(user.home.join(".config"))),
        crate::elevated_session::InvocationContext::ElevatedUnresolved => None,
    }
}

/// Returns the desktop names from `$XDG_CURRENT_DESKTOP` (colon-separated,
/// original case). Empty when the variable is unset or empty.
pub(super) fn current_desktops() -> Vec<String> {
    crate::elevated_session::env_var("XDG_CURRENT_DESKTOP")
        .and_then(|value| value.into_string().ok())
        .unwrap_or_default()
        .split(':')
        .filter(|desktop| !desktop.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
#[path = "tests/xdg_environment.rs"]
mod tests;
