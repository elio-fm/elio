use super::super::theme::Palette;
use super::entries::render_entries;
use super::preview::render_preview;
use super::sidebar::render_sidebar;
use crate::{
    app::{App, FrameState},
    config::{self, PaneWeights},
};
use ratatui::{Frame, layout::Rect};

const LEGACY_WIDE_SIDEBAR_WIDTH: u16 = 24;
const LEGACY_NARROW_SIDEBAR_WIDTH: u16 = 22;
const LEGACY_NARROW_PREVIEW_HEIGHT: u16 = 11;
const LEGACY_HORIZONTAL_SIDEBAR_MIN_WIDTH: u16 = 18;
const LEGACY_HORIZONTAL_ENTRIES_MIN_WIDTH: u16 = 26;
const LEGACY_HORIZONTAL_PREVIEW_MIN_WIDTH: u16 = 22;
const CUSTOM_SIDEBAR_MIN_WIDTH: u16 = 16;
const CUSTOM_ENTRIES_MIN_WIDTH: u16 = 28;
const CUSTOM_PREVIEW_MIN_WIDTH: u16 = 24;
const CUSTOM_STACKED_ENTRIES_MIN_HEIGHT: u16 = 10;
const CUSTOM_STACKED_PREVIEW_DESIRED_HEIGHT: u16 = LEGACY_NARROW_PREVIEW_HEIGHT;
const CUSTOM_STACKED_PREVIEW_MIN_HEIGHT: u16 = 8;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct BodyLayout {
    pub sidebar: Option<Rect>,
    pub entries: Option<Rect>,
    pub preview: Option<Rect>,
}

#[derive(Clone, Copy)]
enum PaneRole {
    Sidebar,
    Entries,
    Preview,
}

#[derive(Clone, Copy)]
struct WeightedPane {
    role: PaneRole,
    weight: u16,
}

pub(in crate::ui) fn render_body(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    state: &mut FrameState,
    palette: Palette,
) {
    let layout = resolve_body_layout(area, config::layout().panes);

    if let Some(sidebar) = layout.sidebar {
        render_sidebar(frame, sidebar, app, state, palette);
    }
    if let Some(entries) = layout.entries {
        render_entries(frame, entries, app, state, palette);
    }
    if let Some(preview) = layout.preview {
        render_preview(frame, preview, app, state, palette);
    }
}

pub(super) fn resolve_body_layout(area: Rect, pane_weights: Option<PaneWeights>) -> BodyLayout {
    pane_weights.map_or_else(
        || legacy_body_layout(area),
        |weights| custom_body_layout(area, weights),
    )
}

fn legacy_body_layout(area: Rect) -> BodyLayout {
    if let Some(layout) = legacy_horizontal_body_layout(area) {
        return layout;
    }

    if let Some(layout) = legacy_stacked_body_layout(area) {
        return layout;
    }

    legacy_sidebar_and_entries_layout(area)
}

fn legacy_horizontal_body_layout(area: Rect) -> Option<BodyLayout> {
    let (sidebar, content) = split_sidebar_and_content(
        area,
        LEGACY_WIDE_SIDEBAR_WIDTH,
        LEGACY_HORIZONTAL_SIDEBAR_MIN_WIDTH,
        LEGACY_HORIZONTAL_ENTRIES_MIN_WIDTH.saturating_add(LEGACY_HORIZONTAL_PREVIEW_MIN_WIDTH),
    )?;
    let widths = allocate_weighted_lengths(content.width, vec![54, 46]);
    if widths.len() != 2
        || widths[0] < LEGACY_HORIZONTAL_ENTRIES_MIN_WIDTH
        || widths[1] < LEGACY_HORIZONTAL_PREVIEW_MIN_WIDTH
    {
        return None;
    }

    let entries = Rect {
        x: content.x,
        y: content.y,
        width: widths[0],
        height: content.height,
    };
    let preview = Rect {
        x: content.x.saturating_add(widths[0]),
        y: content.y,
        width: widths[1],
        height: content.height,
    };

    Some(BodyLayout {
        sidebar: non_empty(sidebar),
        entries: non_empty(entries),
        preview: non_empty(preview),
    })
}

fn legacy_stacked_body_layout(area: Rect) -> Option<BodyLayout> {
    let (sidebar, content) = split_sidebar_and_content(
        area,
        LEGACY_NARROW_SIDEBAR_WIDTH,
        LEGACY_NARROW_SIDEBAR_WIDTH,
        CUSTOM_ENTRIES_MIN_WIDTH.max(CUSTOM_PREVIEW_MIN_WIDTH),
    )?;
    let (entries, preview) =
        split_stacked_content_with_height(content, LEGACY_NARROW_PREVIEW_HEIGHT)?;

    Some(BodyLayout {
        sidebar: non_empty(sidebar),
        entries: non_empty(entries),
        preview: non_empty(preview),
    })
}

fn legacy_sidebar_and_entries_layout(area: Rect) -> BodyLayout {
    if let Some((sidebar, entries)) = split_sidebar_and_content(
        area,
        LEGACY_NARROW_SIDEBAR_WIDTH,
        CUSTOM_SIDEBAR_MIN_WIDTH,
        CUSTOM_ENTRIES_MIN_WIDTH,
    ) {
        return BodyLayout {
            sidebar: non_empty(sidebar),
            entries: non_empty(entries),
            preview: None,
        };
    }

    BodyLayout {
        sidebar: None,
        entries: non_empty(area),
        preview: None,
    }
}

fn custom_body_layout(area: Rect, weights: PaneWeights) -> BodyLayout {
    let show_sidebar = weights.places > 0;
    let show_preview = weights.preview > 0;

    let mut panes = Vec::with_capacity(3);
    if show_sidebar {
        panes.push(WeightedPane {
            role: PaneRole::Sidebar,
            weight: weights.places,
        });
    }
    panes.push(WeightedPane {
        role: PaneRole::Entries,
        weight: weights.files,
    });
    if show_preview {
        panes.push(WeightedPane {
            role: PaneRole::Preview,
            weight: weights.preview,
        });
    }

    if let Some(layout) = horizontal_body_layout_with_mins(area, &panes) {
        return layout;
    }

    if show_preview && let Some(layout) = stacked_body_layout_with_mins(area, weights) {
        return layout;
    }

    match (show_sidebar, show_preview) {
        (false, false) => BodyLayout {
            sidebar: None,
            entries: non_empty(area),
            preview: None,
        },
        (true, false) => sidebar_and_entries_layout(area, weights),
        (false, true) => BodyLayout {
            sidebar: None,
            entries: non_empty(area),
            preview: None,
        },
        (true, true) => sidebar_and_entries_layout(area, weights),
    }
}

fn sidebar_and_entries_layout(area: Rect, weights: PaneWeights) -> BodyLayout {
    let panes = [
        WeightedPane {
            role: PaneRole::Sidebar,
            weight: weights.places,
        },
        WeightedPane {
            role: PaneRole::Entries,
            weight: weights.files,
        },
    ];
    horizontal_body_layout_with_mins(area, &panes)
        .unwrap_or_else(|| horizontal_body_layout_best_effort(area, &panes))
}

fn horizontal_body_layout_with_mins(area: Rect, panes: &[WeightedPane]) -> Option<BodyLayout> {
    let widths = allocate_weighted_lengths_with_mins(
        area.width,
        panes.iter().map(|pane| pane.weight).collect(),
        panes.iter().map(|pane| pane_min_width(pane.role)).collect(),
    )?;
    Some(body_layout_from_widths(area, panes, widths))
}

fn horizontal_body_layout_best_effort(area: Rect, panes: &[WeightedPane]) -> BodyLayout {
    let widths =
        allocate_weighted_lengths(area.width, panes.iter().map(|pane| pane.weight).collect());
    body_layout_from_widths(area, panes, widths)
}

fn body_layout_from_widths(area: Rect, panes: &[WeightedPane], widths: Vec<u16>) -> BodyLayout {
    let mut x = area.x;
    let mut layout = BodyLayout::default();

    for (pane, width) in panes.iter().zip(widths) {
        let rect = Rect {
            x,
            y: area.y,
            width,
            height: area.height,
        };
        x = x.saturating_add(width);
        match pane.role {
            PaneRole::Sidebar => layout.sidebar = non_empty(rect),
            PaneRole::Entries => layout.entries = non_empty(rect),
            PaneRole::Preview => layout.preview = non_empty(rect),
        }
    }

    layout
}

fn stacked_body_layout_with_mins(area: Rect, weights: PaneWeights) -> Option<BodyLayout> {
    let show_sidebar = weights.places > 0;
    let (sidebar, content) = if show_sidebar {
        let widths = allocate_weighted_lengths_with_mins(
            area.width,
            vec![
                weights.places,
                weights.files.saturating_add(weights.preview),
            ],
            vec![
                CUSTOM_SIDEBAR_MIN_WIDTH,
                CUSTOM_ENTRIES_MIN_WIDTH.max(CUSTOM_PREVIEW_MIN_WIDTH),
            ],
        )?;
        let sidebar = Rect {
            x: area.x,
            y: area.y,
            width: widths[0],
            height: area.height,
        };
        let content = Rect {
            x: area.x.saturating_add(widths[0]),
            y: area.y,
            width: widths[1],
            height: area.height,
        };
        (non_empty(sidebar), content)
    } else {
        (None, area)
    };

    let (entries, preview) =
        split_stacked_content_with_height(content, CUSTOM_STACKED_PREVIEW_DESIRED_HEIGHT)?;
    Some(BodyLayout {
        sidebar,
        entries: non_empty(entries),
        preview: non_empty(preview),
    })
}

fn split_sidebar_and_content(
    area: Rect,
    preferred_sidebar_width: u16,
    minimum_sidebar_width: u16,
    minimum_content_width: u16,
) -> Option<(Rect, Rect)> {
    if area.width < minimum_sidebar_width.saturating_add(minimum_content_width) {
        return None;
    }

    let sidebar_width = preferred_sidebar_width
        .min(area.width.saturating_sub(minimum_content_width))
        .max(minimum_sidebar_width);
    let content_width = area.width.saturating_sub(sidebar_width);

    let sidebar = Rect {
        x: area.x,
        y: area.y,
        width: sidebar_width,
        height: area.height,
    };
    let content = Rect {
        x: area.x.saturating_add(sidebar_width),
        y: area.y,
        width: content_width,
        height: area.height,
    };

    Some((sidebar, content))
}

fn split_stacked_content_with_height(
    area: Rect,
    desired_preview_height: u16,
) -> Option<(Rect, Rect)> {
    if area.height
        < CUSTOM_STACKED_ENTRIES_MIN_HEIGHT.saturating_add(CUSTOM_STACKED_PREVIEW_MIN_HEIGHT)
    {
        return None;
    }

    let preview_height = area
        .height
        .saturating_sub(CUSTOM_STACKED_ENTRIES_MIN_HEIGHT)
        .min(desired_preview_height);
    if preview_height < CUSTOM_STACKED_PREVIEW_MIN_HEIGHT {
        return None;
    }
    let entries_height = area.height.saturating_sub(preview_height);
    if entries_height < CUSTOM_STACKED_ENTRIES_MIN_HEIGHT {
        return None;
    }

    let entries = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: entries_height,
    };
    let preview = Rect {
        x: area.x,
        y: area.y.saturating_add(entries_height),
        width: area.width,
        height: preview_height,
    };

    Some((entries, preview))
}

fn pane_min_width(role: PaneRole) -> u16 {
    match role {
        PaneRole::Sidebar => CUSTOM_SIDEBAR_MIN_WIDTH,
        PaneRole::Entries => CUSTOM_ENTRIES_MIN_WIDTH,
        PaneRole::Preview => CUSTOM_PREVIEW_MIN_WIDTH,
    }
}

fn allocate_weighted_lengths_with_mins(
    total: u16,
    weights: Vec<u16>,
    mins: Vec<u16>,
) -> Option<Vec<u16>> {
    if weights.len() != mins.len() {
        return None;
    }

    let min_total = mins.iter().copied().sum::<u16>();
    if total < min_total {
        return None;
    }

    let extra = allocate_weighted_lengths(total.saturating_sub(min_total), weights);
    Some(
        mins.into_iter()
            .zip(extra)
            .map(|(min, extra)| min.saturating_add(extra))
            .collect(),
    )
}

fn allocate_weighted_lengths(total: u16, weights: Vec<u16>) -> Vec<u16> {
    if weights.is_empty() {
        return Vec::new();
    }
    if total == 0 {
        return vec![0; weights.len()];
    }

    let total_weight: u32 = weights.iter().map(|weight| *weight as u32).sum();

    let mut lengths = vec![0; weights.len()];
    let mut assigned = 0u16;
    let mut remainders = Vec::with_capacity(weights.len());

    for (index, weight) in weights.iter().copied().enumerate() {
        let product = total as u32 * weight as u32;
        let portion = if total_weight == 0 {
            0
        } else {
            (product / total_weight) as u16
        };
        lengths[index] = portion;
        assigned = assigned.saturating_add(portion);
        remainders.push((
            if total_weight == 0 {
                0
            } else {
                product % total_weight
            },
            index,
        ));
    }

    remainders.sort_by(|left, right| right.cmp(left));
    for (_, index) in remainders
        .into_iter()
        .take(total.saturating_sub(assigned) as usize)
    {
        lengths[index] = lengths[index].saturating_add(1);
    }

    if total >= weights.iter().filter(|weight| **weight > 0).count() as u16 {
        ensure_nonzero_positive_widths(&weights, &mut lengths);
    }

    lengths
}

fn non_empty(rect: Rect) -> Option<Rect> {
    (!rect.is_empty()).then_some(rect)
}

fn ensure_nonzero_positive_widths(weights: &[u16], lengths: &mut [u16]) {
    for (index, weight) in weights.iter().copied().enumerate() {
        if weight == 0 || lengths[index] > 0 {
            continue;
        }

        if let Some((donor_index, _)) = lengths
            .iter()
            .copied()
            .enumerate()
            .filter(|(donor_index, width)| weights[*donor_index] > 0 && *width > 1)
            .max_by_key(|(_, width)| *width)
        {
            lengths[donor_index] = lengths[donor_index].saturating_sub(1);
            lengths[index] = 1;
        }
    }
}
