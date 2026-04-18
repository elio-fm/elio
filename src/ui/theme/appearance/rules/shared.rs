use super::super::types::RuleOverride;
use crate::core::FileClass;
use ratatui::style::Color;

pub(in crate::ui::theme::appearance) fn rule_class(class: FileClass) -> RuleOverride {
    RuleOverride {
        class: Some(class),
        ..RuleOverride::default()
    }
}

pub(super) fn rule_document_file() -> RuleOverride {
    RuleOverride {
        class: Some(FileClass::Document),
        icon: Some("󰈬".to_string()),
        color: Some(rgb(88, 142, 255)),
    }
}

pub(super) fn rule_ebook_file() -> RuleOverride {
    RuleOverride {
        class: Some(FileClass::Document),
        icon: Some("󱗖".to_string()),
        color: Some(rgb(211, 170, 124)),
    }
}

pub(super) fn rule_spreadsheet_file() -> RuleOverride {
    RuleOverride {
        class: Some(FileClass::Document),
        icon: Some("󱎏".to_string()),
        color: Some(rgb(78, 178, 116)),
    }
}

pub(super) fn rule_presentation_file() -> RuleOverride {
    RuleOverride {
        class: Some(FileClass::Document),
        icon: Some("󱎐".to_string()),
        color: Some(rgb(232, 139, 63)),
    }
}

pub(in crate::ui::theme::appearance) fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub(in crate::ui::theme::appearance) fn rgb(red: u8, green: u8, blue: u8) -> Color {
    Color::Rgb(red, green, blue)
}
