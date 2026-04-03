use crate::core::FileClass;
use ratatui::style::Color;
use std::{collections::HashMap, path::PathBuf};

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
    pub yank_bar: Color,
    pub cut_bar: Color,
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
pub(super) struct ClassStyle {
    pub(super) icon: String,
    pub(super) color: Color,
}

#[derive(Clone, Default)]
pub(super) struct RuleOverride {
    pub(super) class: Option<FileClass>,
    pub(super) icon: Option<String>,
    pub(super) color: Option<Color>,
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

pub(crate) struct ResolvedAppearance<'a> {
    #[cfg(test)]
    pub class: FileClass,
    pub icon: &'a str,
    pub color: Color,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct EntryClassCacheKey {
    pub(super) path: PathBuf,
    pub(super) is_dir: bool,
    pub(super) size: u64,
    pub(super) modified: Option<(u64, u32)>,
}
