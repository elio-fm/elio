mod keys;
mod layout;
mod places;
mod ui;

fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}
