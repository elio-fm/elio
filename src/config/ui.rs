use serde::Deserialize;

#[derive(Clone, Copy)]
pub(crate) struct UiConfig {
    pub show_top_bar: bool,
    pub grid_zoom: u8,
    pub show_hidden: bool,
    pub start_in_grid: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_top_bar: false,
            grid_zoom: 1,
            show_hidden: false,
            start_in_grid: false,
        }
    }
}

#[derive(Deserialize, Default)]
pub(super) struct UiConfigOverride {
    show_top_bar: Option<bool>,
    grid_zoom: Option<i64>,
    show_hidden: Option<bool>,
    start_in_grid: Option<bool>,
}

impl UiConfig {
    pub(super) fn apply_override(&mut self, overrides: UiConfigOverride) {
        if let Some(show_top_bar) = overrides.show_top_bar {
            self.show_top_bar = show_top_bar;
        }
        if let Some(zoom) = overrides.grid_zoom {
            self.grid_zoom = zoom.clamp(0, 2) as u8;
        }
        if let Some(show_hidden) = overrides.show_hidden {
            self.show_hidden = show_hidden;
        }
        if let Some(start_in_grid) = overrides.start_in_grid {
            self.start_in_grid = start_in_grid;
        }
    }
}
