#[path = "fallback/c_like.rs"]
mod c_like;
#[path = "fallback/cmake.rs"]
mod cmake;
#[path = "fallback/common.rs"]
mod common;
#[path = "fallback/data.rs"]
mod data;
#[path = "fallback/js_like.rs"]
mod js_like;
#[path = "fallback/logs.rs"]
mod logs;
#[path = "fallback/make.rs"]
mod make;
#[path = "fallback/markup.rs"]
mod markup;
#[path = "fallback/nix.rs"]
mod nix;
#[path = "fallback/python.rs"]
mod python;
#[path = "fallback/shell.rs"]
mod shell;
#[cfg(test)]
#[path = "fallback/tests.rs"]
mod tests;

use crate::{appearance, file_facts::FallbackSyntax};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub(super) fn render_fallback_code_preview(
    text: &str,
    syntax: FallbackSyntax,
    line_numbers: bool,
) -> Vec<Line<'static>> {
    let code_palette = appearance::code_preview_palette();
    let source_lines = super::collect_preview_lines(text);
    let number_width = super::line_number_width(source_lines.len());
    let mut rendered = Vec::new();
    let mut jsonc_block_comment = false;
    let mut markup_block_comment = false;
    let mut css_block_comment = false;
    let mut c_like_block_comment = false;
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

        let body = match syntax {
            FallbackSyntax::JsLike => js_like::highlight_js_like_line(line, code_palette),
            FallbackSyntax::CLike => {
                c_like::highlight_c_like_line(line, code_palette, &mut c_like_block_comment)
            }
            FallbackSyntax::Python => {
                python::highlight_python_line(line, code_palette, &mut python_state)
            }
            FallbackSyntax::Make => make::highlight_make_line(line, code_palette),
            FallbackSyntax::Shell => {
                shell::highlight_shell_line(line, code_palette, &mut shell_state)
            }
            FallbackSyntax::Nix => nix::highlight_nix_line(line, code_palette),
            FallbackSyntax::CMake => cmake::highlight_cmake_line(line, code_palette),
            FallbackSyntax::Markup => {
                markup::highlight_markup_line(line, code_palette, &mut markup_block_comment)
            }
            FallbackSyntax::Css => {
                markup::highlight_css_line(line, code_palette, &mut css_block_comment)
            }
            FallbackSyntax::Toml => data::highlight_toml_line(line, code_palette),
            FallbackSyntax::Json => data::highlight_json_line(line, code_palette),
            FallbackSyntax::Jsonc => {
                data::highlight_jsonc_line(line, code_palette, &mut jsonc_block_comment)
            }
            FallbackSyntax::Yaml => data::highlight_yaml_line(line, code_palette),
            FallbackSyntax::Log => logs::highlight_log_line(line, code_palette),
            FallbackSyntax::Ini | FallbackSyntax::DesktopEntry => {
                data::highlight_ini_line(line, code_palette, syntax == FallbackSyntax::DesktopEntry)
            }
        };
        spans.extend(body);
        rendered.push(Line::from(spans));
    }

    if rendered.is_empty() {
        rendered.push(Line::from("File is empty"));
    }
    rendered
}
