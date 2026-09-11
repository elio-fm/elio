use super::item_styling::{ClassStyle, RuleOverride};
use crate::{config, file_classification::FileClass};
use ratatui::style::Color;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub(super) const DEFAULT_THEME_TOML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/themes/default/theme.toml"
));

static ACTIVE_THEME: OnceLock<Theme> = OnceLock::new();

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub bg: Color,
    pub chrome: Color,
    pub chrome_alt: Color,
    pub chip_text: Color,
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
    pub yank_bar: Color,
    pub cut_bar: Color,
    pub progress_bar: Color,
    pub grid_selection_band: Color,
    pub grid_yank_band: Color,
    pub grid_cut_band: Color,
    pub trash_bar: Color,
    pub restore_bar: Color,
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
pub(super) struct PreviewTheme {
    pub(super) code: CodePreviewPalette,
}

#[derive(Clone)]
pub(super) struct Theme {
    pub(super) palette: Palette,
    pub(super) preview: PreviewTheme,
    pub(super) classes: HashMap<FileClass, ClassStyle>,
    pub(super) extensions: HashMap<String, RuleOverride>,
    pub(super) files: HashMap<String, RuleOverride>,
    pub(super) directories: HashMap<String, RuleOverride>,
}

pub(crate) fn initialize(path: Option<&Path>) -> anyhow::Result<()> {
    if ACTIVE_THEME.get().is_none() {
        let theme = load_theme_from_disk(path)?;
        let _ = ACTIVE_THEME.set(theme);
    }
    Ok(())
}

pub(crate) fn palette() -> Palette {
    active_theme().palette
}

pub(crate) fn code_preview_palette() -> CodePreviewPalette {
    active_theme().preview.code
}

pub(super) fn active_theme() -> &'static Theme {
    ACTIVE_THEME.get_or_init(Theme::default_theme)
}

pub(super) fn load_theme_from_disk(override_path: Option<&Path>) -> anyhow::Result<Theme> {
    let is_override = override_path.is_some();
    let Some(path) = override_path.map(Path::to_path_buf).or_else(theme_path) else {
        return Ok(Theme::default_theme());
    };
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if !is_override && error.kind() == io::ErrorKind::NotFound => {
            return Ok(Theme::default_theme());
        }
        Err(error) if is_override => {
            anyhow::bail!(
                "elio: failed to read theme from {}: {error}",
                path.display()
            );
        }
        Err(error) => {
            eprintln!(
                "elio: failed to read theme from {}: {error}",
                path.display()
            );
            return Ok(Theme::default_theme());
        }
    };

    Ok(match Theme::from_config_str(&contents) {
        Ok(theme) => theme,
        Err(error) => {
            eprintln!(
                "elio: failed to load theme from {}: {error}",
                path.display()
            );
            Theme::default_theme()
        }
    })
}

fn theme_path() -> Option<PathBuf> {
    config::config_dir().map(|dir| dir.join("theme.toml"))
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
    chip_text: Option<String>,
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
    yank_bar: Option<String>,
    cut_bar: Option<String>,
    progress_bar: Option<String>,
    grid_selection_band: Option<String>,
    grid_yank_band: Option<String>,
    grid_cut_band: Option<String>,
    trash_bar: Option<String>,
    restore_bar: Option<String>,
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

impl Theme {
    pub(super) fn default_theme() -> Self {
        Self::from_default_config_str(DEFAULT_THEME_TOML)
            .unwrap_or_else(|error| panic!("elio: failed to load built-in default theme: {error}"))
    }

    pub(super) fn from_default_config_str(config: &str) -> anyhow::Result<Self> {
        let parsed: ThemeFile = toml::from_str(config)?;
        parsed.into_complete_theme()
    }

    pub(super) fn from_config_str(config: &str) -> anyhow::Result<Self> {
        Self::apply_config_on(Self::default_theme(), config)
    }

    pub(super) fn apply_config_on(mut theme: Self, config: &str) -> anyhow::Result<Self> {
        let parsed: ThemeFile = toml::from_str(config)?;
        theme.apply_overrides(parsed)?;
        Ok(theme)
    }

    fn apply_overrides(&mut self, parsed: ThemeFile) -> anyhow::Result<()> {
        let mut symlink_directory_color_overridden = false;
        let mut broken_symlink_color_overridden = false;

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
                let ClassStyleOverride { icon, color } = override_style;
                let style = self
                    .classes
                    .get_mut(&class)
                    .expect("built-in theme defines every file class");
                if let Some(icon) = icon {
                    style.icon = icon;
                }
                if let Some(color) = color {
                    if class == FileClass::SymlinkDirectory {
                        symlink_directory_color_overridden = true;
                    }
                    if class == FileClass::BrokenSymlink {
                        broken_symlink_color_overridden = true;
                    }
                    style.color = parse_color(&color)?;
                }
            }
        }
        self.apply_derived_symlink_class_colors(
            symlink_directory_color_overridden,
            broken_symlink_color_overridden,
        );

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

    fn apply_derived_symlink_class_colors(
        &mut self,
        symlink_directory_color_overridden: bool,
        broken_symlink_color_overridden: bool,
    ) {
        if !symlink_directory_color_overridden
            && let Some(color) = self
                .classes
                .get(&FileClass::Directory)
                .map(|style| style.color)
            && let Some(style) = self.classes.get_mut(&FileClass::SymlinkDirectory)
        {
            style.color = color;
        }

        let invalid_color = self.preview.code.invalid;
        if !broken_symlink_color_overridden
            && let Some(style) = self.classes.get_mut(&FileClass::BrokenSymlink)
        {
            style.color = invalid_color;
        }
    }
}

impl ThemeFile {
    fn into_complete_theme(self) -> anyhow::Result<Theme> {
        Ok(Theme {
            palette: required(self.palette, "palette")?.into_complete_palette()?,
            preview: required(self.preview, "preview")?.into_complete_preview()?,
            classes: parse_complete_classes(required(self.classes, "classes")?)?,
            extensions: parse_rule_map(required(self.extensions, "extensions")?)?,
            files: parse_rule_map(required(self.files, "files")?)?,
            directories: parse_rule_map(required(self.directories, "directories")?)?,
        })
    }
}

impl PaletteOverride {
    fn into_complete_palette(self) -> anyhow::Result<Palette> {
        macro_rules! color {
            ($field:ident) => {
                parse_required_color(self.$field, concat!("palette.", stringify!($field)))?
            };
        }

        Ok(Palette {
            bg: color!(bg),
            chrome: color!(chrome),
            chrome_alt: color!(chrome_alt),
            chip_text: color!(chip_text),
            panel: color!(panel),
            panel_alt: color!(panel_alt),
            surface: color!(surface),
            elevated: color!(elevated),
            border: color!(border),
            text: color!(text),
            muted: color!(muted),
            accent: color!(accent),
            accent_soft: color!(accent_soft),
            accent_text: color!(accent_text),
            selected_bg: color!(selected_bg),
            selected_border: color!(selected_border),
            selection_bar: color!(selection_bar),
            yank_bar: color!(yank_bar),
            cut_bar: color!(cut_bar),
            progress_bar: color!(progress_bar),
            grid_selection_band: color!(grid_selection_band),
            grid_yank_band: color!(grid_yank_band),
            grid_cut_band: color!(grid_cut_band),
            trash_bar: color!(trash_bar),
            restore_bar: color!(restore_bar),
            sidebar_active: color!(sidebar_active),
            button_bg: color!(button_bg),
            button_disabled_bg: color!(button_disabled_bg),
            path_bg: color!(path_bg),
        })
    }
}

impl PreviewOverride {
    fn into_complete_preview(self) -> anyhow::Result<PreviewTheme> {
        Ok(PreviewTheme {
            code: required(self.code, "preview.code")?.into_complete_palette()?,
        })
    }
}

impl CodePreviewOverride {
    fn into_complete_palette(self) -> anyhow::Result<CodePreviewPalette> {
        macro_rules! color {
            ($field:ident) => {
                parse_required_color(self.$field, concat!("preview.code.", stringify!($field)))?
            };
        }

        Ok(CodePreviewPalette {
            fg: color!(fg),
            bg: color!(bg),
            selection_bg: color!(selection_bg),
            selection_fg: color!(selection_fg),
            caret: color!(caret),
            line_highlight: color!(line_highlight),
            line_number: color!(line_number),
            comment: color!(comment),
            string: color!(string),
            constant: color!(constant),
            keyword: color!(keyword),
            function: color!(function),
            r#type: color!(r#type),
            parameter: color!(parameter),
            tag: color!(tag),
            operator: color!(operator),
            r#macro: color!(r#macro),
            invalid: color!(invalid),
        })
    }
}

fn required<T>(value: Option<T>, name: &str) -> anyhow::Result<T> {
    value.ok_or_else(|| anyhow::anyhow!("built-in theme is missing `{name}`"))
}

fn parse_required_color(value: Option<String>, name: &str) -> anyhow::Result<Color> {
    parse_color(&required(value, name)?)
}

fn parse_complete_classes(
    source: HashMap<String, ClassStyleOverride>,
) -> anyhow::Result<HashMap<FileClass, ClassStyle>> {
    let mut classes = HashMap::new();
    for (name, style) in source {
        let class =
            parse_class_name(&name).ok_or_else(|| anyhow::anyhow!("unknown class `{name}`"))?;
        classes.insert(
            class,
            ClassStyle {
                icon: required(style.icon, &format!("classes.{name}.icon"))?,
                color: parse_required_color(style.color, &format!("classes.{name}.color"))?,
            },
        );
    }

    for name in [
        "directory",
        "symlink_directory",
        "broken_symlink",
        "code",
        "config",
        "document",
        "license",
        "image",
        "audio",
        "video",
        "archive",
        "font",
        "data",
        "file",
    ] {
        let class = parse_class_name(name).expect("known file class");
        if !classes.contains_key(&class) {
            anyhow::bail!("built-in theme is missing `classes.{name}`");
        }
    }

    Ok(classes)
}

fn parse_rule_map(
    source: HashMap<String, RuleOverrideDef>,
) -> anyhow::Result<HashMap<String, RuleOverride>> {
    let mut rules = HashMap::with_capacity(source.len());
    apply_rule_map(&mut rules, source)?;
    Ok(rules)
}

fn apply_palette_overrides(
    palette: &mut Palette,
    overrides: PaletteOverride,
) -> anyhow::Result<()> {
    apply_palette_color(&mut palette.bg, overrides.bg)?;
    apply_palette_color(&mut palette.chrome, overrides.chrome)?;
    apply_palette_color(&mut palette.chrome_alt, overrides.chrome_alt)?;
    apply_palette_color(&mut palette.chip_text, overrides.chip_text)?;
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
    apply_palette_color(&mut palette.yank_bar, overrides.yank_bar)?;
    apply_palette_color(&mut palette.cut_bar, overrides.cut_bar)?;
    apply_palette_color(&mut palette.progress_bar, overrides.progress_bar)?;
    apply_palette_color(
        &mut palette.grid_selection_band,
        overrides.grid_selection_band,
    )?;
    apply_palette_color(&mut palette.grid_yank_band, overrides.grid_yank_band)?;
    apply_palette_color(&mut palette.grid_cut_band, overrides.grid_cut_band)?;
    apply_palette_color(&mut palette.trash_bar, overrides.trash_bar)?;
    apply_palette_color(&mut palette.restore_bar, overrides.restore_bar)?;
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
        RuleOverrideDef::Class(class) => Ok(RuleOverride {
            class: Some(
                parse_class_name(&class)
                    .ok_or_else(|| anyhow::anyhow!("unknown class `{class}`"))?,
            ),
            ..RuleOverride::default()
        }),
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

pub(super) fn parse_class_name(name: &str) -> Option<FileClass> {
    match normalize_key(name).as_str() {
        "directory" | "dir" | "folder" => Some(FileClass::Directory),
        "symlink_directory" => Some(FileClass::SymlinkDirectory),
        "broken_symlink" => Some(FileClass::BrokenSymlink),
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

pub(super) fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub(super) fn parse_color(value: &str) -> anyhow::Result<Color> {
    let trimmed = value.trim();
    let normalized = trimmed.to_ascii_lowercase();
    match normalized.as_str() {
        "none" | "transparent" => return Ok(Color::Reset),
        "ansi-black" => return Ok(Color::Black),
        "ansi-red" => return Ok(Color::Red),
        "ansi-green" => return Ok(Color::Green),
        "ansi-yellow" => return Ok(Color::Yellow),
        "ansi-blue" => return Ok(Color::Blue),
        "ansi-magenta" => return Ok(Color::Magenta),
        "ansi-cyan" => return Ok(Color::Cyan),
        "ansi-white" => return Ok(Color::Gray),
        "ansi-bright-black" => return Ok(Color::DarkGray),
        "ansi-bright-red" => return Ok(Color::LightRed),
        "ansi-bright-green" => return Ok(Color::LightGreen),
        "ansi-bright-yellow" => return Ok(Color::LightYellow),
        "ansi-bright-blue" => return Ok(Color::LightBlue),
        "ansi-bright-magenta" => return Ok(Color::LightMagenta),
        "ansi-bright-cyan" => return Ok(Color::LightCyan),
        "ansi-bright-white" => return Ok(Color::White),
        _ => {}
    }
    if let Some(index) = normalized.strip_prefix("indexed-") {
        return Ok(Color::Indexed(index.parse()?));
    }

    let hex = trimmed.trim_start_matches('#');
    if hex.len() != 6 {
        anyhow::bail!("invalid color {value}");
    }

    let red = u8::from_str_radix(&hex[0..2], 16)?;
    let green = u8::from_str_radix(&hex[2..4], 16)?;
    let blue = u8::from_str_radix(&hex[4..6], 16)?;
    Ok(Color::Rgb(red, green, blue))
}
