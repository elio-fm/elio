use super::{
    PreparedStaticImage, SIXEL_DCS_CACHE_LIMIT, STATIC_IMAGE_INLINE_PAYLOAD_CACHE_LIMIT,
    STATIC_IMAGE_RENDER_CACHE_LIMIT, SixelDcsKey, StaticImageKey, StaticImageOverlayRequest,
};
use crate::app::App;
use std::{fs, path::PathBuf, sync::Arc};

impl App {
    fn cached_static_image_display_path(&mut self, key: &StaticImageKey) -> Option<PathBuf> {
        if let Some(path) = self.preview.image.rendered_images.get(key)
            && path.exists()
        {
            return Some(path.clone());
        }

        self.preview.image.rendered_images.remove(key);
        self.preview
            .image
            .render_order
            .retain(|queued| queued != key);
        None
    }

    fn cached_static_image_inline_payload(&self, key: &StaticImageKey) -> Option<Arc<str>> {
        self.preview.image.inline_payloads.get(key).cloned()
    }

    pub(super) fn remember_static_image_inline_payload(
        &mut self,
        key: StaticImageKey,
        payload: Arc<str>,
    ) {
        self.preview
            .image
            .inline_payloads
            .insert(key.clone(), payload);
        self.preview
            .image
            .payload_order
            .retain(|queued| queued != &key);
        self.preview.image.payload_order.push_back(key);
        while self.preview.image.payload_order.len() > STATIC_IMAGE_INLINE_PAYLOAD_CACHE_LIMIT {
            if let Some(stale_key) = self.preview.image.payload_order.pop_front() {
                self.preview.image.inline_payloads.remove(&stale_key);
            }
        }
    }

    pub(super) fn cached_prepared_static_image_for_overlay(
        &mut self,
        key: &StaticImageKey,
        request: &StaticImageOverlayRequest,
    ) -> Option<PreparedStaticImage> {
        let dimensions = self.preview.image.dimensions.get(key).copied()?;
        let inline_payload = if request.prepare_inline_payload {
            Some(self.cached_static_image_inline_payload(key)?)
        } else {
            None
        };
        if self.static_image_can_use_source_path(request) {
            return Some(PreparedStaticImage {
                display_path: request.path.clone(),
                dimensions,
                inline_payload,
            });
        }

        let display_path = self.cached_static_image_display_path(key)?;
        Some(PreparedStaticImage {
            display_path,
            dimensions,
            inline_payload,
        })
    }

    /// Look up a pre-encoded Sixel DCS buffer in the cache.
    pub(in crate::app::overlays) fn cached_sixel_dcs(
        &self,
        key: &SixelDcsKey,
    ) -> Option<Arc<[u8]>> {
        self.preview.image.sixel_dcs_payloads.get(key).cloned()
    }

    /// Insert a Sixel DCS buffer into the LRU cache, evicting the oldest
    /// entry once the limit is reached.
    pub(in crate::app::overlays) fn remember_sixel_dcs(
        &mut self,
        key: SixelDcsKey,
        dcs: Arc<[u8]>,
    ) {
        self.preview
            .image
            .sixel_dcs_payloads
            .insert(key.clone(), dcs);
        self.preview
            .image
            .sixel_dcs_order
            .retain(|queued| queued != &key);
        self.preview.image.sixel_dcs_order.push_back(key);
        while self.preview.image.sixel_dcs_order.len() > SIXEL_DCS_CACHE_LIMIT {
            if let Some(stale_key) = self.preview.image.sixel_dcs_order.pop_front() {
                self.preview.image.sixel_dcs_payloads.remove(&stale_key);
            }
        }
    }

    pub(super) fn remember_rendered_static_image(&mut self, key: StaticImageKey, path: PathBuf) {
        self.preview.image.rendered_images.insert(key.clone(), path);
        self.preview
            .image
            .render_order
            .retain(|queued| queued != &key);
        self.preview.image.render_order.push_back(key);
        while self.preview.image.render_order.len() > STATIC_IMAGE_RENDER_CACHE_LIMIT {
            if let Some(stale_key) = self.preview.image.render_order.pop_front()
                && let Some(stale_path) = self.preview.image.rendered_images.remove(&stale_key)
            {
                let _ = fs::remove_file(stale_path);
            }
        }
    }
}
