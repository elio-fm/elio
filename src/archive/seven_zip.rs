use super::{
    creation::{
        CreateArchivePlan, CreateArchiveProgress, CreateArchiveSummary, archive_entry_name,
        archive_name, archived_symlink_target, count_archive_items, unique_staging_path,
    },
    extraction::{
        ArchivePassword, ExtractError, ExtractPlan, ExtractProgress, ExtractResult, ExtractSummary,
    },
    path_safety::{
        DeferredSymlink, checked_output_name, extract_deferred_symlinks, safe_relative_link_target,
    },
};
use anyhow::{Context, Result, anyhow};
use sevenz_rust2::{
    ArchiveEntry, ArchiveReader, ArchiveWriter, Error, Password,
    encoder_options::{AesEncoderOptions, Lzma2Options},
};
use std::{
    fs::{self, File},
    io::{self, Cursor, Read},
    path::{Path, PathBuf},
};

const UNIX_ATTR_FLAG: u32 = 0x8000;
const FILE_ATTR: u32 = 0x20;

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
    let file = File::create_new(&staging_path)
        .with_context(|| format!("Could not create {}", staging_path.display()))?;
    let mut writer = ArchiveWriter::new(file).context("Could not start 7z archive")?;
    if let Some(password) = plan.options.encryption.password() {
        writer.set_content_methods(vec![
            AesEncoderOptions::new(Password::new(password.as_str())).into(),
            Lzma2Options::default().into(),
        ]);
    }

    let result = write_archive(&mut writer, &plan.sources, total, &mut progress, cancelled)
        .and_then(|completed| {
            writer.finish().context("Could not finish 7z archive")?;
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
    writer: &mut ArchiveWriter<File>,
    sources: &[PathBuf],
    total: usize,
    progress: &mut F,
    cancelled: C,
) -> Result<usize>
where
    F: FnMut(CreateArchiveProgress),
    C: Fn() -> bool,
{
    let mut state = CreateState {
        completed: 0,
        total,
        progress,
        cancelled,
        root_source: PathBuf::new(),
        root_archive_path: PathBuf::new(),
    };
    let mut sorted_sources = sources.to_vec();
    sorted_sources.sort_by_key(|source| archive_name(source));

    for source in sorted_sources {
        let root_name = archive_name(&source);
        state.root_source = source.clone();
        state.root_archive_path = PathBuf::from(&root_name);
        add_path(writer, &source, Path::new(&root_name), &mut state)?;
    }

    Ok(state.completed)
}

struct CreateState<'a, F, C> {
    completed: usize,
    total: usize,
    progress: &'a mut F,
    cancelled: C,
    root_source: PathBuf,
    root_archive_path: PathBuf,
}

fn add_path<F, C>(
    writer: &mut ArchiveWriter<File>,
    source: &Path,
    archive_path: &Path,
    state: &mut CreateState<'_, F, C>,
) -> Result<()>
where
    F: FnMut(CreateArchiveProgress),
    C: Fn() -> bool,
{
    if (state.cancelled)() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(source)
        .with_context(|| format!("Could not inspect {}", source.display()))?;
    if metadata.file_type().is_symlink() {
        add_symlink(writer, source, archive_path, state)?;
        return Ok(());
    }
    let name = archive_entry_name(archive_path, false)?;
    let entry = ArchiveEntry::from_path(source, name);
    if metadata.is_dir() {
        writer
            .push_archive_entry::<&[u8]>(entry, None)
            .context("Could not write 7z directory")?;
        complete_entry(state);

        let mut children = fs::read_dir(source)
            .with_context(|| format!("Could not read {}", source.display()))?
            .collect::<io::Result<Vec<_>>>()?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let child_path = child.path();
            add_path(
                writer,
                &child_path,
                &archive_path.join(child.file_name()),
                state,
            )?;
        }
    } else {
        let input =
            File::open(source).with_context(|| format!("Could not open {}", source.display()))?;
        writer
            .push_archive_entry(entry, Some(input))
            .context("Could not write 7z entry")?;
        complete_entry(state);
    }
    Ok(())
}

fn add_symlink<F, C>(
    writer: &mut ArchiveWriter<File>,
    source: &Path,
    archive_path: &Path,
    state: &mut CreateState<'_, F, C>,
) -> Result<()>
where
    F: FnMut(CreateArchiveProgress),
    C: Fn() -> bool,
{
    let target = fs::read_link(source)
        .with_context(|| format!("Could not read symlink {}", source.display()))?;
    let target = archived_symlink_target(
        archive_path,
        &state.root_source,
        &state.root_archive_path,
        &target,
    );
    let contents = target.to_string_lossy().into_owned().into_bytes();
    let entry = ArchiveEntry {
        name: archive_entry_name(archive_path, false)?,
        has_stream: true,
        size: contents.len() as u64,
        has_windows_attributes: true,
        windows_attributes: unix_attributes(0o120777),
        ..Default::default()
    };
    writer
        .push_archive_entry(entry, Some(Cursor::new(contents)))
        .context("Could not write 7z symlink")?;
    complete_entry(state);
    Ok(())
}

fn complete_entry<F, C>(state: &mut CreateState<'_, F, C>)
where
    F: FnMut(CreateArchiveProgress),
{
    state.completed += 1;
    (state.progress)(CreateArchiveProgress {
        completed: state.completed,
        total: state.total,
    });
}

fn unix_attributes(mode: u32) -> u32 {
    (mode << 16) | UNIX_ATTR_FLAG | FILE_ATTR
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
    let password_provided = password.is_some();
    let password = password
        .map(ArchivePassword::as_seven_zip_password)
        .unwrap_or_else(Password::empty);
    let mut archive =
        ArchiveReader::new(file, password).map_err(|error| map_error(error, password_provided))?;
    let total = archive.archive().files.len();
    let mut completed = 0usize;
    let mut skipped_links = 0usize;
    let mut symlinks = Vec::new();
    let mut extract_error = None;

    progress(ExtractProgress {
        completed,
        total: Some(total),
    });
    archive
        .for_each_entries(|entry, reader| {
            if cancelled() {
                return Ok(false);
            }
            if let Err(error) =
                extract_entry(plan, entry, reader, &mut symlinks, &mut skipped_links)
            {
                extract_error = Some(error);
                return Ok(false);
            }
            completed += 1;
            progress(ExtractProgress {
                completed,
                total: Some(total),
            });
            Ok(true)
        })
        .map_err(|error| map_error(error, password_provided))?;

    if let Some(error) = extract_error {
        return Err(error);
    }
    skipped_links +=
        extract_deferred_symlinks(&plan.dest_dir, symlinks).map_err(ExtractError::Other)?;
    Ok(ExtractSummary {
        dest_dir: plan.dest_dir.clone(),
        completed,
        total: Some(total),
        skipped_links,
    })
}

fn extract_entry(
    plan: &ExtractPlan,
    entry: &ArchiveEntry,
    reader: &mut dyn Read,
    symlinks: &mut Vec<DeferredSymlink>,
    skipped_links: &mut usize,
) -> ExtractResult<()> {
    if entry.is_anti_item {
        return Ok(());
    }
    let out_path = checked_output_name(&plan.dest_dir, &entry.name)?;
    if entry.is_directory {
        fs::create_dir_all(&out_path)
            .with_context(|| format!("Could not create {}", out_path.display()))?;
    } else if is_symlink(entry) {
        let mut target = String::new();
        reader
            .take(entry.size)
            .read_to_string(&mut target)
            .context("Could not read 7z symlink target")?;
        let target = PathBuf::from(target);
        if safe_relative_link_target(&plan.dest_dir, &out_path, &target) {
            symlinks.push(DeferredSymlink {
                path: out_path,
                target,
            });
        } else {
            *skipped_links += 1;
        }
    } else {
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Could not create {}", parent.display()))?;
        }
        let mut out = File::create(&out_path)
            .with_context(|| format!("Could not create {}", out_path.display()))?;
        let mut limited = reader.take(entry.size);
        io::copy(&mut limited, &mut out)
            .with_context(|| format!("Could not write {}", out_path.display()))?;
    }
    Ok(())
}

fn is_symlink(entry: &ArchiveEntry) -> bool {
    entry.has_windows_attributes && ((entry.windows_attributes >> 16) & 0o170000 == 0o120000)
}

fn map_error(error: Error, password_provided: bool) -> ExtractError {
    match error {
        Error::PasswordRequired => ExtractError::PasswordRequired,
        Error::MaybeBadPassword(_) => ExtractError::BadPassword,
        Error::ChecksumVerificationFailed if password_provided => ExtractError::BadPassword,
        Error::UnsupportedCompressionMethod(method)
            if method.to_ascii_lowercase().contains("aes") =>
        {
            ExtractError::UnsupportedEncryption
        }
        Error::Unsupported(message) if message.to_ascii_lowercase().contains("aes") => {
            ExtractError::UnsupportedEncryption
        }
        error => ExtractError::Other(anyhow!("Could not read 7z archive: {error}")),
    }
}
