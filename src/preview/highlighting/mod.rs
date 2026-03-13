mod brace_like;
mod cmake;
mod common;
mod data;
mod js_like;
mod logs;
mod make;
mod markup;
mod nix;
mod python;
mod shell;
#[cfg(test)]
mod tests;

use crate::{file_info::HighlightLanguage, ui::theme};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub(super) fn render_code_preview(
    text: &str,
    language: Option<HighlightLanguage>,
    line_numbers: bool,
) -> Vec<Line<'static>> {
    match language {
        Some(language) => render_highlighted_code_preview(text, language, line_numbers),
        None => render_plain_code_preview(text, line_numbers),
    }
}

pub(super) fn render_markdown_code_preview(
    text: &str,
    language_token: &str,
    line_numbers: bool,
) -> Vec<Line<'static>> {
    render_code_preview(
        text,
        HighlightLanguage::from_language_token(language_token),
        line_numbers,
    )
}

#[cfg(test)]
fn render_highlighted_code_preview_for_tests(
    text: &str,
    language: HighlightLanguage,
    line_numbers: bool,
) -> Vec<Line<'static>> {
    render_code_preview(text, Some(language), line_numbers)
}

fn render_highlighted_code_preview(
    text: &str,
    language: HighlightLanguage,
    line_numbers: bool,
) -> Vec<Line<'static>> {
    let code_palette = theme::code_preview_palette();
    let source_lines = super::collect_preview_lines(text);
    let number_width = super::line_number_width(source_lines.len());
    let mut rendered = Vec::new();
    let mut jsonc_block_comment = false;
    let mut markup_block_comment = false;
    let mut css_block_comment = false;
    let mut brace_like_block_comment = false;
    let mut python_state = python::PythonState::default();
    let mut shell_state = shell::ShellState::default();

    for (index, line) in source_lines.iter().enumerate() {
        let mut spans = Vec::new();
        if line_numbers {
            spans.push(super::line_number_span(index + 1, number_width));
        } else {
            spans.push(Span::styled(
                "│ ",
                Style::default().fg(code_palette.line_number),
            ));
        }

        let body = match language {
            HighlightLanguage::JsLike => js_like::highlight_js_like_line(line, code_palette),
            HighlightLanguage::CLike => brace_like::highlight_brace_like_line(
                line,
                code_palette,
                &mut brace_like_block_comment,
            ),
            HighlightLanguage::Python => {
                python::highlight_python_line(line, code_palette, &mut python_state)
            }
            HighlightLanguage::Make => make::highlight_make_line(line, code_palette),
            HighlightLanguage::Shell => {
                shell::highlight_shell_line(line, code_palette, &mut shell_state)
            }
            HighlightLanguage::Nix => nix::highlight_nix_line(line, code_palette),
            HighlightLanguage::CMake => cmake::highlight_cmake_line(line, code_palette),
            HighlightLanguage::Markup => {
                markup::highlight_markup_line(line, code_palette, &mut markup_block_comment)
            }
            HighlightLanguage::Css => {
                markup::highlight_css_line(line, code_palette, &mut css_block_comment)
            }
            HighlightLanguage::Toml => data::highlight_toml_line(line, code_palette),
            HighlightLanguage::Json => data::highlight_json_line(line, code_palette),
            HighlightLanguage::Jsonc => {
                data::highlight_jsonc_line(line, code_palette, &mut jsonc_block_comment)
            }
            HighlightLanguage::Yaml => data::highlight_yaml_line(line, code_palette),
            HighlightLanguage::Log => logs::highlight_log_line(line, code_palette),
            HighlightLanguage::Ini | HighlightLanguage::DesktopEntry => data::highlight_ini_line(
                line,
                code_palette,
                language == HighlightLanguage::DesktopEntry,
            ),
        };
        spans.extend(body);
        rendered.push(Line::from(spans));
    }

    if rendered.is_empty() {
        rendered.push(Line::from("File is empty"));
    }
    rendered
}

fn render_plain_code_preview(text: &str, line_numbers: bool) -> Vec<Line<'static>> {
    let code_palette = theme::code_preview_palette();
    let source_lines = super::collect_preview_lines(text);
    let number_width = super::line_number_width(source_lines.len());
    let mut rendered = Vec::new();

    for (index, line) in source_lines.iter().enumerate() {
        let mut spans = Vec::new();
        if line_numbers {
            spans.push(super::line_number_span(index + 1, number_width));
        } else {
            spans.push(Span::styled(
                "│ ",
                Style::default().fg(code_palette.line_number),
            ));
        }
        spans.push(Span::styled(
            super::expand_tabs(line),
            Style::default().fg(code_palette.fg),
        ));
        rendered.push(Line::from(spans));
    }

    if rendered.is_empty() {
        rendered.push(Line::from("File is empty"));
    }
    rendered
}
