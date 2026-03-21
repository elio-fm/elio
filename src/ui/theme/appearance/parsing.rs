use super::{
    default_class_style, rgb, rule_class,
    types::{CodePreviewPalette, Palette, PreviewTheme, RuleOverride, Theme},
};
use crate::app::FileClass;
use ratatui::style::Color;
use serde::Deserialize;
use std::collections::HashMap;

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

impl Theme {
    pub(super) fn from_config_str(config: &str) -> anyhow::Result<Self> {
        Self::apply_config_on(Self::default_theme(), config)
    }

    pub(super) fn apply_config_on(mut theme: Self, config: &str) -> anyhow::Result<Self> {
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

pub(super) fn parse_class_name(name: &str) -> Option<FileClass> {
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

pub(super) fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub(super) fn parse_color(value: &str) -> anyhow::Result<Color> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 {
        anyhow::bail!("invalid color {value}");
    }

    let red = u8::from_str_radix(&hex[0..2], 16)?;
    let green = u8::from_str_radix(&hex[2..4], 16)?;
    let blue = u8::from_str_radix(&hex[4..6], 16)?;
    Ok(rgb(red, green, blue))
}
