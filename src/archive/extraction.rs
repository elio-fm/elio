use anyhow::{Context, Result, anyhow, bail};
use sevenz_rust2::Password as SevenZipPassword;
use std::{
    error::Error,
    fmt::{self, Display},
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExtractFormat {
    Zip,
    Tar,
    TarGzip,
    TarXz,
    TarBzip2,
    TarZstd,
    SevenZip,
    Rar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExtractBackend {
    Zip,
    Tar(ExtractFormat),
    SevenZip,
    ExternalSevenZip,
}

impl ExtractFormat {
    pub(crate) const SUPPORTED_MESSAGE: &'static str =
        "Extraction supports ZIP, 7z, RAR, TAR, TAR.GZ, TAR.XZ, TAR.BZ2, and TAR.ZST";

    pub(crate) fn detect(path: &Path) -> Option<Self> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())?
            .to_ascii_lowercase();
        if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            return Some(Self::TarGzip);
        }
        if name.ends_with(".tar.xz") || name.ends_with(".txz") {
            return Some(Self::TarXz);
        }
        if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") || name.ends_with(".tbz") {
            return Some(Self::TarBzip2);
        }
        if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
            return Some(Self::TarZstd);
        }
        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("zip") => Some(Self::Zip),
            Some("7z") => Some(Self::SevenZip),
            Some("rar") => Some(Self::Rar),
            Some("tar") => Some(Self::Tar),
            _ => None,
        }
    }

    pub(crate) fn stem_for_destination(path: &Path) -> Option<String> {
        let name = path.file_name()?.to_string_lossy();
        let lower = name.to_ascii_lowercase();
        let stem = [
            ".tar.bz2", ".tar.zst", ".tar.gz", ".tar.xz", ".tbz2", ".tzst", ".tgz", ".txz", ".tbz",
            ".zip", ".7z", ".rar", ".tar",
        ]
        .iter()
        .find_map(|suffix| {
            lower
                .ends_with(suffix)
                .then(|| &name[..name.len() - suffix.len()])
        })?;
        let trimmed = stem.trim();
        Some(if trimmed.is_empty() {
            "archive".to_string()
        } else {
            trimmed.to_string()
        })
    }

    pub(crate) fn backend(self) -> ExtractBackend {
        match self {
            Self::Zip => ExtractBackend::Zip,
            Self::SevenZip => ExtractBackend::SevenZip,
            Self::Rar => ExtractBackend::ExternalSevenZip,
            Self::Tar | Self::TarGzip | Self::TarXz | Self::TarBzip2 | Self::TarZstd => {
                ExtractBackend::Tar(self)
            }
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Zip => "ZIP",
            Self::Tar => "TAR",
            Self::TarGzip => "TAR.GZ",
            Self::TarXz => "TAR.XZ",
            Self::TarBzip2 => "TAR.BZ2",
            Self::TarZstd => "TAR.ZST",
            Self::SevenZip => "7z",
            Self::Rar => "RAR",
        }
    }
}

fn unique_destination(parent: &Path, stem: &str) -> PathBuf {
    let first = parent.join(stem);
    if fs::symlink_metadata(&first).is_err() {
        return first;
    }
    for index in 1u32.. {
        let candidate = parent.join(format!("{stem}_{index}"));
        if fs::symlink_metadata(&candidate).is_err() {
            return candidate;
        }
    }
    unreachable!("unique destination search should not overflow")
}

#[cfg(all(test, unix))]
use super::external_commands::run_seven_zip_command as run_external_seven_zip_command;
#[cfg(test)]
use super::{
    external_commands::{
        available_seven_zip as available_external_seven_zip,
        parse_seven_zip_entries as parse_external_seven_zip_entries,
        validate_entry_path as validate_external_entry_path,
    },
    path_safety::checked_output_path,
};
#[cfg(test)]
use std::fs::File;
#[cfg(all(test, unix))]
use std::process::Command;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExtractPlan {
    pub(crate) archive_path: PathBuf,
    pub(crate) dest_dir: PathBuf,
    pub(crate) backend: ExtractBackend,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExtractProgress {
    pub(crate) completed: usize,
    pub(crate) total: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExtractSummary {
    pub(crate) dest_dir: PathBuf,
    pub(crate) completed: usize,
    pub(crate) total: Option<usize>,
    pub(crate) skipped_links: usize,
}

#[derive(Clone, Default, Eq, PartialEq)]
pub(crate) struct ArchivePassword(String);

impl ArchivePassword {
    pub(crate) fn new(password: impl Into<String>) -> Self {
        Self(password.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn as_seven_zip_password(&self) -> SevenZipPassword {
        SevenZipPassword::new(&self.0)
    }

    pub(super) fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl fmt::Debug for ArchivePassword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ArchivePassword(<redacted>)")
    }
}

#[derive(Debug)]
pub(crate) enum ExtractError {
    PasswordRequired,
    BadPassword,
    UnsupportedEncryption,
    MissingTool(&'static str),
    UnsafeArchivePath,
    ExternalFailed(&'static str),
    Other(anyhow::Error),
}

impl Display for ExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PasswordRequired => f.write_str("archive requires a password"),
            Self::BadPassword => f.write_str("wrong password"),
            Self::UnsupportedEncryption => f.write_str("unsupported encrypted archive"),
            Self::MissingTool(tool) => write!(f, "install {tool}"),
            Self::UnsafeArchivePath => f.write_str("archive contains unsafe paths"),
            Self::ExternalFailed(tool) => write!(f, "{tool} failed"),
            Self::Other(error) => Display::fmt(error, f),
        }
    }
}

impl Error for ExtractError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Other(error) => error.source(),
            _ => None,
        }
    }
}

impl From<anyhow::Error> for ExtractError {
    fn from(error: anyhow::Error) -> Self {
        Self::Other(error)
    }
}

pub(super) type ExtractResult<T> = std::result::Result<T, ExtractError>;

pub(crate) fn plan_extract(path: &Path) -> Result<ExtractPlan> {
    let format =
        ExtractFormat::detect(path).ok_or_else(|| anyhow!(ExtractFormat::SUPPORTED_MESSAGE))?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Cannot determine archive parent directory"))?;
    let stem = ExtractFormat::stem_for_destination(path)
        .ok_or_else(|| anyhow!("Cannot determine extraction folder name"))?;
    Ok(ExtractPlan {
        archive_path: path.to_path_buf(),
        dest_dir: unique_destination(parent, &stem),
        backend: format.backend(),
    })
}

pub(crate) fn extract_archive_with_password<F, C>(
    plan: &ExtractPlan,
    password: Option<&ArchivePassword>,
    mut progress: F,
    cancelled: C,
) -> ExtractResult<ExtractSummary>
where
    F: FnMut(ExtractProgress),
    C: FnMut() -> bool,
{
    preflight_extract(plan, password)?;

    let staging_dir = staging_destination(&plan.dest_dir)?;
    let extraction_plan = ExtractPlan {
        archive_path: plan.archive_path.clone(),
        dest_dir: staging_dir.clone(),
        backend: plan.backend,
    };
    fs::create_dir(&staging_dir)
        .with_context(|| format!("Could not create {}", staging_dir.display()))?;

    let result = match extraction_plan.backend {
        ExtractBackend::Zip => {
            super::zip::extract(&extraction_plan, password, &mut progress, cancelled)
        }
        ExtractBackend::Tar(format) => {
            super::tar::extract(format, &extraction_plan, &mut progress, cancelled)
        }
        ExtractBackend::SevenZip => {
            super::seven_zip::extract(&extraction_plan, password, &mut progress, cancelled)
        }
        ExtractBackend::ExternalSevenZip => {
            super::external_commands::extract(&extraction_plan, password, &mut progress, cancelled)
        }
    };
    let summary = match result {
        Ok(summary) => summary,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(error);
        }
    };

    let dest_dir = if plan.dest_dir.exists() {
        let parent = plan
            .dest_dir
            .parent()
            .ok_or_else(|| anyhow!("Cannot determine extraction parent"))?;
        let name = plan
            .dest_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("Cannot determine extraction folder name"))?;
        unique_destination(parent, name)
    } else {
        plan.dest_dir.clone()
    };
    fs::rename(&staging_dir, &dest_dir).with_context(|| {
        format!(
            "Could not move {} to {}",
            staging_dir.display(),
            dest_dir.display()
        )
    })?;

    Ok(ExtractSummary {
        dest_dir,
        completed: summary.completed,
        total: summary.total,
        skipped_links: summary.skipped_links,
    })
}

fn preflight_extract(plan: &ExtractPlan, password: Option<&ArchivePassword>) -> ExtractResult<()> {
    match plan.backend {
        ExtractBackend::Zip => super::zip::preflight(&plan.archive_path, password),
        ExtractBackend::SevenZip | ExtractBackend::Tar(_) => Ok(()),
        ExtractBackend::ExternalSevenZip => {
            super::external_commands::preflight(&plan.archive_path, password)
        }
    }
}

fn staging_destination(dest_dir: &Path) -> Result<PathBuf> {
    let parent = dest_dir
        .parent()
        .ok_or_else(|| anyhow!("Cannot determine extraction parent"))?;
    let name = dest_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("Cannot determine extraction folder name"))?;
    for attempt in 0..1000 {
        let candidate = parent.join(format!(
            ".{name}.elio-extracting-{}-{attempt}",
            std::process::id()
        ));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    bail!("Could not reserve extraction workspace")
}

#[cfg(test)]
#[path = "tests/extraction.rs"]
mod tests;
