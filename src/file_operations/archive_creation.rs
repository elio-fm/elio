use super::FileOperationsState;
use super::archive_extraction::{ArchivePasswordOverlay, ArchivePasswordPurpose};
use crate::archive::{
    ArchiveEncryption, CreateArchiveFormat, CreateArchiveOptions, normalize_archive_output_name,
};
use std::path::PathBuf;

pub(crate) struct ArchiveCreateProgress {
    pub(crate) completed: usize,
    pub(crate) total: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ArchiveCreateRequest {
    pub(crate) token: u64,
    pub(crate) cwd: PathBuf,
    pub(crate) sources: Vec<PathBuf>,
    pub(crate) output_name: String,
    pub(crate) options: CreateArchiveOptions,
}

#[derive(Debug)]
pub(crate) struct ArchiveCreateOverlay {
    pub(crate) sources: Vec<PathBuf>,
    pub(crate) source_names: Vec<String>,
    pub(crate) source_scroll: usize,
    pub(crate) input: String,
    pub(crate) cursor_col: usize,
    pub(crate) options: CreateArchiveOptions,
    pub(crate) error: Option<String>,
}

fn archive_create_default_cursor_col(name: &str) -> usize {
    let base = name.strip_suffix(".zip").unwrap_or(name);
    base.chars().count()
}

fn archive_create_effective_format(overlay: &ArchiveCreateOverlay) -> Option<CreateArchiveFormat> {
    normalize_archive_output_name(&overlay.input)
        .map(|(_, format)| format)
        .ok()
}

fn archive_source_label(path: &std::path::Path) -> String {
    let mut name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("item")
        .to_string();
    if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir())
        && !name.ends_with('/')
    {
        name.push('/');
    }
    name
}

impl FileOperationsState {
    pub(crate) fn open_archive_create_prompt(
        &mut self,
        sources: Vec<PathBuf>,
        default_name: String,
    ) {
        let source_names = sources
            .iter()
            .map(|path| archive_source_label(path))
            .collect();
        self.trash = None;
        self.restore = None;
        self.archive_password = None;
        self.create = None;
        self.rename = None;
        self.bulk_rename = None;
        self.copy = None;
        self.archive_create = Some(ArchiveCreateOverlay {
            sources,
            source_names,
            source_scroll: 0,
            cursor_col: archive_create_default_cursor_col(&default_name),
            input: default_name,
            options: CreateArchiveOptions::default(),
            error: None,
        });
    }

    pub(crate) fn confirm_archive_create(
        &mut self,
        cwd: &std::path::Path,
    ) -> Option<ArchiveCreateRequest> {
        let overlay = self.archive_create.as_ref()?;
        let (output_name, format) = match normalize_archive_output_name(&overlay.input) {
            Ok(normalized) => normalized,
            Err(error) => {
                if let Some(overlay) = &mut self.archive_create {
                    overlay.error = Some(error.to_string());
                }
                return None;
            }
        };
        let sources = overlay.sources.clone();
        let mut options = overlay.options.clone();
        options.format = format;
        if options.encryption.is_password_set() && !options.format.supports_encryption() {
            if let Some(overlay) = &mut self.archive_create {
                overlay.error = None;
            }
            return None;
        }
        if let Err(error) =
            crate::archive::plan_create_archive(cwd, sources.clone(), &output_name, options.clone())
        {
            if let Some(overlay) = &mut self.archive_create {
                overlay.error = Some(error.to_string());
            }
            return None;
        }
        let token = self.archive_create_token.wrapping_add(1);
        self.archive_create_token = token;
        self.archive_create_progress = Some(ArchiveCreateProgress {
            completed: 0,
            total: 0,
        });
        self.archive_create_source_cwd = Some(cwd.to_path_buf());
        self.archive_create_path = Some(cwd.join(&output_name));
        Some(ArchiveCreateRequest {
            token,
            cwd: cwd.to_path_buf(),
            sources,
            output_name,
            options,
        })
    }

    pub(crate) fn accept_archive_create_submission(&mut self) {
        self.archive_create = None;
    }

    pub(crate) fn reject_archive_create_submission(&mut self) {
        self.archive_create_progress = None;
        self.archive_create_source_cwd = None;
        self.archive_create_path = None;
    }

    pub(crate) fn open_archive_create_password_prompt(&mut self) {
        let Some(overlay) = &self.archive_create else {
            return;
        };
        let password_set = overlay.options.encryption.is_password_set();
        let Some(format) = archive_create_effective_format(overlay) else {
            if !password_set {
                self.show_archive_password_format_hint();
            }
            return;
        };
        if !format.supports_encryption() {
            if !password_set {
                self.show_archive_password_format_hint();
            }
            return;
        }
        let input = match &overlay.options.encryption {
            ArchiveEncryption::Password(password) => password.as_str().to_string(),
            ArchiveEncryption::None => String::new(),
        };
        self.archive_password = Some(ArchivePasswordOverlay {
            purpose: ArchivePasswordPurpose::Create,
            cursor_col: input.chars().count(),
            input,
            visible: false,
            error: None,
        });
    }

    fn show_archive_password_format_hint(&mut self) {
        if let Some(overlay) = &mut self.archive_create {
            overlay.error = Some("Use ZIP or 7Z for passwords".to_string());
        }
    }

    pub(crate) fn remove_archive_create_password(&mut self) -> bool {
        if let Some(overlay) = &mut self.archive_create
            && overlay.options.encryption.is_password_set()
        {
            overlay.options.encryption = ArchiveEncryption::None;
            overlay.error = None;
            return true;
        }
        false
    }

    pub fn archive_create_progress(&self) -> Option<(usize, usize)> {
        self.archive_create_progress
            .as_ref()
            .map(|progress| (progress.completed, progress.total))
    }

    pub fn archive_create_is_open(&self) -> bool {
        self.archive_create.is_some()
    }

    pub fn archive_create_input(&self) -> &str {
        self.archive_create
            .as_ref()
            .map_or("", |overlay| overlay.input.as_str())
    }

    pub fn archive_create_cursor_col(&self) -> usize {
        self.archive_create
            .as_ref()
            .map_or(0, |overlay| overlay.cursor_col)
    }

    pub fn archive_create_error(&self) -> Option<&str> {
        self.archive_create
            .as_ref()
            .and_then(|overlay| overlay.error.as_deref())
    }

    pub fn archive_create_protection_label(&self) -> &'static str {
        let Some(overlay) = &self.archive_create else {
            return "";
        };
        if overlay.options.encryption.is_password_set() {
            "Password set"
        } else {
            ""
        }
    }

    pub fn archive_create_protection_hint(&self) -> &'static str {
        let Some(overlay) = &self.archive_create else {
            return "";
        };
        match archive_create_effective_format(overlay) {
            Some(format) if format.supports_encryption() => {
                if overlay.options.encryption.is_password_set() {
                    "Alt+P change  Alt+R remove"
                } else {
                    "Alt+P add password"
                }
            }
            Some(_) | None => {
                if overlay.options.encryption.is_password_set() {
                    "Switch format or remove"
                } else {
                    ""
                }
            }
        }
    }

    pub fn archive_create_source_names(&self) -> &[String] {
        self.archive_create
            .as_ref()
            .map_or(&[], |overlay| overlay.source_names.as_slice())
    }

    pub fn archive_create_title(&self) -> String {
        let Some(overlay) = &self.archive_create else {
            return "Create archive".to_string();
        };
        let files = overlay
            .source_names
            .iter()
            .filter(|name| !name.ends_with('/'))
            .count();
        let dirs = overlay.source_names.len().saturating_sub(files);
        match (files, dirs) {
            (1, 0) => "Create archive from 1 file".to_string(),
            (0, 1) => "Create archive from 1 folder".to_string(),
            (f, 0) => format!("Create archive from {f} files"),
            (0, d) => format!("Create archive from {d} folders"),
            (f, d) => format!(
                "Create archive from {f} file{} and {d} folder{}",
                if f == 1 { "" } else { "s" },
                if d == 1 { "" } else { "s" },
            ),
        }
    }

    pub fn archive_create_source_scroll(&self, visible_rows: usize) -> usize {
        self.archive_create.as_ref().map_or(0, |overlay| {
            overlay
                .source_scroll
                .min(overlay.source_names.len().saturating_sub(visible_rows))
        })
    }
}
