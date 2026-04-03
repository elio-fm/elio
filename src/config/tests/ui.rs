use super::super::*;

#[test]
fn config_defaults_hide_top_bar() {
    let config = Config::default_config();
    assert!(!config.ui.show_top_bar);
}

#[test]
fn config_default_hides_hidden_files() {
    let config = Config::default_config();
    assert!(!config.ui.show_hidden);
}

#[test]
fn config_default_starts_in_list_view() {
    let config = Config::default_config();
    assert!(!config.ui.start_in_grid);
}

#[test]
fn config_default_grid_zoom_is_1() {
    let config = Config::default_config();
    assert_eq!(config.ui.grid_zoom, 1);
}

#[test]
fn config_can_enable_show_hidden() {
    let config = Config::from_str("[ui]\nshow_hidden = true").expect("config should parse");
    assert!(config.ui.show_hidden);
}

#[test]
fn config_can_enable_start_in_grid() {
    let config = Config::from_str("[ui]\nstart_in_grid = true").expect("config should parse");
    assert!(config.ui.start_in_grid);
}

#[test]
fn config_can_set_grid_zoom() {
    let config = Config::from_str("[ui]\ngrid_zoom = 0").expect("config should parse");
    assert_eq!(config.ui.grid_zoom, 0);

    let config = Config::from_str("[ui]\ngrid_zoom = 2").expect("config should parse");
    assert_eq!(config.ui.grid_zoom, 2);
}

#[test]
fn config_grid_zoom_clamps_out_of_range() {
    let config = Config::from_str("[ui]\ngrid_zoom = -3").expect("config should parse");
    assert_eq!(config.ui.grid_zoom, 0);

    let config = Config::from_str("[ui]\ngrid_zoom = 4").expect("config should parse");
    assert_eq!(config.ui.grid_zoom, 2);
}

#[test]
fn config_can_enable_top_bar() {
    let config = Config::from_str(
        r#"
[ui]
show_top_bar = true
"#,
    )
    .expect("config should parse");

    assert!(config.ui.show_top_bar);
}
