use super::{LINE_LIMIT, StructuredPreview, styled};
use crate::appearance;
use ratatui::{
    style::Modifier,
    text::{Line, Span},
};
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;

pub(super) fn render_json_preview(text: &str, detail: &'static str) -> Option<StructuredPreview> {
    serde_json::from_str::<JsonValue>(text)
        .ok()
        .map(|value| render_tree_preview(value_to_tree(value), detail))
}

pub(super) fn render_json5_preview(text: &str, detail: &'static str) -> Option<StructuredPreview> {
    json5::from_str::<JsonValue>(text)
        .ok()
        .map(|value| render_tree_preview(value_to_tree(value), detail))
}

pub(super) fn render_toml_preview(text: &str, detail: &'static str) -> Option<StructuredPreview> {
    toml::from_str::<toml::Value>(text)
        .ok()
        .map(|value| render_tree_preview(toml_to_tree(value), detail))
}

pub(super) fn render_yaml_preview(text: &str, detail: &'static str) -> Option<StructuredPreview> {
    serde_yaml::from_str::<YamlValue>(text)
        .ok()
        .map(|value| render_tree_preview(yaml_to_tree(value), detail))
}

#[derive(Clone, Debug)]
enum TreeValue {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<TreeValue>),
    Object(Vec<(String, TreeValue)>),
}

struct TreeRenderer {
    palette: appearance::CodePreviewPalette,
    lines: Vec<Line<'static>>,
    truncated: bool,
}

impl TreeRenderer {
    fn new() -> Self {
        Self {
            palette: appearance::code_preview_palette(),
            lines: Vec::new(),
            truncated: false,
        }
    }

    fn push_line(&mut self, spans: Vec<Span<'static>>) {
        if self.lines.len() >= LINE_LIMIT {
            self.truncated = true;
            return;
        }
        self.lines.push(Line::from(spans));
    }

    fn render_root(&mut self, value: &TreeValue) {
        match value {
            TreeValue::Object(entries) => self.render_object(entries, 0),
            TreeValue::Array(items) => self.render_array(items, 0),
            _ => self.push_line(self.scalar_line(0, None, value)),
        }
    }

    fn render_object(&mut self, entries: &[(String, TreeValue)], indent: usize) {
        for (key, value) in entries {
            if self.truncated {
                return;
            }

            match value {
                TreeValue::Object(children) => {
                    self.push_line(self.key_line(indent, key, "{}"));
                    self.render_object(children, indent + 2);
                }
                TreeValue::Array(items) => {
                    self.push_line(self.key_line(indent, key, "[]"));
                    self.render_array(items, indent + 2);
                }
                _ => self.push_line(self.scalar_line(indent, Some(key), value)),
            }
        }
    }

    fn render_array(&mut self, items: &[TreeValue], indent: usize) {
        for value in items {
            if self.truncated {
                return;
            }

            match value {
                TreeValue::Object(children) => {
                    self.push_line(self.array_prefix_line(indent, "{}"));
                    self.render_object(children, indent + 2);
                }
                TreeValue::Array(nested) => {
                    self.push_line(self.array_prefix_line(indent, "[]"));
                    self.render_array(nested, indent + 2);
                }
                _ => self.push_line(self.array_scalar_line(indent, value)),
            }
        }
    }

    fn key_line(&self, indent: usize, key: &str, suffix: &str) -> Vec<Span<'static>> {
        vec![
            Span::raw(" ".repeat(indent)),
            styled(key, self.palette.function, Modifier::BOLD),
            styled(": ", self.palette.operator, Modifier::empty()),
            styled(suffix, self.palette.operator, Modifier::empty()),
        ]
    }

    fn scalar_line(
        &self,
        indent: usize,
        key: Option<&str>,
        value: &TreeValue,
    ) -> Vec<Span<'static>> {
        let mut spans = vec![Span::raw(" ".repeat(indent))];
        if let Some(key) = key {
            spans.push(styled(key, self.palette.function, Modifier::BOLD));
            spans.push(styled(": ", self.palette.operator, Modifier::empty()));
        }
        spans.extend(value_spans(value, self.palette));
        spans
    }

    fn array_prefix_line(&self, indent: usize, suffix: &str) -> Vec<Span<'static>> {
        vec![
            Span::raw(" ".repeat(indent)),
            styled("- ", self.palette.operator, Modifier::empty()),
            styled(suffix, self.palette.operator, Modifier::empty()),
        ]
    }

    fn array_scalar_line(&self, indent: usize, value: &TreeValue) -> Vec<Span<'static>> {
        let mut spans = vec![
            Span::raw(" ".repeat(indent)),
            styled("- ", self.palette.operator, Modifier::empty()),
        ];
        spans.extend(value_spans(value, self.palette));
        spans
    }
}

fn render_tree_preview(value: TreeValue, detail: &'static str) -> StructuredPreview {
    let mut renderer = TreeRenderer::new();
    renderer.render_root(&value);

    if renderer.lines.is_empty() {
        renderer.push_line(vec![Span::raw("File is empty".to_string())]);
    }

    StructuredPreview {
        lines: renderer.lines,
        detail,
        truncation_note: renderer
            .truncated
            .then(|| format!("showing first {LINE_LIMIT} lines")),
    }
}

fn value_to_tree(value: JsonValue) -> TreeValue {
    match value {
        JsonValue::Null => TreeValue::Null,
        JsonValue::Bool(value) => TreeValue::Bool(value),
        JsonValue::Number(value) => TreeValue::Number(value.to_string()),
        JsonValue::String(value) => TreeValue::String(value),
        JsonValue::Array(values) => {
            TreeValue::Array(values.into_iter().map(value_to_tree).collect())
        }
        JsonValue::Object(values) => TreeValue::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, value_to_tree(value)))
                .collect(),
        ),
    }
}

fn toml_to_tree(value: toml::Value) -> TreeValue {
    match value {
        toml::Value::String(value) => TreeValue::String(value),
        toml::Value::Integer(value) => TreeValue::Number(value.to_string()),
        toml::Value::Float(value) => TreeValue::Number(value.to_string()),
        toml::Value::Boolean(value) => TreeValue::Bool(value),
        toml::Value::Datetime(value) => TreeValue::String(value.to_string()),
        toml::Value::Array(values) => {
            TreeValue::Array(values.into_iter().map(toml_to_tree).collect())
        }
        toml::Value::Table(values) => TreeValue::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, toml_to_tree(value)))
                .collect(),
        ),
    }
}

fn yaml_to_tree(value: YamlValue) -> TreeValue {
    match value {
        YamlValue::Null => TreeValue::Null,
        YamlValue::Bool(value) => TreeValue::Bool(value),
        YamlValue::Number(value) => TreeValue::Number(value.to_string()),
        YamlValue::String(value) => TreeValue::String(value),
        YamlValue::Sequence(values) => {
            TreeValue::Array(values.into_iter().map(yaml_to_tree).collect())
        }
        YamlValue::Mapping(values) => TreeValue::Object(
            values
                .into_iter()
                .map(|(key, value)| (yaml_key_to_string(key), yaml_to_tree(value)))
                .collect(),
        ),
        YamlValue::Tagged(tagged) => yaml_to_tree(tagged.value),
    }
}

fn yaml_key_to_string(value: YamlValue) -> String {
    match value {
        YamlValue::Null => "null".to_string(),
        YamlValue::Bool(value) => value.to_string(),
        YamlValue::Number(value) => value.to_string(),
        YamlValue::String(value) => value,
        YamlValue::Sequence(_) | YamlValue::Mapping(_) | YamlValue::Tagged(_) => {
            format!("{value:?}")
        }
    }
}

fn value_spans(value: &TreeValue, palette: appearance::CodePreviewPalette) -> Vec<Span<'static>> {
    match value {
        TreeValue::Null => vec![styled("null", palette.keyword, Modifier::empty())],
        TreeValue::Bool(value) => vec![styled(
            if *value { "true" } else { "false" },
            palette.keyword,
            Modifier::empty(),
        )],
        TreeValue::Number(value) => vec![styled(value, palette.constant, Modifier::empty())],
        TreeValue::String(value) => vec![styled(
            &format!("\"{value}\""),
            palette.string,
            Modifier::empty(),
        )],
        TreeValue::Array(values) => vec![styled(
            &format!("[{} items]", values.len()),
            palette.r#type,
            Modifier::empty(),
        )],
        TreeValue::Object(values) => vec![styled(
            &format!("{{{} keys}}", values.len()),
            palette.r#type,
            Modifier::empty(),
        )],
    }
}
