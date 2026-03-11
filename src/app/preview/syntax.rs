use super::*;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::{path::Path, ptr, str::FromStr, sync::OnceLock};
use syntect::{
    easy::HighlightLines,
    highlighting::{
        Color as SyntectColor, FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem,
        ThemeSettings,
    },
    parsing::{SyntaxReference, SyntaxSet},
};

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static CODE_THEME: OnceLock<Theme> = OnceLock::new();

pub(super) fn find_code_syntax(
    path: &Path,
    hint: Option<&str>,
) -> Option<&'static SyntaxReference> {
    let syntax_set = syntax_set();
    let syntax = code_syntax_for(path, hint, syntax_set);
    if ptr::eq(syntax, syntax_set.find_syntax_plain_text()) {
        return None;
    }
    Some(syntax)
}

pub(super) fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

pub(super) fn render_code_preview(
    path: &Path,
    text: &str,
    language: Option<&str>,
    line_numbers: bool,
) -> Vec<Line<'static>> {
    let syntax_set = syntax_set();
    let syntax = code_syntax_for(path, language, syntax_set);
    let mut highlighter = HighlightLines::new(syntax, code_theme());

    let source_lines = super::collect_preview_lines(text);
    let number_width = super::line_number_width(source_lines.len());
    let mut rendered = Vec::new();

    for (index, line) in source_lines.iter().enumerate() {
        let mut spans = Vec::new();
        if line_numbers {
            spans.push(super::line_number_span(index + 1, number_width));
        } else {
            let preview = appearance::code_preview_palette();
            spans.push(Span::styled("│ ", Style::default().fg(preview.line_number)));
        }

        match highlighter.highlight_line(line, syntax_set) {
            Ok(ranges) => {
                for (style, segment) in ranges {
                    spans.push(Span::styled(
                        super::expand_tabs(segment),
                        ratatui_style_from_syntect(style),
                    ));
                }
            }
            Err(_) => spans.push(Span::styled(
                super::expand_tabs(line),
                Style::default().fg(appearance::palette().text),
            )),
        }
        rendered.push(Line::from(spans));
    }

    if rendered.is_empty() {
        rendered.push(Line::from("File is empty"));
    }
    rendered
}

fn code_syntax_for<'a>(
    path: &Path,
    language: Option<&str>,
    syntax_set: &'a SyntaxSet,
) -> &'a SyntaxReference {
    if let Some(language) = language
        && let Some(syntax) = syntax_set.find_syntax_by_token(language)
    {
        return syntax;
    }

    if let Ok(Some(syntax)) = syntax_set.find_syntax_for_file(path) {
        return syntax;
    }

    path.extension()
        .and_then(|extension| extension.to_str())
        .and_then(|extension| syntax_set.find_syntax_by_extension(extension))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
}

fn code_theme() -> &'static Theme {
    CODE_THEME.get_or_init(build_code_theme)
}

fn build_code_theme() -> Theme {
    let preview = appearance::code_preview_palette();
    Theme {
        name: Some("elio-preview".to_string()),
        author: Some("Elio".to_string()),
        settings: ThemeSettings {
            foreground: Some(syntect_color(preview.fg)),
            background: Some(syntect_color(preview.bg)),
            selection: Some(syntect_color(preview.selection_bg)),
            selection_foreground: Some(syntect_color(preview.selection_fg)),
            caret: Some(syntect_color(preview.caret)),
            line_highlight: Some(syntect_color(preview.line_highlight)),
            ..ThemeSettings::default()
        },
        scopes: vec![
            theme_item(
                "comment",
                Some(preview.comment),
                None,
                Some(FontStyle::ITALIC),
            ),
            theme_item("string", Some(preview.string), None, None),
            theme_item(
                "constant.numeric, constant.language, constant.character.escape",
                Some(preview.constant),
                None,
                None,
            ),
            theme_item(
                "keyword, storage",
                Some(preview.keyword),
                None,
                Some(FontStyle::BOLD),
            ),
            theme_item(
                "entity.name.function, entity.name.function.method, support.function, meta.function-call, variable.function",
                Some(preview.function),
                None,
                None,
            ),
            theme_item(
                "entity.name.type, support.type, support.class",
                Some(preview.r#type),
                None,
                None,
            ),
            theme_item(
                "variable.parameter, entity.other.attribute-name, support.type.property-name, meta.attribute-with-value entity.other.attribute-name",
                Some(preview.parameter),
                None,
                None,
            ),
            theme_item(
                "entity.name.tag, meta.tag, entity.name.tag.doctype, meta.tag.sgml.doctype",
                Some(preview.tag),
                None,
                None,
            ),
            theme_item(
                "keyword.operator, punctuation.definition.tag, punctuation.separator.key-value, punctuation.separator.attribute-value",
                Some(preview.operator),
                None,
                None,
            ),
            theme_item(
                "entity.name.function.preprocessor, support.function.macro",
                Some(preview.r#macro),
                None,
                Some(FontStyle::BOLD),
            ),
            theme_item(
                "invalid",
                Some(preview.invalid),
                None,
                Some(FontStyle::BOLD),
            ),
        ],
    }
}

fn theme_item(
    selectors: &str,
    foreground: Option<Color>,
    background: Option<Color>,
    font_style: Option<FontStyle>,
) -> ThemeItem {
    ThemeItem {
        scope: ScopeSelectors::from_str(selectors).expect("preview theme selectors should parse"),
        style: StyleModifier {
            foreground: foreground.map(syntect_color),
            background: background.map(syntect_color),
            font_style,
        },
    }
}

fn syntect_color(color: Color) -> SyntectColor {
    match color {
        Color::Rgb(r, g, b) => SyntectColor { r, g, b, a: 0xFF },
        Color::Black => SyntectColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0xFF,
        },
        Color::Red => SyntectColor {
            r: 0xFF,
            g: 0,
            b: 0,
            a: 0xFF,
        },
        Color::Green => SyntectColor {
            r: 0,
            g: 0xFF,
            b: 0,
            a: 0xFF,
        },
        Color::Yellow => SyntectColor {
            r: 0xFF,
            g: 0xFF,
            b: 0,
            a: 0xFF,
        },
        Color::Blue => SyntectColor {
            r: 0,
            g: 0,
            b: 0xFF,
            a: 0xFF,
        },
        Color::Magenta => SyntectColor {
            r: 0xFF,
            g: 0,
            b: 0xFF,
            a: 0xFF,
        },
        Color::Cyan => SyntectColor {
            r: 0,
            g: 0xFF,
            b: 0xFF,
            a: 0xFF,
        },
        Color::Gray => SyntectColor {
            r: 0x80,
            g: 0x80,
            b: 0x80,
            a: 0xFF,
        },
        Color::DarkGray => SyntectColor {
            r: 0x55,
            g: 0x55,
            b: 0x55,
            a: 0xFF,
        },
        Color::LightRed => SyntectColor {
            r: 0xFF,
            g: 0x55,
            b: 0x55,
            a: 0xFF,
        },
        Color::LightGreen => SyntectColor {
            r: 0x55,
            g: 0xFF,
            b: 0x55,
            a: 0xFF,
        },
        Color::LightYellow => SyntectColor {
            r: 0xFF,
            g: 0xFF,
            b: 0x55,
            a: 0xFF,
        },
        Color::LightBlue => SyntectColor {
            r: 0x55,
            g: 0x55,
            b: 0xFF,
            a: 0xFF,
        },
        Color::LightMagenta => SyntectColor {
            r: 0xFF,
            g: 0x55,
            b: 0xFF,
            a: 0xFF,
        },
        Color::LightCyan => SyntectColor {
            r: 0x55,
            g: 0xFF,
            b: 0xFF,
            a: 0xFF,
        },
        Color::White => SyntectColor {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF,
            a: 0xFF,
        },
        Color::Indexed(value) => SyntectColor {
            r: value,
            g: value,
            b: value,
            a: 0xFF,
        },
        Color::Reset => SyntectColor {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF,
            a: 0xFF,
        },
    }
}

fn ratatui_style_from_syntect(style: syntect::highlighting::Style) -> Style {
    let mut ratatui = Style::default().fg(Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    ));

    if style.font_style.contains(FontStyle::BOLD) {
        ratatui = ratatui.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        ratatui = ratatui.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        ratatui = ratatui.add_modifier(Modifier::UNDERLINED);
    }

    ratatui
}
