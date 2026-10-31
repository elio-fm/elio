use crate::{
    app::{App, ScreenRegions, ScrollbarTarget},
    filesystem::{format_item_count, format_size, format_time_ago, sanitize_terminal_text},
    preview::PreviewDirectoryStatsState,
    theme::{self, Palette},
    ui::{
        helpers,
        scrollbars::{ScrollbarContext, ScrollbarSpec, render_overlay_scrollbar},
    },
};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

const DETAIL_LABEL_WIDTH: usize = 17;

pub(in crate::ui) fn render_item_details_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut ScreenRegions,
    palette: Palette,
) {
    let Some(details) = app.overlays.item_details.as_ref() else {
        return;
    };

    let popup_width = area.width.saturating_sub(4).clamp(10, 76);
    let popup_height = area.height.saturating_sub(2).clamp(4, 16);
    let popup = helpers::centered_rect(area, popup_width, popup_height);
    state.item_details_panel = Some(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        helpers::panel_block(" Item details ", palette.panel, palette),
        popup,
    );

    let mut inner = helpers::inner_with_padding(popup);
    if inner.width > 1 {
        inner.x = inner.x.saturating_add(1);
        inner.width = inner.width.saturating_sub(1);
    }
    let appearance = theme::resolve_entry(&details.entry);
    let icon_width = inner.width.min(if details.icon_png_path.is_some() {
        4
    } else {
        1
    });
    let stack_name = inner.width <= icon_width.saturating_add(8);
    let header_height = inner.height.min(if stack_name { 4 } else { 3 });
    let header = Rect {
        height: header_height,
        ..inner
    };
    let body = Rect {
        y: inner.y.saturating_add(header_height),
        height: inner.height.saturating_sub(header_height),
        ..inner
    };
    let icon_area = Rect {
        x: header.x,
        y: header.y,
        width: header.width.min(icon_width),
        height: if stack_name {
            header.height.saturating_sub(1).min(3)
        } else {
            header.height.min(3)
        },
    };
    state.item_details_icon_area = details.icon_png_path.as_ref().map(|_| icon_area);
    render_large_icon(frame, details, icon_area, appearance.color, palette);

    let pdf_document_key = crate::preview::PdfDocumentKey::from_entry(&details.entry);
    let pdf_page_count = app
        .preview
        .pdf
        .document_page_counts
        .get(&pdf_document_key)
        .map(usize::to_string);
    let document_page_count = details
        .document_page_count
        .as_deref()
        .or(pdf_page_count.as_deref());
    let document_author = details.document_author.as_deref().or_else(|| {
        app.preview
            .pdf
            .document_authors
            .get(&pdf_document_key)
            .map(String::as_str)
    });

    let name = sanitize_terminal_text(&details.entry.name);
    let name_x = if stack_name {
        header.x
    } else {
        header.x.saturating_add(icon_area.width).saturating_add(1)
    };
    let name_y = if stack_name {
        header.y.saturating_add(icon_area.height)
    } else {
        header.y.saturating_add(1)
    };
    let name_area = Rect {
        x: name_x,
        y: name_y,
        width: header.x.saturating_add(header.width).saturating_sub(name_x),
        height: header
            .y
            .saturating_add(header.height)
            .saturating_sub(name_y),
    };
    let name_width = helpers::display_width(&name);
    let available_name_width = usize::from(name_area.width);
    let name_width_limit = if name_width > available_name_width {
        available_name_width.saturating_sub(1)
    } else {
        available_name_width
    };
    let name = helpers::truncate_middle(&name, name_width_limit);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            name,
            Style::default()
                .fg(palette.text)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true }),
        name_area,
    );

    let mut rows = vec![
        detail_row("Type", &details.kind_label, palette),
        detail_row(
            "Size",
            if details.is_directory() {
                directory_summary(app, details).unwrap_or_else(|| "—".to_owned())
            } else {
                format_size(details.entry.size)
            }
            .as_str(),
            palette,
        ),
    ];
    if let Some(unpacked_size) = details.archive_unpacked_size {
        rows.push(detail_row(
            "Unpacked size",
            &format_size(unpacked_size),
            palette,
        ));
    }
    if let Some(link) = details.entry.symlink.as_ref() {
        rows.insert(
            1,
            detail_row(
                "Link target",
                link.target
                    .as_deref()
                    .map(|path| sanitize_terminal_text(&path.display().to_string()))
                    .as_deref()
                    .unwrap_or("Unavailable"),
                palette,
            ),
        );
    }
    if let Some((width, height)) = details.image_dimensions {
        rows.push(detail_row(
            "Dimensions",
            &format!("{width} × {height} px"),
            palette,
        ));
    }
    if let Some(dimensions) = details.media_dimensions.as_deref() {
        rows.push(detail_row("Dimensions", dimensions, palette));
    }
    if let Some(duration) = details.media_duration.as_deref() {
        rows.push(detail_row("Duration", duration, palette));
    }
    if let Some(codec) = details.media_codec.as_deref() {
        rows.push(detail_row("Video codec", codec, palette));
    }
    if let Some(frame_rate) = details.media_frame_rate.as_deref() {
        rows.push(detail_row("FPS", frame_rate, palette));
    }
    if let Some(artist) = details.audio_artist.as_deref() {
        rows.push(detail_row("Artist", artist, palette));
    }
    if let Some(album) = details.audio_album.as_deref() {
        rows.push(detail_row("Album", album, palette));
    }
    if let Some(entry_count) = details.archive_entry_count {
        rows.push(detail_row(
            "Contents",
            &format!("{entry_count} items"),
            palette,
        ));
    }
    if let Some(password_protected) = details.archive_password_protected {
        rows.push(detail_row(
            "Password",
            if password_protected {
                "Protected"
            } else {
                "No"
            },
            palette,
        ));
    }
    if let Some(page_count) = document_page_count {
        rows.push(detail_row("Pages", page_count, palette));
    }
    if let Some(author) = document_author {
        rows.push(detail_row("Author", author, palette));
    }
    if let Some(writer) = details.comic_writer.as_deref() {
        rows.push(detail_row("Writer", writer, palette));
    }
    if let Some(penciller) = details.comic_penciller.as_deref() {
        rows.push(detail_row("Penciller", penciller, palette));
    }
    if let Some(language) = details.language {
        rows.push(detail_row("Language", language, palette));
    }
    rows.push(detail_row(
        "Modified",
        details
            .modified
            .map(format_time_ago)
            .as_deref()
            .unwrap_or("Unknown"),
        palette,
    ));
    if let Some(created) = details.created {
        rows.push(detail_row("Created", &format_time_ago(created), palette));
    }
    rows.push(detail_row(
        "Permissions",
        if details.entry.readonly {
            "Read-only"
        } else {
            "Read and write"
        },
        palette,
    ));
    let value_column_width = DETAIL_LABEL_WIDTH + 1;
    let location_width = usize::from(body.width.saturating_sub(value_column_width as u16).max(1));
    let location = helpers::stable_path_label(&details.path, location_width);
    rows.push(detail_row("Location", &location, palette));
    if let Some(original_location) = details.original_location.as_deref() {
        let original_location = original_location
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(original_location);
        let original_location = helpers::stable_path_label(original_location, location_width);
        rows.push(detail_row("Original location", &original_location, palette));
    }
    if let Some(trashed_at) = details.trashed_at {
        rows.push(detail_row("Trashed", &format_time_ago(trashed_at), palette));
    }
    if body.height == 0 || body.width == 0 {
        state.item_details_scroll_max = 0;
        state.item_details_rows_visible = 0;
        return;
    }

    let mut content = body;
    let initial_total = wrapped_line_count(&rows, content.width);
    let mut total = initial_total;
    let mut scrollbar = None;
    if total > content.height as usize && content.width > 1 {
        content.width = content.width.saturating_sub(1);
        scrollbar = Some(Rect {
            x: content.x.saturating_add(content.width),
            y: content.y,
            width: 1,
            height: content.height,
        });
        total = wrapped_line_count(&rows, content.width);
    }
    let visible = content.height as usize;
    let max_scroll = total.saturating_sub(visible);
    let scroll_top = app.overlays.item_details_scroll.min(max_scroll);
    state.item_details_scroll_max = max_scroll;
    state.item_details_rows_visible = visible;
    frame.render_widget(
        Paragraph::new(rows)
            .wrap(Wrap { trim: true })
            .scroll((scroll_top.min(u16::MAX as usize) as u16, 0)),
        content,
    );
    if let Some(scrollbar) = scrollbar {
        render_overlay_scrollbar(
            frame,
            scrollbar,
            ScrollbarSpec::new(total, visible, scroll_top),
            palette,
            ScrollbarContext::new(state, ScrollbarTarget::ItemDetails),
        );
    }
}

fn wrapped_line_count(rows: &[Line<'_>], width: u16) -> usize {
    let width = usize::from(width.max(1));
    rows.iter()
        .map(|line| {
            let line_width = line
                .spans
                .iter()
                .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
                .sum::<usize>();
            line_width.max(1).div_ceil(width)
        })
        .sum()
}

fn directory_summary(app: &App, details: &crate::item_details::ItemDetails) -> Option<String> {
    let stats = app.preview.state.directory_stats.as_ref()?;
    if stats.path() != &details.entry.path {
        return None;
    }
    match stats {
        PreviewDirectoryStatsState::Loading { .. } => Some("Calculating…".to_owned()),
        PreviewDirectoryStatsState::Complete { stats, .. } => Some(format!(
            "{} • {}",
            format_item_count(stats.item_count),
            format_size(stats.total_size_bytes)
        )),
        PreviewDirectoryStatsState::Incomplete { partial, .. } => Some(format!(
            "At least {} • at least {}",
            format_item_count(partial.item_count),
            format_size(partial.total_size_bytes),
        )),
    }
}

fn render_large_icon(
    frame: &mut Frame<'_>,
    details: &crate::item_details::ItemDetails,
    area: Rect,
    color: ratatui::style::Color,
    palette: Palette,
) {
    if details.icon_png_path.is_some() {
        frame.render_widget(
            Paragraph::new(" ".repeat(usize::from(area.width)))
                .style(Style::default().bg(palette.panel)),
            area,
        );
        return;
    }
    frame.render_widget(
        Paragraph::new(Span::styled(
            details.icon,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Left)
        .style(Style::default().bg(palette.panel)),
        Rect {
            y: area.y.saturating_add(area.height.saturating_sub(1) / 2),
            height: 1,
            ..area
        },
    );
}

fn detail_row(label: &str, value: &str, palette: Palette) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<DETAIL_LABEL_WIDTH$} "),
            Style::default().fg(palette.muted),
        ),
        Span::styled(value.to_owned(), Style::default().fg(palette.text)),
    ])
}
