use super::extract::ArchivePassword;
use anyhow::{Context, Result, anyhow, bail};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io,
    path::{Component, Path, PathBuf},
};
use zip::{AesMode, CompressionMethod, ZipWriter, write::FileOptions};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CreateArchiveFormat {
    Zip,
}

impl CreateArchiveFormat {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Zip => "ZIP",
        }
    }

    pub(crate) fn supports_encryption(self) -> bool {
        match self {
            Self::Zip => true,
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

    fn password(&self) -> Option<&ArchivePassword> {
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

pub(crate) fn normalize_zip_output_name(input: &str) -> Result<String> {
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

    let lower = trimmed.to_ascii_lowercase();
    if lower.ends_with(".zip") {
        Ok(trimmed.to_string())
    } else if path.extension().is_none() {
        Ok(format!("{trimmed}.zip"))
    } else {
        bail!("Archive creation supports ZIP");
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
    options: CreateArchiveOptions,
) -> Result<CreateArchivePlan> {
    if sources.is_empty() {
        bail!("Select items to archive");
    }
    let output_name = normalize_zip_output_name(output_name)?;
    if options.encryption.is_password_set() && !options.format.supports_encryption() {
        bail!(
            "{} does not support password protection",
            options.format.label()
        );
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

pub(crate) fn create_zip_archive<F, C>(
    plan: &CreateArchivePlan,
    mut progress: F,
    cancelled: C,
) -> Result<CreateArchiveSummary>
where
    F: FnMut(CreateArchiveProgress),
    C: Fn() -> bool,
{
    let total = count_archive_items(&plan.sources)?;
    progress(CreateArchiveProgress {
        completed: 0,
        total,
    });

    let staging_path = unique_staging_path(&plan.output_path)?;
    let file = match File::create_new(&staging_path) {
        Ok(file) => file,
        Err(error) => {
            return Err(
                anyhow!(error).context(format!("Could not create {}", staging_path.display()))
            );
        }
    };

    let result = write_zip_archive(
        file,
        &plan.sources,
        total,
        plan.options.encryption.password(),
        &mut progress,
        cancelled,
    )
    .and_then(|completed| {
        fs::rename(&staging_path, &plan.output_path).with_context(|| {
            format!(
                "Could not move {} to {}",
                staging_path.display(),
                plan.output_path.display()
            )
        })?;
        Ok(CreateArchiveSummary {
            output_path: plan.output_path.clone(),
            completed,
        })
    });

    if result.is_err() {
        let _ = fs::remove_file(&staging_path);
    }

    result
}

fn write_zip_archive<F, C>(
    file: File,
    sources: &[PathBuf],
    total: usize,
    password: Option<&ArchivePassword>,
    progress: &mut F,
    cancelled: C,
) -> Result<usize>
where
    F: FnMut(CreateArchiveProgress),
    C: Fn() -> bool,
{
    let mut writer = ZipWriter::new(file);
    let mut completed = 0usize;
    let settings = ZipCreateSettings { total, password };
    let mut sorted_sources = sources.to_vec();
    sorted_sources.sort_by_key(|source| archive_name(source));

    for source in sorted_sources {
        let root_name = archive_name(&source);
        add_path_to_zip(
            &mut writer,
            &source,
            Path::new(&root_name),
            &mut completed,
            settings,
            progress,
            &cancelled,
        )?;
    }

    writer.finish().context("Could not finish ZIP archive")?;
    Ok(completed)
}

#[derive(Clone, Copy)]
struct ZipCreateSettings<'a> {
    total: usize,
    password: Option<&'a ArchivePassword>,
}

fn add_path_to_zip<F, C>(
    writer: &mut ZipWriter<File>,
    source: &Path,
    archive_path: &Path,
    completed: &mut usize,
    settings: ZipCreateSettings<'_>,
    progress: &mut F,
    cancelled: &C,
) -> Result<()>
where
    F: FnMut(CreateArchiveProgress),
    C: Fn() -> bool,
{
    if cancelled() {
        bail!("Archive creation cancelled");
    }

    let metadata = fs::symlink_metadata(source)
        .with_context(|| format!("Could not inspect {}", source.display()))?;
    if metadata.file_type().is_symlink() {
        add_symlink(writer, source, archive_path, completed, settings, progress)?;
        return Ok(());
    }
    if metadata.is_dir() {
        add_directory(writer, archive_path, &metadata, settings.password)?;
        *completed += 1;
        progress(CreateArchiveProgress {
            completed: *completed,
            total: settings.total,
        });
        let mut children = fs::read_dir(source)
            .with_context(|| format!("Could not read {}", source.display()))?
            .collect::<io::Result<Vec<_>>>()?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let child_name = child.file_name();
            let child_archive_path = archive_path.join(child_name);
            add_path_to_zip(
                writer,
                &child.path(),
                &child_archive_path,
                completed,
                settings,
                progress,
                cancelled,
            )?;
        }
        return Ok(());
    }
    if metadata.is_file() {
        add_file(writer, source, archive_path, &metadata, settings.password)?;
        *completed += 1;
        progress(CreateArchiveProgress {
            completed: *completed,
            total: settings.total,
        });
        return Ok(());
    }

    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("item");
    bail!("Cannot archive {name}");
}

fn add_file(
    writer: &mut ZipWriter<File>,
    source: &Path,
    archive_path: &Path,
    metadata: &fs::Metadata,
    password: Option<&ArchivePassword>,
) -> Result<()> {
    let name = zip_entry_name(archive_path, false)?;
    writer
        .start_file(name, file_options(metadata, password))
        .context("Could not write ZIP entry")?;
    let mut input =
        File::open(source).with_context(|| format!("Could not open {}", source.display()))?;
    io::copy(&mut input, writer).context("Could not write ZIP entry")?;
    Ok(())
}

fn add_directory(
    writer: &mut ZipWriter<File>,
    archive_path: &Path,
    metadata: &fs::Metadata,
    password: Option<&ArchivePassword>,
) -> Result<()> {
    let name = zip_entry_name(archive_path, true)?;
    writer
        .add_directory(name, file_options(metadata, password))
        .context("Could not write ZIP directory")?;
    Ok(())
}

fn add_symlink<F>(
    writer: &mut ZipWriter<File>,
    source: &Path,
    archive_path: &Path,
    completed: &mut usize,
    settings: ZipCreateSettings<'_>,
    progress: &mut F,
) -> Result<()>
where
    F: FnMut(CreateArchiveProgress),
{
    let target = fs::read_link(source)
        .with_context(|| format!("Could not read symlink {}", source.display()))?;
    let name = zip_entry_name(archive_path, false)?;
    let options = apply_zip_encryption(
        FileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .unix_permissions(0o120777),
        settings.password,
    );
    writer
        .start_file(name, options)
        .context("Could not write ZIP symlink")?;
    let target = target.to_string_lossy();
    io::copy(&mut target.as_bytes(), writer).context("Could not write ZIP symlink")?;
    *completed += 1;
    progress(CreateArchiveProgress {
        completed: *completed,
        total: settings.total,
    });
    Ok(())
}

fn file_options<'a>(
    metadata: &fs::Metadata,
    password: Option<&'a ArchivePassword>,
) -> FileOptions<'a, ()> {
    let options = FileOptions::default().compression_method(CompressionMethod::Stored);
    let options = {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            options.unix_permissions(metadata.permissions().mode())
        }
        #[cfg(not(unix))]
        {
            let _ = metadata;
            options
        }
    };
    apply_zip_encryption(options, password)
}

fn apply_zip_encryption<'a>(
    options: FileOptions<'a, ()>,
    password: Option<&'a ArchivePassword>,
) -> FileOptions<'a, ()> {
    match password {
        Some(password) => options.with_aes_encryption(AesMode::Aes256, password.as_str()),
        None => options,
    }
}

fn zip_entry_name(path: &Path, is_dir: bool) -> Result<String> {
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

fn archive_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("item")
        .to_string()
}

fn count_archive_items(sources: &[PathBuf]) -> Result<usize> {
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

fn unique_staging_path(output_path: &Path) -> Result<PathBuf> {
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
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("elio-create-archive-{label}-{unique}"))
    }

    #[test]
    fn normalizes_zip_names() {
        assert_eq!(normalize_zip_output_name("backup").unwrap(), "backup.zip");
        assert_eq!(
            normalize_zip_output_name("backup.zip").unwrap(),
            "backup.zip"
        );
        assert_eq!(
            normalize_zip_output_name("backup.7z")
                .unwrap_err()
                .to_string(),
            "Archive creation supports ZIP"
        );
        assert_eq!(
            normalize_zip_output_name("../backup.zip")
                .unwrap_err()
                .to_string(),
            "Use a filename, not a path"
        );
    }

    #[test]
    fn creates_zip_with_top_level_names() {
        let root = temp_path("top-level");
        fs::create_dir_all(root.join("src/nested")).unwrap();
        fs::write(root.join("src/lib.rs"), "lib").unwrap();
        fs::write(root.join("src/nested/mod.rs"), "mod").unwrap();
        fs::write(root.join("README.md"), "readme").unwrap();

        let plan = plan_create_zip_archive(
            &root,
            vec![root.join("README.md"), root.join("src")],
            "archive.zip",
        )
        .unwrap();
        create_zip_archive(&plan, |_| {}, || false).unwrap();

        let file = File::open(root.join("archive.zip")).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut names = (0..archive.len())
            .map(|index| archive.by_index(index).unwrap().name().to_string())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            vec![
                "README.md",
                "src/",
                "src/lib.rs",
                "src/nested/",
                "src/nested/mod.rs"
            ]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_output_inside_selected_folder() {
        let root = temp_path("self");
        fs::create_dir_all(root.join("src")).unwrap();
        let error =
            plan_create_zip_archive(&root.join("src"), vec![root.join("src")], "archive.zip")
                .unwrap_err();
        assert_eq!(error.to_string(), "Output is inside selected folder");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_duplicate_top_level_names() {
        let root = temp_path("duplicate");
        fs::create_dir_all(root.join("left")).unwrap();
        fs::create_dir_all(root.join("right")).unwrap();
        fs::write(root.join("left/item.txt"), "left").unwrap();
        fs::write(root.join("right/item.txt"), "right").unwrap();
        let error = plan_create_zip_archive(
            &root,
            vec![root.join("left/item.txt"), root.join("right/item.txt")],
            "archive.zip",
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Archive would contain duplicate item.txt"
        );
        let _ = fs::remove_dir_all(root);
    }
}
