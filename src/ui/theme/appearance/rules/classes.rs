use super::super::types::ClassStyle;
use super::shared::rgb;
use crate::core::FileClass;
use std::collections::HashMap;

pub(super) fn base_class_styles() -> HashMap<FileClass, ClassStyle> {
    HashMap::from([
        (
            FileClass::Directory,
            default_class_style(FileClass::Directory),
        ),
        (FileClass::Code, default_class_style(FileClass::Code)),
        (FileClass::Config, default_class_style(FileClass::Config)),
        (
            FileClass::Document,
            default_class_style(FileClass::Document),
        ),
        (FileClass::License, default_class_style(FileClass::License)),
        (FileClass::Image, default_class_style(FileClass::Image)),
        (FileClass::Audio, default_class_style(FileClass::Audio)),
        (FileClass::Video, default_class_style(FileClass::Video)),
        (FileClass::Archive, default_class_style(FileClass::Archive)),
        (FileClass::Font, default_class_style(FileClass::Font)),
        (FileClass::Data, default_class_style(FileClass::Data)),
        (FileClass::File, default_class_style(FileClass::File)),
    ])
}

pub(in crate::ui::theme::appearance) fn default_class_style(class: FileClass) -> ClassStyle {
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
