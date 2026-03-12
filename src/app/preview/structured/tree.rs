use super::{styled, StructuredPreview, LINE_LIMIT};
use crate::appearance;
use ratatui::{
    style::Modifier,
    text::{Line, Span},
};
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;

const INLINE_ARRAY_LIMIT: usize = 4;
const INLINE_OBJECT_LIMIT: usize = 3;
const INLINE_RENDER_LIMIT: usize = 64;
const STRING_PREVIEW_LIMIT: usize = 72;

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
        self.push_line(self.root_summary_line(value));
        self.push_line(vec![Span::raw(String::new())]);
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

            if let Some(inline) = self.inline_value(value) {
                self.push_line(self.inline_key_line(indent, key, inline));
                continue;
            }

            match value {
                TreeValue::Object(children) => {
                    self.push_line(self.key_line(indent, key, &container_summary(value)));
                    self.render_object(children, indent + 2);
                }
                TreeValue::Array(items) => {
                    self.push_line(self.key_line(indent, key, &container_summary(value)));
                    self.render_array(items, indent + 2);
                }
                _ => self.push_line(self.scalar_line(indent, Some(key), value)),
            }
        }
    }

    fn render_array(&mut self, items: &[TreeValue], indent: usize) {
        for (index, value) in items.iter().enumerate() {
            if self.truncated {
                return;
            }

            if let Some(inline) = self.inline_value(value) {
                self.push_line(self.array_scalar_line(indent, index, inline));
                continue;
            }

            match value {
                TreeValue::Object(children) => {
                    self.push_line(self.array_prefix_line(
                        indent,
                        index,
                        &container_summary(value),
                    ));
                    self.render_object(children, indent + 2);
                }
                TreeValue::Array(nested) => {
                    self.push_line(self.array_prefix_line(
                        indent,
                        index,
                        &container_summary(value),
                    ));
                    self.render_array(nested, indent + 2);
                }
                _ => self.push_line(self.array_scalar_line(
                    indent,
                    index,
                    value_spans(value, self.palette),
                )),
            }
        }
    }

    fn root_summary_line(&self, value: &TreeValue) -> Vec<Span<'static>> {
        let stats = value.stats();
        let mut spans = vec![
            styled("root", self.palette.parameter, Modifier::BOLD),
            styled(": ", self.palette.operator, Modifier::empty()),
            styled(value.kind_label(), self.palette.r#type, Modifier::empty()),
            Span::raw("  ".to_string()),
            Span::raw(value.root_extent_label()),
        ];
        if stats.max_depth > 1 {
            spans.push(Span::raw("  ".to_string()));
            spans.push(styled("depth", self.palette.parameter, Modifier::BOLD));
            spans.push(styled(": ", self.palette.operator, Modifier::empty()));
            spans.push(Span::raw(stats.max_depth.to_string()));
        }
        if stats.objects + stats.arrays > 1 {
            spans.push(Span::raw("  ".to_string()));
            spans.push(styled("containers", self.palette.parameter, Modifier::BOLD));
            spans.push(styled(": ", self.palette.operator, Modifier::empty()));
            spans.push(Span::raw((stats.objects + stats.arrays).to_string()));
        }
        spans
    }

    fn key_line(&self, indent: usize, key: &str, suffix: &str) -> Vec<Span<'static>> {
        vec![
            Span::raw(" ".repeat(indent)),
            styled(key, self.palette.function, Modifier::BOLD),
            styled(": ", self.palette.operator, Modifier::empty()),
            styled(suffix, self.palette.r#type, Modifier::empty()),
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

    fn inline_key_line(
        &self,
        indent: usize,
        key: &str,
        inline_value: Vec<Span<'static>>,
    ) -> Vec<Span<'static>> {
        let mut spans = vec![Span::raw(" ".repeat(indent))];
        spans.push(styled(key, self.palette.function, Modifier::BOLD));
        spans.push(styled(": ", self.palette.operator, Modifier::empty()));
        spans.extend(inline_value);
        spans
    }

    fn array_prefix_line(&self, indent: usize, index: usize, suffix: &str) -> Vec<Span<'static>> {
        vec![
            Span::raw(" ".repeat(indent)),
            styled(
                &format!("[{index}]"),
                self.palette.parameter,
                Modifier::BOLD,
            ),
            styled(": ", self.palette.operator, Modifier::empty()),
            styled(suffix, self.palette.r#type, Modifier::empty()),
        ]
    }

    fn array_scalar_line(
        &self,
        indent: usize,
        index: usize,
        value_spans: Vec<Span<'static>>,
    ) -> Vec<Span<'static>> {
        let mut spans = vec![
            Span::raw(" ".repeat(indent)),
            styled(
                &format!("[{index}]"),
                self.palette.parameter,
                Modifier::BOLD,
            ),
            styled(": ", self.palette.operator, Modifier::empty()),
        ];
        spans.extend(value_spans);
        spans
    }

    fn inline_value(&self, value: &TreeValue) -> Option<Vec<Span<'static>>> {
        let rendered = render_inline_value(value)?;
        if rendered.chars().count() > INLINE_RENDER_LIMIT {
            return None;
        }
        Some(vec![styled(
            &rendered,
            self.palette.r#type,
            Modifier::empty(),
        )])
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

#[derive(Clone, Copy, Debug, Default)]
struct TreeStats {
    objects: usize,
    arrays: usize,
    scalars: usize,
    max_depth: usize,
}

impl TreeValue {
    fn stats(&self) -> TreeStats {
        match self {
            Self::Null | Self::Bool(_) | Self::Number(_) | Self::String(_) => TreeStats {
                scalars: 1,
                max_depth: 1,
                ..TreeStats::default()
            },
            Self::Array(values) => {
                let mut stats = TreeStats {
                    arrays: 1,
                    max_depth: 1,
                    ..TreeStats::default()
                };
                for value in values {
                    let child = value.stats();
                    stats.objects += child.objects;
                    stats.arrays += child.arrays;
                    stats.scalars += child.scalars;
                    stats.max_depth = stats.max_depth.max(child.max_depth + 1);
                }
                stats
            }
            Self::Object(values) => {
                let mut stats = TreeStats {
                    objects: 1,
                    max_depth: 1,
                    ..TreeStats::default()
                };
                for (_, value) in values {
                    let child = value.stats();
                    stats.objects += child.objects;
                    stats.arrays += child.arrays;
                    stats.scalars += child.scalars;
                    stats.max_depth = stats.max_depth.max(child.max_depth + 1);
                }
                stats
            }
        }
    }

    fn kind_label(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }

    fn root_extent_label(&self) -> String {
        match self {
            Self::Object(values) => format!("{} keys", values.len()),
            Self::Array(values) => format!("{} items", values.len()),
            Self::String(value) => format!("{} chars", value.chars().count()),
            _ => "scalar".to_string(),
        }
    }
}

fn container_summary(value: &TreeValue) -> String {
    match value {
        TreeValue::Object(values) => format!("{{{} keys}}", values.len()),
        TreeValue::Array(values) => format!("[{} items]", values.len()),
        _ => value.kind_label().to_string(),
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
        TreeValue::String(value) => {
            let truncated = truncate_string(value, STRING_PREVIEW_LIMIT);
            let mut spans = vec![styled(
                &format!("\"{}\"", escaped_string(&truncated)),
                palette.string,
                Modifier::empty(),
            )];
            if truncated != *value {
                spans.push(Span::raw(" ".to_string()));
                spans.push(styled(
                    &format!("({} chars)", value.chars().count()),
                    palette.comment,
                    Modifier::empty(),
                ));
            }
            spans
        }
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

fn render_inline_value(value: &TreeValue) -> Option<String> {
    match value {
        TreeValue::Null => Some("null".to_string()),
        TreeValue::Bool(value) => Some(value.to_string()),
        TreeValue::Number(value) => Some(value.clone()),
        TreeValue::String(value) => {
            if value.chars().count() > 24 {
                return None;
            }
            Some(format!("\"{}\"", escaped_string(value)))
        }
        TreeValue::Array(values) => {
            if values.len() > INLINE_ARRAY_LIMIT
                || values.iter().any(|value| !is_inline_scalar(value))
            {
                return None;
            }
            let rendered = values
                .iter()
                .map(render_inline_value)
                .collect::<Option<Vec<_>>>()?
                .join(", ");
            Some(format!("[{rendered}]"))
        }
        TreeValue::Object(values) => {
            if values.len() > INLINE_OBJECT_LIMIT
                || values.iter().any(|(_, value)| !is_inline_scalar(value))
            {
                return None;
            }
            let rendered = values
                .iter()
                .map(|(key, value)| {
                    render_inline_value(value).map(|value| format!("{key}: {value}"))
                })
                .collect::<Option<Vec<_>>>()?
                .join(", ");
            Some(format!("{{{rendered}}}"))
        }
    }
}

fn is_inline_scalar(value: &TreeValue) -> bool {
    matches!(
        value,
        TreeValue::Null | TreeValue::Bool(_) | TreeValue::Number(_) | TreeValue::String(_)
    )
}

fn truncate_string(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }
    let kept = value
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    format!("{kept}…")
}

fn escaped_string(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{{{:x}}}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped
}
