use super::{
    creation::{
        CreateArchivePlan, CreateArchiveProgress, CreateArchiveSummary, archive_name,
        archived_symlink_target, count_archive_items, unique_staging_path,
    },
    extraction::{ExtractFormat, ExtractPlan, ExtractProgress, ExtractResult, ExtractSummary},
    path_safety::{
        DeferredSymlink, checked_output_path, extract_deferred_symlinks, safe_relative_link_target,
    },
};
use anyhow::{Context, Result, bail};
use bzip2::read::BzDecoder;
use bzip2::{Compression as BzCompression, write::BzEncoder};
use flate2::{Compression as GzCompression, read::GzDecoder, write::GzEncoder};
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};
use xz2::read::XzDecoder;
use zstd::stream::read::Decoder as ZstdDecoder;

pub(super) enum Compression {
    None,
    Gzip,
    Xz,
    Bzip2,
}

pub(super) fn create<F, C>(
    plan: &CreateArchivePlan,
    compression: Compression,
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
    let result = match compression {
        Compression::None => write_archive(file, &plan.sources, total, &mut progress, cancelled)
            .map(|(completed, _)| completed),
        Compression::Gzip => {
            let encoder = GzEncoder::new(file, GzCompression::default());
            let (completed, encoder) =
                write_archive(encoder, &plan.sources, total, &mut progress, cancelled)?;
            encoder
                .finish()
                .context("Could not finish TAR.GZ archive")?;
            Ok(completed)
        }
        Compression::Xz => {
            let encoder = xz2::write::XzEncoder::new(file, 6);
            let (completed, encoder) =
                write_archive(encoder, &plan.sources, total, &mut progress, cancelled)?;
            encoder
                .finish()
                .context("Could not finish TAR.XZ archive")?;
            Ok(completed)
        }
        Compression::Bzip2 => {
            let encoder = BzEncoder::new(file, BzCompression::default());
            let (completed, encoder) =
                write_archive(encoder, &plan.sources, total, &mut progress, cancelled)?;
            encoder
                .finish()
                .context("Could not finish TAR.BZ2 archive")?;
            Ok(completed)
        }
    }
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

fn write_archive<W, F, C>(
    writer: W,
    sources: &[PathBuf],
    total: usize,
    progress: &mut F,
    cancelled: C,
) -> Result<(usize, W)>
where
    W: Write,
    F: FnMut(CreateArchiveProgress),
    C: Fn() -> bool,
{
    let mut builder = tar::Builder::new(writer);
    let mut state = CreateState {
        completed: 0,
        total,
        progress,
        cancelled: &cancelled,
        root_source: PathBuf::new(),
        root_archive_path: PathBuf::new(),
    };
    let mut sorted_sources = sources.to_vec();
    sorted_sources.sort_by_key(|source| archive_name(source));

    for source in sorted_sources {
        let root_name = archive_name(&source);
        state.root_source = source.clone();
        state.root_archive_path = PathBuf::from(&root_name);
        add_path(&mut builder, &source, Path::new(&root_name), &mut state)?;
    }

    builder.finish().context("Could not finish TAR archive")?;
    let completed = state.completed;
    let writer = builder
        .into_inner()
        .context("Could not finish TAR archive")?;
    Ok((completed, writer))
}

struct CreateState<'a, F, C> {
    completed: usize,
    total: usize,
    progress: &'a mut F,
    cancelled: &'a C,
    root_source: PathBuf,
    root_archive_path: PathBuf,
}

fn add_path<W, F, C>(
    builder: &mut tar::Builder<W>,
    source: &Path,
    archive_path: &Path,
    state: &mut CreateState<'_, F, C>,
) -> Result<()>
where
    W: Write,
    F: FnMut(CreateArchiveProgress),
    C: Fn() -> bool,
{
    if (state.cancelled)() {
        bail!("Archive creation cancelled");
    }

    let metadata = fs::symlink_metadata(source)
        .with_context(|| format!("Could not inspect {}", source.display()))?;
    if metadata.file_type().is_symlink() {
        add_symlink(
            builder,
            source,
            archive_path,
            &state.root_source,
            &state.root_archive_path,
        )?;
        state.completed += 1;
        (state.progress)(CreateArchiveProgress {
            completed: state.completed,
            total: state.total,
        });
        return Ok(());
    }
    if metadata.is_dir() {
        builder
            .append_dir(archive_path, source)
            .context("Could not write TAR directory")?;
        state.completed += 1;
        (state.progress)(CreateArchiveProgress {
            completed: state.completed,
            total: state.total,
        });
        let mut children = fs::read_dir(source)
            .with_context(|| format!("Could not read {}", source.display()))?
            .collect::<io::Result<Vec<_>>>()?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let child_name = child.file_name();
            let child_archive_path = archive_path.join(child_name);
            add_path(builder, &child.path(), &child_archive_path, state)?;
        }
        return Ok(());
    }
    if metadata.is_file() {
        let mut file =
            File::open(source).with_context(|| format!("Could not open {}", source.display()))?;
        builder
            .append_file(archive_path, &mut file)
            .context("Could not write TAR entry")?;
        state.completed += 1;
        (state.progress)(CreateArchiveProgress {
            completed: state.completed,
            total: state.total,
        });
        return Ok(());
    }

    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("item");
    bail!("Cannot archive {name}");
}

fn add_symlink<W>(
    builder: &mut tar::Builder<W>,
    source: &Path,
    archive_path: &Path,
    root_source: &Path,
    root_archive_path: &Path,
) -> Result<()>
where
    W: Write,
{
    let target = fs::read_link(source)
        .with_context(|| format!("Could not read symlink {}", source.display()))?;
    let target = archived_symlink_target(archive_path, root_source, root_archive_path, &target);
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_size(0);
    builder
        .append_link(&mut header, archive_path, &target)
        .context("Could not write TAR symlink")?;
    Ok(())
}

pub(super) fn extract<F, C>(
    format: ExtractFormat,
    plan: &ExtractPlan,
    progress: &mut F,
    cancelled: C,
) -> ExtractResult<ExtractSummary>
where
    F: FnMut(ExtractProgress),
    C: FnMut() -> bool,
{
    match format {
        ExtractFormat::Tar => extract_with(format, plan, Ok, progress, cancelled),
        ExtractFormat::TarGzip => extract_with(
            format,
            plan,
            |file| Ok(GzDecoder::new(file)),
            progress,
            cancelled,
        ),
        ExtractFormat::TarXz => extract_with(
            format,
            plan,
            |file| Ok(XzDecoder::new(file)),
            progress,
            cancelled,
        ),
        ExtractFormat::TarBzip2 => extract_with(
            format,
            plan,
            |file| Ok(BzDecoder::new(file)),
            progress,
            cancelled,
        ),
        ExtractFormat::TarZstd => extract_with(format, plan, ZstdDecoder::new, progress, cancelled),
        ExtractFormat::Zip | ExtractFormat::SevenZip | ExtractFormat::Rar => {
            unreachable!("non-TAR archives use their own extraction backends")
        }
    }
}

fn extract_with<R, D, F, C>(
    format: ExtractFormat,
    plan: &ExtractPlan,
    decoder: D,
    progress: &mut F,
    cancelled: C,
) -> ExtractResult<ExtractSummary>
where
    R: io::Read,
    D: Fn(File) -> Result<R, std::io::Error>,
    F: FnMut(ExtractProgress),
    C: FnMut() -> bool,
{
    let count_file = File::open(&plan.archive_path)
        .with_context(|| format!("Could not open {}", plan.archive_path.display()))?;
    let total = count_entries(decoder(count_file).context("Could not initialize TAR decoder")?)
        .with_context(|| format!("Could not read {} archive", format.label()))?;
    let file = File::open(&plan.archive_path)
        .with_context(|| format!("Could not open {}", plan.archive_path.display()))?;
    extract_reader(
        plan,
        decoder(file).context("Could not initialize TAR decoder")?,
        Some(total),
        progress,
        cancelled,
    )
}

fn count_entries<R: io::Read>(reader: R) -> Result<usize> {
    let mut archive = tar::Archive::new(reader);
    let mut total = 0usize;
    for entry in archive.entries()? {
        entry?;
        total += 1;
    }
    Ok(total)
}

fn extract_reader<R, F, C>(
    plan: &ExtractPlan,
    reader: R,
    total: Option<usize>,
    progress: &mut F,
    mut cancelled: C,
) -> ExtractResult<ExtractSummary>
where
    R: io::Read,
    F: FnMut(ExtractProgress),
    C: FnMut() -> bool,
{
    let mut archive = tar::Archive::new(reader);
    let mut completed = 0usize;
    let mut skipped_links = 0usize;
    let mut symlinks = Vec::new();
    progress(ExtractProgress { completed, total });
    for entry in archive.entries().context("Could not read TAR entries")? {
        if cancelled() {
            break;
        }
        let mut entry = entry.context("Could not read TAR entry")?;
        let entry_type = entry.header().entry_type();
        let path = entry.path().context("Could not read TAR entry path")?;
        let out_path = checked_output_path(&plan.dest_dir, path.as_ref())?;
        if entry_type.is_symlink() {
            match entry
                .link_name()
                .context("Could not read TAR symlink target")?
            {
                Some(target)
                    if safe_relative_link_target(&plan.dest_dir, &out_path, target.as_ref()) =>
                {
                    symlinks.push(DeferredSymlink {
                        path: out_path,
                        target: target.into_owned(),
                    });
                }
                _ => skipped_links += 1,
            }
        } else if entry_type.is_hard_link() {
            skipped_links += 1;
        } else if entry_type.is_dir() {
            fs::create_dir_all(&out_path)
                .with_context(|| format!("Could not create {}", out_path.display()))?;
        } else if entry_type.is_file() {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Could not create {}", parent.display()))?;
            }
            entry
                .unpack(&out_path)
                .with_context(|| format!("Could not extract {}", out_path.display()))?;
        }
        completed += 1;
        progress(ExtractProgress { completed, total });
    }
    skipped_links += extract_deferred_symlinks(&plan.dest_dir, symlinks)?;
    Ok(ExtractSummary {
        dest_dir: plan.dest_dir.clone(),
        completed,
        total,
        skipped_links,
    })
}
