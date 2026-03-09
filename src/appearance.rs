use crate::app::{Entry, EntryKind, FileClass};
use ratatui::style::Color;
use serde::Deserialize;
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

static ACTIVE_THEME: OnceLock<Theme> = OnceLock::new();

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub bg: Color,
    pub chrome: Color,
    pub chrome_alt: Color,
    pub panel: Color,
    pub panel_alt: Color,
    pub surface: Color,
    pub elevated: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub accent_text: Color,
    pub selected_bg: Color,
    pub selected_border: Color,
    pub sidebar_active: Color,
    pub button_bg: Color,
    pub button_disabled_bg: Color,
    pub path_bg: Color,
}

#[derive(Clone)]
struct ClassStyle {
    icon: String,
    color: Color,
}

#[derive(Clone, Default)]
struct RuleOverride {
    class: Option<FileClass>,
    icon: Option<String>,
    color: Option<Color>,
}

#[derive(Clone)]
struct Theme {
    palette: Palette,
    classes: HashMap<FileClass, ClassStyle>,
    extensions: HashMap<String, RuleOverride>,
    files: HashMap<String, RuleOverride>,
    directories: HashMap<String, RuleOverride>,
}

pub(crate) struct ResolvedAppearance<'a> {
    pub class: FileClass,
    pub icon: &'a str,
    pub color: Color,
}

#[derive(Deserialize, Default)]
struct ThemeFile {
    palette: Option<PaletteOverride>,
    classes: Option<HashMap<String, ClassStyleOverride>>,
    extensions: Option<HashMap<String, RuleOverrideDef>>,
    files: Option<HashMap<String, RuleOverrideDef>>,
    directories: Option<HashMap<String, RuleOverrideDef>>,
}

#[derive(Deserialize, Default)]
struct PaletteOverride {
    bg: Option<String>,
    chrome: Option<String>,
    chrome_alt: Option<String>,
    panel: Option<String>,
    panel_alt: Option<String>,
    surface: Option<String>,
    elevated: Option<String>,
    border: Option<String>,
    text: Option<String>,
    muted: Option<String>,
    accent: Option<String>,
    accent_soft: Option<String>,
    accent_text: Option<String>,
    selected_bg: Option<String>,
    selected_border: Option<String>,
    sidebar_active: Option<String>,
    button_bg: Option<String>,
    button_disabled_bg: Option<String>,
    path_bg: Option<String>,
}

#[derive(Deserialize, Default)]
struct ClassStyleOverride {
    icon: Option<String>,
    color: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RuleOverrideDef {
    Class(String),
    Rich {
        class: Option<String>,
        icon: Option<String>,
        color: Option<String>,
    },
}

pub(crate) fn initialize() {
    let _ = ACTIVE_THEME.get_or_init(load_theme_from_disk);
}

pub(crate) fn palette() -> Palette {
    active_theme().palette
}

pub(crate) fn classify_path(path: &Path, kind: EntryKind) -> FileClass {
    resolve_path(path, kind).class
}

pub(crate) fn resolve_path(path: &Path, kind: EntryKind) -> ResolvedAppearance<'static> {
    active_theme().resolve(path, kind)
}

pub(crate) fn folder_color(entry: &Entry) -> Color {
    resolve_path(&entry.path, entry.kind).color
}

fn active_theme() -> &'static Theme {
    ACTIVE_THEME.get_or_init(Theme::default_theme)
}

fn load_theme_from_disk() -> Theme {
    let Some(path) = theme_path() else {
        return Theme::default_theme();
    };
    let Ok(contents) = fs::read_to_string(&path) else {
        return Theme::default_theme();
    };

    match Theme::from_config_str(&contents) {
        Ok(theme) => theme,
        Err(error) => {
            eprintln!("elio: failed to load theme from {}: {error}", path.display());
            Theme::default_theme()
        }
    }
}

fn theme_path() -> Option<PathBuf> {
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(config_home).join("elio/theme.toml"));
    }

    env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/elio/theme.toml"))
}

impl Theme {
    fn default_theme() -> Self {
        let mut classes = HashMap::new();
        classes.insert(
            FileClass::Directory,
            ClassStyle {
                icon: "󰉋".to_string(),
                color: rgb(65, 143, 222),
            },
        );
        classes.insert(
            FileClass::Code,
            ClassStyle {
                icon: "󰆍".to_string(),
                color: rgb(87, 196, 155),
            },
        );
        classes.insert(
            FileClass::Config,
            ClassStyle {
                icon: "󰒓".to_string(),
                color: rgb(121, 188, 255),
            },
        );
        classes.insert(
            FileClass::Document,
            ClassStyle {
                icon: "󰈙".to_string(),
                color: rgb(112, 182, 117),
            },
        );
        classes.insert(
            FileClass::Image,
            ClassStyle {
                icon: "󰋩".to_string(),
                color: rgb(86, 156, 214),
            },
        );
        classes.insert(
            FileClass::Audio,
            ClassStyle {
                icon: "󰎆".to_string(),
                color: rgb(138, 110, 214),
            },
        );
        classes.insert(
            FileClass::Video,
            ClassStyle {
                icon: "󰈫".to_string(),
                color: rgb(204, 112, 79),
            },
        );
        classes.insert(
            FileClass::Archive,
            ClassStyle {
                icon: "󰗄".to_string(),
                color: rgb(191, 142, 74),
            },
        );
        classes.insert(
            FileClass::Font,
            ClassStyle {
                icon: "󰛖".to_string(),
                color: rgb(196, 148, 92),
            },
        );
        classes.insert(
            FileClass::Data,
            ClassStyle {
                icon: "󰆼".to_string(),
                color: rgb(92, 192, 201),
            },
        );
        classes.insert(
            FileClass::File,
            ClassStyle {
                icon: "󰈔".to_string(),
                color: rgb(98, 109, 122),
            },
        );

        let extensions = HashMap::from([
            ("rs".to_string(), rule_class(FileClass::Code)),
            ("js".to_string(), rule_class(FileClass::Code)),
            ("ts".to_string(), rule_class(FileClass::Code)),
            ("tsx".to_string(), rule_class(FileClass::Code)),
            ("jsx".to_string(), rule_class(FileClass::Code)),
            ("py".to_string(), rule_class(FileClass::Code)),
            ("go".to_string(), rule_class(FileClass::Code)),
            ("c".to_string(), rule_class(FileClass::Code)),
            ("cpp".to_string(), rule_class(FileClass::Code)),
            ("h".to_string(), rule_class(FileClass::Code)),
            ("hpp".to_string(), rule_class(FileClass::Code)),
            ("java".to_string(), rule_class(FileClass::Code)),
            ("lua".to_string(), rule_class(FileClass::Code)),
            ("php".to_string(), rule_class(FileClass::Code)),
            ("rb".to_string(), rule_class(FileClass::Code)),
            ("swift".to_string(), rule_class(FileClass::Code)),
            ("kt".to_string(), rule_class(FileClass::Code)),
            ("sh".to_string(), rule_class(FileClass::Code)),
            ("bash".to_string(), rule_class(FileClass::Code)),
            ("zsh".to_string(), rule_class(FileClass::Code)),
            ("fish".to_string(), rule_class(FileClass::Code)),
            ("json".to_string(), rule_class(FileClass::Config)),
            ("toml".to_string(), rule_class(FileClass::Config)),
            ("yaml".to_string(), rule_class(FileClass::Config)),
            ("yml".to_string(), rule_class(FileClass::Config)),
            ("ini".to_string(), rule_class(FileClass::Config)),
            ("conf".to_string(), rule_class(FileClass::Config)),
            ("cfg".to_string(), rule_class(FileClass::Config)),
            ("ron".to_string(), rule_class(FileClass::Config)),
            ("env".to_string(), rule_class(FileClass::Config)),
            ("md".to_string(), rule_class(FileClass::Document)),
            ("txt".to_string(), rule_class(FileClass::Document)),
            ("rst".to_string(), rule_class(FileClass::Document)),
            ("pdf".to_string(), rule_class(FileClass::Document)),
            ("doc".to_string(), rule_class(FileClass::Document)),
            ("docx".to_string(), rule_class(FileClass::Document)),
            ("odt".to_string(), rule_class(FileClass::Document)),
            ("png".to_string(), rule_class(FileClass::Image)),
            ("jpg".to_string(), rule_class(FileClass::Image)),
            ("jpeg".to_string(), rule_class(FileClass::Image)),
            ("gif".to_string(), rule_class(FileClass::Image)),
            ("svg".to_string(), rule_class(FileClass::Image)),
            ("webp".to_string(), rule_class(FileClass::Image)),
            ("avif".to_string(), rule_class(FileClass::Image)),
            ("mp3".to_string(), rule_class(FileClass::Audio)),
            ("wav".to_string(), rule_class(FileClass::Audio)),
            ("flac".to_string(), rule_class(FileClass::Audio)),
            ("ogg".to_string(), rule_class(FileClass::Audio)),
            ("m4a".to_string(), rule_class(FileClass::Audio)),
            ("mp4".to_string(), rule_class(FileClass::Video)),
            ("mkv".to_string(), rule_class(FileClass::Video)),
            ("mov".to_string(), rule_class(FileClass::Video)),
            ("webm".to_string(), rule_class(FileClass::Video)),
            ("avi".to_string(), rule_class(FileClass::Video)),
            ("zip".to_string(), rule_class(FileClass::Archive)),
            ("tar".to_string(), rule_class(FileClass::Archive)),
            ("gz".to_string(), rule_class(FileClass::Archive)),
            ("xz".to_string(), rule_class(FileClass::Archive)),
            ("bz2".to_string(), rule_class(FileClass::Archive)),
            ("7z".to_string(), rule_class(FileClass::Archive)),
            ("ttf".to_string(), rule_class(FileClass::Font)),
            ("otf".to_string(), rule_class(FileClass::Font)),
            ("woff".to_string(), rule_class(FileClass::Font)),
            ("woff2".to_string(), rule_class(FileClass::Font)),
            ("csv".to_string(), rule_class(FileClass::Data)),
            ("tsv".to_string(), rule_class(FileClass::Data)),
            ("sql".to_string(), rule_class(FileClass::Data)),
            ("sqlite".to_string(), rule_class(FileClass::Data)),
            ("db".to_string(), rule_class(FileClass::Data)),
            ("parquet".to_string(), rule_class(FileClass::Data)),
        ]);

        let files = HashMap::from([
            (
                normalize_key("Cargo.toml"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰣖".to_string()),
                    color: None,
                },
            ),
            (
                normalize_key("Cargo.lock"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰌾".to_string()),
                    color: None,
                },
            ),
            (
                normalize_key("package.json"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(226, 180, 80)),
                },
            ),
            (
                normalize_key("package-lock.json"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("".to_string()),
                    color: Some(rgb(210, 146, 89)),
                },
            ),
            (
                normalize_key("Dockerfile"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰡨".to_string()),
                    color: Some(rgb(94, 162, 227)),
                },
            ),
            (
                normalize_key("compose.yml"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰡨".to_string()),
                    color: Some(rgb(94, 162, 227)),
                },
            ),
            (
                normalize_key("compose.yaml"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰡨".to_string()),
                    color: Some(rgb(94, 162, 227)),
                },
            ),
            (
                normalize_key("README.md"),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("󰈙".to_string()),
                    color: Some(rgb(125, 201, 120)),
                },
            ),
            (
                normalize_key("LICENSE"),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("󰿃".to_string()),
                    color: Some(rgb(190, 205, 120)),
                },
            ),
            (
                normalize_key(".gitignore"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰊢".to_string()),
                    color: Some(rgb(232, 153, 88)),
                },
            ),
            (
                normalize_key(".env"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰒓".to_string()),
                    color: Some(rgb(144, 192, 121)),
                },
            ),
        ]);

        Self {
            palette: Palette {
                bg: rgb(10, 14, 20),
                chrome: rgb(16, 21, 30),
                chrome_alt: rgb(24, 32, 43),
                panel: rgb(18, 25, 35),
                panel_alt: rgb(14, 20, 28),
                surface: rgb(22, 30, 41),
                elevated: rgb(27, 37, 50),
                border: rgb(49, 67, 87),
                text: rgb(238, 243, 248),
                muted: rgb(158, 172, 189),
                accent: rgb(102, 186, 255),
                accent_soft: rgb(34, 57, 79),
                accent_text: rgb(207, 234, 255),
                selected_bg: rgb(36, 56, 78),
                selected_border: rgb(112, 196, 255),
                sidebar_active: rgb(31, 47, 65),
                button_bg: rgb(29, 39, 52),
                button_disabled_bg: rgb(20, 27, 37),
                path_bg: rgb(28, 37, 49),
            },
            classes,
            extensions,
            files,
            directories: HashMap::new(),
        }
    }

    fn from_config_str(config: &str) -> anyhow::Result<Self> {
        let mut theme = Self::default_theme();
        let parsed: ThemeFile = toml::from_str(config)?;
        theme.apply_overrides(parsed)?;
        Ok(theme)
    }

    fn apply_overrides(&mut self, parsed: ThemeFile) -> anyhow::Result<()> {
        if let Some(palette) = parsed.palette {
            apply_palette_overrides(&mut self.palette, palette)?;
        }

        if let Some(classes) = parsed.classes {
            for (name, override_style) in classes {
                let class = parse_class_name(&name)
                    .ok_or_else(|| anyhow::anyhow!("unknown class `{name}`"))?;
                let style = self.classes.entry(class).or_insert_with(|| default_class_style(class));
                if let Some(icon) = override_style.icon {
                    style.icon = icon;
                }
                if let Some(color) = override_style.color {
                    style.color = parse_color(&color)?;
                }
            }
        }

        if let Some(extensions) = parsed.extensions {
            apply_rule_map(&mut self.extensions, extensions)?;
        }
        if let Some(files) = parsed.files {
            apply_rule_map(&mut self.files, files)?;
        }
        if let Some(directories) = parsed.directories {
            apply_rule_map(&mut self.directories, directories)?;
        }

        Ok(())
    }

    fn resolve(&self, path: &Path, kind: EntryKind) -> ResolvedAppearance<'_> {
        let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        let normalized_name = normalize_key(file_name);
        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or_default().to_ascii_lowercase();

        let exact_rule = match kind {
            EntryKind::Directory => self.directories.get(&normalized_name),
            EntryKind::File => self.files.get(&normalized_name),
        };
        let ext_rule = (kind == EntryKind::File)
            .then(|| self.extensions.get(&ext))
            .flatten();

        let class = exact_rule
            .and_then(|rule| rule.class)
            .or_else(|| ext_rule.and_then(|rule| rule.class))
            .unwrap_or_else(|| builtin_classify_path(path, kind));

        let base = self
            .classes
            .get(&class)
            .unwrap_or_else(|| self.classes.get(&FileClass::File).expect("default file style"));

        let icon = exact_rule
            .and_then(|rule| rule.icon.as_deref())
            .or_else(|| ext_rule.and_then(|rule| rule.icon.as_deref()))
            .unwrap_or(base.icon.as_str());
        let color = exact_rule
            .and_then(|rule| rule.color)
            .or_else(|| ext_rule.and_then(|rule| rule.color))
            .unwrap_or(base.color);

        ResolvedAppearance { class, icon, color }
    }
}

fn apply_palette_overrides(palette: &mut Palette, overrides: PaletteOverride) -> anyhow::Result<()> {
    apply_palette_color(&mut palette.bg, overrides.bg)?;
    apply_palette_color(&mut palette.chrome, overrides.chrome)?;
    apply_palette_color(&mut palette.chrome_alt, overrides.chrome_alt)?;
    apply_palette_color(&mut palette.panel, overrides.panel)?;
    apply_palette_color(&mut palette.panel_alt, overrides.panel_alt)?;
    apply_palette_color(&mut palette.surface, overrides.surface)?;
    apply_palette_color(&mut palette.elevated, overrides.elevated)?;
    apply_palette_color(&mut palette.border, overrides.border)?;
    apply_palette_color(&mut palette.text, overrides.text)?;
    apply_palette_color(&mut palette.muted, overrides.muted)?;
    apply_palette_color(&mut palette.accent, overrides.accent)?;
    apply_palette_color(&mut palette.accent_soft, overrides.accent_soft)?;
    apply_palette_color(&mut palette.accent_text, overrides.accent_text)?;
    apply_palette_color(&mut palette.selected_bg, overrides.selected_bg)?;
    apply_palette_color(&mut palette.selected_border, overrides.selected_border)?;
    apply_palette_color(&mut palette.sidebar_active, overrides.sidebar_active)?;
    apply_palette_color(&mut palette.button_bg, overrides.button_bg)?;
    apply_palette_color(&mut palette.button_disabled_bg, overrides.button_disabled_bg)?;
    apply_palette_color(&mut palette.path_bg, overrides.path_bg)?;
    Ok(())
}

fn apply_palette_color(target: &mut Color, value: Option<String>) -> anyhow::Result<()> {
    if let Some(value) = value {
        *target = parse_color(&value)?;
    }
    Ok(())
}

fn apply_rule_map(
    target: &mut HashMap<String, RuleOverride>,
    source: HashMap<String, RuleOverrideDef>,
) -> anyhow::Result<()> {
    for (key, value) in source {
        target.insert(normalize_key(&key), parse_rule_override(value)?);
    }
    Ok(())
}

fn parse_rule_override(value: RuleOverrideDef) -> anyhow::Result<RuleOverride> {
    match value {
        RuleOverrideDef::Class(class) => Ok(rule_class(
            parse_class_name(&class)
                .ok_or_else(|| anyhow::anyhow!("unknown class `{class}`"))?,
        )),
        RuleOverrideDef::Rich { class, icon, color } => Ok(RuleOverride {
            class: match class {
                Some(class) => Some(
                    parse_class_name(&class)
                        .ok_or_else(|| anyhow::anyhow!("unknown class `{class}`"))?,
                ),
                None => None,
            },
            icon,
            color: match color {
                Some(color) => Some(parse_color(&color)?),
                None => None,
            },
        }),
    }
}

fn default_class_style(class: FileClass) -> ClassStyle {
    Theme::default_theme()
        .classes
        .remove(&class)
        .unwrap_or(ClassStyle {
            icon: "󰈔".to_string(),
            color: rgb(98, 109, 122),
        })
}

fn rule_class(class: FileClass) -> RuleOverride {
    RuleOverride {
        class: Some(class),
        ..RuleOverride::default()
    }
}

fn builtin_classify_path(path: &Path, kind: EntryKind) -> FileClass {
    if kind == EntryKind::Directory {
        return FileClass::Directory;
    }

    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" | "js" | "ts" | "tsx" | "jsx" | "py" | "go" | "c" | "cpp" | "h" | "hpp"
        | "java" | "lua" | "php" | "rb" | "swift" | "kt" | "sh" | "bash" | "zsh" | "fish" => {
            FileClass::Code
        }
        "json" | "toml" | "yaml" | "yml" | "ini" | "conf" | "cfg" | "ron" | "env" => {
            FileClass::Config
        }
        "md" | "txt" | "rst" | "pdf" | "doc" | "docx" | "odt" => FileClass::Document,
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "avif" => FileClass::Image,
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => FileClass::Audio,
        "mp4" | "mkv" | "mov" | "webm" | "avi" => FileClass::Video,
        "zip" | "tar" | "gz" | "xz" | "bz2" | "7z" => FileClass::Archive,
        "ttf" | "otf" | "woff" | "woff2" => FileClass::Font,
        "csv" | "tsv" | "sql" | "sqlite" | "db" | "parquet" => FileClass::Data,
        _ => FileClass::File,
    }
}

fn parse_class_name(name: &str) -> Option<FileClass> {
    match normalize_key(name).as_str() {
        "directory" | "dir" | "folder" => Some(FileClass::Directory),
        "code" => Some(FileClass::Code),
        "config" => Some(FileClass::Config),
        "document" | "doc" | "text" => Some(FileClass::Document),
        "image" | "img" => Some(FileClass::Image),
        "audio" => Some(FileClass::Audio),
        "video" => Some(FileClass::Video),
        "archive" | "compressed" => Some(FileClass::Archive),
        "font" => Some(FileClass::Font),
        "data" => Some(FileClass::Data),
        "file" | "plain" => Some(FileClass::File),
        _ => None,
    }
}

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn parse_color(value: &str) -> anyhow::Result<Color> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 {
        anyhow::bail!("invalid color {value}");
    }

    let red = u8::from_str_radix(&hex[0..2], 16)?;
    let green = u8::from_str_radix(&hex[2..4], 16)?;
    let blue = u8::from_str_radix(&hex[4..6], 16)?;
    Ok(rgb(red, green, blue))
}

fn rgb(red: u8, green: u8, blue: u8) -> Color {
    Color::Rgb(red, green, blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_file_rules_override_extension_defaults() {
        let theme = Theme::default_theme();
        let resolved = theme.resolve(Path::new("Cargo.lock"), EntryKind::File);
        assert_eq!(resolved.class, FileClass::Data);
        assert_eq!(resolved.icon, "󰌾");
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

        let resolved = theme.resolve(Path::new("poetry.lock"), EntryKind::File);
        assert_eq!(resolved.class, FileClass::Data);
        assert_eq!(resolved.icon, "L");
    }
}
