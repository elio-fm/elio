use super::{resolve::builtin_classify_path, rules::rgb, *};
use std::{
    env,
    ffi::OsString,
    fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

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
        "default-light" => include_str!("../../../../examples/themes/default-light/theme.toml"),
        "blush-light" => include_str!("../../../../examples/themes/blush-light/theme.toml"),
        "amber-dusk" => include_str!("../../../../examples/themes/amber-dusk/theme.toml"),
        "catppuccin-mocha" => {
            include_str!("../../../../examples/themes/catppuccin-mocha/theme.toml")
        }
        "tokyo-night" => include_str!("../../../../examples/themes/tokyo-night/theme.toml"),
        "navi" => include_str!("../../../../examples/themes/navi/theme.toml"),
        "neon-cherry" => include_str!("../../../../examples/themes/neon-cherry/theme.toml"),
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

#[test]
fn built_in_default_theme_asset_matches_runtime_default_theme() {
    let built_in_asset = load_built_in_default_theme_asset();
    let runtime_default = Theme::default_theme();

    assert_eq!(built_in_asset.palette.bg, runtime_default.palette.bg);
    assert_eq!(
        built_in_asset.palette.selected_bg,
        runtime_default.palette.selected_bg
    );
    assert_eq!(
        built_in_asset.preview.code.keyword,
        runtime_default.preview.code.keyword,
    );
    assert_eq!(
        built_in_asset.preview.code.function,
        runtime_default.preview.code.function,
    );

    for (path, kind) in [
        ("projects", EntryKind::Directory),
        ("Downloads", EntryKind::Directory),
        ("Cargo.toml", EntryKind::File),
        ("Cargo.lock", EntryKind::File),
        ("README.md", EntryKind::File),
        ("main.rs", EntryKind::File),
    ] {
        let built_in = built_in_asset.resolve(Path::new(path), kind);
        let runtime = runtime_default.resolve(Path::new(path), kind);
        assert_eq!(
            built_in.class, runtime.class,
            "{path} should keep its class"
        );
        assert_eq!(built_in.icon, runtime.icon, "{path} should keep its icon");
        assert_eq!(
            built_in.color, runtime.color,
            "{path} should keep its color"
        );
    }
}

#[test]
fn load_theme_from_disk_reads_theme_file_from_xdg_config_home() {
    let (config_home, path, _guard) = write_theme_file(
        "load-theme-from-disk",
        r##"
[classes.code]
icon = "X"
color = "#112233"

[directories.projects]
icon = "P"
color = "#334455"

[preview.code]
keyword = "#abcdef"
"##,
    );
    let _xdg = EnvVarGuard::set_path("XDG_CONFIG_HOME", &config_home);

    let theme = load_theme_from_disk();

    assert_eq!(theme.preview.code.keyword, rgb(0xab, 0xcd, 0xef));
    assert_eq!(theme.classes.get(&FileClass::Code).unwrap().icon, "X");
    assert_eq!(
        theme.classes.get(&FileClass::Code).unwrap().color,
        rgb(0x11, 0x22, 0x33)
    );
    let projects = theme.resolve(Path::new("projects"), EntryKind::Directory);
    assert_eq!(projects.class, FileClass::Directory);
    assert_eq!(projects.icon, "P");
    assert_eq!(projects.color, rgb(0x33, 0x44, 0x55));

    fs::remove_file(path).expect("failed to remove theme file");
    fs::remove_dir_all(config_home).expect("failed to remove config root");
}

#[test]
fn load_theme_from_disk_falls_back_to_default_theme_for_invalid_theme_file() {
    let (config_home, path, _guard) = write_theme_file(
        "load-theme-invalid",
        r##"
[preview.code]
keyword = "#12"
"##,
    );
    let _xdg = EnvVarGuard::set_path("XDG_CONFIG_HOME", &config_home);

    let theme = load_theme_from_disk();
    let default_theme = Theme::default_theme();

    assert_eq!(theme.palette.bg, default_theme.palette.bg);
    assert_eq!(
        theme.preview.code.keyword,
        default_theme.preview.code.keyword
    );
    assert_eq!(
        theme.resolve(Path::new("Cargo.lock"), EntryKind::File).icon,
        default_theme
            .resolve(Path::new("Cargo.lock"), EntryKind::File)
            .icon,
    );

    fs::remove_file(path).expect("failed to remove theme file");
    fs::remove_dir_all(config_home).expect("failed to remove config root");
}

#[test]
fn exact_file_rules_override_extension_defaults() {
    let theme = Theme::default_theme();
    let resolved = theme.resolve(Path::new("Cargo.lock"), EntryKind::File);
    assert_eq!(resolved.class, FileClass::Data);
    assert_eq!(resolved.icon, "󰈡");
}

#[test]
fn theme_file_overrides_class_icon_and_palette() {
    let theme = Theme::from_config_str(
        r##"
[classes.code]
icon = "X"
color = "#112233"

[files."special.rs"]
icon = "Y"
color = "#abcdef"
class = "document"
"##,
    )
    .expect("theme should parse");

    let resolved = theme.resolve(Path::new("special.rs"), EntryKind::File);
    assert_eq!(resolved.class, FileClass::Document);
    assert_eq!(resolved.icon, "Y");
    assert_eq!(resolved.color, rgb(0xab, 0xcd, 0xef));
}

#[test]
fn extension_rules_can_be_overridden_from_config() {
    let theme = Theme::from_config_str(
        r##"
[extensions.lock]
class = "data"
icon = "L"
"##,
    )
    .expect("theme should parse");

    let resolved = theme.resolve(Path::new("custom.lock"), EntryKind::File);
    assert_eq!(resolved.class, FileClass::Data);
    assert_eq!(resolved.icon, "L");
}

#[test]
fn directory_rules_can_be_overridden_from_config() {
    let theme = Theme::from_config_str(
        r##"
[directories.docs]
class = "document"
icon = "D"
color = "#102030"
"##,
    )
    .expect("theme should parse");

    let resolved = theme.resolve(Path::new("docs"), EntryKind::Directory);
    assert_eq!(resolved.class, FileClass::Document);
    assert_eq!(resolved.icon, "D");
    assert_eq!(resolved.color, rgb(0x10, 0x20, 0x30));
}

#[test]
fn generic_lock_files_use_file_lock_icon() {
    let theme = Theme::default_theme();
    let resolved = theme.resolve(Path::new("custom.lock"), EntryKind::File);
    assert_eq!(resolved.class, FileClass::Data);
    assert_eq!(resolved.icon, "󰈡");
    assert_eq!(resolved.color, rgb(89, 222, 148));

    let cargo = theme.resolve(Path::new("Cargo.lock"), EntryKind::File);
    assert_eq!(cargo.icon, "󰈡");

    let package_lock = theme.resolve(Path::new("package-lock.json"), EntryKind::File);
    assert_eq!(package_lock.icon, "󰈡");

    let poetry = theme.resolve(Path::new("poetry.lock"), EntryKind::File);
    assert_eq!(poetry.icon, "󰈡");
}

#[test]
fn code_preview_colors_can_be_overridden_from_config() {
    let theme = Theme::from_config_str(
        r##"
[preview.code]
keyword = "#123456"
function = "#abcdef"
macro = "#fedcba"
"##,
    )
    .expect("theme should parse");

    assert_eq!(theme.preview.code.keyword, rgb(0x12, 0x34, 0x56));
    assert_eq!(theme.preview.code.function, rgb(0xab, 0xcd, 0xef));
    assert_eq!(theme.preview.code.r#macro, rgb(0xfe, 0xdc, 0xba));
}

#[test]
fn unknown_rule_classes_are_rejected_during_theme_parsing() {
    let error = match Theme::from_config_str(
        r##"
[extensions.rs]
class = "not-a-real-class"
"##,
    ) {
        Ok(_) => panic!("theme parsing should reject unknown classes"),
        Err(error) => error,
    };

    assert!(
        error.to_string().contains("unknown class"),
        "unexpected parse error: {error}",
    );
}

#[test]
fn blush_light_example_theme_parses_as_user_theme_and_applies_custom_icon_and_code_colors() {
    let theme = load_alternate_example_theme("blush-light");

    assert_eq!(theme.preview.code.keyword, rgb(0xd8, 0x63, 0x92));
    assert_eq!(theme.preview.code.function, rgb(0x8f, 0x71, 0xbf));

    let directory = theme.resolve(Path::new("projects"), EntryKind::Directory);
    assert_eq!(directory.class, FileClass::Directory);
    assert_eq!(directory.icon, "󰉋");
    assert_eq!(directory.color, rgb(0xd4, 0x6b, 0x93));

    let rust = theme.resolve(Path::new("main.rs"), EntryKind::File);
    assert_eq!(rust.class, FileClass::Code);
    assert_eq!(rust.icon, "");
    assert_eq!(rust.color, rgb(0xca, 0x81, 0x68));

    let readme = theme.resolve(Path::new("README.md"), EntryKind::File);
    assert_eq!(readme.class, FileClass::Document);
    assert_eq!(readme.icon, "");
    assert_eq!(readme.color, rgb(0xbb, 0x90, 0x7b));
}

#[test]
fn default_light_example_theme_parses_as_user_theme_and_preserves_default_icon_mappings() {
    let theme = load_alternate_example_theme("default-light");

    assert_eq!(theme.palette.bg, rgb(0xef, 0xf2, 0xf5));
    assert_eq!(theme.preview.code.keyword, rgb(0x7a, 0xae, 0xff));
    assert_eq!(theme.preview.code.function, rgb(0x46, 0x9f, 0xc3));
    assert_eq!(theme.preview.code.string, rgb(0x4d, 0x92, 0x79));
    assert_eq!(theme.preview.code.r#type, rgb(0x8a, 0x74, 0xc8));

    let directory = theme.resolve(Path::new("projects"), EntryKind::Directory);
    assert_eq!(directory.class, FileClass::Directory);
    assert_eq!(directory.icon, "󰉋");
    assert_eq!(directory.color, rgb(0x5b, 0xa8, 0xff));

    let downloads = theme.resolve(Path::new("Downloads"), EntryKind::Directory);
    assert_eq!(downloads.class, FileClass::Directory);
    assert_eq!(downloads.icon, "󰉍");
    assert_eq!(downloads.color, rgb(0xb9, 0x97, 0x3e));

    let pictures = theme.resolve(Path::new("Pictures"), EntryKind::Directory);
    assert_eq!(pictures.class, FileClass::Directory);
    assert_eq!(pictures.icon, "󰉏");
    assert_eq!(pictures.color, rgb(0x55, 0xa7, 0x9e));

    let music = theme.resolve(Path::new("Music"), EntryKind::Directory);
    assert_eq!(music.class, FileClass::Directory);
    assert_eq!(music.icon, "󱍙");
    assert_eq!(music.color, rgb(0x9a, 0x81, 0xcf));

    let src = theme.resolve(Path::new("src"), EntryKind::Directory);
    assert_eq!(src.class, FileClass::Directory);
    assert_eq!(src.icon, "󰉋");
    assert_eq!(src.color, rgb(0x5b, 0xa8, 0xff));

    let shell = theme.resolve(Path::new("deploy.sh"), EntryKind::File);
    assert_eq!(shell.class, FileClass::Code);
    assert_eq!(shell.icon, "");
    assert_eq!(shell.color, rgb(0x69, 0x78, 0x8b));

    let rust = theme.resolve(Path::new("main.rs"), EntryKind::File);
    assert_eq!(rust.class, FileClass::Code);
    assert_eq!(rust.icon, "");
    assert_eq!(rust.color, rgb(0xb8, 0x74, 0x45));

    let package = theme.resolve(Path::new("package.json"), EntryKind::File);
    assert_eq!(package.class, FileClass::Config);
    assert_eq!(package.icon, "󰏗");
    assert_eq!(package.color, rgb(0x7d, 0xb0, 0xff));

    let readme = theme.resolve(Path::new("README.md"), EntryKind::File);
    assert_eq!(readme.class, FileClass::Document);
    assert_eq!(readme.icon, "");
    assert_eq!(readme.color, rgb(0xab, 0x97, 0x7a));

    let turbo = theme.resolve(Path::new("turbo.json"), EntryKind::File);
    assert_eq!(turbo.class, FileClass::Config);
    assert_eq!(turbo.icon, "󰐷");
    assert_eq!(turbo.color, rgb(0x72, 0x81, 0x95));
}

#[test]
fn amber_dusk_example_theme_parses_as_user_theme_and_applies_warm_dark_palette() {
    let theme = load_alternate_example_theme("amber-dusk");

    assert_eq!(theme.palette.bg, rgb(0x12, 0x0f, 0x0d));
    assert_eq!(theme.preview.code.keyword, rgb(0xcf, 0x98, 0x51));
    assert_eq!(theme.preview.code.function, rgb(0x7f, 0xa7, 0xa5));

    let directory = theme.resolve(Path::new("projects"), EntryKind::Directory);
    assert_eq!(directory.class, FileClass::Directory);
    assert_eq!(directory.icon, "󰉋");
    assert_eq!(directory.color, rgb(0xcf, 0x9c, 0x67));

    let downloads = theme.resolve(Path::new("Downloads"), EntryKind::Directory);
    assert_eq!(downloads.class, FileClass::Directory);
    assert_eq!(downloads.icon, "󰉍");
    assert_eq!(downloads.color, rgb(0xd4, 0xa4, 0x66));

    let src = theme.resolve(Path::new("src"), EntryKind::Directory);
    assert_eq!(src.class, FileClass::Directory);
    assert_eq!(src.icon, "󰉋");
    assert_eq!(src.color, rgb(0xcf, 0x9c, 0x67));

    let vendor = theme.resolve(Path::new("vendor"), EntryKind::Directory);
    assert_eq!(vendor.class, FileClass::Directory);
    assert_eq!(vendor.icon, "󰉋");
    assert_eq!(vendor.color, rgb(0xcf, 0x9c, 0x67));

    let rust = theme.resolve(Path::new("main.rs"), EntryKind::File);
    assert_eq!(rust.class, FileClass::Code);
    assert_eq!(rust.icon, "");
    assert_eq!(rust.color, rgb(0xc5, 0x8a, 0x5e));
}

#[test]
fn catppuccin_mocha_example_theme_parses_as_user_theme_and_applies_palette_consistently() {
    let theme = load_alternate_example_theme("catppuccin-mocha");

    assert_eq!(theme.palette.bg, rgb(0x1e, 0x1e, 0x2e));
    assert_eq!(theme.palette.selected_bg, rgb(0x45, 0x47, 0x5a));
    assert_ne!(theme.palette.selected_bg, theme.palette.surface);
    assert_eq!(theme.preview.code.keyword, rgb(0xcb, 0xa6, 0xf7));
    assert_eq!(theme.preview.code.function, rgb(0x89, 0xb4, 0xfa));
    assert_eq!(theme.preview.code.string, rgb(0xa6, 0xe3, 0xa1));
    assert_eq!(theme.preview.code.r#type, rgb(0xf9, 0xe2, 0xaf));

    let directory = theme.resolve(Path::new("projects"), EntryKind::Directory);
    assert_eq!(directory.class, FileClass::Directory);
    assert_eq!(directory.icon, "󰉋");
    assert_eq!(directory.color, rgb(0x89, 0xb4, 0xfa));

    let downloads = theme.resolve(Path::new("Downloads"), EntryKind::Directory);
    assert_eq!(downloads.class, FileClass::Directory);
    assert_eq!(downloads.icon, "󰉍");
    assert_eq!(downloads.color, rgb(0xf9, 0xe2, 0xaf));

    let pictures = theme.resolve(Path::new("Pictures"), EntryKind::Directory);
    assert_eq!(pictures.class, FileClass::Directory);
    assert_eq!(pictures.icon, "󰉏");
    assert_eq!(pictures.color, rgb(0x94, 0xe2, 0xd5));

    let music = theme.resolve(Path::new("Music"), EntryKind::Directory);
    assert_eq!(music.class, FileClass::Directory);
    assert_eq!(music.icon, "󱍙");
    assert_eq!(music.color, rgb(0xcb, 0xa6, 0xf7));

    let src = theme.resolve(Path::new("src"), EntryKind::Directory);
    assert_eq!(src.class, FileClass::Directory);
    assert_eq!(src.icon, "󰉋");
    assert_eq!(src.color, rgb(0x89, 0xb4, 0xfa));

    let rust = theme.resolve(Path::new("main.rs"), EntryKind::File);
    assert_eq!(rust.class, FileClass::Code);
    assert_eq!(rust.icon, "");
    assert_eq!(rust.color, rgb(0xfa, 0xb3, 0x87));

    let package = theme.resolve(Path::new("package.json"), EntryKind::File);
    assert_eq!(package.class, FileClass::Config);
    assert_eq!(package.icon, "󰏗");
    assert_eq!(package.color, rgb(0x89, 0xb4, 0xfa));

    let readme = theme.resolve(Path::new("README.md"), EntryKind::File);
    assert_eq!(readme.class, FileClass::Document);
    assert_eq!(readme.icon, "");
    assert_eq!(readme.color, rgb(0xf9, 0xe2, 0xaf));
}

#[test]
fn tokyo_night_example_theme_parses_as_user_theme_and_applies_palette_consistently() {
    let theme = load_alternate_example_theme("tokyo-night");

    assert_eq!(theme.palette.bg, rgb(0x1a, 0x1b, 0x26));
    assert_eq!(theme.preview.code.keyword, rgb(0xbb, 0x9a, 0xf7));
    assert_eq!(theme.preview.code.function, rgb(0x7d, 0xcf, 0xff));
    assert_eq!(theme.preview.code.string, rgb(0x9e, 0xce, 0x6a));
    assert_eq!(theme.preview.code.r#type, rgb(0xe0, 0xaf, 0x68));

    let directory = theme.resolve(Path::new("projects"), EntryKind::Directory);
    assert_eq!(directory.class, FileClass::Directory);
    assert_eq!(directory.icon, "󰉋");
    assert_eq!(directory.color, rgb(0x7a, 0xa2, 0xf7));

    let downloads = theme.resolve(Path::new("Downloads"), EntryKind::Directory);
    assert_eq!(downloads.class, FileClass::Directory);
    assert_eq!(downloads.icon, "󰉍");
    assert_eq!(downloads.color, rgb(0xe0, 0xaf, 0x68));

    let pictures = theme.resolve(Path::new("Pictures"), EntryKind::Directory);
    assert_eq!(pictures.class, FileClass::Directory);
    assert_eq!(pictures.icon, "󰉏");
    assert_eq!(pictures.color, rgb(0x73, 0xda, 0xca));

    let music = theme.resolve(Path::new("Music"), EntryKind::Directory);
    assert_eq!(music.class, FileClass::Directory);
    assert_eq!(music.icon, "󱍙");
    assert_eq!(music.color, rgb(0xbb, 0x9a, 0xf7));

    let src = theme.resolve(Path::new("src"), EntryKind::Directory);
    assert_eq!(src.class, FileClass::Directory);
    assert_eq!(src.icon, "󰉋");
    assert_eq!(src.color, rgb(0x7a, 0xa2, 0xf7));

    let rust = theme.resolve(Path::new("main.rs"), EntryKind::File);
    assert_eq!(rust.class, FileClass::Code);
    assert_eq!(rust.icon, "");
    assert_eq!(rust.color, rgb(0xff, 0x9e, 0x64));

    let package = theme.resolve(Path::new("package.json"), EntryKind::File);
    assert_eq!(package.class, FileClass::Config);
    assert_eq!(package.icon, "󰏗");
    assert_eq!(package.color, rgb(0x7a, 0xa2, 0xf7));

    let readme = theme.resolve(Path::new("README.md"), EntryKind::File);
    assert_eq!(readme.class, FileClass::Document);
    assert_eq!(readme.icon, "");
    assert_eq!(readme.color, rgb(0xe0, 0xaf, 0x68));
}

#[test]
fn built_in_default_theme_uses_normal_folder_color_for_generic_dev_directories() {
    let theme = load_built_in_default_theme_asset();
    assert_uses_normal_folder_color_for_generic_dev_directories(&theme, "built-in default");
}

#[test]
fn alternate_example_themes_use_normal_folder_color_for_generic_dev_directories() {
    for label in ALTERNATE_EXAMPLE_THEME_NAMES {
        let theme = load_alternate_example_theme(label);
        assert_uses_normal_folder_color_for_generic_dev_directories(&theme, label);
    }
}

#[test]
fn default_theme_assigns_specific_icons_for_common_dev_paths() {
    let theme = Theme::default_theme();

    let ts = theme.resolve(Path::new("main.ts"), EntryKind::File);
    assert_eq!(ts.icon, "");

    let json = theme.resolve(Path::new("data.json"), EntryKind::File);
    assert_eq!(json.class, FileClass::Config);
    assert_eq!(json.icon, "");
    assert_eq!(json.color, rgb(125, 176, 255));

    let package = theme.resolve(Path::new("package.json"), EntryKind::File);
    assert_eq!(package.icon, "󰏗");

    let modules = theme.resolve(Path::new("node_modules"), EntryKind::Directory);
    assert_eq!(modules.icon, "󰏗");

    let docs = theme.resolve(Path::new("docs"), EntryKind::Directory);
    assert_eq!(docs.class, FileClass::Directory);
    assert_eq!(docs.icon, "󱧷");
    assert_eq!(docs.color, rgb(91, 168, 255));

    let bin = theme.resolve(Path::new("bin"), EntryKind::Directory);
    assert_eq!(bin.class, FileClass::Directory);
    assert_eq!(bin.icon, "󱁿");
    assert_eq!(bin.color, rgb(91, 168, 255));

    let lib = theme.resolve(Path::new("lib"), EntryKind::Directory);
    assert_eq!(lib.class, FileClass::Directory);
    assert_eq!(lib.icon, "󰉋");
    assert_eq!(lib.color, rgb(91, 168, 255));

    let target = theme.resolve(Path::new("target"), EntryKind::Directory);
    assert_eq!(target.class, FileClass::Directory);
    assert_eq!(target.icon, "󱧽");
    assert_eq!(target.color, rgb(91, 168, 255));

    let dist = theme.resolve(Path::new("dist"), EntryKind::Directory);
    assert_eq!(dist.class, FileClass::Directory);
    assert_eq!(dist.icon, "󰉋");
    assert_eq!(dist.color, rgb(91, 168, 255));

    let out = theme.resolve(Path::new("out"), EntryKind::Directory);
    assert_eq!(out.class, FileClass::Directory);
    assert_eq!(out.icon, "󰉋");
    assert_eq!(out.color, rgb(91, 168, 255));

    let xml = theme.resolve(Path::new("config.xml"), EntryKind::File);
    assert_eq!(xml.class, FileClass::Code);
    assert_eq!(xml.icon, "󰗀");
    assert_eq!(xml.color, rgb(179, 140, 255));

    let csharp = theme.resolve(Path::new("Program.cs"), EntryKind::File);
    assert_eq!(csharp.class, FileClass::Code);
    assert_eq!(csharp.icon, "󰌛");
    assert_eq!(csharp.color, rgb(104, 179, 120));

    let csharp_script = theme.resolve(Path::new("Program.csx"), EntryKind::File);
    assert_eq!(csharp_script.class, FileClass::Code);
    assert_eq!(csharp_script.icon, "󰌛");
    assert_eq!(csharp_script.color, rgb(104, 179, 120));

    let dart = theme.resolve(Path::new("main.dart"), EntryKind::File);
    assert_eq!(dart.class, FileClass::Code);
    assert_eq!(dart.icon, "");
    assert_eq!(dart.color, rgb(56, 213, 255));

    let fortran = theme.resolve(Path::new("solver.f90"), EntryKind::File);
    assert_eq!(fortran.class, FileClass::Code);
    assert_eq!(fortran.icon, "󱈚");
    assert_eq!(fortran.color, rgb(115, 79, 150));

    let fortran_pp = theme.resolve(Path::new("solver.fpp"), EntryKind::File);
    assert_eq!(fortran_pp.class, FileClass::Code);
    assert_eq!(fortran_pp.icon, "󱈚");
    assert_eq!(fortran_pp.color, rgb(115, 79, 150));

    let cobol = theme.resolve(Path::new("ledger.cbl"), EntryKind::File);
    assert_eq!(cobol.class, FileClass::Code);
    assert_eq!(cobol.icon, "");
    assert_eq!(cobol.color, rgb(0, 92, 165));

    let cobol_copybook = theme.resolve(Path::new("customer.cpy"), EntryKind::File);
    assert_eq!(cobol_copybook.class, FileClass::Code);
    assert_eq!(cobol_copybook.icon, "");
    assert_eq!(cobol_copybook.color, rgb(0, 92, 165));

    let elixir = theme.resolve(Path::new("main.ex"), EntryKind::File);
    assert_eq!(elixir.class, FileClass::Code);
    assert_eq!(elixir.icon, "");
    assert_eq!(elixir.color, rgb(155, 143, 199));

    let elixir_script = theme.resolve(Path::new("mix.exs"), EntryKind::File);
    assert_eq!(elixir_script.class, FileClass::Code);
    assert_eq!(elixir_script.icon, "");
    assert_eq!(elixir_script.color, rgb(155, 143, 199));

    let clojure = theme.resolve(Path::new("core.clj"), EntryKind::File);
    assert_eq!(clojure.class, FileClass::Code);
    assert_eq!(clojure.icon, "");
    assert_eq!(clojure.color, rgb(128, 176, 92));

    let clojurescript = theme.resolve(Path::new("app.cljs"), EntryKind::File);
    assert_eq!(clojurescript.class, FileClass::Code);
    assert_eq!(clojurescript.icon, "");
    assert_eq!(clojurescript.color, rgb(128, 176, 92));

    let clojure_data = theme.resolve(Path::new("deps.edn"), EntryKind::File);
    assert_eq!(clojure_data.class, FileClass::Config);
    assert_eq!(clojure_data.icon, "");
    assert_eq!(clojure_data.color, rgb(128, 176, 92));

    let leiningen = theme.resolve(Path::new("project.clj"), EntryKind::File);
    assert_eq!(leiningen.class, FileClass::Config);
    assert_eq!(leiningen.icon, "");
    assert_eq!(leiningen.color, rgb(128, 176, 92));

    let powershell = theme.resolve(Path::new("build.ps1"), EntryKind::File);
    assert_eq!(powershell.class, FileClass::Code);
    assert_eq!(powershell.icon, "󰨊");
    assert_eq!(powershell.color, rgb(95, 153, 219));

    let powershell_module = theme.resolve(Path::new("ElioTools.psm1"), EntryKind::File);
    assert_eq!(powershell_module.class, FileClass::Code);
    assert_eq!(powershell_module.icon, "󰨊");
    assert_eq!(powershell_module.color, rgb(95, 153, 219));

    let powershell_data = theme.resolve(Path::new("ElioTools.psd1"), EntryKind::File);
    assert_eq!(powershell_data.class, FileClass::Config);
    assert_eq!(powershell_data.icon, "󰨊");
    assert_eq!(powershell_data.color, rgb(95, 153, 219));

    let shell = theme.resolve(Path::new("deploy.sh"), EntryKind::File);
    assert_eq!(shell.class, FileClass::Code);
    assert_eq!(shell.icon, "");
    assert_eq!(shell.color, rgb(214, 222, 240));

    let bash = theme.resolve(Path::new("profile.bash"), EntryKind::File);
    assert_eq!(bash.class, FileClass::Code);
    assert_eq!(bash.icon, "");
    assert_eq!(bash.color, rgb(214, 222, 240));

    let zsh = theme.resolve(Path::new("prompt.zsh"), EntryKind::File);
    assert_eq!(zsh.class, FileClass::Code);
    assert_eq!(zsh.icon, "");
    assert_eq!(zsh.color, rgb(214, 222, 240));

    let fish = theme.resolve(Path::new("config.fish"), EntryKind::File);
    assert_eq!(fish.class, FileClass::Code);
    assert_eq!(fish.icon, "");
    assert_eq!(fish.color, rgb(214, 222, 240));
}

#[test]
fn default_theme_assigns_icons_for_new_language_support() {
    let theme = Theme::default_theme();

    let dockerfile = theme.resolve(Path::new("Dockerfile"), EntryKind::File);
    assert_eq!(dockerfile.class, FileClass::Config);
    assert_eq!(dockerfile.icon, "󰡨");

    let sql = theme.resolve(Path::new("schema.sql"), EntryKind::File);
    assert_eq!(sql.icon, "");

    let diff = theme.resolve(Path::new("changes.diff"), EntryKind::File);
    assert_eq!(diff.class, FileClass::Code);
    assert_eq!(diff.icon, "");

    let terraform = theme.resolve(Path::new("main.tf"), EntryKind::File);
    assert_eq!(terraform.class, FileClass::Config);
    assert_eq!(terraform.icon, "");

    let hcl = theme.resolve(Path::new("terraform.lock.hcl"), EntryKind::File);
    assert_eq!(hcl.class, FileClass::Config);
    assert_eq!(hcl.icon, "");

    let groovy = theme.resolve(Path::new("build.gradle"), EntryKind::File);
    assert_eq!(groovy.class, FileClass::Config);
    assert_eq!(groovy.icon, "");

    let scala = theme.resolve(Path::new("build.sbt"), EntryKind::File);
    assert_eq!(scala.class, FileClass::Config);
    assert_eq!(scala.icon, "");

    let perl = theme.resolve(Path::new("script.pl"), EntryKind::File);
    assert_eq!(perl.class, FileClass::Code);
    assert_eq!(perl.icon, "");

    let haskell = theme.resolve(Path::new("Main.hs"), EntryKind::File);
    assert_eq!(haskell.class, FileClass::Code);
    assert_eq!(haskell.icon, "");

    let julia = theme.resolve(Path::new("main.jl"), EntryKind::File);
    assert_eq!(julia.class, FileClass::Code);
    assert_eq!(julia.icon, "");

    let r = theme.resolve(Path::new("analysis.r"), EntryKind::File);
    assert_eq!(r.class, FileClass::Code);
    assert_eq!(r.icon, "󰟔");

    let just = theme.resolve(Path::new("Justfile"), EntryKind::File);
    assert_eq!(just.class, FileClass::Config);
    assert_eq!(just.icon, "");

    let ziggy = theme.resolve(Path::new("config.ziggy"), EntryKind::File);
    assert_eq!(ziggy.class, FileClass::Config);
    assert_eq!(ziggy.icon, "");

    let fortran = theme.resolve(Path::new("solver.f90"), EntryKind::File);
    assert_eq!(fortran.class, FileClass::Code);
    assert_eq!(fortran.icon, "󱈚");

    let cobol = theme.resolve(Path::new("ledger.cbl"), EntryKind::File);
    assert_eq!(cobol.class, FileClass::Code);
    assert_eq!(cobol.icon, "");
}

#[test]
fn detected_license_files_use_license_class_appearance() {
    let theme = Theme::default_theme();
    let (root, path) = write_temp_file(
        "license-appearance",
        "LICENSE.md",
        "# SPDX-License-Identifier: Apache-2.0\n\nLicensed under the Apache License, Version 2.0.\n",
    );

    let resolved = theme.resolve(&path, EntryKind::File);

    assert_eq!(resolved.class, FileClass::License);
    assert_eq!(resolved.icon, "󰿃");
    assert_eq!(resolved.color, rgb(245, 216, 91));
    assert_eq!(
        specific_type_label(&path, EntryKind::File),
        Some("Apache License 2.0")
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn filename_alone_does_not_force_license_appearance() {
    let theme = Theme::default_theme();
    let (root, path) = write_temp_file(
        "license-false-positive",
        "LICENSE",
        "shopping list\n- apples\n- oranges\n",
    );

    let resolved = theme.resolve(&path, EntryKind::File);

    assert_eq!(resolved.class, FileClass::File);
    assert_ne!(resolved.icon, "󰿃");
    assert_eq!(specific_type_label(&path, EntryKind::File), None);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn resolve_entry_cache_respects_entry_metadata_when_builtin_class_changes() {
    let (root, path) = write_temp_file(
        "appearance-cache",
        "third-party.txt",
        "Apache License\nVersion 2.0, January 2004\nhttp://www.apache.org/licenses/LICENSE-2.0\n\nTERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION\n",
    );

    let metadata = fs::metadata(&path).expect("metadata should exist");
    let mut entry = Entry {
        path: path.clone(),
        name: "third-party.txt".to_string(),
        name_key: "third-party.txt".to_string(),
        kind: EntryKind::File,
        size: metadata.len(),
        modified: metadata.modified().ok(),
        readonly: false,
    };

    let initial = resolve_entry(&entry);
    assert_eq!(initial.class, FileClass::License);

    fs::write(&path, "shopping list\n- apples\n- oranges\n").expect("failed to rewrite file");
    let metadata = fs::metadata(&path).expect("updated metadata should exist");
    entry.size = metadata.len();
    entry.modified = metadata.modified().ok();

    let updated = resolve_entry(&entry);
    assert_eq!(updated.class, FileClass::Document);

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn word_processing_documents_get_blue_document_icons() {
    let theme = Theme::default_theme();

    let docx = theme.resolve(Path::new("report.docx"), EntryKind::File);
    assert_eq!(docx.class, FileClass::Document);
    assert_eq!(docx.icon, "󰈬");
    assert_eq!(docx.color, rgb(88, 142, 255));

    let odt = theme.resolve(Path::new("notes.odt"), EntryKind::File);
    assert_eq!(odt.class, FileClass::Document);
    assert_eq!(odt.icon, "󰈬");
    assert_eq!(odt.color, rgb(88, 142, 255));

    let markdown_file = theme.resolve(Path::new("notes.md"), EntryKind::File);
    assert_eq!(markdown_file.class, FileClass::Document);
    assert_eq!(markdown_file.icon, "");
    assert_eq!(markdown_file.color, rgb(211, 170, 124));

    let markdown = theme.resolve(Path::new("README.md"), EntryKind::File);
    assert_eq!(markdown.class, FileClass::Document);
    assert_eq!(markdown.icon, "");
    assert_eq!(markdown.color, rgb(211, 170, 124));

    let authors = theme.resolve(Path::new("AUTHORS"), EntryKind::File);
    assert_eq!(authors.class, FileClass::Document);
    assert_eq!(authors.icon, "󰭘");
    assert_eq!(authors.color, rgb(155, 143, 199));

    let authors_markdown = theme.resolve(Path::new("AUTHORS.md"), EntryKind::File);
    assert_eq!(authors_markdown.class, FileClass::Document);
    assert_eq!(authors_markdown.icon, "󰭘");
    assert_eq!(authors_markdown.color, rgb(155, 143, 199));

    let authors_text = theme.resolve(Path::new("AUTHORS.txt"), EntryKind::File);
    assert_eq!(authors_text.class, FileClass::Document);
    assert_eq!(authors_text.icon, "󰭘");
    assert_eq!(authors_text.color, rgb(155, 143, 199));

    let contributors = theme.resolve(Path::new("CONTRIBUTORS"), EntryKind::File);
    assert_eq!(contributors.class, FileClass::Document);
    assert_eq!(contributors.icon, "󰭘");
    assert_eq!(contributors.color, rgb(155, 143, 199));

    let contributors_markdown = theme.resolve(Path::new("CONTRIBUTORS.md"), EntryKind::File);
    assert_eq!(contributors_markdown.class, FileClass::Document);
    assert_eq!(contributors_markdown.icon, "󰭘");
    assert_eq!(contributors_markdown.color, rgb(155, 143, 199));

    let text = theme.resolve(Path::new("notes.txt"), EntryKind::File);
    assert_eq!(text.class, FileClass::Document);
    assert_eq!(text.icon, "");
    assert_eq!(text.color, rgb(174, 184, 199));

    let epub = theme.resolve(Path::new("novel.epub"), EntryKind::File);
    assert_eq!(epub.class, FileClass::Document);
    assert_eq!(epub.icon, "󱗖");
    assert_eq!(epub.color, rgb(211, 170, 124));

    let comic = theme.resolve(Path::new("issue.cbz"), EntryKind::File);
    assert_eq!(comic.class, FileClass::Archive);
    assert_eq!(comic.icon, "󱗖");
    assert_eq!(comic.color, rgb(211, 170, 124));

    let documents_dir = theme.resolve(Path::new("Documents"), EntryKind::Directory);
    assert_eq!(documents_dir.class, FileClass::Directory);
    assert_eq!(documents_dir.icon, "󰲃");
    assert_eq!(documents_dir.color, rgb(141, 223, 109));

    let archive = theme.resolve(Path::new("bundle.zip"), EntryKind::File);
    assert_eq!(archive.class, FileClass::Archive);
    assert_eq!(archive.color, rgb(207, 111, 63));

    let video = theme.resolve(Path::new("clip.mp4"), EntryKind::File);
    assert_eq!(video.class, FileClass::Video);
    assert_eq!(video.icon, "");
    assert_eq!(video.color, rgb(255, 134, 216));

    let videos_dir = theme.resolve(Path::new("Videos"), EntryKind::Directory);
    assert_eq!(videos_dir.class, FileClass::Directory);
    assert_eq!(videos_dir.icon, "󰕧");
    assert_eq!(videos_dir.color, rgb(255, 134, 216));
}

#[test]
fn spreadsheets_and_presentations_get_family_specific_icons() {
    let theme = Theme::default_theme();

    let xlsx = theme.resolve(Path::new("budget.xlsx"), EntryKind::File);
    assert_eq!(xlsx.class, FileClass::Document);
    assert_eq!(xlsx.icon, "󱎏");
    assert_eq!(xlsx.color, rgb(78, 178, 116));

    let ods = theme.resolve(Path::new("budget.ods"), EntryKind::File);
    assert_eq!(ods.class, FileClass::Document);
    assert_eq!(ods.icon, "󱎏");
    assert_eq!(ods.color, rgb(78, 178, 116));

    let pptx = theme.resolve(Path::new("deck.pptx"), EntryKind::File);
    assert_eq!(pptx.class, FileClass::Document);
    assert_eq!(pptx.icon, "󱎐");
    assert_eq!(pptx.color, rgb(232, 139, 63));

    let odp = theme.resolve(Path::new("deck.odp"), EntryKind::File);
    assert_eq!(odp.class, FileClass::Document);
    assert_eq!(odp.icon, "󱎐");
    assert_eq!(odp.color, rgb(232, 139, 63));
}

#[test]
fn exact_name_rules_win_over_extension_rules() {
    let theme = Theme::from_config_str(
        r##"
[extensions.toml]
class = "data"
icon = "E"

[files."Cargo.toml"]
class = "config"
icon = "F"
"##,
    )
    .expect("theme should parse");

    let resolved = theme.resolve(Path::new("Cargo.toml"), EntryKind::File);
    assert_eq!(resolved.class, FileClass::Config);
    assert_eq!(resolved.icon, "F");
}

#[test]
fn default_theme_uses_toml_icon_for_toml_files() {
    let theme = Theme::default_theme();

    let cargo = theme.resolve(Path::new("Cargo.toml"), EntryKind::File);
    assert_eq!(cargo.class, FileClass::Config);
    assert_eq!(cargo.icon, "");

    let pyproject = theme.resolve(Path::new("pyproject.toml"), EntryKind::File);
    assert_eq!(pyproject.class, FileClass::Config);
    assert_eq!(pyproject.icon, "");

    let rust_toolchain = theme.resolve(Path::new("rust-toolchain.toml"), EntryKind::File);
    assert_eq!(rust_toolchain.class, FileClass::Config);
    assert_eq!(rust_toolchain.icon, "");
}

#[test]
fn matching_is_case_insensitive_and_trimmed() {
    let theme = Theme::from_config_str(
        r##"
[classes." folder "]
icon = "D"
color = "#010203"

[extensions." LOCK "]
class = "data"
icon = "L"

[files." cargo.lock "]
class = "data"
icon = "F"
"##,
    )
    .expect("theme should parse");

    let dir = theme.resolve(Path::new("projects"), EntryKind::Directory);
    assert_eq!(dir.class, FileClass::Directory);
    assert_eq!(theme.classes.get(&FileClass::Directory).unwrap().icon, "D");

    let file = theme.resolve(Path::new("CARGO.LOCK"), EntryKind::File);
    assert_eq!(file.class, FileClass::Data);
    assert_eq!(file.icon, "F");
}

#[test]
fn type_labels_cover_supported_special_files() {
    assert_eq!(
        specific_type_label(Path::new("cover.xcf"), EntryKind::File),
        Some("GIMP image")
    );
    assert_eq!(
        specific_type_label(Path::new("disk.iso"), EntryKind::File),
        Some("ISO disk image")
    );
    assert_eq!(
        specific_type_label(Path::new("package.rpm"), EntryKind::File),
        Some("RPM package")
    );
    assert_eq!(
        specific_type_label(Path::new("ubuntu.torrent"), EntryKind::File),
        Some("BitTorrent file")
    );
    assert_eq!(
        specific_type_label(Path::new("signatures.hash"), EntryKind::File),
        Some("Hash file")
    );
    assert_eq!(
        specific_type_label(Path::new("release.sha1"), EntryKind::File),
        Some("SHA-1 checksum")
    );
    assert_eq!(
        specific_type_label(Path::new("release.sha256"), EntryKind::File),
        Some("SHA-256 checksum")
    );
    assert_eq!(
        specific_type_label(Path::new("release.sha512"), EntryKind::File),
        Some("SHA-512 checksum")
    );
    assert_eq!(
        specific_type_label(Path::new("release.md5"), EntryKind::File),
        Some("MD5 checksum")
    );
    assert_eq!(
        specific_type_label(Path::new("server.log"), EntryKind::File),
        Some("Log file")
    );
    assert_eq!(
        specific_type_label(Path::new("movie.srt"), EntryKind::File),
        Some("SubRip subtitles")
    );
    assert_eq!(
        specific_type_label(Path::new("bindings.keys"), EntryKind::File),
        Some("Keys file")
    );
    assert_eq!(
        specific_type_label(Path::new("identity.p12"), EntryKind::File),
        Some("PKCS#12 certificate")
    );
    assert_eq!(
        specific_type_label(Path::new("identity.pfx"), EntryKind::File),
        Some("PKCS#12 certificate")
    );
    assert_eq!(
        specific_type_label(Path::new("fullchain.pem"), EntryKind::File),
        Some("PEM certificate")
    );
    assert_eq!(
        specific_type_label(Path::new("server.crt"), EntryKind::File),
        Some("Certificate")
    );
    assert_eq!(
        specific_type_label(Path::new("server.cer"), EntryKind::File),
        Some("Certificate")
    );
    assert_eq!(
        specific_type_label(Path::new("server.csr"), EntryKind::File),
        Some("Certificate signing request")
    );
    assert_eq!(
        specific_type_label(Path::new("id_ed25519.key"), EntryKind::File),
        Some("Private key")
    );
    assert_eq!(
        specific_type_label(Path::new("package.deb"), EntryKind::File),
        Some("Debian package")
    );
    assert_eq!(
        specific_type_label(Path::new("app.apk"), EntryKind::File),
        Some("Android package")
    );
    assert_eq!(
        specific_type_label(Path::new("bundle.aab"), EntryKind::File),
        Some("Android App Bundle")
    );
    assert_eq!(
        specific_type_label(Path::new("deck.apkg"), EntryKind::File),
        Some("Anki package")
    );
    assert_eq!(
        specific_type_label(Path::new("archive.zst"), EntryKind::File),
        Some("Zstandard archive")
    );
    assert_eq!(
        specific_type_label(Path::new("theme.zest"), EntryKind::File),
        Some("Zest archive")
    );
    assert_eq!(
        specific_type_label(Path::new("Elio.AppImage"), EntryKind::File),
        Some("AppImage bundle")
    );
    assert_eq!(
        specific_type_label(Path::new("PKGBUILD"), EntryKind::File),
        Some("Arch build script")
    );
    assert_eq!(
        specific_type_label(Path::new("setup.exe"), EntryKind::File),
        Some("Windows executable")
    );
    assert_eq!(
        specific_type_label(Path::new("app.jar"), EntryKind::File),
        Some("Java archive")
    );
}

#[test]
fn builtin_classification_covers_new_special_file_types() {
    assert_eq!(
        builtin_classify_path(Path::new("cover.xcf"), EntryKind::File),
        FileClass::Image
    );
    assert_eq!(
        builtin_classify_path(Path::new("favicon.ico"), EntryKind::File),
        FileClass::Image
    );
    assert_eq!(
        builtin_classify_path(Path::new("disk.iso"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("package.rpm"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("package.deb"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("app.apk"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("bundle.aab"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("deck.apkg"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("archive.zst"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("app.jar"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("archive.zest"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("Elio.AppImage"), EntryKind::File),
        FileClass::Archive
    );
    assert_eq!(
        builtin_classify_path(Path::new("ubuntu.torrent"), EntryKind::File),
        FileClass::Data
    );
    assert_eq!(
        builtin_classify_path(Path::new("signatures.hash"), EntryKind::File),
        FileClass::Data
    );
    assert_eq!(
        builtin_classify_path(Path::new("release.sha1"), EntryKind::File),
        FileClass::Data
    );
    assert_eq!(
        builtin_classify_path(Path::new("release.sha256"), EntryKind::File),
        FileClass::Data
    );
    assert_eq!(
        builtin_classify_path(Path::new("release.sha512"), EntryKind::File),
        FileClass::Data
    );
    assert_eq!(
        builtin_classify_path(Path::new("release.md5"), EntryKind::File),
        FileClass::Data
    );
    assert_eq!(
        builtin_classify_path(Path::new("server.log"), EntryKind::File),
        FileClass::Document
    );
    assert_eq!(
        builtin_classify_path(Path::new("movie.srt"), EntryKind::File),
        FileClass::Document
    );
    assert_eq!(
        builtin_classify_path(Path::new("bindings.keys"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("identity.p12"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("identity.pfx"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("fullchain.pem"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("server.crt"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("server.cer"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("server.csr"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("id_ed25519.key"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("PKGBUILD"), EntryKind::File),
        FileClass::Config
    );
    assert_eq!(
        builtin_classify_path(Path::new("setup.exe"), EntryKind::File),
        FileClass::File
    );
}
