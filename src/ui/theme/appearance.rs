use crate::{
    app::{Entry, EntryKind, FileClass},
    config, file_info,
};
use ratatui::style::Color;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

static ACTIVE_THEME: OnceLock<Theme> = OnceLock::new();
static ENTRY_CLASS_CACHE: OnceLock<Mutex<HashMap<EntryClassCacheKey, FileClass>>> = OnceLock::new();

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
    pub selection_bar: Color,
    pub sidebar_active: Color,
    pub button_bg: Color,
    pub button_disabled_bg: Color,
    pub path_bg: Color,
}

#[derive(Clone, Copy)]
pub(crate) struct CodePreviewPalette {
    pub fg: Color,
    pub bg: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub caret: Color,
    pub line_highlight: Color,
    pub line_number: Color,
    pub comment: Color,
    pub string: Color,
    pub constant: Color,
    pub keyword: Color,
    pub function: Color,
    pub r#type: Color,
    pub parameter: Color,
    pub tag: Color,
    pub operator: Color,
    pub r#macro: Color,
    pub invalid: Color,
}

#[derive(Clone, Copy)]
struct PreviewTheme {
    code: CodePreviewPalette,
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
    preview: PreviewTheme,
    classes: HashMap<FileClass, ClassStyle>,
    extensions: HashMap<String, RuleOverride>,
    files: HashMap<String, RuleOverride>,
    directories: HashMap<String, RuleOverride>,
}

pub(crate) struct ResolvedAppearance<'a> {
    #[cfg(test)]
    pub class: FileClass,
    pub icon: &'a str,
    pub color: Color,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct EntryClassCacheKey {
    path: PathBuf,
    is_dir: bool,
    size: u64,
    modified: Option<(u64, u32)>,
}

#[derive(Deserialize, Default)]
struct ThemeFile {
    palette: Option<PaletteOverride>,
    preview: Option<PreviewOverride>,
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
    selection_bar: Option<String>,
    sidebar_active: Option<String>,
    button_bg: Option<String>,
    button_disabled_bg: Option<String>,
    path_bg: Option<String>,
}

#[derive(Deserialize, Default)]
struct PreviewOverride {
    code: Option<CodePreviewOverride>,
}

#[derive(Deserialize, Default)]
struct CodePreviewOverride {
    fg: Option<String>,
    bg: Option<String>,
    selection_bg: Option<String>,
    selection_fg: Option<String>,
    caret: Option<String>,
    line_highlight: Option<String>,
    line_number: Option<String>,
    comment: Option<String>,
    string: Option<String>,
    constant: Option<String>,
    keyword: Option<String>,
    function: Option<String>,
    r#type: Option<String>,
    parameter: Option<String>,
    tag: Option<String>,
    operator: Option<String>,
    r#macro: Option<String>,
    invalid: Option<String>,
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

pub(crate) fn code_preview_palette() -> CodePreviewPalette {
    active_theme().preview.code
}

pub(crate) fn resolve_path(path: &Path, kind: EntryKind) -> ResolvedAppearance<'static> {
    active_theme().resolve(path, kind)
}

pub(crate) fn resolve_entry(entry: &Entry) -> ResolvedAppearance<'static> {
    let builtin_class = builtin_classify_entry(entry);
    active_theme().resolve_with_builtin_class(&entry.path, entry.kind, builtin_class)
}

#[cfg(test)]
pub(crate) fn specific_type_label(path: &Path, kind: EntryKind) -> Option<&'static str> {
    file_info::inspect_path(path, kind).specific_type_label
}

fn active_theme() -> &'static Theme {
    ACTIVE_THEME.get_or_init(Theme::default_theme)
}

fn entry_class_cache() -> &'static Mutex<HashMap<EntryClassCacheKey, FileClass>> {
    ENTRY_CLASS_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn load_theme_from_disk() -> Theme {
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

impl Theme {
    fn default_theme() -> Self {
        Self::apply_config_on(
            Self::base_theme(),
            include_str!("../../../examples/themes/default/theme.toml"),
        )
        .unwrap_or_else(|error| {
            eprintln!("elio: failed to load built-in default theme: {error}");
            Self::base_theme()
        })
    }

    fn base_theme() -> Self {
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
            FileClass::License,
            ClassStyle {
                icon: "󰿃".to_string(),
                color: rgb(245, 216, 91),
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
                icon: "".to_string(),
                color: rgb(204, 112, 79),
            },
        );
        classes.insert(
            FileClass::Archive,
            ClassStyle {
                icon: "󰗄".to_string(),
                color: rgb(207, 111, 63),
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
            (
                "sql".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(92, 192, 201)),
                },
            ),
            (
                "diff".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(255, 184, 107)),
                },
            ),
            (
                "patch".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(255, 184, 107)),
                },
            ),
            (
                "hcl".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "tf".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "tfvars".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "tfbackend".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "groovy".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(112, 182, 117)),
                },
            ),
            (
                "gvy".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(112, 182, 117)),
                },
            ),
            (
                "gradle".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(112, 182, 117)),
                },
            ),
            (
                "scala".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(232, 90, 90)),
                },
            ),
            (
                "sbt".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(232, 90, 90)),
                },
            ),
            (
                "pl".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(125, 176, 255)),
                },
            ),
            (
                "pm".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(125, 176, 255)),
                },
            ),
            (
                "pod".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(125, 176, 255)),
                },
            ),
            (
                "hs".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "lhs".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "jl".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(193, 120, 255)),
                },
            ),
            (
                "r".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰟔".to_string()),
                    color: Some(rgb(95, 153, 219)),
                },
            ),
            (
                "just".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(255, 184, 107)),
                },
            ),
            (
                "ziggy".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(245, 173, 64)),
                },
            ),
            ("py".to_string(), rule_class(FileClass::Code)),
            ("go".to_string(), rule_class(FileClass::Code)),
            ("c".to_string(), rule_class(FileClass::Code)),
            ("cpp".to_string(), rule_class(FileClass::Code)),
            ("h".to_string(), rule_class(FileClass::Code)),
            ("hpp".to_string(), rule_class(FileClass::Code)),
            (
                "cs".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰌛".to_string()),
                    color: Some(rgb(104, 179, 120)),
                },
            ),
            (
                "csx".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰌛".to_string()),
                    color: Some(rgb(104, 179, 120)),
                },
            ),
            (
                "dart".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(56, 213, 255)),
                },
            ),
            (
                "f".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󱈚".to_string()),
                    color: Some(rgb(115, 79, 150)),
                },
            ),
            (
                "for".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󱈚".to_string()),
                    color: Some(rgb(115, 79, 150)),
                },
            ),
            (
                "f90".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󱈚".to_string()),
                    color: Some(rgb(115, 79, 150)),
                },
            ),
            (
                "f95".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󱈚".to_string()),
                    color: Some(rgb(115, 79, 150)),
                },
            ),
            (
                "f03".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󱈚".to_string()),
                    color: Some(rgb(115, 79, 150)),
                },
            ),
            (
                "f08".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󱈚".to_string()),
                    color: Some(rgb(115, 79, 150)),
                },
            ),
            (
                "fpp".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󱈚".to_string()),
                    color: Some(rgb(115, 79, 150)),
                },
            ),
            (
                "cbl".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(0, 92, 165)),
                },
            ),
            (
                "cob".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(0, 92, 165)),
                },
            ),
            (
                "cobol".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(0, 92, 165)),
                },
            ),
            (
                "cpy".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(0, 92, 165)),
                },
            ),
            ("java".to_string(), rule_class(FileClass::Code)),
            ("lua".to_string(), rule_class(FileClass::Code)),
            ("php".to_string(), rule_class(FileClass::Code)),
            ("rb".to_string(), rule_class(FileClass::Code)),
            (
                "ex".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(155, 143, 199)),
                },
            ),
            (
                "exs".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(155, 143, 199)),
                },
            ),
            (
                "clj".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            (
                "cljs".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            (
                "cljc".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            (
                "edn".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            ("swift".to_string(), rule_class(FileClass::Code)),
            ("kt".to_string(), rule_class(FileClass::Code)),
            (
                "ps1".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰨊".to_string()),
                    color: Some(rgb(95, 153, 219)),
                },
            ),
            (
                "psm1".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰨊".to_string()),
                    color: Some(rgb(95, 153, 219)),
                },
            ),
            (
                "psd1".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰨊".to_string()),
                    color: Some(rgb(95, 153, 219)),
                },
            ),
            (
                "sh".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(214, 222, 240)),
                },
            ),
            (
                "bash".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(214, 222, 240)),
                },
            ),
            (
                "zsh".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(214, 222, 240)),
                },
            ),
            (
                "fish".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(214, 222, 240)),
                },
            ),
            (
                "json".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(125, 176, 255)),
                },
            ),
            (
                "jsonc".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(125, 176, 255)),
                },
            ),
            (
                "json5".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(125, 176, 255)),
                },
            ),
            (
                "toml".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: None,
                },
            ),
            ("yaml".to_string(), rule_class(FileClass::Config)),
            ("yml".to_string(), rule_class(FileClass::Config)),
            ("ini".to_string(), rule_class(FileClass::Config)),
            ("conf".to_string(), rule_class(FileClass::Config)),
            ("cfg".to_string(), rule_class(FileClass::Config)),
            ("desktop".to_string(), rule_class(FileClass::Config)),
            ("ron".to_string(), rule_class(FileClass::Config)),
            ("env".to_string(), rule_class(FileClass::Config)),
            (
                "xml".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰗀".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "xsd".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰗀".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "xsl".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰗀".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "xslt".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰗀".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "md".to_string(),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            (
                "markdown".to_string(),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            (
                "mdown".to_string(),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            (
                "mkd".to_string(),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            (
                "mdx".to_string(),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            (
                "txt".to_string(),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("".to_string()),
                    color: Some(rgb(174, 184, 199)),
                },
            ),
            ("rst".to_string(), rule_class(FileClass::Document)),
            (
                "lock".to_string(),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(89, 222, 148)),
                },
            ),
            ("pdf".to_string(), rule_class(FileClass::Document)),
            (
                "epub".to_string(),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("󱗖".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            (
                "cbz".to_string(),
                RuleOverride {
                    class: Some(FileClass::Archive),
                    icon: Some("󱗖".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            ("doc".to_string(), rule_document_file()),
            ("docx".to_string(), rule_document_file()),
            ("docm".to_string(), rule_document_file()),
            ("odt".to_string(), rule_document_file()),
            ("ods".to_string(), rule_spreadsheet_file()),
            ("xlsx".to_string(), rule_spreadsheet_file()),
            ("xlsm".to_string(), rule_spreadsheet_file()),
            ("odp".to_string(), rule_presentation_file()),
            ("pptx".to_string(), rule_presentation_file()),
            ("pptm".to_string(), rule_presentation_file()),
            ("pages".to_string(), rule_document_file()),
            ("png".to_string(), rule_class(FileClass::Image)),
            ("jpg".to_string(), rule_class(FileClass::Image)),
            ("jpeg".to_string(), rule_class(FileClass::Image)),
            ("gif".to_string(), rule_class(FileClass::Image)),
            ("svg".to_string(), rule_class(FileClass::Image)),
            ("webp".to_string(), rule_class(FileClass::Image)),
            ("avif".to_string(), rule_class(FileClass::Image)),
            ("xcf".to_string(), rule_class(FileClass::Image)),
            ("ico".to_string(), rule_class(FileClass::Image)),
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
            ("iso".to_string(), rule_class(FileClass::Archive)),
            ("rpm".to_string(), rule_class(FileClass::Archive)),
            ("deb".to_string(), rule_class(FileClass::Archive)),
            ("apk".to_string(), rule_class(FileClass::Archive)),
            ("aab".to_string(), rule_class(FileClass::Archive)),
            ("apkg".to_string(), rule_class(FileClass::Archive)),
            ("zst".to_string(), rule_class(FileClass::Archive)),
            ("jar".to_string(), rule_class(FileClass::Archive)),
            ("zest".to_string(), rule_class(FileClass::Archive)),
            ("appimage".to_string(), rule_class(FileClass::Archive)),
            ("ttf".to_string(), rule_class(FileClass::Font)),
            ("otf".to_string(), rule_class(FileClass::Font)),
            ("woff".to_string(), rule_class(FileClass::Font)),
            ("woff2".to_string(), rule_class(FileClass::Font)),
            ("csv".to_string(), rule_class(FileClass::Data)),
            ("tsv".to_string(), rule_class(FileClass::Data)),
            ("sqlite".to_string(), rule_class(FileClass::Data)),
            ("db".to_string(), rule_class(FileClass::Data)),
            ("parquet".to_string(), rule_class(FileClass::Data)),
            ("torrent".to_string(), rule_class(FileClass::Data)),
            ("hash".to_string(), rule_class(FileClass::Data)),
            ("sha1".to_string(), rule_class(FileClass::Data)),
            ("sha256".to_string(), rule_class(FileClass::Data)),
            ("sha512".to_string(), rule_class(FileClass::Data)),
            ("md5".to_string(), rule_class(FileClass::Data)),
            ("log".to_string(), rule_class(FileClass::Document)),
            ("srt".to_string(), rule_class(FileClass::Document)),
            ("keys".to_string(), rule_class(FileClass::Config)),
            ("p12".to_string(), rule_class(FileClass::Config)),
            ("pfx".to_string(), rule_class(FileClass::Config)),
            ("pem".to_string(), rule_class(FileClass::Config)),
            ("crt".to_string(), rule_class(FileClass::Config)),
            ("cer".to_string(), rule_class(FileClass::Config)),
            ("csr".to_string(), rule_class(FileClass::Config)),
            ("key".to_string(), rule_class(FileClass::Config)),
            ("exe".to_string(), rule_class(FileClass::File)),
        ]);

        let files = HashMap::from([
            (
                normalize_key("Cargo.lock"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
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
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(210, 146, 89)),
                },
            ),
            (
                normalize_key("pnpm-lock.yaml"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(255, 184, 107)),
                },
            ),
            (
                normalize_key("yarn.lock"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(36, 217, 184)),
                },
            ),
            (
                normalize_key("bun.lock"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(247, 200, 94)),
                },
            ),
            (
                normalize_key("bun.lockb"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(247, 200, 94)),
                },
            ),
            (
                normalize_key("poetry.lock"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(141, 223, 109)),
                },
            ),
            (
                normalize_key("Pipfile.lock"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(89, 222, 148)),
                },
            ),
            (
                normalize_key("uv.lock"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(89, 222, 148)),
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
                normalize_key("Containerfile"),
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
                normalize_key(".terraform.lock.hcl"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                normalize_key("build.gradle"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(112, 182, 117)),
                },
            ),
            (
                normalize_key("settings.gradle"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(112, 182, 117)),
                },
            ),
            (
                normalize_key("init.gradle"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(112, 182, 117)),
                },
            ),
            (
                normalize_key("build.sbt"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(232, 90, 90)),
                },
            ),
            (
                normalize_key(".rprofile"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰟔".to_string()),
                    color: Some(rgb(95, 153, 219)),
                },
            ),
            (
                normalize_key("project.clj"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            (
                normalize_key("deps.edn"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            (
                normalize_key("bb.edn"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            (
                normalize_key("shadow-cljs.edn"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            (
                normalize_key("Justfile"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(255, 184, 107)),
                },
            ),
            (
                normalize_key(".justfile"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(255, 184, 107)),
                },
            ),
            (
                normalize_key("build.zig.zon"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(245, 173, 64)),
                },
            ),
            (
                normalize_key("README.md"),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            (
                normalize_key("AUTHORS"),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("󰭘".to_string()),
                    color: Some(rgb(155, 143, 199)),
                },
            ),
            (
                normalize_key("AUTHORS.md"),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("󰭘".to_string()),
                    color: Some(rgb(155, 143, 199)),
                },
            ),
            (
                normalize_key("AUTHORS.txt"),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("󰭘".to_string()),
                    color: Some(rgb(155, 143, 199)),
                },
            ),
            (
                normalize_key("CONTRIBUTORS"),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("󰭘".to_string()),
                    color: Some(rgb(155, 143, 199)),
                },
            ),
            (
                normalize_key("CONTRIBUTORS.md"),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("󰭘".to_string()),
                    color: Some(rgb(155, 143, 199)),
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
            (
                normalize_key("PKGBUILD"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(102, 187, 255)),
                },
            ),
        ]);

        Self {
            palette: Palette {
                bg: rgb(2, 5, 12),
                chrome: rgb(7, 13, 22),
                chrome_alt: rgb(11, 18, 32),
                panel: rgb(9, 16, 27),
                panel_alt: rgb(6, 11, 20),
                surface: rgb(16, 25, 42),
                elevated: rgb(21, 32, 54),
                border: rgb(53, 80, 111),
                text: rgb(237, 244, 255),
                muted: rgb(142, 162, 191),
                accent: rgb(126, 196, 255),
                accent_soft: rgb(20, 54, 87),
                accent_text: rgb(234, 245, 255),
                selected_bg: rgb(32, 64, 100),
                selected_border: rgb(149, 211, 255),
                selection_bar: rgb(255, 178, 86),
                sidebar_active: rgb(27, 56, 88),
                button_bg: rgb(14, 23, 38),
                button_disabled_bg: rgb(8, 16, 27),
                path_bg: rgb(12, 19, 32),
            },
            preview: PreviewTheme {
                code: CodePreviewPalette {
                    fg: rgb(215, 227, 244),
                    bg: rgb(10, 13, 18),
                    selection_bg: rgb(18, 42, 63),
                    selection_fg: rgb(242, 247, 255),
                    caret: rgb(18, 210, 255),
                    line_highlight: rgb(16, 21, 31),
                    line_number: rgb(123, 144, 167),
                    comment: rgb(111, 131, 153),
                    string: rgb(121, 231, 213),
                    constant: rgb(255, 166, 87),
                    keyword: rgb(255, 120, 198),
                    function: rgb(54, 215, 255),
                    r#type: rgb(179, 140, 255),
                    parameter: rgb(255, 216, 102),
                    tag: rgb(89, 222, 148),
                    operator: rgb(138, 231, 255),
                    r#macro: rgb(255, 143, 64),
                    invalid: rgb(255, 133, 133),
                },
            },
            classes,
            extensions,
            files,
            directories: HashMap::new(),
        }
    }

    fn from_config_str(config: &str) -> anyhow::Result<Self> {
        Self::apply_config_on(Self::default_theme(), config)
    }

    fn apply_config_on(mut theme: Self, config: &str) -> anyhow::Result<Self> {
        let parsed: ThemeFile = toml::from_str(config)?;
        theme.apply_overrides(parsed)?;
        Ok(theme)
    }

    fn apply_overrides(&mut self, parsed: ThemeFile) -> anyhow::Result<()> {
        if let Some(palette) = parsed.palette {
            apply_palette_overrides(&mut self.palette, palette)?;
        }
        if let Some(preview) = parsed.preview {
            apply_preview_overrides(&mut self.preview, preview)?;
        }

        if let Some(classes) = parsed.classes {
            for (name, override_style) in classes {
                let class = parse_class_name(&name)
                    .ok_or_else(|| anyhow::anyhow!("unknown class `{name}`"))?;
                let style = self
                    .classes
                    .entry(class)
                    .or_insert_with(|| default_class_style(class));
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
        let builtin_class = builtin_classify_path(path, kind);
        self.resolve_with_builtin_class(path, kind, builtin_class)
    }

    fn resolve_with_builtin_class(
        &self,
        path: &Path,
        kind: EntryKind,
        builtin_class: FileClass,
    ) -> ResolvedAppearance<'_> {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let normalized_name = normalize_key(file_name);
        let ext = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        let exact_rule = match kind {
            EntryKind::Directory => self.directories.get(&normalized_name),
            EntryKind::File => self.files.get(&normalized_name),
        };
        let ext_rule = (kind == EntryKind::File)
            .then(|| self.extensions.get(&ext))
            .flatten();
        let prefer_builtin_license = exact_rule.is_none() && builtin_class == FileClass::License;

        let class = exact_rule
            .and_then(|rule| rule.class)
            .or(prefer_builtin_license.then_some(FileClass::License))
            .or_else(|| ext_rule.and_then(|rule| rule.class))
            .unwrap_or(builtin_class);

        let base = self.classes.get(&class).unwrap_or_else(|| {
            self.classes
                .get(&FileClass::File)
                .expect("default file style")
        });

        let icon = exact_rule
            .and_then(|rule| rule.icon.as_deref())
            .or_else(|| {
                (!prefer_builtin_license)
                    .then(|| ext_rule.and_then(|rule| rule.icon.as_deref()))
                    .flatten()
            })
            .unwrap_or(base.icon.as_str());
        let color = exact_rule
            .and_then(|rule| rule.color)
            .or_else(|| {
                (!prefer_builtin_license)
                    .then(|| ext_rule.and_then(|rule| rule.color))
                    .flatten()
            })
            .unwrap_or(base.color);

        ResolvedAppearance {
            #[cfg(test)]
            class,
            icon,
            color,
        }
    }
}

fn apply_palette_overrides(
    palette: &mut Palette,
    overrides: PaletteOverride,
) -> anyhow::Result<()> {
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
    apply_palette_color(&mut palette.selection_bar, overrides.selection_bar)?;
    apply_palette_color(&mut palette.sidebar_active, overrides.sidebar_active)?;
    apply_palette_color(&mut palette.button_bg, overrides.button_bg)?;
    apply_palette_color(
        &mut palette.button_disabled_bg,
        overrides.button_disabled_bg,
    )?;
    apply_palette_color(&mut palette.path_bg, overrides.path_bg)?;
    Ok(())
}

fn apply_palette_color(target: &mut Color, value: Option<String>) -> anyhow::Result<()> {
    if let Some(value) = value {
        *target = parse_color(&value)?;
    }
    Ok(())
}

fn apply_preview_overrides(
    preview: &mut PreviewTheme,
    overrides: PreviewOverride,
) -> anyhow::Result<()> {
    if let Some(code) = overrides.code {
        apply_code_preview_overrides(&mut preview.code, code)?;
    }
    Ok(())
}

fn apply_code_preview_overrides(
    code: &mut CodePreviewPalette,
    overrides: CodePreviewOverride,
) -> anyhow::Result<()> {
    apply_palette_color(&mut code.fg, overrides.fg)?;
    apply_palette_color(&mut code.bg, overrides.bg)?;
    apply_palette_color(&mut code.selection_bg, overrides.selection_bg)?;
    apply_palette_color(&mut code.selection_fg, overrides.selection_fg)?;
    apply_palette_color(&mut code.caret, overrides.caret)?;
    apply_palette_color(&mut code.line_highlight, overrides.line_highlight)?;
    apply_palette_color(&mut code.line_number, overrides.line_number)?;
    apply_palette_color(&mut code.comment, overrides.comment)?;
    apply_palette_color(&mut code.string, overrides.string)?;
    apply_palette_color(&mut code.constant, overrides.constant)?;
    apply_palette_color(&mut code.keyword, overrides.keyword)?;
    apply_palette_color(&mut code.function, overrides.function)?;
    apply_palette_color(&mut code.r#type, overrides.r#type)?;
    apply_palette_color(&mut code.parameter, overrides.parameter)?;
    apply_palette_color(&mut code.tag, overrides.tag)?;
    apply_palette_color(&mut code.operator, overrides.operator)?;
    apply_palette_color(&mut code.r#macro, overrides.r#macro)?;
    apply_palette_color(&mut code.invalid, overrides.invalid)?;
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
        RuleOverrideDef::Class(class) => {
            Ok(rule_class(parse_class_name(&class).ok_or_else(|| {
                anyhow::anyhow!("unknown class `{class}`")
            })?))
        }
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
    match class {
        FileClass::Directory => ClassStyle {
            icon: "󰉋".to_string(),
            color: rgb(65, 143, 222),
        },
        FileClass::Code => ClassStyle {
            icon: "󰆍".to_string(),
            color: rgb(87, 196, 155),
        },
        FileClass::Config => ClassStyle {
            icon: "󰒓".to_string(),
            color: rgb(121, 188, 255),
        },
        FileClass::Document => ClassStyle {
            icon: "󰈙".to_string(),
            color: rgb(112, 182, 117),
        },
        FileClass::License => ClassStyle {
            icon: "󰿃".to_string(),
            color: rgb(245, 216, 91),
        },
        FileClass::Image => ClassStyle {
            icon: "󰋩".to_string(),
            color: rgb(86, 156, 214),
        },
        FileClass::Audio => ClassStyle {
            icon: "󰎆".to_string(),
            color: rgb(138, 110, 214),
        },
        FileClass::Video => ClassStyle {
            icon: "".to_string(),
            color: rgb(204, 112, 79),
        },
        FileClass::Archive => ClassStyle {
            icon: "󰗄".to_string(),
            color: rgb(207, 111, 63),
        },
        FileClass::Font => ClassStyle {
            icon: "󰛖".to_string(),
            color: rgb(196, 148, 92),
        },
        FileClass::Data => ClassStyle {
            icon: "󰆼".to_string(),
            color: rgb(92, 192, 201),
        },
        FileClass::File => ClassStyle {
            icon: "󰈔".to_string(),
            color: rgb(98, 109, 122),
        },
    }
}

fn rule_class(class: FileClass) -> RuleOverride {
    RuleOverride {
        class: Some(class),
        ..RuleOverride::default()
    }
}

fn rule_document_file() -> RuleOverride {
    RuleOverride {
        class: Some(FileClass::Document),
        icon: Some("󰈬".to_string()),
        color: Some(rgb(88, 142, 255)),
    }
}

fn rule_spreadsheet_file() -> RuleOverride {
    RuleOverride {
        class: Some(FileClass::Document),
        icon: Some("󱎏".to_string()),
        color: Some(rgb(78, 178, 116)),
    }
}

fn rule_presentation_file() -> RuleOverride {
    RuleOverride {
        class: Some(FileClass::Document),
        icon: Some("󱎐".to_string()),
        color: Some(rgb(232, 139, 63)),
    }
}

fn builtin_classify_path(path: &Path, kind: EntryKind) -> FileClass {
    file_info::inspect_path(path, kind).builtin_class
}

fn builtin_classify_entry(entry: &Entry) -> FileClass {
    let key = EntryClassCacheKey {
        path: entry.path.clone(),
        is_dir: entry.kind == EntryKind::Directory,
        size: entry.size,
        modified: fingerprint_time(entry.modified),
    };

    if let Some(class) = entry_class_cache()
        .lock()
        .expect("entry class cache lock")
        .get(&key)
        .copied()
    {
        return class;
    }

    let class = file_info::inspect_path_cached(&entry.path, entry.kind, entry.size, entry.modified)
        .builtin_class;
    entry_class_cache()
        .lock()
        .expect("entry class cache lock")
        .insert(key, class);
    class
}

fn fingerprint_time(modified: Option<SystemTime>) -> Option<(u64, u32)> {
    modified
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| (duration.as_secs(), duration.subsec_nanos()))
}

fn parse_class_name(name: &str) -> Option<FileClass> {
    match normalize_key(name).as_str() {
        "directory" | "dir" | "folder" => Some(FileClass::Directory),
        "code" => Some(FileClass::Code),
        "config" => Some(FileClass::Config),
        "document" | "doc" | "text" => Some(FileClass::Document),
        "license" | "licence" | "legal" => Some(FileClass::License),
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
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

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
            "default-light" => include_str!("../../../examples/themes/default-light/theme.toml"),
            "blush-light" => include_str!("../../../examples/themes/blush-light/theme.toml"),
            "amber-dusk" => include_str!("../../../examples/themes/amber-dusk/theme.toml"),
            "catppuccin-mocha" => {
                include_str!("../../../examples/themes/catppuccin-mocha/theme.toml")
            }
            "tokyo-night" => include_str!("../../../examples/themes/tokyo-night/theme.toml"),
            "navi" => include_str!("../../../examples/themes/navi/theme.toml"),
            "neon-cherry" => include_str!("../../../examples/themes/neon-cherry/theme.toml"),
            _ => panic!("unknown alternate example theme fixture: {name}"),
        }
    }

    fn load_alternate_example_theme(name: &str) -> Theme {
        Theme::from_config_str(alternate_example_theme_config(name)).unwrap_or_else(|error| {
            panic!("{name} example theme should parse as a user theme: {error}")
        })
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
        let theme = Theme::default_theme();
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
}
