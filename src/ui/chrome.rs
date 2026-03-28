use super::helpers;
use super::theme::Palette;
use crate::app::{App, ClipOp, FrameState};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

pub(super) fn render_toolbar(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    helpers::fill_area(frame, area, palette.chrome, palette.text);
    let block = Block::default()
        .style(Style::default().bg(palette.chrome).fg(palette.text))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(palette.border));
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    let control_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(23),
            Constraint::Min(2),
            Constraint::Length(39),
        ])
        .split(inner);
    let nav_buttons = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(7),
        ])
        .split(control_row[0]);
    let meta = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(16),
            Constraint::Length(13),
            Constraint::Length(10),
        ])
        .split(control_row[2]);

    state.back_button = Some(nav_buttons[0]);
    state.forward_button = Some(nav_buttons[1]);
    state.parent_button = Some(nav_buttons[2]);
    state.hidden_button = Some(meta[1]);
    state.view_button = Some(meta[2]);

    helpers::render_button(
        frame,
        nav_buttons[0],
        "Back",
        "󰁍",
        app.can_go_back(),
        palette,
    );
    helpers::render_button(
        frame,
        nav_buttons[1],
        "Next",
        "󰁔",
        app.can_go_forward(),
        palette,
    );
    helpers::render_button(frame, nav_buttons[2], "Up", "󰁝", true, palette);
    frame.render_widget(
        Paragraph::new(Line::from(vec![helpers::chip_span(
            &format!("Sort: {}", app.sort_mode.label()),
            palette.button_bg,
            palette.text,
            true,
        )]))
        .alignment(Alignment::Right)
        .style(Style::default().bg(palette.chrome).fg(palette.text)),
        meta[0],
    );
    helpers::render_button(
        frame,
        meta[1],
        if app.show_hidden {
            "Hidden On"
        } else {
            "Hidden Off"
        },
        "󰈉",
        true,
        palette,
    );
    helpers::render_button(frame, meta[2], app.view_mode.label(), "󰕮", true, palette);
}

const STATUS_MIN_LEFT_WIDTH: u16 = 24;
const STATUS_IDLE_RIGHT_WIDTH: u16 = 34;
const STATUS_RIGHT_PADDING: usize = 2;

pub(super) fn render_status(frame: &mut Frame<'_>, area: Rect, app: &App, palette: Palette) {
    helpers::fill_area(frame, area, palette.chrome, palette.text);
    let status_message = app.status_message();
    let status_width = status_section_width(area.width, status_message);
    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(STATUS_MIN_LEFT_WIDTH),
            Constraint::Length(status_width),
        ])
        .split(area);

    let right_text = if status_message.is_empty() {
        status_idle_hint().to_string()
    } else {
        helpers::clamp_label(status_message, sections[1].width as usize)
    };
    let clip = app.clipboard_info();
    let sel_count = app.selection_count();
    let paste_prog = app.paste_progress();
    let trash_prog = app.trash_progress();
    let restore_prog = app.restore_progress();

    // Build the left line: optional progress chips (trash takes priority,
    // then restore, then paste; all take over the clipboard slot), optional
    // selection chip, then the path/position summary.
    let left_line = {
        let mut spans: Vec<Span<'_>> = Vec::new();
        let mut chips_width: u16 = 0;

        if let Some((completed, total, permanent)) = trash_prog {
            let label = if permanent {
                format!(" Deleting {completed}/{total} ")
            } else {
                // Batched trash has no per-item progress; show an
                // indeterminate indicator rather than a misleading 0/N.
                let noun = if total == 1 { "item" } else { "items" };
                format!(" Trashing {total} {noun}… ")
            };
            chips_width += label.len() as u16 + 2;
            spans.push(Span::styled(
                label,
                Style::default()
                    .bg(palette.trash_bar)
                    .fg(palette.chrome)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw("  "));
        } else if let Some((completed, total)) = restore_prog {
            let noun = if total == 1 { "item" } else { "items" };
            let label = format!(" Restoring {completed}/{total} {noun} ");
            chips_width += label.len() as u16 + 2;
            spans.push(Span::styled(
                label,
                Style::default()
                    .bg(palette.restore_bar)
                    .fg(palette.chrome)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw("  "));
        } else if let Some((completed, total, op)) = paste_prog {
            let verb = match op {
                ClipOp::Yank => "Copying",
                ClipOp::Cut => "Moving",
            };
            let color = match op {
                ClipOp::Yank => palette.yank_bar,
                ClipOp::Cut => palette.cut_bar,
            };
            let label = format!(" {verb} {completed}/{total} ");
            chips_width += label.len() as u16 + 2;
            spans.push(Span::styled(
                label,
                Style::default()
                    .bg(color)
                    .fg(palette.chrome)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw("  "));
        } else if let Some((clip_count, clip_op)) = clip {
            let (label, color) = match clip_op {
                ClipOp::Yank => (format!(" {clip_count} yanked "), palette.yank_bar),
                ClipOp::Cut => (format!(" {clip_count} cut "), palette.cut_bar),
            };
            chips_width += label.len() as u16 + 2;
            spans.push(Span::styled(
                label,
                Style::default()
                    .bg(color)
                    .fg(palette.chrome)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw("  "));
        }

        if sel_count > 0 {
            let chip = format!(" {sel_count} selected ");
            chips_width += chip.len() as u16 + 2;
            spans.push(Span::styled(
                chip,
                Style::default()
                    .bg(palette.selection_bar)
                    .fg(palette.chrome)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw("  "));
        }

        let summary_width = sections[0].width.saturating_sub(chips_width) as usize;
        spans.push(Span::styled(
            helpers::truncate_middle(&app.selection_summary(), summary_width),
            Style::default()
                .fg(palette.text)
                .add_modifier(Modifier::BOLD),
        ));

        Line::from(spans)
    };
    frame.render_widget(
        Paragraph::new(left_line).style(Style::default().bg(palette.chrome)),
        sections[0],
    );
    frame.render_widget(
        Paragraph::new(right_text)
            .alignment(Alignment::Right)
            .style(Style::default().bg(palette.chrome).fg(palette.muted)),
        sections[1],
    );
}

fn status_section_width(total_width: u16, status_message: &str) -> u16 {
    let max_right_width = total_width.saturating_sub(STATUS_MIN_LEFT_WIDTH).max(1);
    if status_message.is_empty() {
        return STATUS_IDLE_RIGHT_WIDTH.min(max_right_width).max(1);
    }

    let desired = helpers::display_width(status_message).saturating_add(STATUS_RIGHT_PADDING);
    desired
        .max(STATUS_IDLE_RIGHT_WIDTH as usize)
        .min(max_right_width as usize)
        .max(1) as u16
}

fn status_idle_hint() -> &'static str {
    "f folders  ^F files  ? help"
}

#[cfg(test)]
mod tests {
    use super::{status_idle_hint, status_section_width};
    use crate::ui::helpers;

    #[test]
    fn idle_status_keeps_the_compact_help_width() {
        assert_eq!(status_section_width(100, ""), 34);
    }

    #[test]
    fn real_status_messages_expand_beyond_the_idle_width() {
        assert!(status_section_width(100, "Clipboard helper not found while copying") > 34);
    }

    #[test]
    fn narrow_status_messages_truncate_at_the_end() {
        let rendered = helpers::clamp_label("Clipboard helper not found", 18);
        assert_eq!(rendered, "Clipboard helper …");
    }

    #[test]
    fn idle_hint_stays_unchanged() {
        assert_eq!(status_idle_hint(), "f folders  ^F files  ? help");
    }
}
