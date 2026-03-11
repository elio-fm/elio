use super::theme::Palette;
use crate::app::sanitize_terminal_text;
use ratatui::{
    Frame,
    layout::{Alignment, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};
use std::{env, path::Path};

pub(super) fn render_empty_state(frame: &mut Frame<'_>, area: Rect, label: &str, palette: Palette) {
    fill_area(frame, area, palette.panel_alt, palette.muted);
    frame.render_widget(
        Paragraph::new(label)
            .alignment(Alignment::Center)
            .style(Style::default().bg(palette.panel_alt).fg(palette.muted)),
        area,
    );
}

pub(super) fn fill_area(frame: &mut Frame<'_>, area: Rect, bg: Color, fg: Color) {
    frame.render_widget(Clear, area);
    frame.render_widget(Block::default().style(Style::default().bg(bg).fg(fg)), area);
}

pub(super) fn render_panel_title(
    frame: &mut Frame<'_>,
    area: Rect,
    line: Line<'static>,
    bg: Color,
    fg: Color,
) {
    if area.width <= 2 || area.height == 0 {
        return;
    }

    let title_area = Rect {
        x: area.x.saturating_add(1),
        y: area.y,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    fill_area(frame, title_area, bg, fg);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(bg).fg(fg)),
        title_area,
    );
}

pub(super) fn render_button(
    frame: &mut Frame<'_>,
    rect: Rect,
    label: &str,
    icon: &str,
    enabled: bool,
    palette: Palette,
) {
    let bg = if enabled {
        palette.button_bg
    } else {
        palette.button_disabled_bg
    };
    let fg = if enabled { palette.text } else { palette.muted };
    frame.render_widget(Block::default().style(Style::default().bg(bg).fg(fg)), rect);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                icon,
                Style::default().fg(if enabled {
                    palette.accent
                } else {
                    palette.muted
                }),
            ),
            Span::raw(" "),
            Span::styled(
                label.to_string(),
                Style::default().fg(fg).add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Center)
        .style(Style::default().bg(bg).fg(fg)),
        rect,
    );
}

pub(super) fn panel_block<'a>(title: &'a str, bg: Color, palette: Palette) -> Block<'a> {
    Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(palette.accent_text)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(bg).fg(palette.text))
        .border_style(Style::default().fg(palette.border))
}

pub(super) fn rounded_block(bg: Color, border: Color) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(bg))
        .border_style(Style::default().fg(border))
}

pub(super) fn chip_span<'a>(label: &'a str, bg: Color, fg: Color, bold: bool) -> Span<'a> {
    let style = if bold {
        Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(bg).fg(fg)
    };
    Span::styled(format!(" {label} "), style)
}

pub(super) fn help_row(key: &str, action: &str, palette: Palette) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{key:<12}"),
            Style::default()
                .fg(palette.accent_text)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(action.to_string(), Style::default().fg(palette.muted)),
    ])
}

#[derive(Clone, Copy)]
pub(super) struct GridZoomSpec {
    pub tile_width_hint: u16,
    pub min_tile_width: u16,
    pub tile_height: u16,
    pub gap_x: u16,
    pub gap_y: u16,
    pub padding_x: u16,
    pub emphasize_icon: bool,
    pub show_kind_hint: bool,
}

pub(super) fn grid_zoom_spec(zoom: u8) -> GridZoomSpec {
    match zoom {
        0 => GridZoomSpec {
            tile_width_hint: 18,
            min_tile_width: 17,
            tile_height: 4,
            gap_x: 1,
            gap_y: 0,
            padding_x: 1,
            emphasize_icon: false,
            show_kind_hint: false,
        },
        1 => GridZoomSpec {
            tile_width_hint: 24,
            min_tile_width: 21,
            tile_height: 5,
            gap_x: 1,
            gap_y: 1,
            padding_x: 1,
            emphasize_icon: false,
            show_kind_hint: false,
        },
        2 => GridZoomSpec {
            tile_width_hint: 27,
            min_tile_width: 24,
            tile_height: 6,
            gap_x: 2,
            gap_y: 1,
            padding_x: 2,
            emphasize_icon: true,
            show_kind_hint: false,
        },
        3 => GridZoomSpec {
            tile_width_hint: 30,
            min_tile_width: 26,
            tile_height: 7,
            gap_x: 2,
            gap_y: 1,
            padding_x: 2,
            emphasize_icon: true,
            show_kind_hint: true,
        },
        _ => GridZoomSpec {
            tile_width_hint: 36,
            min_tile_width: 30,
            tile_height: 8,
            gap_x: 3,
            gap_y: 1,
            padding_x: 3,
            emphasize_icon: true,
            show_kind_hint: true,
        },
    }
}

pub(super) fn list_row_height(zoom: u8) -> u16 {
    match zoom {
        0 | 1 => 1,
        2 | 3 => 2,
        _ => 3,
    }
}

pub(super) fn stable_path_label(path: &Path, max_chars: usize) -> String {
    let display = if let Some(home) = env::var_os("HOME") {
        let home = std::path::PathBuf::from(home);
        if let Ok(stripped) = path.strip_prefix(&home) {
            if stripped.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~/{}", stripped.display())
            }
        } else {
            path.display().to_string()
        }
    } else {
        path.display().to_string()
    };
    truncate_path_tail(&display, max_chars.max(8))
}

pub(super) fn path_is_active(current: &Path, candidate: &Path) -> bool {
    current == candidate
}

fn truncate_path_tail(path: &str, max_chars: usize) -> String {
    let path = sanitize_terminal_text(path);
    if path.chars().count() <= max_chars {
        return path;
    }

    let prefix = if path.starts_with("~/") {
        "~/"
    } else if path.starts_with('/') {
        "/"
    } else {
        ""
    };

    let parts = path
        .trim_start_matches("~/")
        .trim_start_matches('/')
        .split('/')
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return path.to_string();
    }

    let last = parts.last().copied().unwrap_or_default();
    let reserve = prefix.chars().count() + last.chars().count() + 4;
    if reserve >= max_chars {
        return truncate_middle(&path, max_chars);
    }

    let mut result = format!("{prefix}…/{last}");
    if result.chars().count() > max_chars {
        result = truncate_middle(&path, max_chars);
    }
    result
}

pub(super) fn truncate_middle(text: &str, max_chars: usize) -> String {
    let text = sanitize_terminal_text(text);
    let chars = text.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return text;
    }
    if max_chars <= 1 {
        return "…".to_string();
    }

    let head = max_chars / 2;
    let tail = max_chars.saturating_sub(head + 1);
    let start = chars.iter().take(head).collect::<String>();
    let end = chars
        .iter()
        .skip(chars.len().saturating_sub(tail))
        .collect::<String>();
    format!("{start}…{end}")
}

pub(super) fn clamp_label(label: &str, max_chars: usize) -> String {
    let label = sanitize_terminal_text(label);
    let count = label.chars().count();
    if count <= max_chars {
        return label;
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let head = label.chars().take(max_chars - 1).collect::<String>();
    format!("{head}…")
}

pub(super) fn inner_with_padding(rect: Rect) -> Rect {
    rect.inner(Margin {
        horizontal: 1,
        vertical: 1,
    })
}

pub(super) fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2)).max(10);
    let height = height.min(area.height.saturating_sub(2)).max(4);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}
