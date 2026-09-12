use super::FileOperationsState;
use anyhow::Result;
use std::{fs, path::Path, path::PathBuf};

pub(crate) struct CreateOverlay {
    pub(crate) lines: Vec<String>,
    pub(crate) cursor_line: usize,
    pub(crate) cursor_col: usize,
    pub(crate) preferred_col: usize,
    pub(crate) line_errors: Vec<Option<String>>,
}

pub(crate) struct CreateCompletion {
    pub(crate) reselect_path: Option<PathBuf>,
    pub(crate) status: String,
}

struct ParsedCreateItem {
    raw: String,
    name: String,
    is_dir: bool,
}

fn parse_create_line(line: &str) -> ParsedCreateItem {
    let is_dir = line.starts_with('/') || line.ends_with('/');
    let name = line.trim_matches('/').to_string();
    ParsedCreateItem {
        raw: line.to_string(),
        name,
        is_dir,
    }
}

fn validate_parsed_item(item: &ParsedCreateItem, cwd: &Path) -> Option<String> {
    if item.name.is_empty() {
        return Some("Name cannot be empty".to_string());
    }
    if item.name.contains('/') {
        return Some("Name cannot contain /".to_string());
    }
    if cwd.join(&item.name).exists() {
        return Some(format!("\"{}\" already exists", item.name));
    }
    None
}

impl FileOperationsState {
    pub(crate) fn open_create_prompt(&mut self) {
        self.create = Some(CreateOverlay {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            preferred_col: 0,
            line_errors: vec![None],
        });
    }
}

impl FileOperationsState {
    pub(crate) fn confirm_create(&mut self, cwd: &Path) -> Result<Option<CreateCompletion>> {
        let Some(c) = &self.create else {
            return Ok(None);
        };

        let items: Vec<(usize, ParsedCreateItem)> = c
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(index, line)| (index, parse_create_line(line)))
            .collect();

        if items.is_empty() {
            self.create = None;
            return Ok(None);
        }

        let mut errors: Vec<Option<String>> = self
            .create
            .as_ref()
            .expect("create overlay should still be present")
            .lines
            .iter()
            .map(|_| None)
            .collect();
        let mut first_error_line: Option<usize> = None;
        let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (line_idx, item) in &items {
            let msg = if !seen_names.insert(item.name.clone()) {
                Some(format!("\"{}\" appears more than once", item.name))
            } else {
                validate_parsed_item(item, cwd)
            };
            if let Some(msg) = msg {
                errors[*line_idx] = Some(msg);
                if first_error_line.is_none() {
                    first_error_line = Some(*line_idx);
                }
            }
        }

        if let Some(err_line) = first_error_line {
            if let Some(c) = &mut self.create {
                c.line_errors = errors;
                c.cursor_line = err_line;
                c.cursor_col = c.cursor_col.min(c.lines[err_line].chars().count());
                c.preferred_col = c.cursor_col;
            }
            return Ok(None);
        }

        let mut last_path: Option<std::path::PathBuf> = None;
        for (_, item) in &items {
            let path = cwd.join(&item.name);
            let result = if item.is_dir {
                fs::create_dir(&path).map_err(anyhow::Error::from)
            } else {
                fs::File::create_new(&path)
                    .map(|_| ())
                    .map_err(anyhow::Error::from)
            };
            if let Err(error) = result {
                let line_idx = items
                    .iter()
                    .find(|(_, candidate)| candidate.raw == item.raw)
                    .map(|(index, _)| *index)
                    .unwrap_or(0);
                let msg = error
                    .downcast_ref::<std::io::Error>()
                    .and_then(|io_error| match io_error.kind() {
                        std::io::ErrorKind::AlreadyExists => {
                            Some(format!("\"{}\" already exists", item.name))
                        }
                        std::io::ErrorKind::PermissionDenied => {
                            Some(format!("\"{}\" — permission denied", item.name))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| error.to_string());
                if let Some(c) = &mut self.create {
                    c.line_errors[line_idx] = Some(msg);
                    c.cursor_line = line_idx;
                }
                return Ok(None);
            }
            last_path = Some(path);
        }

        self.create = None;
        let files = items.iter().filter(|(_, item)| !item.is_dir).count();
        let dirs = items.iter().filter(|(_, item)| item.is_dir).count();
        let status = match (files, dirs) {
            (1, 0) => format!(
                "Created \"{}\"",
                items
                    .iter()
                    .find(|(_, item)| !item.is_dir)
                    .expect("file item should exist")
                    .1
                    .name
            ),
            (0, 1) => format!(
                "Created \"{}\"",
                items
                    .iter()
                    .find(|(_, item)| item.is_dir)
                    .expect("directory item should exist")
                    .1
                    .name
            ),
            (f, 0) => format!("Created {f} files"),
            (0, d) => format!("Created {d} folders"),
            (f, d) => format!(
                "Created {f} file{} and {d} folder{}",
                if f == 1 { "" } else { "s" },
                if d == 1 { "" } else { "s" },
            ),
        };
        Ok(Some(CreateCompletion {
            reselect_path: last_path,
            status,
        }))
    }
}

impl FileOperationsState {
    pub fn create_is_open(&self) -> bool {
        self.create.is_some()
    }

    pub fn create_line_count(&self) -> usize {
        self.create.as_ref().map_or(0, |c| c.lines.len())
    }

    pub fn create_line(&self, index: usize) -> &str {
        self.create
            .as_ref()
            .and_then(|c| c.lines.get(index))
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn create_cursor_line(&self) -> usize {
        self.create.as_ref().map_or(0, |c| c.cursor_line)
    }

    pub fn create_cursor_col(&self) -> usize {
        self.create.as_ref().map_or(0, |c| c.cursor_col)
    }

    pub fn create_title(&self) -> String {
        let Some(c) = &self.create else {
            return "Create".to_string();
        };
        let files = c
            .lines
            .iter()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && !trimmed.starts_with('/') && !trimmed.ends_with('/')
            })
            .count();
        let dirs = c
            .lines
            .iter()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && (trimmed.starts_with('/') || trimmed.ends_with('/'))
            })
            .count();
        match (files, dirs) {
            (0, 0) => "Create".to_string(),
            (f, 0) => format!("Create {} file{}", f, if f == 1 { "" } else { "s" }),
            (0, d) => format!("Create {} folder{}", d, if d == 1 { "" } else { "s" }),
            (f, d) => format!(
                "Create {} file{} and {} folder{}",
                f,
                if f == 1 { "" } else { "s" },
                d,
                if d == 1 { "" } else { "s" },
            ),
        }
    }

    pub fn create_line_error(&self, index: usize) -> Option<&str> {
        self.create
            .as_ref()
            .and_then(|c| c.line_errors.get(index))
            .and_then(Option::as_deref)
    }
}
