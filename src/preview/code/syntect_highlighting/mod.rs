mod scope_styling;
pub(in crate::preview) mod supported_syntaxes;
mod syntax_loading;
mod syntax_rendering;

#[cfg(test)]
use self::supported_syntaxes::CURATED_SYNTAXES;
#[cfg(test)]
use self::supported_syntaxes::CuratedSyntax;
use self::supported_syntaxes::curated_syntax;
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
    if crate::preview::code::built_in_highlighting::is_shell_syntax(code_syntax) {
        return Ok(
            crate::preview::code::built_in_highlighting::render_shell_script(
                text,
                line_numbers,
                line_limit,
                canceled,
            ),
        );
    }

    let syntax_set = syntax_loading::syntax_set();
    let Some(syntax) = syntax_loading::find_syntax(syntax_set, code_syntax) else {
        return Err(());
    };

    syntax_rendering::render_syntect_code_preview(
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
