mod classes;
mod extensions;
mod files;
mod palette;
mod shared;

use self::{
    classes::base_class_styles,
    extensions::default_extension_rules,
    files::default_file_rules,
    palette::{default_palette, default_preview_theme},
};
pub(in crate::ui::theme::appearance) use self::{
    classes::default_class_style,
    shared::{normalize_key, rgb, rule_class},
};
use super::types::Theme;
use std::collections::HashMap;

impl Theme {
    pub(super) fn base_theme() -> Self {
        Self {
            palette: default_palette(),
            preview: default_preview_theme(),
            classes: base_class_styles(),
            extensions: default_extension_rules(),
            files: default_file_rules(),
            directories: HashMap::new(),
        }
    }
}
