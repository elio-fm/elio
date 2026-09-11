use crate::preview::appearance::CodePalette;
use ratatui::style::Color;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SemanticRole {
    Fg,
    Comment,
    String,
    Constant,
    Keyword,
    Function,
    Type,
    Parameter,
    Tag,
    Operator,
    Macro,
    Invalid,
}

pub(super) fn role_color(role: SemanticRole, palette: CodePalette) -> Color {
    match role {
        SemanticRole::Fg => palette.fg,
        SemanticRole::Comment => palette.comment,
        SemanticRole::String => palette.string,
        SemanticRole::Constant => palette.constant,
        SemanticRole::Keyword => palette.keyword,
        SemanticRole::Function => palette.function,
        SemanticRole::Type => palette.r#type,
        SemanticRole::Parameter => palette.parameter,
        SemanticRole::Tag => palette.tag,
        SemanticRole::Operator => palette.operator,
        SemanticRole::Macro => palette.r#macro,
        SemanticRole::Invalid => palette.invalid,
    }
}
