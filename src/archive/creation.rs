use super::extraction::ArchivePassword;
use anyhow::{Context, Result, anyhow, bail};
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

#[cfg(test)]
use super::zip::create as create_zip_archive;
#[cfg(test)]
use sevenz_rust2::Password as SevenZipPassword;
#[cfg(test)]
use std::fs::File;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CreateArchiveFormat {
    Zip,
    SevenZip,
    Tar,
    TarGzip,
    TarXz,
    TarBzip2,
}

impl CreateArchiveFormat {
    pub(crate) fn supports_encryption(self) -> bool {
        match self {
            Self::Zip | Self::SevenZip => true,
            Self::Tar | Self::TarGzip | Self::TarXz | Self::TarBzip2 => false,
        }
    }

    fn detect_from_name(name: &str) -> Option<Self> {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
            Some(Self::TarGzip)
        } else if lower.ends_with(".tar.xz") || lower.ends_with(".txz") {
            Some(Self::TarXz)
        } else if lower.ends_with(".tar.bz2") || lower.ends_with(".tbz2") || lower.ends_with(".tbz")
        {
            Some(Self::TarBzip2)
        } else if lower.ends_with(".tar") {
            Some(Self::Tar)
        } else if lower.ends_with(".zip") {
            Some(Self::Zip)
        } else if lower.ends_with(".7z") {
            Some(Self::SevenZip)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ArchiveEncryption {
    None,
    Password(ArchivePassword),
}

impl ArchiveEncryption {
    pub(crate) fn is_password_set(&self) -> bool {
        matches!(self, Self::Password(_))
    }

    pub(super) fn password(&self) -> Option<&ArchivePassword> {
        match self {
            Self::None => None,
            Self::Password(password) => Some(password),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CreateArchiveOptions {
    pub(crate) format: CreateArchiveFormat,
    pub(crate) encryption: ArchiveEncryption,
}

impl Default for CreateArchiveOptions {
    fn default() -> Self {
        Self {
            format: CreateArchiveFormat::Zip,
            encryption: ArchiveEncryption::None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CreateArchivePlan {
    pub(crate) sources: Vec<PathBuf>,
    pub(crate) output_path: PathBuf,
    pub(crate) options: CreateArchiveOptions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CreateArchiveProgress {
    pub(crate) completed: usize,
    pub(crate) total: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CreateArchiveSummary {
    pub(crate) output_path: PathBuf,
    pub(crate) completed: usize,
}

pub(crate) fn normalize_archive_output_name(input: &str) -> Result<(String, CreateArchiveFormat)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        bail!("Name cannot be empty");
    }
    let path = Path::new(trimmed);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || trimmed.contains('/')
        || trimmed.contains('\\')
    {
        bail!("Use a filename, not a path");
    }

    if let Some(format) = CreateArchiveFormat::detect_from_name(trimmed) {
        Ok((trimmed.to_string(), format))
    } else if path.extension().is_none() {
        Ok((format!("{trimmed}.zip"), CreateArchiveFormat::Zip))
    } else {
        bail!("Supported: ZIP, 7Z, TAR, TAR.GZ, TAR.XZ, and TAR.BZ2");
    }
}

#[cfg(test)]
pub(crate) fn plan_create_zip_archive(
    cwd: &Path,
    sources: Vec<PathBuf>,
    output_name: &str,
) -> Result<CreateArchivePlan> {
    plan_create_archive(cwd, sources, output_name, CreateArchiveOptions::default())
}

pub(crate) fn plan_create_archive(
    cwd: &Path,
    sources: Vec<PathBuf>,
    output_name: &str,
    mut options: CreateArchiveOptions,
) -> Result<CreateArchivePlan> {
    if sources.is_empty() {
        bail!("Select items to archive");
    }
    let (output_name, format) = normalize_archive_output_name(output_name)?;
    options.format = format;
    if options.encryption.is_password_set() && !options.format.supports_encryption() {
        bail!("Password not supported for this format");
    }
    let output_path = cwd.join(output_name);
    if fs::symlink_metadata(&output_path).is_ok() {
        let name = output_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("archive.zip");
        bail!("{name} already exists");
    }

    let mut root_names = BTreeSet::new();
    for source in &sources {
        let root_name = archive_name(source);
        if !root_names.insert(root_name.clone()) {
            bail!("Archive would contain duplicate {root_name}");
        }
        if !source.exists() && fs::symlink_metadata(source).is_err() {
            let name = source
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("item");
            bail!("{name} no longer exists");
        }
        let metadata = fs::symlink_metadata(source)
            .with_context(|| format!("Could not inspect {}", source.display()))?;
        if metadata.is_dir() && output_path.starts_with(source) {
            bail!("Output is inside selected folder");
        }
    }

    Ok(CreateArchivePlan {
        sources,
        output_path,
        options,
    })
}

pub(crate) fn create_archive<F, C>(
    plan: &CreateArchivePlan,
    progress: F,
    cancelled: C,
) -> Result<CreateArchiveSummary>
where
    F: FnMut(CreateArchiveProgress),
    C: Fn() -> bool,
{
    match plan.options.format {
        CreateArchiveFormat::Zip => super::zip::create(plan, progress, cancelled),
        CreateArchiveFormat::SevenZip => super::seven_zip::create(plan, progress, cancelled),
        CreateArchiveFormat::Tar => {
            super::tar::create(plan, super::tar::Compression::None, progress, cancelled)
        }
        CreateArchiveFormat::TarGzip => {
            super::tar::create(plan, super::tar::Compression::Gzip, progress, cancelled)
        }
        CreateArchiveFormat::TarXz => {
            super::tar::create(plan, super::tar::Compression::Xz, progress, cancelled)
        }
        CreateArchiveFormat::TarBzip2 => {
            super::tar::create(plan, super::tar::Compression::Bzip2, progress, cancelled)
        }
    }
}

pub(super) fn archived_symlink_target(
    archive_path: &Path,
    root_source: &Path,
    root_archive_path: &Path,
    target: &Path,
) -> PathBuf {
    if !target.is_absolute() {
        return target.to_path_buf();
    }
    let Ok(internal_target) = target.strip_prefix(root_source) else {
        return target.to_path_buf();
    };
    let archived_target = root_archive_path.join(internal_target);
    let archive_parent = archive_path.parent().unwrap_or_else(|| Path::new(""));
    relative_archive_path(archive_parent, &archived_target).unwrap_or_else(|| target.to_path_buf())
}

fn relative_archive_path(from_dir: &Path, to_path: &Path) -> Option<PathBuf> {
    let from = from_dir.components().collect::<Vec<_>>();
    let to = to_path.components().collect::<Vec<_>>();
    if from
        .iter()
        .any(|component| !matches!(component, Component::Normal(_)))
        || to
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return None;
    }
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = PathBuf::new();
    for _ in common..from.len() {
        relative.push("..");
    }
    for component in &to[common..] {
        relative.push(component.as_os_str());
    }
    if relative.as_os_str().is_empty() {
        Some(PathBuf::from("."))
    } else {
        Some(relative)
    }
}

pub(super) fn archive_entry_name(path: &Path, is_dir: bool) -> Result<String> {
    let mut out = String::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            bail!("Archive entry contains unsafe path");
        };
        if !out.is_empty() {
            out.push('/');
        }
        let part = part
            .to_str()
            .ok_or_else(|| anyhow!("Archive entry name is not valid UTF-8"))?;
        if part.is_empty() || part == "." || part == ".." {
            bail!("Archive entry contains unsafe path");
        }
        out.push_str(part);
    }
    if out.is_empty() {
        bail!("Archive entry name cannot be empty");
    }
    if is_dir && !out.ends_with('/') {
        out.push('/');
    }
    Ok(out)
}

pub(super) fn archive_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("item")
        .to_string()
}

pub(super) fn count_archive_items(sources: &[PathBuf]) -> Result<usize> {
    let mut total = 0usize;
    for source in sources {
        total += count_path_items(source)?;
    }
    Ok(total.max(1))
}

fn count_path_items(path: &Path) -> Result<usize> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("Could not inspect {}", path.display()))?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        let mut total = 1usize;
        for child in
            fs::read_dir(path).with_context(|| format!("Could not read {}", path.display()))?
        {
            total += count_path_items(&child?.path())?;
        }
        Ok(total)
    } else {
        Ok(1)
    }
}

pub(super) fn unique_staging_path(output_path: &Path) -> Result<PathBuf> {
    let parent = output_path
        .parent()
        .ok_or_else(|| anyhow!("Cannot determine archive parent directory"))?;
    let name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("Archive name is not valid UTF-8"))?;
    let pid = std::process::id();
    for attempt in 0u32..1000 {
        let candidate = parent.join(format!(".{name}.elio-creating-{pid}-{attempt}"));
        if fs::symlink_metadata(&candidate).is_err() {
            return Ok(candidate);
        }
    }
    bail!("Could not create unique archive staging file")
}

#[cfg(test)]
#[path = "tests/creation.rs"]
mod tests;
