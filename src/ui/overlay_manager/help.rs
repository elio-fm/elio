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
    let kb = crate::config::keys();

    let navigation_entries = vec![
        e("↑↓ / jk", "move selection"),
        e("← / h / Backspace", "parent folder"),
        e("→ / l / Enter", "enter folder / open"),
        e("g", "go-to menu"),
        e("G", "last item"),
        e("PageUp / PageDown", "page up / down"),
        e("Tab / Shift+Tab", "cycle places"),
        e("Alt+← / →", "back / forward"),
    ];
    let search_entries = vec![
        e(&kb.search_folders.to_string(), "search folders"),
        e("Ctrl+F", "search files"),
        e("Ctrl+←→", "move by word"),
        e("Ctrl+Backspace", "delete previous word"),
        e("Ctrl+Del", "delete next word"),
        e("Ctrl+W / Alt+D", "fallback word delete"),
    ];
    let clipboard_entries = vec![
        e("Space", "toggle selection"),
        e("Ctrl+A", "select all"),
        e("Esc", "clear selection"),
        e(&kb.yank.to_string(), "yank (copy)"),
        e(&kb.copy_path.to_string(), "copy path details"),
        e(&kb.cut.to_string(), "cut"),
        e(&kb.paste.to_string(), "paste"),
    ];
    let rename_key = format!("{} / F2", kb.rename);
    let rename_trash_key = format!("{} (in trash)", kb.rename);
    let files_entries = vec![
        e(&kb.create.to_string(), "create file or folder"),
        e("Alt/Shift+Enter", "add line in create prompt"),
        e(&kb.trash.to_string(), "trash (delete if in trash)"),
        e(&rename_key, "rename (bulk if selection)"),
        e(&rename_trash_key, "restore from trash"),
        e(&kb.open.to_string(), "open externally"),
    ];
    let scroll_key = format!("{} / {}", kb.scroll_preview_left, kb.scroll_preview_right);
    let view_entries = vec![
        e(&kb.toggle_view.to_string(), "toggle grid / list"),
        e("+ / -", "grid zoom in / out"),
        e(&kb.toggle_hidden.to_string(), "toggle dotfiles"),
        e(&kb.sort.to_string(), "cycle sort"),
        e(&scroll_key, "scroll preview left / right"),
    ];
    let mouse_entries = vec![
        e("Click", "select item"),
        e("Double-click", "open item"),
        e("Wheel", "move selection"),
        e("Shift+Wheel", "scroll preview"),
    ];
    let left_sections = vec![
        HelpSection {
            title: "Navigate",
            entries: navigation_entries,
        },
        HelpSection {
            title: "Search",
            entries: search_entries,
        },
        HelpSection {
            title: "Mouse",
            entries: mouse_entries,
        },
    ];
    let right_sections = vec![
        HelpSection {
            title: "Files",
            entries: files_entries,
        },
        HelpSection {
            title: "Selection & Clipboard",
            entries: clipboard_entries,
        },
        HelpSection {
            title: "View",
            entries: view_entries,
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
        Paragraph::new(help_column_lines(cols[0].width, &left_sections, palette))
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
        Paragraph::new(help_column_lines(cols[2].width, &right_sections, palette))
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

struct HelpEntry {
    key: String,
    action: &'static str,
}

/// Convenience constructor — accepts anything that converts to a `String` for
/// the key so call sites can pass `&str`, `String`, or `&String` uniformly.
fn e(key: &str, action: &'static str) -> HelpEntry {
    HelpEntry {
        key: key.to_string(),
        action,
    }
}

struct HelpSection {
    title: &'static str,
    entries: Vec<HelpEntry>,
}

fn help_column_lines(width: u16, sections: &[HelpSection], palette: Palette) -> Vec<Line<'static>> {
    let content_width = width.max(1) as usize;
    let max_key_width = sections
        .iter()
        .flat_map(|section| section.entries.iter())
        .map(|entry| UnicodeWidthStr::width(entry.key.as_str()))
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
        for entry in &section.entries {
            lines.extend(help_entry_lines(entry, key_width, action_width, palette));
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
    entry: &HelpEntry,
    key_width: usize,
    action_width: usize,
    palette: Palette,
) -> Vec<Line<'static>> {
    let mut wrapped_action = wrap_help_action(entry.action, action_width);
    if wrapped_action.is_empty() {
        wrapped_action.push(String::new());
    }

    let key_padding =
        " ".repeat(key_width.saturating_sub(UnicodeWidthStr::width(entry.key.as_str())));
    let continuation = " ".repeat(key_width + 2);
    let mut lines = Vec::with_capacity(wrapped_action.len());

    lines.push(Line::from(vec![
        Span::styled(
            entry.key.clone(),
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
