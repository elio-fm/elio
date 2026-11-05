use crate::filesystem::SortMode;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DateFormat {
    #[default]
    Relative,
    Absolute,
}

#[derive(Clone, Copy)]
pub(crate) struct UiConfig {
    pub show_top_bar: bool,
    pub grid_zoom: u8,
    pub show_hidden: bool,
    pub folders_first: bool,
    pub default_sort: SortMode,
    pub date_format: DateFormat,
    pub start_in_grid: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_top_bar: false,
            grid_zoom: 1,
            show_hidden: false,
            folders_first: true,
            default_sort: SortMode::Name,
            date_format: DateFormat::Relative,
            start_in_grid: false,
        }
    }
}

#[derive(Deserialize, Default)]
pub(super) struct UiConfigOverride {
    show_top_bar: Option<bool>,
    grid_zoom: Option<i64>,
    show_hidden: Option<bool>,
    folders_first: Option<bool>,
    default_sort: Option<SortMode>,
    date_format: Option<DateFormat>,
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
        if let Some(folders_first) = overrides.folders_first {
            self.folders_first = folders_first;
        }
        if let Some(default_sort) = overrides.default_sort {
            self.default_sort = default_sort;
        }
        if let Some(date_format) = overrides.date_format {
            self.date_format = date_format;
        }
        if let Some(start_in_grid) = overrides.start_in_grid {
            self.start_in_grid = start_in_grid;
        }
    }
}
