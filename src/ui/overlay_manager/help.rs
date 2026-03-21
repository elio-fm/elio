use super::Palette;
use super::{FrameState, helpers};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

pub(in crate::ui) fn render_help(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut FrameState,
    palette: Palette,
) {
    const NAVIGATION_ENTRIES: &[HelpEntry<'_>] = &[
        HelpEntry {
            key: "↑↓ / jk",
            action: "move selection",
        },
        HelpEntry {
            key: "← / h",
            action: "parent folder",
        },
        HelpEntry {
            key: "→ / l",
            action: "enter folder",
        },
        HelpEntry {
            key: "Enter",
            action: "open item",
        },
        HelpEntry {
            key: "Tab",
            action: "next pinned place",
        },
        HelpEntry {
            key: "Shift+Tab",
            action: "previous pinned place",
        },
        HelpEntry {
            key: "Backspace",
            action: "parent folder",
        },
        HelpEntry {
            key: "Alt+Left",
            action: "back",
        },
        HelpEntry {
            key: "Alt+Right",
            action: "forward",
        },
    ];
    const SEARCH_ENTRIES: &[HelpEntry<'_>] = &[
        HelpEntry {
            key: "f",
            action: "search folders",
        },
        HelpEntry {
            key: "Ctrl+F",
            action: "search files",
        },
        HelpEntry {
            key: "Ctrl+←→",
            action: "move by word",
        },
        HelpEntry {
            key: "Ctrl+Backspace",
            action: "delete previous word",
        },
        HelpEntry {
            key: "Ctrl+Del",
            action: "delete next word",
        },
        HelpEntry {
            key: "Ctrl+W / Alt+D",
            action: "fallback word delete",
        },
    ];
    const MOUSE_VIEW_ENTRIES: &[HelpEntry<'_>] = &[
        HelpEntry {
            key: "Click",
            action: "select item",
        },
        HelpEntry {
            key: "Double-click",
            action: "open item",
        },
        HelpEntry {
            key: "Wheel",
            action: "move selection",
        },
        HelpEntry {
            key: "Shift+Wheel",
            action: "scroll code sideways",
        },
        HelpEntry {
            key: "v",
            action: "toggle grid/list",
        },
        HelpEntry {
            key: ".",
            action: "toggle dotfiles",
        },
        HelpEntry {
            key: "s",
            action: "cycle sort",
        },
        HelpEntry {
            key: "o",
            action: "open externally",
        },
        HelpEntry {
            key: "Space",
            action: "toggle selection",
        },
        HelpEntry {
            key: "Ctrl+A",
            action: "select all",
        },
        HelpEntry {
            key: "Esc",
            action: "clear selection",
        },
        HelpEntry {
            key: "a",
            action: "create file or folder",
        },
        HelpEntry {
            key: "d",
            action: "trash (delete if in trash)",
        },
        HelpEntry {
            key: "r / F2",
            action: "rename",
        },
        HelpEntry {
            key: "r (in trash)",
            action: "restore from trash",
        },
    ];
    const LEFT_SECTIONS: &[HelpSection<'_>] = &[
        HelpSection {
            title: "Navigation",
            entries: NAVIGATION_ENTRIES,
        },
        HelpSection {
            title: "Search",
            entries: SEARCH_ENTRIES,
        },
    ];
    const RIGHT_SECTIONS: &[HelpSection<'_>] = &[HelpSection {
        title: "Mouse + View",
        entries: MOUSE_VIEW_ENTRIES,
    }];

    let popup = helpers::centered_rect(area, 82, 22);
    state.help_panel = Some(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(" Controls ", palette.chrome_alt, palette),
        popup,
    );
    let inner = helpers::inner_with_padding(popup);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "󰘳",
                    Style::default()
                        .fg(palette.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    "Keyboard and mouse controls",
                    Style::default()
                        .fg(palette.text)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                helpers::chip_span("navigate", palette.accent_soft, palette.accent_text, true),
                Span::raw(" "),
                helpers::chip_span("search", palette.accent_soft, palette.accent_text, true),
                Span::raw(" "),
                helpers::chip_span("mouse", palette.accent_soft, palette.accent_text, true),
                Span::raw(" "),
                helpers::chip_span("view", palette.accent_soft, palette.accent_text, true),
            ]),
        ])
        .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
        rows[0],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    frame.render_widget(
        Paragraph::new(help_column_lines(cols[0].width, LEFT_SECTIONS, palette))
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text))
            .wrap(Wrap { trim: false }),
        cols[0],
    );

    frame.render_widget(
        Paragraph::new(help_column_lines(cols[1].width, RIGHT_SECTIONS, palette))
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text))
            .wrap(Wrap { trim: false }),
        cols[1],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "? / Esc",
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled("close help", Style::default().fg(palette.muted)),
        ]))
        .alignment(Alignment::Right)
        .style(Style::default().bg(palette.chrome_alt).fg(palette.muted)),
        rows[2],
    );
}

#[derive(Clone, Copy)]
struct HelpEntry<'a> {
    key: &'a str,
    action: &'a str,
}

#[derive(Clone, Copy)]
struct HelpSection<'a> {
    title: &'a str,
    entries: &'a [HelpEntry<'a>],
}

fn help_column_lines(
    width: u16,
    sections: &[HelpSection<'_>],
    palette: Palette,
) -> Vec<Line<'static>> {
    let content_width = width.max(1) as usize;
    let max_key_width = sections
        .iter()
        .flat_map(|section| section.entries.iter())
        .map(|entry| UnicodeWidthStr::width(entry.key))
        .max()
        .unwrap_or(0);
    let gap_width = 2usize;
    let mut key_width = max_key_width.min(16);
    let min_action_width = 14usize.min(content_width.saturating_sub(gap_width + 1));
    if key_width + gap_width + min_action_width > content_width {
        key_width = content_width.saturating_sub(gap_width + min_action_width);
    }
    key_width = key_width
        .max(4)
        .min(content_width.saturating_sub(gap_width + 1));
    let action_width = content_width.saturating_sub(key_width + gap_width).max(1);

    let mut lines = Vec::new();
    for section in sections {
        lines.push(help_section_title(section.title, palette));
        for entry in section.entries {
            lines.extend(help_entry_lines(*entry, key_width, action_width, palette));
        }
    }
    lines
}

fn help_section_title(title: &str, palette: Palette) -> Line<'static> {
    Line::from(vec![Span::styled(
        title.to_string(),
        Style::default()
            .fg(palette.accent)
            .add_modifier(Modifier::BOLD),
    )])
}

fn help_entry_lines(
    entry: HelpEntry<'_>,
    key_width: usize,
    action_width: usize,
    palette: Palette,
) -> Vec<Line<'static>> {
    let mut wrapped_action = wrap_help_action(entry.action, action_width);
    if wrapped_action.is_empty() {
        wrapped_action.push(String::new());
    }

    let key_padding = " ".repeat(key_width.saturating_sub(UnicodeWidthStr::width(entry.key)));
    let continuation = " ".repeat(key_width + 2);
    let mut lines = Vec::with_capacity(wrapped_action.len());

    lines.push(Line::from(vec![
        Span::styled(
            format!("{}{}", entry.key, key_padding),
            Style::default()
                .fg(palette.accent_text)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(wrapped_action.remove(0), Style::default().fg(palette.muted)),
    ]));

    for line in wrapped_action {
        lines.push(Line::from(vec![
            Span::raw(continuation.clone()),
            Span::styled(line, Style::default().fg(palette.muted)),
        ]));
    }

    lines
}

fn wrap_help_action(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() || width == 0 {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;

    for word in text.split_whitespace() {
        let word_width = UnicodeWidthStr::width(word);
        let separator_width = usize::from(!current.is_empty());
        if !current.is_empty() && current_width + separator_width + word_width > width {
            lines.push(current);
            current = word.to_string();
            current_width = word_width;
            continue;
        }

        if !current.is_empty() {
            current.push(' ');
            current_width += 1;
        }
        current.push_str(word);
        current_width += word_width;
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}
