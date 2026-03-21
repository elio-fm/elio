use super::{
    PreparedStaticImage, STATIC_IMAGE_INLINE_PAYLOAD_CACHE_LIMIT, STATIC_IMAGE_RENDER_CACHE_LIMIT,
    StaticImageKey, StaticImageOverlayRequest,
};
use crate::app::App;
use std::{fs, path::PathBuf, sync::Arc};

impl App {
    fn cached_static_image_display_path(&mut self, key: &StaticImageKey) -> Option<PathBuf> {
        if let Some(path) = self.image_preview.rendered_images.get(key)
            && path.exists()
        {
            return Some(path.clone());
        }

        self.image_preview.rendered_images.remove(key);
        self.image_preview
            .render_order
            .retain(|queued| queued != key);
        None
    }

    fn cached_static_image_inline_payload(&self, key: &StaticImageKey) -> Option<Arc<str>> {
        self.image_preview.inline_payloads.get(key).cloned()
    }

    pub(super) fn remember_static_image_inline_payload(
        &mut self,
        key: StaticImageKey,
        payload: Arc<str>,
    ) {
        self.image_preview
            .inline_payloads
            .insert(key.clone(), payload);
        self.image_preview
            .payload_order
            .retain(|queued| queued != &key);
        self.image_preview.payload_order.push_back(key);
        while self.image_preview.payload_order.len() > STATIC_IMAGE_INLINE_PAYLOAD_CACHE_LIMIT {
            if let Some(stale_key) = self.image_preview.payload_order.pop_front() {
                self.image_preview.inline_payloads.remove(&stale_key);
            }
        }
    }

    pub(super) fn cached_prepared_static_image_for_overlay(
        &mut self,
        key: &StaticImageKey,
        request: &StaticImageOverlayRequest,
    ) -> Option<PreparedStaticImage> {
        let dimensions = self.image_preview.dimensions.get(key).copied()?;
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

    pub(super) fn remember_rendered_static_image(&mut self, key: StaticImageKey, path: PathBuf) {
        self.image_preview.rendered_images.insert(key.clone(), path);
        self.image_preview
            .render_order
            .retain(|queued| queued != &key);
        self.image_preview.render_order.push_back(key);
        while self.image_preview.render_order.len() > STATIC_IMAGE_RENDER_CACHE_LIMIT {
            if let Some(stale_key) = self.image_preview.render_order.pop_front()
                && let Some(stale_path) = self.image_preview.rendered_images.remove(&stale_key)
            {
                let _ = fs::remove_file(stale_path);
            }
        }
    }
}
