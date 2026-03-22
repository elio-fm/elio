use super::{resolve::builtin_classify_path, rules::rgb, *};
use std::{
    env,
    ffi::OsString,
    fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

mod builtin;
mod classification;
mod examples;
mod overrides;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvVarGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvVarGuard {
    fn set_path(key: &'static str, value: &Path) -> Self {
        let original = env::var_os(key);
        unsafe {
            env::set_var(key, value);
        }
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match self.original.as_ref() {
            Some(value) => unsafe {
                env::set_var(self.key, value);
            },
            None => unsafe {
                env::remove_var(self.key);
            },
        }
    }
}

fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-theme-{label}-{unique}"))
}

fn write_temp_file(label: &str, file_name: &str, contents: &str) -> (PathBuf, PathBuf) {
    let root = temp_path(label);
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join(file_name);
    fs::write(&path, contents).expect("failed to write temp file");
    (root, path)
}

const GENERIC_DEV_DIRECTORIES: &[&str] = &[
    "node_modules",
    "tests",
    "test",
    "__tests__",
    "scripts",
    "build",
    "dist",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".astro",
    "assets",
    "coverage",
    "tmp",
    "temp",
    "out",
    "target",
    "bin",
    "lib",
    "vendor",
    "src",
    "config",
    "docs",
];

const ALTERNATE_EXAMPLE_THEME_NAMES: &[&str] = &[
    "default-light",
    "blush-light",
    "amber-dusk",
    "catppuccin-mocha",
    "tokyo-night",
    "navi",
    "neon-cherry",
];

fn alternate_example_theme_config(name: &str) -> &'static str {
    match name {
        "default-light" => include_str!("../../../../../examples/themes/default-light/theme.toml"),
        "blush-light" => include_str!("../../../../../examples/themes/blush-light/theme.toml"),
        "amber-dusk" => include_str!("../../../../../examples/themes/amber-dusk/theme.toml"),
        "catppuccin-mocha" => {
            include_str!("../../../../../examples/themes/catppuccin-mocha/theme.toml")
        }
        "tokyo-night" => include_str!("../../../../../examples/themes/tokyo-night/theme.toml"),
        "navi" => include_str!("../../../../../examples/themes/navi/theme.toml"),
        "neon-cherry" => include_str!("../../../../../examples/themes/neon-cherry/theme.toml"),
        _ => panic!("unknown alternate example theme fixture: {name}"),
    }
}

fn load_alternate_example_theme(name: &str) -> Theme {
    Theme::from_config_str(alternate_example_theme_config(name)).unwrap_or_else(|error| {
        panic!("{name} example theme should parse as a user theme: {error}")
    })
}

fn load_built_in_default_theme_asset() -> Theme {
    Theme::apply_config_on(Theme::base_theme(), DEFAULT_THEME_TOML)
        .expect("built-in default theme asset should parse")
}

fn write_theme_file(
    label: &str,
    contents: &str,
) -> (PathBuf, PathBuf, std::sync::MutexGuard<'static, ()>) {
    let guard = env_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let config_home = temp_path(label);
    let theme_dir = config_home.join("elio");
    fs::create_dir_all(&theme_dir).expect("failed to create theme config dir");
    let path = theme_dir.join("theme.toml");
    fs::write(&path, contents).expect("failed to write theme file");
    (config_home, path, guard)
}

fn assert_uses_normal_folder_color_for_generic_dev_directories(theme: &Theme, label: &str) {
    let normal_folder_color = theme
        .resolve(Path::new("projects"), EntryKind::Directory)
        .color;

    for directory in GENERIC_DEV_DIRECTORIES {
        let resolved = theme.resolve(Path::new(directory), EntryKind::Directory);
        assert_eq!(
            resolved.class,
            FileClass::Directory,
            "{label}: {directory} should resolve as a directory",
        );
        assert_eq!(
            resolved.color, normal_folder_color,
            "{label}: {directory} should use the normal folder color",
        );
    }
}
