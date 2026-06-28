use super::format::{ExtractBackend, ExtractFormat, unique_destination};
use anyhow::{Context, Result, anyhow, bail};
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use sevenz_rust2::{
    ArchiveEntry as SevenZipEntry, ArchiveReader, Error as SevenZipError,
    Password as SevenZipPassword,
};
use std::{
    error::Error,
    fmt::{self, Display},
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};
use xz2::read::XzDecoder;
use zstd::stream::read::Decoder as ZstdDecoder;

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
}

#[derive(Clone, Default, Eq, PartialEq)]
pub(crate) struct ArchivePassword(String);

impl ArchivePassword {
    pub(crate) fn new(password: impl Into<String>) -> Self {
        Self(password.into())
    }

    fn as_seven_zip_password(&self) -> SevenZipPassword {
        SevenZipPassword::new(&self.0)
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
    Other(anyhow::Error),
}

impl Display for ExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PasswordRequired => f.write_str("archive requires a password"),
            Self::BadPassword => f.write_str("wrong password"),
            Self::UnsupportedEncryption => f.write_str("unsupported encrypted archive"),
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

type ExtractResult<T> = std::result::Result<T, ExtractError>;

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
        ExtractBackend::Zip => extract_zip(&extraction_plan, &mut progress, cancelled),
        ExtractBackend::Tar(format) => {
            extract_tar(format, &extraction_plan, &mut progress, cancelled)
        }
        ExtractBackend::SevenZip => {
            extract_seven_zip(&extraction_plan, password, &mut progress, cancelled)
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
    })
}

fn preflight_extract(plan: &ExtractPlan, _password: Option<&ArchivePassword>) -> ExtractResult<()> {
    match plan.backend {
        ExtractBackend::Zip => preflight_zip(&plan.archive_path),
        ExtractBackend::SevenZip | ExtractBackend::Tar(_) => Ok(()),
    }
}

fn preflight_zip(archive_path: &Path) -> ExtractResult<()> {
    let file = File::open(archive_path)
        .with_context(|| format!("Could not open {}", archive_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("Could not read ZIP archive")?;
    let total = archive.len();
    reject_encrypted_zip_entries(&mut archive, total)
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

fn extract_zip<F, C>(
    plan: &ExtractPlan,
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
    reject_encrypted_zip_entries(&mut archive, total)?;
    let mut completed = 0usize;
    progress(ExtractProgress {
        completed,
        total: Some(total),
    });

    for index in 0..total {
        if cancelled() {
            break;
        }
        let mut entry = archive
            .by_index(index)
            .context("Could not read ZIP entry")?;
        let Some(enclosed) = entry.enclosed_name() else {
            return Err(anyhow!("Archive entry escapes the destination: {}", entry.name()).into());
        };
        let out_path = checked_output_path(&plan.dest_dir, enclosed.as_ref())?;
        if entry.is_dir() {
            fs::create_dir_all(&out_path)
                .with_context(|| format!("Could not create {}", out_path.display()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Could not create {}", parent.display()))?;
            }
            let mut out = File::create(&out_path)
                .with_context(|| format!("Could not create {}", out_path.display()))?;
            io::copy(&mut entry, &mut out)
                .with_context(|| format!("Could not write {}", out_path.display()))?;
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

    Ok(ExtractSummary {
        dest_dir: plan.dest_dir.clone(),
        completed,
        total: Some(total),
    })
}

fn reject_encrypted_zip_entries<R: Read + io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    total: usize,
) -> ExtractResult<()> {
    for index in 0..total {
        let entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(error) if error.to_string().contains("Password required") => {
                return Err(ExtractError::UnsupportedEncryption);
            }
            Err(error) => return Err(anyhow!(error).context("Could not read ZIP entry").into()),
        };
        if entry.encrypted() {
            return Err(ExtractError::UnsupportedEncryption);
        }
    }
    Ok(())
}

fn extract_tar<F, C>(
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
        ExtractFormat::Tar => extract_tar_with(format, plan, Ok, progress, cancelled),
        ExtractFormat::TarGzip => extract_tar_with(
            format,
            plan,
            |file| Ok(GzDecoder::new(file)),
            progress,
            cancelled,
        ),
        ExtractFormat::TarXz => extract_tar_with(
            format,
            plan,
            |file| Ok(XzDecoder::new(file)),
            progress,
            cancelled,
        ),
        ExtractFormat::TarBzip2 => extract_tar_with(
            format,
            plan,
            |file| Ok(BzDecoder::new(file)),
            progress,
            cancelled,
        ),
        ExtractFormat::TarZstd => {
            extract_tar_with(format, plan, ZstdDecoder::new, progress, cancelled)
        }
        ExtractFormat::Zip | ExtractFormat::SevenZip => {
            unreachable!("non-TAR archives use their own native backends")
        }
    }
}

fn extract_seven_zip<F, C>(
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
        .unwrap_or_else(SevenZipPassword::empty);
    let mut archive = ArchiveReader::new(file, password)
        .map_err(|error| map_seven_zip_error(error, password_provided))?;
    let total = archive.archive().files.len();
    let mut completed = 0usize;
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
            if let Err(error) = extract_seven_zip_entry(plan, entry, reader) {
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
        .map_err(|error| map_seven_zip_error(error, password_provided))?;

    if let Some(error) = extract_error {
        return Err(error);
    }
    Ok(ExtractSummary {
        dest_dir: plan.dest_dir.clone(),
        completed,
        total: Some(total),
    })
}

fn extract_seven_zip_entry(
    plan: &ExtractPlan,
    entry: &SevenZipEntry,
    reader: &mut dyn Read,
) -> ExtractResult<()> {
    if entry.is_anti_item {
        return Ok(());
    }
    let out_path = checked_output_name(&plan.dest_dir, &entry.name)?;
    if entry.is_directory {
        fs::create_dir_all(&out_path)
            .with_context(|| format!("Could not create {}", out_path.display()))?;
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

fn extract_tar_with<R, D, F, C>(
    format: ExtractFormat,
    plan: &ExtractPlan,
    decoder: D,
    progress: &mut F,
    cancelled: C,
) -> ExtractResult<ExtractSummary>
where
    R: Read,
    D: Fn(File) -> Result<R, std::io::Error>,
    F: FnMut(ExtractProgress),
    C: FnMut() -> bool,
{
    let count_file = File::open(&plan.archive_path)
        .with_context(|| format!("Could not open {}", plan.archive_path.display()))?;
    let total = count_tar_entries(decoder(count_file).context("Could not initialize TAR decoder")?)
        .with_context(|| format!("Could not read {} archive", format.label()))?;
    let file = File::open(&plan.archive_path)
        .with_context(|| format!("Could not open {}", plan.archive_path.display()))?;
    extract_tar_reader(
        plan,
        decoder(file).context("Could not initialize TAR decoder")?,
        Some(total),
        progress,
        cancelled,
    )
}

fn count_tar_entries<R: Read>(reader: R) -> Result<usize> {
    let mut archive = tar::Archive::new(reader);
    let mut total = 0usize;
    for entry in archive.entries()? {
        entry?;
        total += 1;
    }
    Ok(total)
}

fn extract_tar_reader<R, F, C>(
    plan: &ExtractPlan,
    reader: R,
    total: Option<usize>,
    progress: &mut F,
    mut cancelled: C,
) -> ExtractResult<ExtractSummary>
where
    R: Read,
    F: FnMut(ExtractProgress),
    C: FnMut() -> bool,
{
    let mut archive = tar::Archive::new(reader);
    let mut completed = 0usize;
    progress(ExtractProgress { completed, total });
    for entry in archive.entries().context("Could not read TAR entries")? {
        if cancelled() {
            break;
        }
        let mut entry = entry.context("Could not read TAR entry")?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            completed += 1;
            progress(ExtractProgress { completed, total });
            continue;
        }
        let path = entry.path().context("Could not read TAR entry path")?;
        let out_path = checked_output_path(&plan.dest_dir, path.as_ref())?;
        if entry_type.is_dir() {
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
    Ok(ExtractSummary {
        dest_dir: plan.dest_dir.clone(),
        completed,
        total,
    })
}

fn checked_output_path(dest_dir: &Path, entry_path: &Path) -> Result<PathBuf> {
    let mut relative = PathBuf::new();
    for component in entry_path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!(
                    "Archive entry escapes the destination: {}",
                    entry_path.display()
                );
            }
        }
    }
    if relative.as_os_str().is_empty() {
        bail!("Archive entry has an empty path");
    }
    let out = dest_dir.join(relative);
    if !out.starts_with(dest_dir) {
        bail!(
            "Archive entry escapes the destination: {}",
            entry_path.display()
        );
    }
    Ok(out)
}

fn checked_output_name(dest_dir: &Path, entry_name: &str) -> Result<PathBuf> {
    let normalized = entry_name.replace('\\', "/");
    checked_output_path(dest_dir, Path::new(&normalized))
}

fn map_seven_zip_error(error: SevenZipError, password_provided: bool) -> ExtractError {
    match error {
        SevenZipError::PasswordRequired => ExtractError::PasswordRequired,
        SevenZipError::MaybeBadPassword(_) => ExtractError::BadPassword,
        SevenZipError::ChecksumVerificationFailed if password_provided => ExtractError::BadPassword,
        SevenZipError::UnsupportedCompressionMethod(method)
            if method.to_ascii_lowercase().contains("aes") =>
        {
            ExtractError::UnsupportedEncryption
        }
        SevenZipError::Unsupported(message) if message.to_ascii_lowercase().contains("aes") => {
            ExtractError::UnsupportedEncryption
        }
        error => ExtractError::Other(anyhow!("Could not read 7z archive: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("elio-archive-extract-{label}-{unique}"))
    }

    fn archive_test_password(root: &Path) -> String {
        root.file_name()
            .expect("temp root should have a file name")
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn rejects_escaping_paths() {
        let dest = Path::new("/tmp/out");
        assert!(checked_output_path(dest, Path::new("../evil")).is_err());
        assert!(checked_output_path(dest, Path::new("/evil")).is_err());
        assert_eq!(
            checked_output_path(dest, Path::new("ok/file.txt")).unwrap(),
            dest.join("ok/file.txt")
        );
    }

    #[test]
    fn extracts_zip_archive() {
        let root = temp_path("zip");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.zip");
        {
            let file = File::create(&archive_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            zip.add_directory("dir/", options).unwrap();
            zip.start_file("dir/file.txt", options).unwrap();
            zip.write_all(b"hello").unwrap();
            zip.finish().unwrap();
        }
        let plan = plan_extract(&archive_path).unwrap();
        let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();
        assert_eq!(summary.completed, 2);
        assert_eq!(
            fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
            "hello"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn encrypted_zip_fails_before_creating_destination() {
        use zip::unstable::write::FileOptionsExt;

        let root = temp_path("zip-encrypted");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.zip");
        {
            let file = File::create(&archive_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .with_deprecated_encryption(b"secret")
                .unwrap();
            zip.start_file("file.txt", options).unwrap();
            zip.write_all(b"hello").unwrap();
            zip.finish().unwrap();
        }

        let plan = plan_extract(&archive_path).unwrap();
        let error = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap_err();

        assert!(
            matches!(error, ExtractError::UnsupportedEncryption),
            "expected unsupported encryption, got {error:?}"
        );
        assert!(!plan.dest_dir.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_tar_archive() {
        let root = temp_path("tar");
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source");
        fs::create_dir_all(source.join("dir")).unwrap();
        fs::write(source.join("dir/file.txt"), "hello").unwrap();
        let archive_path = root.join("sample.tar");
        {
            let file = File::create(&archive_path).unwrap();
            let mut tar = tar::Builder::new(file);
            tar.append_dir("dir", source.join("dir")).unwrap();
            tar.append_path_with_name(source.join("dir/file.txt"), "dir/file.txt")
                .unwrap();
            tar.finish().unwrap();
        }
        let plan = plan_extract(&archive_path).unwrap();
        let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();
        assert_eq!(summary.completed, 2);
        assert_eq!(
            fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
            "hello"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_tar_gzip_archive() {
        let root = temp_path("tgz");
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source");
        fs::create_dir_all(source.join("dir")).unwrap();
        fs::write(source.join("dir/file.txt"), "hello").unwrap();
        let archive_path = root.join("sample.tar.gz");
        {
            let file = File::create(&archive_path).unwrap();
            let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            let mut tar = tar::Builder::new(enc);
            tar.append_dir("dir", source.join("dir")).unwrap();
            tar.append_path_with_name(source.join("dir/file.txt"), "dir/file.txt")
                .unwrap();
            tar.finish().unwrap();
        }
        let plan = plan_extract(&archive_path).unwrap();
        let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();
        assert_eq!(summary.completed, 2);
        assert_eq!(
            fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
            "hello"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_tar_xz_archive() {
        let root = temp_path("txz");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.tar.xz");
        write_compressed_tar(&archive_path, |file| {
            Ok(xz2::write::XzEncoder::new(file, 6))
        });

        let plan = plan_extract(&archive_path).unwrap();
        let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();

        assert_eq!(summary.completed, 2);
        assert_eq!(
            fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
            "hello"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_tar_bzip2_archive() {
        let root = temp_path("tbz2");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.tar.bz2");
        write_compressed_tar(&archive_path, |file| {
            Ok(bzip2::write::BzEncoder::new(
                file,
                bzip2::Compression::default(),
            ))
        });

        let plan = plan_extract(&archive_path).unwrap();
        let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();

        assert_eq!(summary.completed, 2);
        assert_eq!(
            fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
            "hello"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_tar_zstd_archive() {
        let root = temp_path("tzst");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.tar.zst");
        write_zstd_tar(&archive_path);

        let plan = plan_extract(&archive_path).unwrap();
        let summary = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap();

        assert_eq!(summary.completed, 2);
        assert_eq!(
            fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
            "hello"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_seven_zip_archive() {
        let root = temp_path("7z");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.7z");
        write_seven_zip(
            &archive_path,
            &[("dir", None), ("dir/file.txt", Some(b"hello"))],
        );

        let plan = plan_extract(&archive_path).unwrap();
        let mut progress = Vec::new();
        let summary =
            extract_archive_with_password(&plan, None, |update| progress.push(update), || false)
                .unwrap();

        assert_eq!(summary.completed, 2);
        assert_eq!(summary.total, Some(2));
        assert_eq!(
            progress
                .last()
                .map(|update| (update.completed, update.total)),
            Some((2, Some(2)))
        );
        assert_eq!(
            fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
            "hello"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archive_password_debug_is_redacted() {
        let password = std::any::type_name::<ArchivePassword>();
        let rendered = format!("{:?}", ArchivePassword::new(password));

        assert_eq!(rendered, "ArchivePassword(<redacted>)");
        assert!(!rendered.contains(password));
    }

    #[test]
    fn encrypted_seven_zip_requires_password() {
        let root = temp_path("7z-encrypted-required");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.7z");
        let password = archive_test_password(&root);
        write_encrypted_seven_zip(
            &archive_path,
            &password,
            &[("dir", None), ("dir/file.txt", Some(b"hello"))],
        );

        let plan = plan_extract(&archive_path).unwrap();
        let error = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap_err();

        assert!(matches!(error, ExtractError::PasswordRequired));
        assert!(!plan.dest_dir.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn encrypted_seven_zip_rejects_wrong_password() {
        let root = temp_path("7z-encrypted-wrong");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.7z");
        let password = archive_test_password(&root);
        let wrong_password = format!("{password}-wrong");
        write_encrypted_seven_zip(
            &archive_path,
            &password,
            &[("dir", None), ("dir/file.txt", Some(b"hello"))],
        );

        let plan = plan_extract(&archive_path).unwrap();
        let error = extract_archive_with_password(
            &plan,
            Some(&ArchivePassword::new(wrong_password)),
            |_| {},
            || false,
        )
        .unwrap_err();

        assert!(matches!(error, ExtractError::BadPassword));
        assert!(!plan.dest_dir.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn encrypted_seven_zip_extracts_with_password() {
        let root = temp_path("7z-encrypted-ok");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.7z");
        let password = archive_test_password(&root);
        write_encrypted_seven_zip(
            &archive_path,
            &password,
            &[("dir", None), ("dir/file.txt", Some(b"hello"))],
        );

        let plan = plan_extract(&archive_path).unwrap();
        let summary = extract_archive_with_password(
            &plan,
            Some(&ArchivePassword::new(password)),
            |_| {},
            || false,
        )
        .unwrap();

        assert_eq!(summary.completed, 2);
        assert_eq!(summary.total, Some(2));
        assert_eq!(
            fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
            "hello"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_escaping_seven_zip_paths() {
        let root = temp_path("7z-slip");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.7z");
        write_seven_zip(&archive_path, &[("../evil.txt", Some(b"bad"))]);

        let plan = plan_extract(&archive_path).unwrap();
        let err = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap_err();

        assert!(err.to_string().contains("escapes the destination"));
        assert!(!root.join("evil.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    fn write_compressed_tar<W, D>(archive_path: &Path, encoder: D)
    where
        W: Write,
        D: FnOnce(File) -> std::result::Result<W, std::io::Error>,
    {
        let root = archive_path.parent().unwrap();
        let source = root.join("source");
        fs::create_dir_all(source.join("dir")).unwrap();
        fs::write(source.join("dir/file.txt"), "hello").unwrap();

        let file = File::create(archive_path).unwrap();
        let writer = encoder(file).unwrap();
        let mut tar = tar::Builder::new(writer);
        tar.append_dir("dir", source.join("dir")).unwrap();
        tar.append_path_with_name(source.join("dir/file.txt"), "dir/file.txt")
            .unwrap();
        tar.finish().unwrap();
    }

    fn write_zstd_tar(archive_path: &Path) {
        let root = archive_path.parent().unwrap();
        let source = root.join("source");
        fs::create_dir_all(source.join("dir")).unwrap();
        fs::write(source.join("dir/file.txt"), "hello").unwrap();

        let mut tar_bytes = Vec::new();
        {
            let mut tar = tar::Builder::new(&mut tar_bytes);
            tar.append_dir("dir", source.join("dir")).unwrap();
            tar.append_path_with_name(source.join("dir/file.txt"), "dir/file.txt")
                .unwrap();
            tar.finish().unwrap();
        }
        let compressed = zstd::stream::encode_all(tar_bytes.as_slice(), 0).unwrap();
        fs::write(archive_path, compressed).unwrap();
    }

    fn write_seven_zip(archive_path: &Path, entries: &[(&str, Option<&[u8]>)]) {
        let mut writer = sevenz_rust2::ArchiveWriter::create(archive_path).unwrap();
        for (name, contents) in entries {
            let entry = if contents.is_some() {
                sevenz_rust2::ArchiveEntry::new_file(name)
            } else {
                sevenz_rust2::ArchiveEntry::new_directory(name)
            };
            match contents {
                Some(contents) => {
                    writer.push_archive_entry(entry, Some(*contents)).unwrap();
                }
                None => {
                    writer.push_archive_entry::<&[u8]>(entry, None).unwrap();
                }
            }
        }
        writer.finish().unwrap();
    }

    fn write_encrypted_seven_zip(
        archive_path: &Path,
        password: &str,
        entries: &[(&str, Option<&[u8]>)],
    ) {
        let mut writer = sevenz_rust2::ArchiveWriter::create(archive_path).unwrap();
        writer.set_content_methods(vec![
            sevenz_rust2::encoder_options::AesEncoderOptions::new(sevenz_rust2::Password::new(
                password,
            ))
            .into(),
            sevenz_rust2::encoder_options::Lzma2Options::default().into(),
        ]);
        for (name, contents) in entries {
            let entry = if contents.is_some() {
                sevenz_rust2::ArchiveEntry::new_file(name)
            } else {
                sevenz_rust2::ArchiveEntry::new_directory(name)
            };
            match contents {
                Some(contents) => {
                    writer.push_archive_entry(entry, Some(*contents)).unwrap();
                }
                None => {
                    writer.push_archive_entry::<&[u8]>(entry, None).unwrap();
                }
            }
        }
        writer.finish().unwrap();
    }
}
