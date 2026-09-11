use super::helpers;
use super::theme::Palette;
use crate::app::{App, ClipOp};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

const STATUS_MIN_LEFT_WIDTH: u16 = 24;
const STATUS_RIGHT_PADDING: usize = 2;
const GIT_BRANCH_MAX_WIDTH: usize = 24;
const LOCAL_FILTER_INDICATOR_MAX_WIDTH: usize = 24;
const FOOTER_MIN_NAME_WIDTH: usize = 6;
const FOOTER_MIN_GIT_BRANCH_WIDTH: usize = 3;

pub(super) fn render_status_bar(frame: &mut Frame<'_>, area: Rect, app: &App, palette: Palette) {
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

    let right_text = helpers::clamp_label(status_message, sections[1].width as usize);
    if app.local_filter_is_editing() {
        render_local_filter_status(frame, sections[0], app, palette);
        frame.render_widget(
            Paragraph::new(right_text)
                .alignment(Alignment::Right)
                .style(Style::default().bg(palette.chrome).fg(palette.muted)),
            sections[1],
        );
        return;
    }

    let clip = app.clipboard_info();
    let sel_count = app.selection_count();
    let paste_prog = app.paste_progress();
    let archive_create_prog = app.archive_create_progress();
    let archive_prog = app.archive_extract_progress();
    let queued_pastes = app.queued_paste_count();
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
                    .fg(palette.chip_text)
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
                    .fg(palette.chip_text)
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
            let label = if queued_pastes == 0 {
                format!(" {verb} {completed}/{total} ")
            } else {
                format!(" {verb} {completed}/{total} (+{queued_pastes} queued) ")
            };
            chips_width += label.len() as u16 + 2;
            spans.push(Span::styled(
                label,
                Style::default()
                    .bg(color)
                    .fg(palette.chip_text)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw("  "));
        } else if let Some((completed, total)) = archive_create_prog {
            let label = if total == 0 {
                " Creating archive… ".to_string()
            } else {
                format!(" Creating archive {completed}/{total} ")
            };
            chips_width += label.len() as u16 + 2;
            spans.push(Span::styled(
                label,
                Style::default()
                    .bg(palette.progress_bar)
                    .fg(palette.chip_text)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw("  "));
        } else if let Some((completed, total)) = archive_prog {
            let label = match total {
                Some(total) => format!(" Extracting {completed}/{total} "),
                None => " Extracting… ".to_string(),
            };
            chips_width += label.len() as u16 + 2;
            spans.push(Span::styled(
                label,
                Style::default()
                    .bg(palette.progress_bar)
                    .fg(palette.chip_text)
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
                    .fg(palette.chip_text)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw("  "));
        }

        if app.local_filter_has_query() {
            let label = local_filter_indicator(app.local_filter_query());
            chips_width += helpers::display_width(&label) as u16 + 2;
            spans.push(Span::styled(
                label,
                Style::default()
                    .fg(palette.muted)
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
                    .fg(palette.chip_text)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw("  "));
        }

        let available_after_chips = sections[0].width.saturating_sub(chips_width) as usize;
        let summary = app.selection_summary();
        let desired_summary_width = helpers::display_width(&summary).min(available_after_chips);
        let git_label = app.git_branch().and_then(|branch| {
            git_label_for_width(
                branch,
                app.git_dirty(),
                available_after_chips
                    .saturating_sub(desired_summary_width)
                    .saturating_sub(helpers::display_width(" │ ")),
            )
        });
        let git_width = git_label
            .as_deref()
            .map(|label| helpers::display_width(" │ ") + helpers::display_width(label))
            .unwrap_or(0);

        let summary_width = available_after_chips.saturating_sub(git_width);
        spans.push(Span::styled(
            compact_footer_summary(&summary, summary_width),
            Style::default()
                .fg(palette.text)
                .add_modifier(Modifier::BOLD),
        ));
        if let Some(label) = git_label {
            spans.push(Span::styled(" │ ", Style::default().fg(palette.muted)));
            spans.push(Span::styled(
                label,
                Style::default()
                    .fg(palette.muted)
                    .add_modifier(Modifier::BOLD),
            ));
        }

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

fn render_local_filter_status(frame: &mut Frame<'_>, area: Rect, app: &App, palette: Palette) {
    if area.width == 0 {
        return;
    }

    let query = app.local_filter_query();
    let text = format!("/{query}");
    let (rendered, cursor_offset) = helpers::input_window(
        &text,
        app.local_filter_cursor().saturating_add(1),
        area.width,
    );
    frame.render_widget(
        Paragraph::new(rendered).style(Style::default().bg(palette.chrome).fg(palette.text)),
        area,
    );

    let cursor_x = area
        .x
        .saturating_add(cursor_offset)
        .min(area.x + area.width.saturating_sub(1));
    frame.set_cursor_position((cursor_x, area.y));
}

fn local_filter_indicator(query: &str) -> String {
    format!(
        "/{}",
        helpers::truncate_middle(query, LOCAL_FILTER_INDICATOR_MAX_WIDTH.saturating_sub(1))
    )
}

pub(in crate::ui) fn status_section_width(total_width: u16, status_message: &str) -> u16 {
    let max_right_width = total_width.saturating_sub(STATUS_MIN_LEFT_WIDTH).max(1);
    if status_message.is_empty() {
        return 1;
    }

    let desired = helpers::display_width(status_message).saturating_add(STATUS_RIGHT_PADDING);
    desired.min(max_right_width as usize).max(1) as u16
}

pub(in crate::ui) fn compact_footer_summary(summary: &str, width: usize) -> String {
    let Some((position, name)) = summary.split_once("  ") else {
        return helpers::truncate_middle(summary, width);
    };
    let position_width = helpers::display_width(position);
    let min_name_width = helpers::display_width(name).min(FOOTER_MIN_NAME_WIDTH);
    if width <= position_width || width < position_width + 2 + min_name_width {
        return helpers::truncate_middle(position, width);
    }

    format!(
        "{position}  {}",
        helpers::truncate_middle(name, width - position_width - 2)
    )
}

pub(in crate::ui) fn git_label_for_width(
    branch: &str,
    dirty: bool,
    width: usize,
) -> Option<String> {
    let dirty_suffix = if dirty { " *" } else { "" };
    let fixed_width = helpers::display_width(" ") + helpers::display_width(dirty_suffix);
    let branch_width = width.saturating_sub(fixed_width).min(GIT_BRANCH_MAX_WIDTH);
    if branch_width < FOOTER_MIN_GIT_BRANCH_WIDTH {
        return None;
    }

    Some(format!(
        " {}{dirty_suffix}",
        helpers::truncate_middle(branch, branch_width)
    ))
}
