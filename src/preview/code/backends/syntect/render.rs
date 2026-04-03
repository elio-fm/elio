use super::semantics::{SemanticRole, role_color, semantic_role_for_token};
use crate::preview::appearance;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use syntect::{
    easy::ScopeRangeIterator,
    parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet},
};

pub(super) fn render_syntect_code_preview<F>(
    text: &str,
    syntax_set: &SyntaxSet,
    syntax: &SyntaxReference,
    line_numbers: bool,
    line_limit: usize,
    canceled: &F,
) -> Result<Vec<Line<'static>>, ()>
where
    F: Fn() -> bool,
{
    let source_lines = crate::preview::collect_preview_lines_with_limit(
        text,
        crate::preview::clamp_code_preview_line_limit(line_limit),
    );
    let number_width = crate::preview::line_number_width(source_lines.len());
    let code_palette = appearance::code_palette();
    let mut parse_state = ParseState::new(syntax);
    let mut scope_stack = ScopeStack::new();
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

        // Append \n so that "newlines" mode syntaxes (loaded with
        // load_defaults_newlines) properly terminate line comments and other
        // line-scoped constructs. Without it, a trailing -- or # comment on
        // one line bleeds its scope into every subsequent line.
        let line_with_nl = format!("{line}\n");
        let ops = parse_state
            .parse_line(&line_with_nl, syntax_set)
            .map_err(|_| ())?;

        // Accumulate consecutive tokens of the same style into a single span.
        // Syntect emits one range per grammar token (punctuation, keyword, etc.),
        // but adjacent tokens that map to the same semantic role produce identical
        // styles. Merging them reduces the span count per line, which directly
        // lowers the number of terminal escape sequences written on each repaint.
        let mut pending_text = String::new();
        let mut pending_style: Option<Style> = None;

        for (range, op) in ScopeRangeIterator::new(&ops, &line_with_nl) {
            scope_stack.apply(op).map_err(|_| ())?;
            let token = line_with_nl[range].trim_end_matches('\n');
            if token.is_empty() {
                continue;
            }
            let role = semantic_role_for_token(token, scope_stack.as_slice());
            let mut style = Style::default().fg(role_color(role, code_palette));
            if role == SemanticRole::Invalid {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            let expanded = crate::preview::expand_tabs(token);
            match pending_style {
                Some(s) if s == style => {
                    pending_text.push_str(&expanded);
                }
                Some(s) => {
                    spans.push(Span::styled(std::mem::take(&mut pending_text), s));
                    pending_text = expanded;
                    pending_style = Some(style);
                }
                None => {
                    pending_text = expanded;
                    pending_style = Some(style);
                }
            }
        }
        if let Some(s) = pending_style {
            spans.push(Span::styled(pending_text, s));
        }

        rendered.push(Line::from(spans));
    }

    if rendered.is_empty() && !canceled() {
        rendered.push(Line::from("File is empty"));
    }

    Ok(rendered)
}
