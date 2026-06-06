use crate::app::FrameState;
use crate::config::KeyBindings;
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

    let navigation_entries = navigation_entries(kb);
    let search_entries = vec![
        e(&kb.zoxide.to_string(), "zoxide history"),
        e(&kb.search_folders.to_string(), "search folders"),
        e(&kb.search_files.to_string(), "search files"),
        e("Ctrl+←→", "move by word"),
        e("Ctrl+Backspace", "delete previous word"),
        e("Ctrl+Del", "delete next word"),
        e("Ctrl+W / Alt+D", "fallback word delete"),
    ];
    let clipboard_entries = clipboard_entries(kb);
    let rename_key = kb.rename.to_string();
    let restore_key = format!("{} (in trash)", kb.restore_from_trash);
    let files_entries = vec![
        e(&kb.create.to_string(), "create file or folder"),
        e("Alt/Shift+Enter", "add line in create prompt"),
        e(&kb.trash.to_string(), "trash (delete if in trash)"),
        e(&kb.delete_permanently.to_string(), "delete permanently"),
        e(&rename_key, "rename (bulk if selection)"),
        e(&restore_key, "restore from trash"),
        e(&kb.shell.to_string(), "open shell here"),
        e(&kb.open.to_string(), "open with default app"),
        e(&kb.open_with.to_string(), "open with"),
    ];
    let view_entries = vec![
        e(&kb.toggle_view.to_string(), "toggle grid / list"),
        e("+ / -", "grid zoom in / out"),
        e(&kb.toggle_hidden.to_string(), "toggle dotfiles"),
        e(&kb.sort.to_string(), "cycle sort"),
        e(&kb.quit.to_string(), "quit"),
        e(&kb.quit_without_cd.to_string(), "quit without cd"),
    ];
    let preview_vertical_key =
        format_preview_scroll_key(&kb.scroll_preview_up, &kb.scroll_preview_down);
    let preview_horizontal_key =
        format_preview_scroll_key(&kb.scroll_preview_left, &kb.scroll_preview_right);
    let preview_entries = vec![
        e(&preview_vertical_key, "step page or scroll"),
        e(&preview_horizontal_key, "scroll left / right"),
    ];
    let mouse_entries = vec![
        e("Click", "select item"),
        e("Double-click", "open item"),
        e("Wheel", "scroll"),
        e("Shift+Wheel", "scroll sideways"),
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
            title: "Selection & Clipboard",
            entries: clipboard_entries,
        },
    ];
    let right_sections = vec![
        HelpSection {
            title: "Mouse",
            entries: mouse_entries,
        },
        HelpSection {
            title: "File Actions",
            entries: files_entries,
        },
        HelpSection {
            title: "Preview",
            entries: preview_entries,
        },
        HelpSection {
            title: "View",
            entries: view_entries,
        },
    ];

    let popup = helpers::centered_rect(area, 90, 33);
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
            helpers::chip_span("selection", palette.accent_soft, palette.accent_text, true),
            Span::raw(" "),
            helpers::chip_span("mouse", palette.accent_soft, palette.accent_text, true),
            Span::raw(" "),
            helpers::chip_span("actions", palette.accent_soft, palette.accent_text, true),
            Span::raw(" "),
            helpers::chip_span("preview", palette.accent_soft, palette.accent_text, true),
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
            Constraint::Length(46),
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

fn navigation_entries(kb: &KeyBindings) -> Vec<HelpEntry> {
    let parent_key = format_key_pair(&kb.nav_left, &kb.go_parent);
    let page_key = format_key_pair(&kb.page_up, &kb.page_down);
    let place_key = format_key_pair(&kb.cycle_places_next, &kb.cycle_places_previous);
    let history_key = format_key_pair(&kb.history_back, &kb.history_forward);

    vec![
        e(&kb.nav_up.to_string(), "move up"),
        e(&kb.nav_down.to_string(), "move down"),
        e(&parent_key, "parent folder"),
        e(&kb.nav_right.to_string(), "enter folder"),
        e(&kb.open_or_enter.to_string(), "enter folder / open"),
        e(&kb.go_to.to_string(), "go-to menu"),
        e(&kb.jump_first.to_string(), "first item"),
        e(&kb.jump_last.to_string(), "last item"),
        e(&page_key, "page up / down"),
        e(&place_key, "cycle places"),
        e(&history_key, "back / forward"),
    ]
}

fn clipboard_entries(kb: &KeyBindings) -> Vec<HelpEntry> {
    vec![
        e(&kb.toggle_selection.to_string(), "toggle selection"),
        e(&kb.select_all.to_string(), "select all"),
        e("Esc", "clear selection"),
        e(&kb.yank.to_string(), "yank (copy)"),
        e(&kb.copy_path.to_string(), "copy path details"),
        e(&kb.cut.to_string(), "cut"),
        e(&kb.paste.to_string(), "paste"),
    ]
}

fn format_key_pair(first: &crate::config::KeyList, second: &crate::config::KeyList) -> String {
    match (first.to_string(), second.to_string()) {
        (first, second) if first.is_empty() => second,
        (first, second) if second.is_empty() => first,
        (first, second) => format!("{first} / {second}"),
    }
}

/// Render a preview-scroll key pair: defaults like 'H'/'L' show as "Shift+H / Shift+L";
/// overrides such as '<'/'>' show as "< / >".
fn format_preview_scroll_key(
    low: &crate::config::KeyList,
    high: &crate::config::KeyList,
) -> String {
    match (low.single_char(), high.single_char()) {
        (Some(low), Some(high)) if low.is_ascii_uppercase() && high.is_ascii_uppercase() => {
            format!("Shift+{low} / Shift+{high}")
        }
        _ => format!("{low} / {high}"),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry_key(entries: &[HelpEntry], action: &str) -> String {
        entries
            .iter()
            .find(|entry| entry.action == action)
            .unwrap_or_else(|| panic!("missing help entry for {action:?}"))
            .key
            .clone()
    }

    #[test]
    fn help_uses_configurable_browser_control_defaults() {
        let kb = KeyBindings::default();
        let navigation = navigation_entries(&kb);
        let clipboard = clipboard_entries(&kb);

        assert_eq!(entry_key(&navigation, "go-to menu"), "g");
        assert_eq!(entry_key(&navigation, "first item"), "Home");
        assert_eq!(entry_key(&navigation, "last item"), "G/End");
        assert_eq!(
            entry_key(&navigation, "page up / down"),
            "PageUp / PageDown"
        );
        assert_eq!(entry_key(&navigation, "cycle places"), "Tab / Shift+Tab");
        assert_eq!(entry_key(&navigation, "back / forward"), "Alt+← / Alt+→");
        assert_eq!(entry_key(&clipboard, "toggle selection"), "Space");
        assert_eq!(entry_key(&clipboard, "select all"), "Ctrl+A");
    }

    #[test]
    fn help_reflects_rebound_browser_controls() {
        let kb = KeyBindings::from_toml_str(
            r#"[keys]
go_to = "u"
toggle_selection = "t"
cycle_places_next = "n"
cycle_places_previous = "P"
page_up = "<"
page_down = ">"
jump_first = "1"
jump_last = "2"
select_all = "A"
history_back = "alt+h"
history_forward = "alt+l"
"#,
        );
        let navigation = navigation_entries(&kb);
        let clipboard = clipboard_entries(&kb);

        assert_eq!(entry_key(&navigation, "go-to menu"), "u");
        assert_eq!(entry_key(&navigation, "first item"), "1");
        assert_eq!(entry_key(&navigation, "last item"), "2");
        assert_eq!(entry_key(&navigation, "page up / down"), "< / >");
        assert_eq!(entry_key(&navigation, "cycle places"), "n / P");
        assert_eq!(entry_key(&navigation, "back / forward"), "Alt+H / Alt+L");
        assert_eq!(entry_key(&clipboard, "toggle selection"), "t");
        assert_eq!(entry_key(&clipboard, "select all"), "A");
    }
}
