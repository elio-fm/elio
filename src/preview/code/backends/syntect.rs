use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use std::sync::OnceLock;
use syntect::{
    easy::HighlightLines,
    highlighting::{FontStyle, Style as SyntectStyle, Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};

// Keep syntect routing opt-in until each language family is verified against the bundled syntax set.
const ENABLED_SYNTAXES: &[&str] = &[];
const DEFAULT_THEME_NAMES: &[&str] = &["InspiredGitHub", "base16-ocean.dark"];

pub(in crate::preview::code) fn is_enabled(code_syntax: &str) -> bool {
    ENABLED_SYNTAXES.contains(&code_syntax)
}

pub(in crate::preview::code) fn render_syntect_code_preview<F>(
    code_syntax: &str,
    text: &str,
    line_numbers: bool,
    line_limit: usize,
    canceled: &F,
) -> Result<Vec<Line<'static>>, ()>
where
    F: Fn() -> bool,
{
    let Some(theme) = theme() else {
        return Err(());
    };
    let syntax_set = syntax_set();
    let Some(syntax) = find_syntax(syntax_set, code_syntax) else {
        return Err(());
    };

    let source_lines = crate::preview::collect_preview_lines_with_limit(
        text,
        crate::preview::clamp_code_preview_line_limit(line_limit),
    );
    let number_width = crate::preview::line_number_width(source_lines.len());
    let code_palette = crate::ui::theme::code_preview_palette();
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut rendered = Vec::new();

    for (index, line) in source_lines.iter().enumerate() {
        if canceled() {
            break;
        }

        let mut spans = Vec::new();
        if line_numbers {
            spans.push(crate::preview::line_number_span(index + 1, number_width));
        } else {
            spans.push(Span::styled(
                "│ ",
                Style::default().fg(code_palette.line_number),
            ));
        }

        let highlighted = highlighter
            .highlight_line(line, syntax_set)
            .map_err(|_| ())?;
        spans.extend(highlighted.iter().map(syntect_span));
        rendered.push(Line::from(spans));
    }

    if rendered.is_empty() && !canceled() {
        rendered.push(Line::from("File is empty"));
    }

    Ok(rendered)
}

fn syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme() -> Option<&'static Theme> {
    static THEME: OnceLock<Option<Theme>> = OnceLock::new();
    THEME
        .get_or_init(|| {
            let themes = ThemeSet::load_defaults().themes;
            DEFAULT_THEME_NAMES
                .iter()
                .find_map(|name| themes.get(*name).cloned())
                .or_else(|| themes.into_values().next())
        })
        .as_ref()
}

fn find_syntax<'a>(syntax_set: &'a SyntaxSet, code_syntax: &str) -> Option<&'a SyntaxReference> {
    let lookup_token = syntect_lookup_token(code_syntax);
    syntax_set
        .find_syntax_by_token(lookup_token)
        .or_else(|| syntax_set.find_syntax_by_extension(lookup_token))
}

fn syntect_lookup_token(code_syntax: &str) -> &str {
    match code_syntax {
        "javascript" => "js",
        "typescript" => "ts",
        "rust" => "rs",
        "kotlin" => "kt",
        "ruby" => "rb",
        "python" => "py",
        "bash" | "zsh" | "ksh" | "fish" => "sh",
        "make" => "makefile",
        _ => code_syntax,
    }
}

fn syntect_span(style: &(SyntectStyle, &str)) -> Span<'static> {
    let mut rendered_style = Style::default().fg(ratatui::style::Color::Rgb(
        style.0.foreground.r,
        style.0.foreground.g,
        style.0.foreground.b,
    ));

    if style.0.font_style.contains(FontStyle::BOLD) {
        rendered_style = rendered_style.add_modifier(Modifier::BOLD);
    }
    if style.0.font_style.contains(FontStyle::ITALIC) {
        rendered_style = rendered_style.add_modifier(Modifier::ITALIC);
    }
    if style.0.font_style.contains(FontStyle::UNDERLINE) {
        rendered_style = rendered_style.add_modifier(Modifier::UNDERLINED);
    }

    Span::styled(crate::preview::expand_tabs(style.1), rendered_style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_syntaxes_cover_initial_canaries() {
        let syntax_set = syntax_set();

        for code_syntax in [
            "javascript",
            "rust",
            "go",
            "bash",
            "html",
            "xml",
            "css",
            "lua",
        ] {
            assert!(
                find_syntax(syntax_set, code_syntax).is_some(),
                "missing syntect syntax for {code_syntax}"
            );
        }
    }

    #[test]
    fn direct_syntect_rendering_supports_registry_canonical_ids() {
        let rendered = render_syntect_code_preview("rust", "fn main() {}\n", true, 20, &|| false)
            .expect("rust syntax should render through syntect");

        assert!(
            rendered[0]
                .spans
                .iter()
                .any(|span| span.content.contains("fn"))
        );
    }

    #[test]
    fn missing_bundled_syntaxes_return_errors_for_safe_fallback() {
        for code_syntax in ["typescript", "tsx", "nix", "cmake"] {
            assert!(
                render_syntect_code_preview(code_syntax, "sample\n", true, 20, &|| false).is_err(),
                "expected {code_syntax} to fail until a curated syntect bundle is added"
            );
        }
    }
}
