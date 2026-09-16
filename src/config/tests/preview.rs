use super::super::*;

#[test]
fn preview_tab_width_defaults_to_four() {
    assert_eq!(Config::default_config().preview.tab_width, 4);
    for source in ["", "[preview]"] {
        assert_eq!(Config::from_str(source).unwrap().preview.tab_width, 4);
    }
}

#[test]
fn preview_tab_width_accepts_full_range_and_clamps_outside_it() {
    for width in (-1..=17).chain([i64::MIN, 83294, i64::MAX]) {
        let config = Config::from_str(&format!("[preview]\ntab_width = {width}")).unwrap();
        assert_eq!(i64::from(config.preview.tab_width), width.clamp(1, 16));
    }
}

#[test]
fn preview_tab_width_rejects_non_integers() {
    for value in ["\"4\"", "4.5", "true"] {
        assert!(Config::from_str(&format!("[preview]\ntab_width = {value}")).is_err());
    }
}
