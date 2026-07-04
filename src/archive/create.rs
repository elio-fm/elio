use super::extract::ArchivePassword;
use anyhow::{Context, Result, anyhow, bail};
use bzip2::{Compression as BzCompression, write::BzEncoder};
use flate2::{Compression as GzCompression, write::GzEncoder};
use sevenz_rust2::{
    ArchiveEntry as SevenZipEntry, ArchiveWriter, Password as SevenZipPassword,
    encoder_options::{AesEncoderOptions, Lzma2Options},
};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{self, Cursor, Write},
    path::{Component, Path, PathBuf},
};
use zip::{AesMode, CompressionMethod, ZipWriter, write::FileOptions};

const SEVEN_ZIP_UNIX_ATTR_FLAG: u32 = 0x8000;
const SEVEN_ZIP_FILE_ATTR: u32 = 0x20;

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
        CreateArchiveFormat::Zip => create_zip_archive(plan, progress, cancelled),
        CreateArchiveFormat::SevenZip => create_seven_zip_archive(plan, progress, cancelled),
        CreateArchiveFormat::Tar => create_tar_archive(plan, progress, cancelled),
        CreateArchiveFormat::TarGzip => create_tar_gzip_archive(plan, progress, cancelled),
        CreateArchiveFormat::TarXz => create_tar_xz_archive(plan, progress, cancelled),
        CreateArchiveFormat::TarBzip2 => create_tar_bzip2_archive(plan, progress, cancelled),
    }
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
            ZipEntryPaths {
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

fn create_seven_zip_archive<F, C>(
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
            AesEncoderOptions::new(SevenZipPassword::new(password.as_str())).into(),
            Lzma2Options::default().into(),
        ]);
    }

    let result =
        write_seven_zip_archive(&mut writer, &plan.sources, total, &mut progress, cancelled)
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

fn write_seven_zip_archive<F, C>(
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
    let mut state = SevenZipCreateState {
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
        add_path_to_seven_zip(writer, &source, Path::new(&root_name), &mut state)?;
    }

    Ok(state.completed)
}

struct SevenZipCreateState<'a, F, C> {
    completed: usize,
    total: usize,
    progress: &'a mut F,
    cancelled: C,
    root_source: PathBuf,
    root_archive_path: PathBuf,
}

fn add_path_to_seven_zip<F, C>(
    writer: &mut ArchiveWriter<File>,
    source: &Path,
    archive_path: &Path,
    state: &mut SevenZipCreateState<'_, F, C>,
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
        add_symlink_to_seven_zip(writer, source, archive_path, state)?;
        return Ok(());
    }
    let name = seven_zip_entry_name(archive_path)?;
    let entry = SevenZipEntry::from_path(source, name);
    if metadata.is_dir() {
        writer
            .push_archive_entry::<&[u8]>(entry, None)
            .context("Could not write 7z directory")?;
        complete_seven_zip_entry(state);

        let mut children = fs::read_dir(source)
            .with_context(|| format!("Could not read {}", source.display()))?
            .collect::<io::Result<Vec<_>>>()?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let child_path = child.path();
            add_path_to_seven_zip(
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
        complete_seven_zip_entry(state);
    }
    Ok(())
}

fn add_symlink_to_seven_zip<F, C>(
    writer: &mut ArchiveWriter<File>,
    source: &Path,
    archive_path: &Path,
    state: &mut SevenZipCreateState<'_, F, C>,
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
    let entry = SevenZipEntry {
        name: seven_zip_entry_name(archive_path)?,
        has_stream: true,
        size: contents.len() as u64,
        has_windows_attributes: true,
        windows_attributes: seven_zip_unix_attributes(0o120777),
        ..Default::default()
    };
    writer
        .push_archive_entry(entry, Some(Cursor::new(contents)))
        .context("Could not write 7z symlink")?;
    complete_seven_zip_entry(state);
    Ok(())
}

fn complete_seven_zip_entry<F, C>(state: &mut SevenZipCreateState<'_, F, C>)
where
    F: FnMut(CreateArchiveProgress),
{
    state.completed += 1;
    (state.progress)(CreateArchiveProgress {
        completed: state.completed,
        total: state.total,
    });
}

fn seven_zip_unix_attributes(mode: u32) -> u32 {
    (mode << 16) | SEVEN_ZIP_UNIX_ATTR_FLAG | SEVEN_ZIP_FILE_ATTR
}

fn seven_zip_entry_name(path: &Path) -> Result<String> {
    zip_entry_name(path, false)
}

fn create_tar_archive<F, C>(
    plan: &CreateArchivePlan,
    progress: F,
    cancelled: C,
) -> Result<CreateArchiveSummary>
where
    F: FnMut(CreateArchiveProgress),
    C: Fn() -> bool,
{
    create_tar_with(plan, TarSink::Plain, progress, cancelled)
}

fn create_tar_gzip_archive<F, C>(
    plan: &CreateArchivePlan,
    progress: F,
    cancelled: C,
) -> Result<CreateArchiveSummary>
where
    F: FnMut(CreateArchiveProgress),
    C: Fn() -> bool,
{
    create_tar_with(plan, TarSink::Gzip, progress, cancelled)
}

fn create_tar_xz_archive<F, C>(
    plan: &CreateArchivePlan,
    progress: F,
    cancelled: C,
) -> Result<CreateArchiveSummary>
where
    F: FnMut(CreateArchiveProgress),
    C: Fn() -> bool,
{
    create_tar_with(plan, TarSink::Xz, progress, cancelled)
}

fn create_tar_bzip2_archive<F, C>(
    plan: &CreateArchivePlan,
    progress: F,
    cancelled: C,
) -> Result<CreateArchiveSummary>
where
    F: FnMut(CreateArchiveProgress),
    C: Fn() -> bool,
{
    create_tar_with(plan, TarSink::Bzip2, progress, cancelled)
}

enum TarSink {
    Plain,
    Gzip,
    Xz,
    Bzip2,
}

fn create_tar_with<F, C>(
    plan: &CreateArchivePlan,
    sink: TarSink,
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
    let result = match sink {
        TarSink::Plain => write_tar_archive(file, &plan.sources, total, &mut progress, cancelled)
            .map(|(completed, _)| completed),
        TarSink::Gzip => {
            let encoder = GzEncoder::new(file, GzCompression::default());
            let (completed, encoder) =
                write_tar_archive(encoder, &plan.sources, total, &mut progress, cancelled)?;
            encoder
                .finish()
                .context("Could not finish TAR.GZ archive")?;
            Ok(completed)
        }
        TarSink::Xz => {
            let encoder = xz2::write::XzEncoder::new(file, 6);
            let (completed, encoder) =
                write_tar_archive(encoder, &plan.sources, total, &mut progress, cancelled)?;
            encoder
                .finish()
                .context("Could not finish TAR.XZ archive")?;
            Ok(completed)
        }
        TarSink::Bzip2 => {
            let encoder = BzEncoder::new(file, BzCompression::default());
            let (completed, encoder) =
                write_tar_archive(encoder, &plan.sources, total, &mut progress, cancelled)?;
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

fn write_tar_archive<W, F, C>(
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
    let mut state = TarCreateState {
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
        add_path_to_tar(&mut builder, &source, Path::new(&root_name), &mut state)?;
    }

    builder.finish().context("Could not finish TAR archive")?;
    let completed = state.completed;
    let writer = builder
        .into_inner()
        .context("Could not finish TAR archive")?;
    Ok((completed, writer))
}

struct TarCreateState<'a, F, C> {
    completed: usize,
    total: usize,
    progress: &'a mut F,
    cancelled: &'a C,
    root_source: PathBuf,
    root_archive_path: PathBuf,
}

fn add_path_to_tar<W, F, C>(
    builder: &mut tar::Builder<W>,
    source: &Path,
    archive_path: &Path,
    state: &mut TarCreateState<'_, F, C>,
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
        add_symlink_to_tar(
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
            add_path_to_tar(builder, &child.path(), &child_archive_path, state)?;
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

fn add_symlink_to_tar<W>(
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

fn archived_symlink_target(
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

#[derive(Clone, Copy)]
struct ZipCreateSettings<'a> {
    total: usize,
    password: Option<&'a ArchivePassword>,
}

#[derive(Clone, Copy)]
struct ZipEntryPaths<'a> {
    source: &'a Path,
    archive_path: &'a Path,
    root_source: &'a Path,
    root_archive_path: &'a Path,
}

fn add_path_to_zip<F, C>(
    writer: &mut ZipWriter<File>,
    paths: ZipEntryPaths<'_>,
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
            add_path_to_zip(
                writer,
                ZipEntryPaths {
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
    paths: ZipEntryPaths<'_>,
    completed: &mut usize,
    settings: ZipCreateSettings<'_>,
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
    let name = zip_entry_name(paths.archive_path, false)?;
    let options = apply_zip_encryption(
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
        assert_eq!(
            normalize_archive_output_name("backup").unwrap(),
            ("backup.zip".to_string(), CreateArchiveFormat::Zip)
        );
        assert_eq!(
            normalize_archive_output_name("backup.zip").unwrap(),
            ("backup.zip".to_string(), CreateArchiveFormat::Zip)
        );
        assert_eq!(
            normalize_archive_output_name("backup.tar").unwrap(),
            ("backup.tar".to_string(), CreateArchiveFormat::Tar)
        );
        assert_eq!(
            normalize_archive_output_name("backup.tar.gz").unwrap(),
            ("backup.tar.gz".to_string(), CreateArchiveFormat::TarGzip)
        );
        assert_eq!(
            normalize_archive_output_name("backup.tgz").unwrap(),
            ("backup.tgz".to_string(), CreateArchiveFormat::TarGzip)
        );
        assert_eq!(
            normalize_archive_output_name("backup.tar.xz").unwrap(),
            ("backup.tar.xz".to_string(), CreateArchiveFormat::TarXz)
        );
        assert_eq!(
            normalize_archive_output_name("backup.txz").unwrap(),
            ("backup.txz".to_string(), CreateArchiveFormat::TarXz)
        );
        assert_eq!(
            normalize_archive_output_name("backup.tar.bz2").unwrap(),
            ("backup.tar.bz2".to_string(), CreateArchiveFormat::TarBzip2)
        );
        assert_eq!(
            normalize_archive_output_name("backup.tbz2").unwrap(),
            ("backup.tbz2".to_string(), CreateArchiveFormat::TarBzip2)
        );
        assert_eq!(
            normalize_archive_output_name("backup.tbz").unwrap(),
            ("backup.tbz".to_string(), CreateArchiveFormat::TarBzip2)
        );
        assert_eq!(
            normalize_archive_output_name("backup.7z").unwrap(),
            ("backup.7z".to_string(), CreateArchiveFormat::SevenZip)
        );
        assert_eq!(
            normalize_archive_output_name("backup.z")
                .unwrap_err()
                .to_string(),
            "Supported: ZIP, 7Z, TAR, TAR.GZ, TAR.XZ, and TAR.BZ2"
        );
        assert_eq!(
            normalize_archive_output_name("../backup.zip")
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
    fn creates_seven_zip_with_top_level_names() {
        let root = temp_path("seven-zip-top-level");
        fs::create_dir_all(root.join("src/nested")).unwrap();
        fs::write(root.join("src/lib.rs"), "lib").unwrap();
        fs::write(root.join("src/nested/mod.rs"), "mod").unwrap();
        fs::write(root.join("README.md"), "readme").unwrap();

        let plan = plan_create_archive(
            &root,
            vec![root.join("README.md"), root.join("src")],
            "archive.7z",
            CreateArchiveOptions::default(),
        )
        .unwrap();
        create_archive(&plan, |_| {}, || false).unwrap();

        let file = File::open(root.join("archive.7z")).unwrap();
        let mut archive =
            sevenz_rust2::ArchiveReader::new(file, SevenZipPassword::empty()).unwrap();
        let mut names = Vec::new();
        archive
            .for_each_entries(|entry, _reader| {
                names.push(entry.name().to_string());
                Ok(true)
            })
            .unwrap();
        names.sort();
        assert_eq!(
            names,
            vec![
                "README.md",
                "src",
                "src/lib.rs",
                "src/nested",
                "src/nested/mod.rs"
            ]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn creates_encrypted_seven_zip() {
        let root = temp_path("seven-zip-password");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("secret.txt"), "secret").unwrap();
        let password = root.display().to_string();
        let options = CreateArchiveOptions {
            format: CreateArchiveFormat::SevenZip,
            encryption: ArchiveEncryption::Password(ArchivePassword::new(&password)),
        };
        let plan = plan_create_archive(&root, vec![root.join("secret.txt")], "secret.7z", options)
            .unwrap();
        create_archive(&plan, |_| {}, || false).unwrap();

        let file = File::open(root.join("secret.7z")).unwrap();
        let mut archive =
            sevenz_rust2::ArchiveReader::new(file, SevenZipPassword::new(&password)).unwrap();
        let mut contents = String::new();
        archive
            .for_each_entries(|entry, reader| {
                assert_eq!(entry.name(), "secret.txt");
                reader.read_to_string(&mut contents).unwrap();
                Ok(true)
            })
            .unwrap();
        assert_eq!(contents, "secret");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn creates_tar_with_top_level_names() {
        let root = temp_path("tar-top-level");
        fs::create_dir_all(root.join("src/nested")).unwrap();
        fs::write(root.join("src/lib.rs"), "lib").unwrap();
        fs::write(root.join("src/nested/mod.rs"), "mod").unwrap();
        fs::write(root.join("README.md"), "readme").unwrap();

        let plan = plan_create_archive(
            &root,
            vec![root.join("README.md"), root.join("src")],
            "archive.tar",
            CreateArchiveOptions::default(),
        )
        .unwrap();
        create_archive(&plan, |_| {}, || false).unwrap();

        let file = File::open(root.join("archive.tar")).unwrap();
        let mut archive = tar::Archive::new(file);
        let mut names = archive
            .entries()
            .unwrap()
            .map(|entry| entry.unwrap().path().unwrap().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            vec![
                "README.md",
                "src",
                "src/lib.rs",
                "src/nested",
                "src/nested/mod.rs"
            ]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg(unix)]
    fn creates_tar_with_symlink_entries() {
        use std::os::unix::fs::symlink;

        let root = temp_path("tar-symlink");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("target.txt"), "target").unwrap();
        symlink("target.txt", root.join("link.txt")).unwrap();
        symlink("missing.txt", root.join("broken.txt")).unwrap();

        let plan = plan_create_archive(
            &root,
            vec![root.join("link.txt"), root.join("broken.txt")],
            "archive.tar",
            CreateArchiveOptions::default(),
        )
        .unwrap();
        create_archive(&plan, |_| {}, || false).unwrap();

        let file = File::open(root.join("archive.tar")).unwrap();
        let mut archive = tar::Archive::new(file);
        let mut entries = archive
            .entries()
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (
                    entry.path().unwrap().to_string_lossy().to_string(),
                    entry.header().entry_type(),
                    entry
                        .link_name()
                        .unwrap()
                        .unwrap()
                        .to_string_lossy()
                        .to_string(),
                )
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(
            entries,
            vec![
                (
                    "broken.txt".to_string(),
                    tar::EntryType::Symlink,
                    "missing.txt".to_string()
                ),
                (
                    "link.txt".to_string(),
                    tar::EntryType::Symlink,
                    "target.txt".to_string()
                )
            ]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg(unix)]
    fn creates_tar_with_portable_internal_absolute_symlink() {
        use std::os::unix::fs::symlink;

        let root = temp_path("tar-absolute-internal-symlink");
        let folder = root.join("folder");
        fs::create_dir_all(folder.join("nested")).unwrap();
        fs::write(folder.join("target.txt"), "target").unwrap();
        symlink(folder.join("target.txt"), folder.join("nested/link.txt")).unwrap();

        let plan = plan_create_archive(
            &root,
            vec![folder.clone()],
            "archive.tar",
            CreateArchiveOptions::default(),
        )
        .unwrap();
        create_archive(&plan, |_| {}, || false).unwrap();

        let file = File::open(root.join("archive.tar")).unwrap();
        let mut archive = tar::Archive::new(file);
        let link_target = archive
            .entries()
            .unwrap()
            .map(|entry| entry.unwrap())
            .find_map(|entry| {
                (entry.path().unwrap().as_ref() == Path::new("folder/nested/link.txt"))
                    .then(|| entry.link_name().unwrap().unwrap().into_owned())
            })
            .unwrap();
        assert_eq!(link_target, Path::new("../target.txt"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg(unix)]
    fn creates_zip_with_portable_internal_absolute_symlink() {
        use std::io::Read;
        use std::os::unix::fs::symlink;

        let root = temp_path("zip-absolute-internal-symlink");
        let folder = root.join("folder");
        fs::create_dir_all(folder.join("nested")).unwrap();
        fs::write(folder.join("target.txt"), "target").unwrap();
        symlink(folder.join("target.txt"), folder.join("nested/link.txt")).unwrap();

        let plan = plan_create_zip_archive(&root, vec![folder], "archive.zip").unwrap();
        create_zip_archive(&plan, |_| {}, || false).unwrap();

        let file = File::open(root.join("archive.zip")).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut entry = archive.by_name("folder/nested/link.txt").unwrap();
        assert_eq!(entry.unix_mode().unwrap() & 0o170000, 0o120000);
        let mut target = String::new();
        entry.read_to_string(&mut target).unwrap();
        assert_eq!(target, "../target.txt");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg(unix)]
    fn creates_seven_zip_with_portable_internal_absolute_symlink() {
        use std::os::unix::fs::symlink;

        let root = temp_path("seven-zip-absolute-internal-symlink");
        let folder = root.join("folder");
        fs::create_dir_all(folder.join("nested")).unwrap();
        fs::write(folder.join("target.txt"), "target").unwrap();
        symlink(folder.join("target.txt"), folder.join("nested/link.txt")).unwrap();

        let plan = plan_create_archive(
            &root,
            vec![folder],
            "archive.7z",
            CreateArchiveOptions::default(),
        )
        .unwrap();
        create_archive(&plan, |_| {}, || false).unwrap();

        let file = File::open(root.join("archive.7z")).unwrap();
        let mut archive =
            sevenz_rust2::ArchiveReader::new(file, SevenZipPassword::empty()).unwrap();
        let mut target = String::new();
        let mut attrs = 0;
        archive
            .for_each_entries(|entry, reader| {
                if entry.name() == "folder/nested/link.txt" {
                    attrs = entry.windows_attributes;
                    reader.read_to_string(&mut target).unwrap();
                }
                Ok(true)
            })
            .unwrap();
        assert_eq!((attrs >> 16) & 0o170000, 0o120000);
        assert_eq!(target, "../target.txt");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn creates_tar_gzip_archive() {
        let root = temp_path("tar-gzip");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("README.md"), "readme").unwrap();

        let plan = plan_create_archive(
            &root,
            vec![root.join("README.md")],
            "archive.tgz",
            CreateArchiveOptions::default(),
        )
        .unwrap();
        create_archive(&plan, |_| {}, || false).unwrap();

        let file = File::open(root.join("archive.tgz")).unwrap();
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        assert_tar_readme(&mut archive);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn creates_tar_xz_archive() {
        let root = temp_path("tar-xz");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("README.md"), "readme").unwrap();

        let plan = plan_create_archive(
            &root,
            vec![root.join("README.md")],
            "archive.txz",
            CreateArchiveOptions::default(),
        )
        .unwrap();
        create_archive(&plan, |_| {}, || false).unwrap();

        let file = File::open(root.join("archive.txz")).unwrap();
        let decoder = xz2::read::XzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        assert_tar_readme(&mut archive);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn creates_tar_bzip2_archive() {
        let root = temp_path("tar-bzip2");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("README.md"), "readme").unwrap();

        let plan = plan_create_archive(
            &root,
            vec![root.join("README.md")],
            "archive.tbz2",
            CreateArchiveOptions::default(),
        )
        .unwrap();
        create_archive(&plan, |_| {}, || false).unwrap();

        let file = File::open(root.join("archive.tbz2")).unwrap();
        let decoder = bzip2::read::BzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        assert_tar_readme(&mut archive);
        let _ = fs::remove_dir_all(root);
    }

    fn assert_tar_readme<R: std::io::Read>(archive: &mut tar::Archive<R>) {
        let mut entry = archive.entries().unwrap().next().unwrap().unwrap();
        assert_eq!(entry.path().unwrap().to_string_lossy(), "README.md");
        let mut contents = String::new();
        use std::io::Read;
        entry.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "readme");
    }

    #[test]
    fn rejects_tar_passwords() {
        let root = temp_path("tar-password");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("README.md"), "readme").unwrap();
        let password = root.file_name().unwrap().to_string_lossy().into_owned();
        let error = plan_create_archive(
            &root,
            vec![root.join("README.md")],
            "archive.tar",
            CreateArchiveOptions {
                format: CreateArchiveFormat::Zip,
                encryption: ArchiveEncryption::Password(ArchivePassword::new(&password)),
            },
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "Password not supported for this format");
        let _ = fs::remove_file(root.join("README.md"));
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
