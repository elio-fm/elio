use super::{
    creation::{
        CreateArchivePlan, CreateArchiveProgress, CreateArchiveSummary, archive_entry_name,
        archive_name, archived_symlink_target, count_archive_items, unique_staging_path,
    },
    extraction::{
        ArchivePassword, ExtractError, ExtractPlan, ExtractProgress, ExtractResult, ExtractSummary,
    },
    path_safety::{
        DeferredSymlink, checked_output_path, extract_deferred_symlinks, safe_relative_link_target,
    },
};
use anyhow::{Context, Result, anyhow, bail};
use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};
use zip::{AesMode, CompressionMethod, ZipWriter, result::ZipError, write::FileOptions};

pub(super) fn create<F, C>(
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

    let result = write_archive(
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

fn write_archive<F, C>(
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
    let settings = CreateSettings { total, password };
    let mut sorted_sources = sources.to_vec();
    sorted_sources.sort_by_key(|source| archive_name(source));

    for source in sorted_sources {
        let root_name = archive_name(&source);
        add_path(
            &mut writer,
            EntryPaths {
                source: &source,
                archive_path: Path::new(&root_name),
                root_source: &source,
                root_archive_path: Path::new(&root_name),
            },
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
struct CreateSettings<'a> {
    total: usize,
    password: Option<&'a ArchivePassword>,
}

#[derive(Clone, Copy)]
struct EntryPaths<'a> {
    source: &'a Path,
    archive_path: &'a Path,
    root_source: &'a Path,
    root_archive_path: &'a Path,
}

fn add_path<F, C>(
    writer: &mut ZipWriter<File>,
    paths: EntryPaths<'_>,
    completed: &mut usize,
    settings: CreateSettings<'_>,
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

    let metadata = fs::symlink_metadata(paths.source)
        .with_context(|| format!("Could not inspect {}", paths.source.display()))?;
    if metadata.file_type().is_symlink() {
        add_symlink(writer, paths, completed, settings, progress)?;
        return Ok(());
    }
    if metadata.is_dir() {
        add_directory(writer, paths.archive_path, &metadata, settings.password)?;
        *completed += 1;
        progress(CreateArchiveProgress {
            completed: *completed,
            total: settings.total,
        });
        let mut children = fs::read_dir(paths.source)
            .with_context(|| format!("Could not read {}", paths.source.display()))?
            .collect::<io::Result<Vec<_>>>()?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let child_path = child.path();
            let child_name = child.file_name();
            let child_archive_path = paths.archive_path.join(child_name);
            add_path(
                writer,
                EntryPaths {
                    source: &child_path,
                    archive_path: &child_archive_path,
                    root_source: paths.root_source,
                    root_archive_path: paths.root_archive_path,
                },
                completed,
                settings,
                progress,
                cancelled,
            )?;
        }
        return Ok(());
    }
    if metadata.is_file() {
        add_file(
            writer,
            paths.source,
            paths.archive_path,
            &metadata,
            settings.password,
        )?;
        *completed += 1;
        progress(CreateArchiveProgress {
            completed: *completed,
            total: settings.total,
        });
        return Ok(());
    }

    let name = paths
        .source
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
    let name = archive_entry_name(archive_path, false)?;
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
    let name = archive_entry_name(archive_path, true)?;
    writer
        .add_directory(name, file_options(metadata, password))
        .context("Could not write ZIP directory")?;
    Ok(())
}

fn add_symlink<F>(
    writer: &mut ZipWriter<File>,
    paths: EntryPaths<'_>,
    completed: &mut usize,
    settings: CreateSettings<'_>,
    progress: &mut F,
) -> Result<()>
where
    F: FnMut(CreateArchiveProgress),
{
    let target = fs::read_link(paths.source)
        .with_context(|| format!("Could not read symlink {}", paths.source.display()))?;
    let target = archived_symlink_target(
        paths.archive_path,
        paths.root_source,
        paths.root_archive_path,
        &target,
    );
    let name = archive_entry_name(paths.archive_path, false)?;
    let options = apply_encryption(
        FileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .unix_permissions(0o120777),
        settings.password,
    );
    writer
        .add_symlink(name, target.to_string_lossy(), options)
        .context("Could not write ZIP symlink")?;
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
    apply_encryption(options, password)
}

fn apply_encryption<'a>(
    options: FileOptions<'a, ()>,
    password: Option<&'a ArchivePassword>,
) -> FileOptions<'a, ()> {
    match password {
        Some(password) => options.with_aes_encryption(AesMode::Aes256, password.as_str()),
        None => options,
    }
}

pub(super) fn preflight(
    archive_path: &Path,
    password: Option<&ArchivePassword>,
) -> ExtractResult<()> {
    let file = File::open(archive_path)
        .with_context(|| format!("Could not open {}", archive_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("Could not read ZIP archive")?;
    let total = archive.len();
    for index in 0..total {
        let _entry = entry_by_index(&mut archive, index, password)?;
    }
    Ok(())
}

pub(super) fn extract<F, C>(
    plan: &ExtractPlan,
    password: Option<&ArchivePassword>,
    progress: &mut F,
    mut cancelled: C,
) -> ExtractResult<ExtractSummary>
where
    F: FnMut(ExtractProgress),
    C: FnMut() -> bool,
{
    let file = File::open(&plan.archive_path)
        .with_context(|| format!("Could not open {}", plan.archive_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("Could not read ZIP archive")?;
    let total = archive.len();
    let mut completed = 0usize;
    let mut skipped_links = 0usize;
    let mut symlinks = Vec::new();
    progress(ExtractProgress {
        completed,
        total: Some(total),
    });

    for index in 0..total {
        if cancelled() {
            break;
        }
        let mut entry = entry_by_index(&mut archive, index, password)?;
        let encrypted = entry.encrypted();
        let Some(enclosed) = entry.enclosed_name() else {
            return Err(anyhow!("Archive entry escapes the destination: {}", entry.name()).into());
        };
        let out_path = checked_output_path(&plan.dest_dir, enclosed.as_ref())?;
        if entry.is_dir() {
            fs::create_dir_all(&out_path)
                .with_context(|| format!("Could not create {}", out_path.display()))?;
        } else if is_symlink(&entry) {
            let mut target = String::new();
            if let Err(error) = io::Read::read_to_string(&mut entry, &mut target) {
                if password.is_some() && encrypted && is_bad_password_io(&error) {
                    return Err(ExtractError::BadPassword);
                }
                return Err(error).context("Could not read ZIP symlink target")?;
            }
            let target = PathBuf::from(target);
            if safe_relative_link_target(&plan.dest_dir, &out_path, &target) {
                symlinks.push(DeferredSymlink {
                    path: out_path,
                    target,
                });
            } else {
                skipped_links += 1;
            }
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Could not create {}", parent.display()))?;
            }
            let mut out = File::create(&out_path)
                .with_context(|| format!("Could not create {}", out_path.display()))?;
            if let Err(error) = io::copy(&mut entry, &mut out) {
                if password.is_some() && encrypted && is_bad_password_io(&error) {
                    return Err(ExtractError::BadPassword);
                }
                return Err(error)
                    .with_context(|| format!("Could not write {}", out_path.display()))?;
            }
            #[cfg(unix)]
            if let Some(mode) = entry.unix_mode() {
                use std::os::unix::fs::PermissionsExt;
                let safe_mode = mode & 0o777;
                let _ = fs::set_permissions(&out_path, fs::Permissions::from_mode(safe_mode));
            }
        }
        completed += 1;
        progress(ExtractProgress {
            completed,
            total: Some(total),
        });
    }
    skipped_links += extract_deferred_symlinks(&plan.dest_dir, symlinks)?;

    Ok(ExtractSummary {
        dest_dir: plan.dest_dir.clone(),
        completed,
        total: Some(total),
        skipped_links,
    })
}

fn is_symlink<R: io::Read>(entry: &zip::read::ZipFile<'_, R>) -> bool {
    entry
        .unix_mode()
        .is_some_and(|mode| mode & 0o170000 == 0o120000)
}

fn entry_by_index<'a, R: io::Read + io::Seek>(
    archive: &'a mut zip::ZipArchive<R>,
    index: usize,
    password: Option<&ArchivePassword>,
) -> ExtractResult<zip::read::ZipFile<'a, R>> {
    let entry = match password {
        Some(password) => archive.by_index_decrypt(index, password.as_bytes()),
        None => archive.by_index(index),
    };
    entry.map_err(map_error)
}

fn map_error(error: ZipError) -> ExtractError {
    match error {
        ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED) => ExtractError::PasswordRequired,
        ZipError::InvalidPassword => ExtractError::BadPassword,
        ZipError::UnsupportedArchive(message) if is_unsupported_encryption(message) => {
            ExtractError::UnsupportedEncryption
        }
        error => ExtractError::Other(anyhow!(error).context("Could not read ZIP entry")),
    }
}

fn is_unsupported_encryption(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("encrypt") || message.contains("decrypt") || message.contains("aes")
}

fn is_bad_password_io(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::InvalidData && error.to_string().contains("Invalid checksum")
}
