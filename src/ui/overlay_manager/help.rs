use crate::app::FrameState;
use crate::ui::{helpers, theme::Palette};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

pub(super) fn render_help(
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
            key: "← / h / Backspace",
            action: "parent folder",
        },
        HelpEntry {
            key: "→ / l / Enter",
            action: "enter folder / open",
        },
        HelpEntry {
            key: "g / G",
            action: "go-to menu / last item",
        },
        HelpEntry {
            key: "PageUp / PageDown",
            action: "page up / down",
        },
        HelpEntry {
            key: "Tab / Shift+Tab",
            action: "cycle sidebar locations",
        },
        HelpEntry {
            key: "Alt+← / →",
            action: "back / forward",
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
    const CLIPBOARD_ENTRIES: &[HelpEntry<'_>] = &[
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
            key: "y",
            action: "yank (copy)",
        },
        HelpEntry {
            key: "c",
            action: "copy path details",
        },
        HelpEntry {
            key: "g",
            action: "go to top, home, downloads, .config, or trash",
        },
        HelpEntry {
            key: "x",
            action: "cut",
        },
        HelpEntry {
            key: "p",
            action: "paste",
        },
    ];
    const FILES_ENTRIES: &[HelpEntry<'_>] = &[
        HelpEntry {
            key: "a",
            action: "create file or folder",
        },
        HelpEntry {
            key: "Alt/Shift+Enter",
            action: "add line in create prompt",
        },
        HelpEntry {
            key: "d",
            action: "trash (delete if in trash)",
        },
        HelpEntry {
            key: "r / F2",
            action: "rename (bulk if selection)",
        },
        HelpEntry {
            key: "r (in trash)",
            action: "restore from trash",
        },
        HelpEntry {
            key: "o",
            action: "open externally",
        },
    ];
    const VIEW_ENTRIES: &[HelpEntry<'_>] = &[
        HelpEntry {
            key: "v",
            action: "toggle grid / list",
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
            key: "< / >",
            action: "scroll preview left / right",
        },
    ];
    const MOUSE_ENTRIES: &[HelpEntry<'_>] = &[
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
            action: "scroll preview",
        },
    ];
    const LEFT_SECTIONS: &[HelpSection<'_>] = &[
        HelpSection {
            title: "Navigate",
            entries: NAVIGATION_ENTRIES,
        },
        HelpSection {
            title: "Search",
            entries: SEARCH_ENTRIES,
        },
        HelpSection {
            title: "Mouse",
            entries: MOUSE_ENTRIES,
        },
    ];
    const RIGHT_SECTIONS: &[HelpSection<'_>] = &[
        HelpSection {
            title: "Files",
            entries: FILES_ENTRIES,
        },
        HelpSection {
            title: "Selection & Clipboard",
            entries: CLIPBOARD_ENTRIES,
        },
        HelpSection {
            title: "View",
            entries: VIEW_ENTRIES,
        },
    ];

    let popup = helpers::centered_rect(area, 88, 26);
    state.help_panel = Some(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::new()
            .title(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    "󰘳",
                    Style::default()
                        .fg(palette.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " Keyboard and mouse controls ",
                    Style::default()
                        .fg(palette.accent_text)
                        .add_modifier(Modifier::BOLD),
                ),
            ]))
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
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(vec![Line::from(vec![
            helpers::chip_span("navigate", palette.accent_soft, palette.accent_text, true),
            Span::raw(" "),
            helpers::chip_span("search", palette.accent_soft, palette.accent_text, true),
            Span::raw(" "),
            helpers::chip_span("mouse", palette.accent_soft, palette.accent_text, true),
            Span::raw(" "),
            helpers::chip_span("files", palette.accent_soft, palette.accent_text, true),
            Span::raw(" "),
            helpers::chip_span("selection", palette.accent_soft, palette.accent_text, true),
            Span::raw(" "),
            helpers::chip_span("view", palette.accent_soft, palette.accent_text, true),
        ])])
        .style(Style::default().bg(palette.chrome_alt).fg(palette.text)),
        rows[0],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(39),
            Constraint::Length(3),
            Constraint::Length(44),
        ])
        .split(rows[1]);

    frame.render_widget(
        Paragraph::new(help_column_lines(cols[0].width, LEFT_SECTIONS, palette))
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text))
            .wrap(Wrap { trim: false }),
        cols[0],
    );

    let divider_lines: Vec<Line<'static>> =
        vec![
            Line::from(Span::styled(" │ ", Style::default().fg(palette.border)));
            cols[1].height as usize
        ];
    frame.render_widget(
        Paragraph::new(divider_lines).style(Style::default().bg(palette.chrome_alt)),
        cols[1],
    );

    frame.render_widget(
        Paragraph::new(help_column_lines(cols[2].width, RIGHT_SECTIONS, palette))
            .style(Style::default().bg(palette.chrome_alt).fg(palette.text))
            .wrap(Wrap { trim: false }),
        cols[2],
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
    let mut key_width = max_key_width.min(17);
    let min_action_width = 14usize.min(content_width.saturating_sub(gap_width + 1));
    if key_width + gap_width + min_action_width > content_width {
        key_width = content_width.saturating_sub(gap_width + min_action_width);
    }
    key_width = key_width
        .max(4)
        .min(content_width.saturating_sub(gap_width + 1));
    let action_width = content_width.saturating_sub(key_width + gap_width).max(1);

    let mut lines = Vec::new();
    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            lines.push(Line::default());
        }
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
            entry.key.to_string(),
            Style::default()
                .fg(palette.accent_text)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(key_padding),
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
