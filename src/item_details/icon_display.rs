use crate::{
    app::App,
    terminal_images::{self, ImageProtocol},
};
use anyhow::{Context, Result};
use ratatui::layout::Rect;

impl App {
    pub(crate) fn stale_item_details_icon_area(&self) -> Option<Rect> {
        let (displayed, displayed_path, _) = self.overlays.item_details_icon_displayed.as_ref()?;
        let current = self
            .overlays
            .item_details
            .as_ref()
            .and_then(|details| details.icon_png_path.as_ref());
        (current != Some(displayed_path)
            || self.input.screen_regions.item_details_icon_area != Some(*displayed)
            || self.overlays.item_details_icon_geometry_invalidated)
            .then_some(*displayed)
    }

    pub(crate) fn invalidate_item_details_icon_geometry(&mut self) {
        if self.overlays.item_details_icon_displayed.is_some() {
            self.overlays.item_details_icon_geometry_invalidated = true;
            self.input.screen_regions.item_details_icon_area = None;
        }
    }

    pub(crate) fn refresh_item_details_icon_for_terminal_geometry(&mut self) {
        let protocol = self.preview.terminal_images.protocol;
        let window = self.cached_terminal_window();
        if matches!(protocol, ImageProtocol::None | ImageProtocol::KittyGraphics) {
            return;
        }
        let Some(details) = self.overlays.item_details.as_mut() else {
            return;
        };
        let appearance = crate::theme::resolve_entry(&details.entry);
        let icon_path = match protocol {
            ImageProtocol::None => None,
            ImageProtocol::KittyGraphics => {
                super::icon_renderer::render_icon_png(details.icon, appearance.color)
            }
            _ => window.and_then(|window| {
                let cell_width = window.pixels_width / u32::from(window.cells_width.max(1));
                let cell_height = window.pixels_height / u32::from(window.cells_height.max(1));
                super::icon_renderer::render_icon_png_for_cell_area(
                    details.icon,
                    appearance.color,
                    cell_width.saturating_mul(4),
                    cell_height.saturating_mul(3),
                )
            }),
        };
        if details.icon_png_path != icon_path {
            if let Some(previous) = std::mem::replace(&mut details.icon_png_path, icon_path) {
                let _ = std::fs::remove_file(previous);
            }
        } else if details.icon_png_path.is_none() {
            details.icon_png_path = icon_path;
        }
    }

    pub(crate) fn clear_stale_item_details_icon(&mut self) -> Result<Vec<u8>> {
        let Some((displayed_area, displayed_path, image_id)) =
            self.overlays.item_details_icon_displayed.clone()
        else {
            return Ok(Vec::new());
        };
        let current_details = self
            .overlays
            .item_details
            .as_ref()
            .and_then(|details| details.icon_png_path.as_ref());
        if current_details == Some(&displayed_path)
            && self.input.screen_regions.item_details_icon_area == Some(displayed_area)
            && !self.overlays.item_details_icon_geometry_invalidated
        {
            return Ok(Vec::new());
        }

        self.overlays.item_details_icon_displayed = None;
        self.overlays.item_details_icon_geometry_invalidated = false;
        if image_id.is_some() {
            terminal_images::clear_item_details_icon(
                self.preview.terminal_images.protocol,
                image_id,
            )
        } else if self.static_image_overlay_displayed() || self.pdf_overlay_displayed() {
            self.clear_preview_overlay()
        } else {
            terminal_images::clear_item_details_icon(
                self.preview.terminal_images.protocol,
                image_id,
            )
            .context("failed to clear item details icon")
        }
    }

    pub(crate) fn present_item_details_icon(&mut self, force_repaint: bool) -> Result<Vec<u8>> {
        let protocol = self.preview.terminal_images.protocol;
        if self.terminal_image_resize_settling() {
            return Ok(Vec::new());
        }
        let Some(area) = self.input.screen_regions.item_details_icon_area else {
            return Ok(Vec::new());
        };
        let Some(path) = self
            .overlays
            .item_details
            .as_ref()
            .and_then(|details| details.icon_png_path.as_deref())
        else {
            return Ok(Vec::new());
        };
        if protocol == ImageProtocol::None {
            return Ok(Vec::new());
        }
        let path = path.to_path_buf();
        if self
            .overlays
            .item_details_icon_displayed
            .as_ref()
            .is_some_and(|(displayed_area, displayed_path, _)| {
                *displayed_area == area && displayed_path == &path
            })
            && !(force_repaint && protocol == ImageProtocol::ItermInline)
        {
            return Ok(Vec::new());
        }

        let (bytes, image_id) = terminal_images::place_item_details_icon(
            protocol,
            &path,
            area,
            self.cached_terminal_window(),
        )
        .context("failed to render item details icon")?;
        self.overlays.item_details_icon_displayed = Some((area, path, image_id));
        Ok(bytes)
    }
}
