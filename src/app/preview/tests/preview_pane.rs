use super::super::preview_pane::preview_pane_disabled_by_layout;
use crate::config::{LayoutConfig, PaneWeights};

#[test]
fn preview_pane_is_disabled_when_custom_preview_weight_is_zero() {
    assert!(preview_pane_disabled_by_layout(LayoutConfig {
        panes: Some(PaneWeights {
            places: 1,
            files: 99,
            preview: 0,
        }),
    }));
    assert!(!preview_pane_disabled_by_layout(LayoutConfig {
        panes: Some(PaneWeights {
            places: 1,
            files: 98,
            preview: 1,
        }),
    }));
    assert!(!preview_pane_disabled_by_layout(LayoutConfig {
        panes: None,
    }));
}
