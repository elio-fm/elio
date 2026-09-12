use super::OpenWithApplication;

#[derive(Clone, Debug)]
pub(crate) struct ApplicationSelectionRow {
    pub(crate) shortcut: Option<char>,
    pub(crate) label: String,
    pub(crate) application: OpenWithApplication,
}

#[derive(Clone, Debug)]
pub(crate) struct ApplicationSelection {
    pub(crate) title: String,
    pub(crate) rows: Vec<ApplicationSelectionRow>,
    pub(crate) selected: usize,
}

impl ApplicationSelection {
    pub(crate) fn new(applications: Vec<OpenWithApplication>, reserved_shortcuts: &[char]) -> Self {
        let mut shortcuts = available_shortcuts(reserved_shortcuts);
        let rows = applications
            .into_iter()
            .map(|application| {
                let shortcut = shortcuts.next();
                let mut label = application.display_name.clone();
                if application.requires_terminal && !is_environment_editor_label(&label) {
                    label.push_str(" (terminal)");
                }
                if application.is_default {
                    label.push_str(" (default)");
                }
                ApplicationSelectionRow {
                    shortcut,
                    label,
                    application,
                }
            })
            .collect();
        Self {
            title: "Open With".to_string(),
            rows,
            selected: 0,
        }
    }

    pub(crate) fn move_selection(&mut self, delta: isize) {
        if self.rows.is_empty() {
            self.selected = 0;
            return;
        }
        let max = self.rows.len().saturating_sub(1) as isize;
        self.selected = (self.selected as isize + delta).clamp(0, max) as usize;
    }

    pub(crate) fn row_index_for_shortcut(&self, shortcut: char) -> Option<usize> {
        let needle = shortcut.to_ascii_lowercase();
        self.rows.iter().position(|row| {
            row.shortcut
                .is_some_and(|shortcut| shortcut.to_ascii_lowercase() == needle)
        })
    }

    pub(crate) fn application_at(&self, index: usize) -> Option<&OpenWithApplication> {
        self.rows.get(index).map(|row| &row.application)
    }

    #[cfg(test)]
    pub(crate) fn from_test_rows(rows: Vec<(String, String, Vec<String>, bool)>) -> Self {
        Self {
            title: "Open With".to_string(),
            rows: rows
                .into_iter()
                .enumerate()
                .map(
                    |(index, (display_name, program, args, requires_terminal))| {
                        ApplicationSelectionRow {
                            shortcut: char::from_digit((index + 1) as u32, 10),
                            label: display_name.clone(),
                            application: OpenWithApplication {
                                display_name,
                                application_id: None,
                                program,
                                args,
                                is_default: false,
                                requires_terminal,
                            },
                        }
                    },
                )
                .collect(),
            selected: 0,
        }
    }
}

const OPEN_WITH_SHORTCUTS: &str = "123456789abcdefghijklmnopqrstuvwxyz";

fn available_shortcuts(reserved: &[char]) -> impl Iterator<Item = char> + '_ {
    OPEN_WITH_SHORTCUTS
        .chars()
        .filter(move |shortcut| !reserved.contains(shortcut))
}

fn is_environment_editor_label(display_name: &str) -> bool {
    display_name.contains("($VISUAL)") || display_name.contains("($EDITOR)")
}

#[cfg(test)]
#[path = "tests/application_selection.rs"]
mod tests;
