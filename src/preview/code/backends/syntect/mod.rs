mod bundle;
mod render;
mod semantics;
mod shell;

#[cfg(test)]
use crate::preview::code::syntax_manifest::CURATED_SYNTAXES;
#[cfg(test)]
use crate::preview::code::syntax_manifest::CuratedSyntax;
use crate::preview::code::syntax_manifest::curated_syntax;
use ratatui::text::Line;

pub(in crate::preview::code) fn is_enabled(code_syntax: &str) -> bool {
    curated_syntax(code_syntax).is_some()
}

#[cfg(test)]
pub(in crate::preview::code) fn supported_syntaxes() -> &'static [CuratedSyntax] {
    CURATED_SYNTAXES
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
    if shell::is_shell_like_syntax(code_syntax) {
        return Ok(shell::render_shell_code_preview(
            text,
            line_numbers,
            line_limit,
            canceled,
        ));
    }

    let syntax_set = bundle::syntax_set();
    let Some(syntax) = bundle::find_syntax(syntax_set, code_syntax) else {
        return Err(());
    };

    render::render_syntect_code_preview(
        text,
        syntax_set,
        syntax,
        line_numbers,
        line_limit,
        canceled,
    )
}

#[cfg(test)]
mod tests;
