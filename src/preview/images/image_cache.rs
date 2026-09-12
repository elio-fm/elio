use super::{
    SixelDcsKey, StaticImageFormat, StaticImageKey, static_image_format_for_cached_path,
    static_image_supports_iterm_source_passthrough,
};
use crate::preview::{PreparedStaticImage, StaticImageOverlayRequest, state::ImagePreviewState};
use crate::terminal_runtime::terminal_images::ImageProtocol;
use std::{fs, path::PathBuf, sync::Arc};

const RENDERED_IMAGE_CACHE_LIMIT: usize = 64;
const INLINE_PAYLOAD_CACHE_LIMIT: usize = 16;
const SIXEL_DCS_CACHE_LIMIT: usize = 128;

impl ImagePreviewState {
    fn cached_display_path(&mut self, key: &StaticImageKey) -> Option<PathBuf> {
        if let Some(path) = self.rendered_images.get(key)
            && path.exists()
        {
            return Some(path.clone());
        }

        self.rendered_images.remove(key);
        self.render_order.retain(|queued| queued != key);
        None
    }

    fn cached_inline_payload(&self, key: &StaticImageKey) -> Option<Arc<str>> {
        self.inline_payloads.get(key).cloned()
    }

    pub(crate) fn remember_inline_payload(&mut self, key: StaticImageKey, payload: Arc<str>) {
        self.inline_payloads.insert(key.clone(), payload);
        self.payload_order.retain(|queued| queued != &key);
        self.payload_order.push_back(key);
        while self.payload_order.len() > INLINE_PAYLOAD_CACHE_LIMIT {
            if let Some(stale_key) = self.payload_order.pop_front() {
                self.inline_payloads.remove(&stale_key);
            }
        }
    }

    pub(crate) fn cached_prepared_image(
        &mut self,
        key: &StaticImageKey,
        request: &StaticImageOverlayRequest,
        protocol: ImageProtocol,
    ) -> Option<PreparedStaticImage> {
        let dimensions = self.dimensions.get(key).copied()?;
        let inline_payload = if request.prepare_inline_payload {
            Some(self.cached_inline_payload(key)?)
        } else {
            None
        };
        if can_use_source_path(protocol, request) {
            return Some(PreparedStaticImage {
                display_path: request.path.clone(),
                dimensions,
                inline_payload,
            });
        }

        let display_path = self.cached_display_path(key)?;
        Some(PreparedStaticImage {
            display_path,
            dimensions,
            inline_payload,
        })
    }

    pub(crate) fn cached_sixel_dcs(&self, key: &SixelDcsKey) -> Option<Arc<[u8]>> {
        self.sixel_dcs_payloads.get(key).cloned()
    }

    pub(crate) fn remember_sixel_dcs(&mut self, key: SixelDcsKey, dcs: Arc<[u8]>) {
        self.sixel_dcs_payloads.insert(key.clone(), dcs);
        self.sixel_dcs_order.retain(|queued| queued != &key);
        self.sixel_dcs_order.push_back(key);
        while self.sixel_dcs_order.len() > SIXEL_DCS_CACHE_LIMIT {
            if let Some(stale_key) = self.sixel_dcs_order.pop_front() {
                self.sixel_dcs_payloads.remove(&stale_key);
            }
        }
    }

    pub(crate) fn remember_rendered_image(&mut self, key: StaticImageKey, path: PathBuf) {
        self.rendered_images.insert(key.clone(), path);
        self.render_order.retain(|queued| queued != &key);
        self.render_order.push_back(key);
        while self.render_order.len() > RENDERED_IMAGE_CACHE_LIMIT {
            if let Some(stale_key) = self.render_order.pop_front()
                && let Some(stale_path) = self.rendered_images.remove(&stale_key)
            {
                let _ = fs::remove_file(stale_path);
            }
        }
    }
}

fn can_use_source_path(protocol: ImageProtocol, request: &StaticImageOverlayRequest) -> bool {
    match protocol {
        ImageProtocol::KittyGraphics | ImageProtocol::KittyDirectGraphics => {
            !request.force_render_to_cache
                && static_image_format_for_cached_path(
                    &request.path,
                    request.size,
                    request.modified,
                ) == Some(StaticImageFormat::Png)
        }
        ImageProtocol::ItermInline => static_image_supports_iterm_source_passthrough(
            &request.path,
            request.size,
            request.modified,
            request.force_render_to_cache,
        ),
        ImageProtocol::Sixel | ImageProtocol::None => false,
    }
}
