use super::{directive_configs, ini_files, log_files, structured_data};
use crate::{file_classification::CustomCodeKind, preview::appearance as theme};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub(in crate::preview::code) fn render_built_in_code_preview<F>(
    kind: CustomCodeKind,
    text: &str,
    line_numbers: bool,
    line_limit: usize,
    canceled: &F,
) -> Vec<Line<'static>>
where
    F: Fn() -> bool,
{
    let code_palette = theme::code_preview_palette();
    let source_lines = crate::preview::collect_preview_lines_with_limit(
        text,
        crate::preview::clamp_code_preview_line_limit(line_limit),
    );
    let number_width = crate::preview::line_number_width(source_lines.len());
    let mut rendered = Vec::new();
    let mut jsonc_block_comment = false;

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

        let body = match kind {
            CustomCodeKind::DirectiveConf => {
                directive_configs::highlight_directive_conf_line(line, code_palette)
            }
            CustomCodeKind::Ini => ini_files::highlight_ini_line(line, code_palette, false),
            CustomCodeKind::DesktopEntry => ini_files::highlight_ini_line(line, code_palette, true),
            CustomCodeKind::Json => structured_data::highlight_json_line(line, code_palette),
            CustomCodeKind::Jsonc => {
                structured_data::highlight_jsonc_line(line, code_palette, &mut jsonc_block_comment)
            }
            CustomCodeKind::Toml => structured_data::highlight_toml_line(line, code_palette),
            CustomCodeKind::Yaml => structured_data::highlight_yaml_line(line, code_palette),
            CustomCodeKind::Log => log_files::highlight_log_line(line, code_palette),
        };
        spans.extend(body);
        rendered.push(Line::from(spans));
    }

    if rendered.is_empty() && !canceled() {
        rendered.push(Line::from("File is empty"));
    }

    rendered
}

pub(super) fn styled_text(
    text: &str,
    color: ratatui::style::Color,
    modifier: ratatui::style::Modifier,
) -> Span<'static> {
    Span::styled(
        text.to_string(),
        Style::default().fg(color).add_modifier(modifier),
    )
}
