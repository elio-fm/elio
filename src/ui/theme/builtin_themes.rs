pub(super) const DEFAULT_THEME_TOML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/themes/default/theme.toml"
));

/// The built-in theme used when config.toml does not select one.
pub(super) const DEFAULT_THEME_NAME: &str = "default";

macro_rules! bundled_theme {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/themes/",
                $name,
                "/theme.toml"
            )),
        )
    };
}

/// Built-in themes selectable by name through the top-level `theme` key in
/// config.toml. Each TOML layers on top of the default theme, exactly like a
/// user `theme.toml` would; `"default"` selects the default theme itself.
/// These are the bundled themes from `examples/themes/`.
const BUILTIN_THEME_OVERRIDES: &[(&str, &str)] = &[
    bundled_theme!("amber-dusk"),
    bundled_theme!("blush-light"),
    bundled_theme!("catppuccin-mocha"),
    bundled_theme!("default-light"),
    bundled_theme!("navi"),
    bundled_theme!("neon-cherry"),
    bundled_theme!("terminal-ansi"),
    bundled_theme!("tokyo-night"),
    bundled_theme!("transparent"),
];

/// The override TOML for a named built-in theme, or `None` for unknown names.
/// `"default"` is handled by the caller (there is nothing to layer).
pub(super) fn builtin_theme_overrides(name: &str) -> Option<&'static str> {
    BUILTIN_THEME_OVERRIDES
        .iter()
        .find(|(theme_name, _)| *theme_name == name)
        .map(|(_, overrides)| *overrides)
}

/// Comma-separated list of every selectable theme name, for error messages.
pub(super) fn available_theme_names() -> String {
    std::iter::once(DEFAULT_THEME_NAME)
        .chain(BUILTIN_THEME_OVERRIDES.iter().map(|(name, _)| *name))
        .collect::<Vec<_>>()
        .join(", ")
}
