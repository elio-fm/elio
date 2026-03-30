use super::semantics::{SemanticRole, role_color, semantic_role_for_token};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use syntect::{
    easy::ScopeRangeIterator,
    parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet},
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
    let code_palette = crate::ui::theme::code_preview_palette();
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
        for (range, op) in ScopeRangeIterator::new(&ops, &line_with_nl) {
            scope_stack.apply(op).map_err(|_| ())?;
            let text = line_with_nl[range].trim_end_matches('\n');
            if text.is_empty() {
                continue;
            }
            spans.push(syntect_span(text, scope_stack.as_slice(), code_palette));
        }
        rendered.push(Line::from(spans));
    }

    if rendered.is_empty() && !canceled() {
        rendered.push(Line::from("File is empty"));
    }

    Ok(rendered)
}

fn syntect_span(
    text: &str,
    scope_stack: &[Scope],
    palette: crate::ui::theme::CodePreviewPalette,
) -> Span<'static> {
    let role = semantic_role_for_token(text, scope_stack);
    let mut rendered_style = Style::default().fg(role_color(role, palette));
    if role == SemanticRole::Invalid {
        rendered_style = rendered_style.add_modifier(Modifier::UNDERLINED);
    }

    Span::styled(crate::preview::expand_tabs(text), rendered_style)
}
