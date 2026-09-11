use super::scope_styling::semantic_role_for_token;
use super::syntax_loading::{find_syntax, syntax_set};
use super::*;
use crate::preview::appearance as theme;
use crate::preview::code::highlighting_styles::SemanticRole;
use std::str::FromStr;
use syntect::{
    easy::ScopeRangeIterator,
    parsing::{ParseState, ScopeStack},
};

fn span_color(line: &Line<'_>, token: &str) -> Option<ratatui::style::Color> {
    line.spans
        .iter()
        .find(|span| span.content.contains(token))
        .and_then(|span| span.style.fg)
}

fn palette_colors() -> Vec<ratatui::style::Color> {
    let palette = theme::code_preview_palette();
    vec![
        palette.fg,
        palette.bg,
        palette.selection_bg,
        palette.selection_fg,
        palette.caret,
        palette.line_highlight,
        palette.line_number,
        palette.comment,
        palette.string,
        palette.constant,
        palette.keyword,
        palette.function,
        palette.r#type,
        palette.parameter,
        palette.tag,
        palette.operator,
        palette.r#macro,
        palette.invalid,
    ]
}

fn token_scopes(code_syntax: &str, text: &str) -> Vec<(String, String)> {
    let syntax_set = syntax_set();
    let syntax = find_syntax(syntax_set, code_syntax).expect("syntax should exist");
    let mut parse_state = ParseState::new(syntax);
    let mut scope_stack = ScopeStack::new();
    let mut tokens = Vec::new();

    for line in text.lines() {
        // Mirror the render path: append \n so newlines-mode grammars
        // properly terminate line comments (same fix as in render.rs).
        let line_with_nl = format!("{line}\n");
        let ops = parse_state
            .parse_line(&line_with_nl, syntax_set)
            .expect("line should parse");
        for (range, op) in ScopeRangeIterator::new(&ops, &line_with_nl) {
            scope_stack.apply(op).expect("scope op should apply");
            let token = line_with_nl[range].trim_end_matches('\n');
            if !token.is_empty() {
                tokens.push((token.to_string(), scope_stack.to_string()));
            }
        }
    }

    tokens
}

mod scope_styling;
mod supported_syntaxes;
mod syntax_rendering;
