use super::*;
use crate::ui::theme;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

#[derive(Clone, Debug, Default)]
struct ListContext {
    next_index: Option<u64>,
}

#[derive(Clone, Debug)]
struct ItemContext {
    prefix: String,
    continuation: String,
    used_prefix: bool,
}

#[derive(Clone, Debug)]
struct CodeBlockContext {
    language: String,
    text: String,
}

#[derive(Clone, Debug)]
struct InlineTarget {
    destination: String,
    is_image: bool,
}

struct MarkdownRenderer {
    palette: theme::Palette,
    lines: Vec<Line<'static>>,
    current: Vec<Span<'static>>,
    styles: Vec<Style>,
    list_stack: Vec<ListContext>,
    current_item: Option<ItemContext>,
    blockquote_depth: usize,
    heading_level: Option<HeadingLevel>,
    code_block: Option<CodeBlockContext>,
    inline_targets: Vec<InlineTarget>,
    pending_table_cell_separator: bool,
}

pub(super) fn render_markdown_preview(text: &str) -> Vec<Line<'static>> {
    let mut renderer = MarkdownRenderer::new(theme::palette());
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_TABLES);

    for event in Parser::new_ext(text, options) {
        if renderer.is_full() {
            break;
        }
        renderer.handle_event(event);
    }

    renderer.finish()
}

impl MarkdownRenderer {
    fn new(palette: theme::Palette) -> Self {
        Self {
            palette,
            lines: Vec::new(),
            current: Vec::new(),
            styles: Vec::new(),
            list_stack: Vec::new(),
            current_item: None,
            blockquote_depth: 0,
            heading_level: None,
            code_block: None,
            inline_targets: Vec::new(),
            pending_table_cell_separator: false,
        }
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        self.flush_line_if_content();
        self.trim_trailing_blank_lines();
        if self.lines.is_empty() {
            self.lines.push(Line::from("File is empty"));
        }
        self.lines.truncate(PREVIEW_RENDER_LINE_LIMIT);
        self.lines
    }

    fn is_full(&self) -> bool {
        self.lines.len() >= PREVIEW_RENDER_LINE_LIMIT
    }

    fn handle_event(&mut self, event: Event<'_>) {
        if let Some(code_block) = &mut self.code_block {
            match event {
                Event::End(TagEnd::CodeBlock) => {
                    let code_block = self.code_block.take().expect("code block should exist");
                    self.render_code_block(code_block);
                    self.push_blank_line();
                }
                Event::Text(text)
                | Event::Code(text)
                | Event::Html(text)
                | Event::InlineHtml(text) => {
                    code_block.text.push_str(&text);
                }
                Event::SoftBreak | Event::HardBreak => code_block.text.push('\n'),
                _ => {}
            }
            return;
        }

        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.push_text(&text),
            Event::Code(text) => self.push_inline_code(&text),
            Event::Html(html) | Event::InlineHtml(html) => {
                self.push_styled_text(&html, Style::default().fg(self.palette.muted))
            }
            Event::SoftBreak => self.line_break(),
            Event::HardBreak => self.line_break(),
            Event::Rule => self.push_rule(),
            Event::TaskListMarker(checked) => self.set_task_marker(checked),
            Event::FootnoteReference(label) => self.push_text(&format!("[{label}]")),
            Event::InlineMath(text) | Event::DisplayMath(text) => self.push_inline_code(&text),
        }
    }

    fn start_tag(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                if self.current_item.is_none() {
                    self.ensure_block_gap();
                }
            }
            Tag::Heading { level, .. } => {
                self.ensure_block_gap();
                self.heading_level = Some(level);
            }
            Tag::BlockQuote(_) => {
                self.ensure_block_gap();
                self.blockquote_depth += 1;
            }
            Tag::List(start) => {
                self.ensure_block_gap();
                self.list_stack.push(ListContext { next_index: start });
            }
            Tag::Item => {
                self.flush_line_if_content();
                self.current_item = Some(self.make_item_context());
            }
            Tag::CodeBlock(kind) => {
                self.ensure_block_gap();
                self.code_block = Some(CodeBlockContext {
                    language: code_block_label(kind),
                    text: String::new(),
                });
            }
            Tag::Emphasis => self
                .styles
                .push(Style::default().add_modifier(Modifier::ITALIC)),
            Tag::Strong => self
                .styles
                .push(Style::default().add_modifier(Modifier::BOLD)),
            Tag::Strikethrough => self
                .styles
                .push(Style::default().add_modifier(Modifier::CROSSED_OUT)),
            Tag::Link { dest_url, .. } => {
                self.styles.push(
                    Style::default()
                        .fg(self.palette.accent)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                );
                self.inline_targets.push(InlineTarget {
                    destination: dest_url.to_string(),
                    is_image: false,
                });
            }
            Tag::Image { dest_url, .. } => {
                self.push_styled_text("image: ", Style::default().fg(self.palette.muted));
                self.styles.push(
                    Style::default()
                        .fg(self.palette.accent)
                        .add_modifier(Modifier::ITALIC),
                );
                self.inline_targets.push(InlineTarget {
                    destination: dest_url.to_string(),
                    is_image: true,
                });
            }
            Tag::Table(_) => {
                self.ensure_block_gap();
                self.pending_table_cell_separator = false;
            }
            Tag::TableHead | Tag::TableRow => {
                self.flush_line_if_content();
                self.pending_table_cell_separator = false;
            }
            Tag::TableCell => {
                if self.pending_table_cell_separator {
                    self.push_styled_text(" | ", Style::default().fg(self.palette.border));
                }
                self.pending_table_cell_separator = true;
            }
            _ => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.flush_line_if_content();
                if self.current_item.is_none() {
                    self.push_blank_line();
                }
            }
            TagEnd::Heading(_) => {
                self.flush_line_if_content();
                self.heading_level = None;
                self.push_blank_line();
            }
            TagEnd::BlockQuote(_) => {
                self.flush_line_if_content();
                self.blockquote_depth = self.blockquote_depth.saturating_sub(1);
                self.push_blank_line();
            }
            TagEnd::List(_) => {
                self.flush_line_if_content();
                self.list_stack.pop();
                self.push_blank_line();
            }
            TagEnd::Item => {
                self.flush_line_if_content();
                self.current_item = None;
            }
            TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Strikethrough
            | TagEnd::Link
            | TagEnd::Image => {
                self.styles.pop();
                if let Some(target) = self.inline_targets.pop() {
                    self.push_inline_destination(&target);
                }
            }
            TagEnd::Table | TagEnd::TableHead | TagEnd::TableRow => {
                self.flush_line_if_content();
                self.pending_table_cell_separator = false;
                if matches!(tag, TagEnd::Table) {
                    self.push_blank_line();
                }
            }
            TagEnd::TableCell => {}
            _ => {}
        }
    }

    fn make_item_context(&mut self) -> ItemContext {
        let depth = self.list_stack.len().saturating_sub(1);
        let indent = "  ".repeat(depth);
        let marker = if let Some(list) = self.list_stack.last_mut() {
            if let Some(next_index) = list.next_index.as_mut() {
                let marker = format!("{next_index}. ");
                *next_index += 1;
                marker
            } else {
                "• ".to_string()
            }
        } else {
            "• ".to_string()
        };
        let continuation = format!("{}{}", indent, " ".repeat(marker.len()));
        ItemContext {
            prefix: format!("{indent}{marker}"),
            continuation,
            used_prefix: false,
        }
    }

    fn set_task_marker(&mut self, checked: bool) {
        let Some(item) = &mut self.current_item else {
            return;
        };
        let indent_len = item.prefix.len().saturating_sub(2);
        let indent = " ".repeat(indent_len);
        let marker = if checked { "󰄬 " } else { "󰄱 " };
        item.prefix = format!("{indent}{marker}");
        item.continuation = format!("{indent}  ");
    }

    fn push_rule(&mut self) {
        self.ensure_block_gap();
        self.push_line(Line::from(Span::styled(
            "────────────────",
            Style::default().fg(self.palette.border),
        )));
        self.push_blank_line();
    }

    fn push_inline_code(&mut self, text: &str) {
        self.ensure_prefix();
        self.current.push(Span::styled(
            text.to_string(),
            Style::default()
                .fg(self.palette.accent_text)
                .bg(self.palette.accent_soft)
                .add_modifier(Modifier::BOLD),
        ));
    }

    fn push_text(&mut self, text: &str) {
        for (index, segment) in text.split('\n').enumerate() {
            if index > 0 {
                self.line_break();
            }
            if segment.is_empty() {
                continue;
            }
            self.push_styled_text(segment, self.current_style());
        }
    }

    fn push_styled_text(&mut self, text: &str, style: Style) {
        self.ensure_prefix();
        self.current.push(Span::styled(text.to_string(), style));
    }

    fn current_style(&self) -> Style {
        let mut style = Style::default().fg(self.palette.text);
        if let Some(level) = self.heading_level {
            style = style.patch(heading_style(level, self.palette));
        }
        if self.blockquote_depth > 0 {
            style = style.patch(Style::default().fg(self.palette.muted));
        }
        for extra in &self.styles {
            style = style.patch(*extra);
        }
        style
    }

    fn ensure_prefix(&mut self) {
        if !self.current.is_empty() {
            return;
        }

        for _ in 0..self.blockquote_depth {
            self.current
                .push(Span::styled("▎ ", Style::default().fg(self.palette.accent)));
        }

        if let Some(item) = &mut self.current_item {
            let prefix = if item.used_prefix {
                item.continuation.clone()
            } else {
                item.used_prefix = true;
                item.prefix.clone()
            };
            if !prefix.is_empty() {
                self.current.push(Span::styled(
                    prefix,
                    Style::default().fg(self.palette.accent),
                ));
            }
        }
    }

    fn ensure_block_gap(&mut self) {
        self.flush_line_if_content();
        self.push_blank_line();
    }

    fn push_blank_line(&mut self) {
        if self.lines.is_empty() || self.last_line_is_blank() {
            return;
        }
        self.push_line(Line::from(String::new()));
    }

    fn last_line_is_blank(&self) -> bool {
        self.lines
            .last()
            .is_some_and(|line| line.spans.iter().all(|span| span.content.is_empty()))
    }

    fn trim_trailing_blank_lines(&mut self) {
        while self.last_line_is_blank() {
            self.lines.pop();
        }
    }

    fn push_inline_destination(&mut self, target: &InlineTarget) {
        let destination = target.destination.trim();
        if destination.is_empty() {
            return;
        }

        self.push_styled_text(" ", Style::default().fg(self.palette.muted));
        self.push_styled_text(
            &format!("({destination})"),
            Style::default()
                .fg(if target.is_image {
                    self.palette.muted
                } else {
                    self.palette.accent
                })
                .add_modifier(if target.is_image {
                    Modifier::ITALIC
                } else {
                    Modifier::UNDERLINED
                }),
        );
    }

    fn line_break(&mut self) {
        self.flush_line();
    }

    fn flush_line_if_content(&mut self) {
        if !self.current.is_empty() {
            self.flush_line();
        }
    }

    fn flush_line(&mut self) {
        if self.is_full() {
            self.current.clear();
            return;
        }
        let line = if self.current.is_empty() {
            Line::from(String::new())
        } else {
            Line::from(std::mem::take(&mut self.current))
        };
        self.push_line(line);
    }

    fn push_line(&mut self, line: Line<'static>) {
        if self.is_full() {
            return;
        }
        self.lines.push(line);
    }

    fn render_code_block(&mut self, code_block: CodeBlockContext) {
        self.flush_line_if_content();
        self.push_line(Line::from(vec![
            Span::styled(
                "󰆍 ",
                Style::default()
                    .fg(self.palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                code_block.language.clone(),
                Style::default()
                    .fg(self.palette.muted)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));

        let preview_spec = super::code::registry::language_for_markdown_fence(&code_block.language)
            .map(|language| language.preview_spec())
            .unwrap_or(crate::file_info::PreviewSpec {
                kind: crate::file_info::PreviewKind::Source,
                language_hint: None,
                code_syntax: None,
                code_backend: crate::file_info::CodeBackend::Plain,
                structured_format: None,
                document_format: None,
            });
        let rendered = super::code::render_code_preview(
            preview_spec,
            &code_block.text,
            false,
            PREVIEW_RENDER_LINE_LIMIT,
            &|| false,
        );
        for line in rendered {
            if self.is_full() {
                break;
            }
            self.push_line(line);
        }
    }
}

fn heading_style(level: HeadingLevel, palette: theme::Palette) -> Style {
    let color = match level {
        HeadingLevel::H1 => palette.accent_text,
        HeadingLevel::H2 => palette.accent,
        HeadingLevel::H3 => palette.text,
        _ => palette.muted,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn code_block_label(kind: CodeBlockKind<'_>) -> String {
    match kind {
        CodeBlockKind::Indented => "text".to_string(),
        CodeBlockKind::Fenced(language) => {
            let trimmed = language.trim();
            if trimmed.is_empty() {
                "text".to_string()
            } else {
                trimmed.to_string()
            }
        }
    }
}
