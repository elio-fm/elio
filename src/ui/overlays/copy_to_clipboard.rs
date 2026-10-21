use crate::app::{App, CopyHit, ScreenRegions};
use crate::{theme::Palette, ui::helpers};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

pub(in crate::ui) fn render_copy_to_clipboard_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut ScreenRegions,
    palette: Palette,
) {
    let row_count = app.file_operations.copy_row_count().max(1);
    let popup_width = area.width.saturating_sub(8).clamp(56, 104);
    let popup_height = 5;
    let popup = Rect {
        x: area.x + area.width.saturating_sub(popup_width) / 2,
        y: area.y + area.height.saturating_sub(popup_height + 2),
        width: popup_width.min(area.width.saturating_sub(2)).max(10),
        height: popup_height.min(area.height.saturating_sub(2)).max(4),
    };
    state.copy_panel = Some(popup);

    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .title(Span::styled(
                format!(" {} ", app.file_operations.copy_title()),
                Style::default().fg(palette.muted),
            ))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text))
            .border_style(Style::default().fg(palette.border)),
        popup,
    );

    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    let action_row = rows[1];
    let action_count = row_count.min(5);
    let gap_count = action_count.saturating_add(1);
    let actions = (0..action_count)
        .map(|index| {
            let shortcut = app
                .file_operations
                .copy_row_shortcut(index)
                .unwrap_or('?')
                .to_ascii_lowercase();
            (
                index,
                shortcut,
                display_label(shortcut, app.file_operations.copy_row_label(index)),
            )
        })
        .collect::<Vec<_>>();
    let label_budget = (action_row.width as usize)
        .saturating_sub(action_count * COPY_ACTION_PREFIX_WIDTH + gap_count * COPY_ACTION_MIN_GAP);
    let labels = fit_copy_labels(
        &actions
            .iter()
            .map(|(_, _, label)| *label)
            .collect::<Vec<_>>(),
        label_budget,
    );
    let action_widths = actions
        .iter()
        .zip(&labels)
        .map(|(_, label)| (COPY_ACTION_PREFIX_WIDTH + helpers::display_width(label)) as u16)
        .collect::<Vec<_>>();
    let action_width = action_widths
        .iter()
        .map(|width| *width as usize)
        .sum::<usize>();
    let gap_width = (action_row.width as usize).saturating_sub(action_width) / gap_count.max(1);
    let group_width = action_width + gap_width * gap_count;
    let mut x =
        action_row.x + action_row.width.saturating_sub(group_width as u16) / 2 + gap_width as u16;

    for (((index, shortcut, _), label), width) in actions.into_iter().zip(labels).zip(action_widths)
    {
        let rect = Rect {
            x,
            y: action_row.y,
            width,
            height: action_row.height,
        };

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(shortcut.to_string(), Style::default().fg(palette.accent)),
                Span::styled(" -> ", Style::default().fg(palette.muted)),
                Span::styled(label, Style::default().fg(palette.text)),
            ]))
            .alignment(Alignment::Center)
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
            rect,
        );
        state.copy_hits.push(CopyHit {
            rect: Rect {
                x: rect.x,
                y: inner.y,
                width: rect.width,
                height: inner.height,
            },
            index,
        });
        x = x.saturating_add(width + gap_width as u16);
    }
}

const COPY_ACTION_PREFIX_WIDTH: usize = 5;
const COPY_ACTION_MIN_GAP: usize = 2;

fn fit_copy_labels(labels: &[&str], budget: usize) -> Vec<String> {
    let desired_widths = labels
        .iter()
        .map(|label| helpers::display_width(label))
        .collect::<Vec<_>>();
    if desired_widths.iter().sum::<usize>() <= budget {
        return labels.iter().map(|label| (*label).to_string()).collect();
    }

    let mut widths = vec![0; labels.len()];
    let mut used = 0;
    while used < budget {
        let mut grew = false;
        for (width, desired) in widths.iter_mut().zip(&desired_widths) {
            if *width < *desired && used < budget {
                *width += 1;
                used += 1;
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }

    labels
        .iter()
        .zip(widths)
        .map(|(label, width)| helpers::clamp_label(label, width))
        .collect()
}

fn display_label(shortcut: char, fallback: &str) -> &str {
    match shortcut {
        'i' => "item",
        'c' => "name",
        'n' => "name w/o ext",
        'p' => "path",
        'd' => "directory",
        _ => fallback,
    }
}
