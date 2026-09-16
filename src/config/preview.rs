use serde::Deserialize;

#[derive(Clone, Copy)]
pub(crate) struct PreviewConfig {
    pub(crate) tab_width: u8,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self { tab_width: 4 }
    }
}

#[derive(Deserialize, Default)]
pub(super) struct PreviewConfigOverride {
    tab_width: Option<i64>,
}

impl PreviewConfig {
    pub(super) fn apply_override(&mut self, overrides: PreviewConfigOverride) {
        if let Some(tab_width) = overrides.tab_width {
            self.tab_width = tab_width.clamp(1, 16) as u8;
        }
    }
}
