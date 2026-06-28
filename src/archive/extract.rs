use super::format::{ExtractBackend, ExtractFormat, unique_destination};
use anyhow::{Context, Result, anyhow, bail};
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use sevenz_rust2::{
    ArchiveEntry as SevenZipEntry, ArchiveReader, Error as SevenZipError,
    Password as SevenZipPassword,
};
use std::{
    env,
    error::Error,
    fmt::{self, Display},
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};
use xz2::read::XzDecoder;
use zip::result::ZipError;
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

    fn as_bytes(&self) -> &[u8] {
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
        ExtractBackend::Zip => extract_zip(&extraction_plan, password, &mut progress, cancelled),
        ExtractBackend::Tar(format) => {
            extract_tar(format, &extraction_plan, &mut progress, cancelled)
        }
        ExtractBackend::SevenZip => {
            extract_seven_zip(&extraction_plan, password, &mut progress, cancelled)
        }
        ExtractBackend::ExternalSevenZip => {
            extract_external_seven_zip(&extraction_plan, password, &mut progress, cancelled)
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

fn preflight_extract(plan: &ExtractPlan, password: Option<&ArchivePassword>) -> ExtractResult<()> {
    match plan.backend {
        ExtractBackend::Zip => preflight_zip(&plan.archive_path, password),
        ExtractBackend::SevenZip | ExtractBackend::Tar(_) => Ok(()),
        ExtractBackend::ExternalSevenZip => {
            preflight_external_seven_zip(&plan.archive_path, password)
        }
    }
}

fn preflight_zip(archive_path: &Path, password: Option<&ArchivePassword>) -> ExtractResult<()> {
    let file = File::open(archive_path)
        .with_context(|| format!("Could not open {}", archive_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("Could not read ZIP archive")?;
    let total = archive.len();
    for index in 0..total {
        let _entry = zip_entry_by_index(&mut archive, index, password)?;
    }
    Ok(())
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
    progress(ExtractProgress {
        completed,
        total: Some(total),
    });

    for index in 0..total {
        if cancelled() {
            break;
        }
        let mut entry = zip_entry_by_index(&mut archive, index, password)?;
        let encrypted = entry.encrypted();
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
            if let Err(error) = io::copy(&mut entry, &mut out) {
                if password.is_some() && encrypted && is_zip_bad_password_io(&error) {
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

    Ok(ExtractSummary {
        dest_dir: plan.dest_dir.clone(),
        completed,
        total: Some(total),
    })
}

fn zip_entry_by_index<'a, R: Read + io::Seek>(
    archive: &'a mut zip::ZipArchive<R>,
    index: usize,
    password: Option<&ArchivePassword>,
) -> ExtractResult<zip::read::ZipFile<'a, R>> {
    let entry = match password {
        Some(password) => archive.by_index_decrypt(index, password.as_bytes()),
        None => archive.by_index(index),
    };
    entry.map_err(map_zip_error)
}

fn map_zip_error(error: ZipError) -> ExtractError {
    match error {
        ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED) => ExtractError::PasswordRequired,
        ZipError::InvalidPassword => ExtractError::BadPassword,
        ZipError::UnsupportedArchive(message) if is_zip_unsupported_encryption(message) => {
            ExtractError::UnsupportedEncryption
        }
        error => ExtractError::Other(anyhow!(error).context("Could not read ZIP entry")),
    }
}

fn is_zip_unsupported_encryption(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("encrypt") || message.contains("decrypt") || message.contains("aes")
}

fn is_zip_bad_password_io(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::InvalidData && error.to_string().contains("Invalid checksum")
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
        ExtractFormat::Zip | ExtractFormat::SevenZip | ExtractFormat::Rar => {
            unreachable!("non-TAR archives use their own extraction backends")
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

const EXTERNAL_SEVEN_ZIP_PROGRAMS: &[&str] = &["7z", "7zz", "7za"];
static EXTERNAL_COMMAND_OUTPUT_COUNTER: AtomicU64 = AtomicU64::new(0);

struct ExternalCommandOutputFiles {
    stdout_path: PathBuf,
    stderr_path: PathBuf,
}

impl ExternalCommandOutputFiles {
    fn create() -> ExtractResult<(Self, File, File)> {
        let temp_dir = env::temp_dir();
        let pid = std::process::id();
        for _ in 0..16 {
            let id = EXTERNAL_COMMAND_OUTPUT_COUNTER.fetch_add(1, Ordering::Relaxed);
            let base = temp_dir.join(format!("elio-7z-output-{pid}-{id}"));
            let stdout_path = base.with_extension("stdout");
            let stderr_path = base.with_extension("stderr");
            let stdout = match create_external_command_capture(&stdout_path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(ExtractError::Other(
                        anyhow!(error).context("Could not create 7z stdout capture"),
                    ));
                }
            };
            let stderr = match create_external_command_capture(&stderr_path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let _ = fs::remove_file(&stdout_path);
                    continue;
                }
                Err(error) => {
                    let _ = fs::remove_file(&stdout_path);
                    return Err(ExtractError::Other(
                        anyhow!(error).context("Could not create 7z stderr capture"),
                    ));
                }
            };
            return Ok((
                Self {
                    stdout_path,
                    stderr_path,
                },
                stdout,
                stderr,
            ));
        }

        Err(ExtractError::Other(anyhow!(
            "Could not create unique 7z output capture files"
        )))
    }

    fn read(&self) -> ExtractResult<(Vec<u8>, Vec<u8>)> {
        let stdout = fs::read(&self.stdout_path).map_err(|error| {
            ExtractError::Other(anyhow!(error).context("Could not read 7z stdout capture"))
        })?;
        let stderr = fs::read(&self.stderr_path).map_err(|error| {
            ExtractError::Other(anyhow!(error).context("Could not read 7z stderr capture"))
        })?;
        Ok((stdout, stderr))
    }
}

fn create_external_command_capture(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

impl Drop for ExternalCommandOutputFiles {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.stdout_path);
        let _ = fs::remove_file(&self.stderr_path);
    }
}

fn extract_external_seven_zip<F, C>(
    plan: &ExtractPlan,
    password: Option<&ArchivePassword>,
    progress: &mut F,
    mut cancelled: C,
) -> ExtractResult<ExtractSummary>
where
    F: FnMut(ExtractProgress),
    C: FnMut() -> bool,
{
    let program = available_external_seven_zip()?;
    let entries = list_external_seven_zip_entries(program, &plan.archive_path, password)?;
    for entry in &entries {
        validate_external_entry_path(&plan.dest_dir, entry)?;
    }

    let total = Some(entries.len());
    progress(ExtractProgress {
        completed: 0,
        total,
    });
    if cancelled() {
        return Ok(ExtractSummary {
            dest_dir: plan.dest_dir.clone(),
            completed: 0,
            total,
        });
    }

    let mut command = Command::new(program);
    command.arg("x").arg("-y");
    command.arg(format!("-o{}", plan.dest_dir.display()));
    if let Some(password) = password {
        command.arg(seven_zip_password_arg(password));
    }
    command.arg("--").arg(&plan.archive_path);
    run_external_seven_zip_command(command, password.is_some(), &mut cancelled)?;

    let completed = entries.len();
    progress(ExtractProgress { completed, total });
    Ok(ExtractSummary {
        dest_dir: plan.dest_dir.clone(),
        completed,
        total,
    })
}

fn seven_zip_password_arg(password: &ArchivePassword) -> String {
    // 7z's reliable non-interactive interface takes passwords as -pPASSWORD.
    // Keep stdin closed so archive extraction jobs cannot hang on password prompts.
    format!("-p{}", password.0)
}

fn available_external_seven_zip() -> ExtractResult<&'static str> {
    let mut last_error = None;
    for &program in EXTERNAL_SEVEN_ZIP_PROGRAMS {
        match Command::new(program)
            .arg("i")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(status) if status.success() => return Ok(program),
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => last_error = Some(error),
        }
    }

    if let Some(error) = last_error {
        Err(ExtractError::Other(
            anyhow!(error).context("Could not run 7z"),
        ))
    } else {
        Err(ExtractError::MissingTool("7z"))
    }
}

fn preflight_external_seven_zip(
    archive_path: &Path,
    password: Option<&ArchivePassword>,
) -> ExtractResult<()> {
    let program = available_external_seven_zip()?;
    list_external_seven_zip_entries(program, archive_path, password)?;
    Ok(())
}

fn list_external_seven_zip_entries(
    program: &'static str,
    archive_path: &Path,
    password: Option<&ArchivePassword>,
) -> ExtractResult<Vec<String>> {
    let mut command = Command::new(program);
    command.arg("l").arg("-slt");
    if let Some(password) = password {
        command.arg(seven_zip_password_arg(password));
    }
    command.arg("--").arg(archive_path);
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(map_external_seven_zip_io_error)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        return Err(map_external_seven_zip_status(
            &output.stdout,
            &output.stderr,
            password.is_some(),
        ));
    }
    if password.is_none() && external_seven_zip_listing_has_encrypted_entries(&stdout) {
        return Err(ExtractError::PasswordRequired);
    }
    Ok(parse_external_seven_zip_entries(&stdout))
}

fn external_seven_zip_listing_has_encrypted_entries(output: &str) -> bool {
    output
        .lines()
        .any(|line| line.trim_end().eq_ignore_ascii_case("Encrypted = +"))
}

fn parse_external_seven_zip_entries(output: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut in_entries = false;
    for raw_line in output.lines() {
        let line = raw_line.trim_end();
        if line == "----------" {
            in_entries = true;
            continue;
        }
        if !in_entries {
            continue;
        }
        match line.strip_prefix("Path = ") {
            Some(path) if !path.trim().is_empty() => entries.push(path.to_string()),
            _ => {}
        }
    }
    entries
}

fn validate_external_entry_path(dest_dir: &Path, entry: &str) -> ExtractResult<()> {
    checked_output_name(dest_dir, entry).map_err(|_| ExtractError::UnsafeArchivePath)?;
    Ok(())
}

fn run_external_seven_zip_command<C>(
    mut command: Command,
    password_provided: bool,
    cancelled: &mut C,
) -> ExtractResult<()>
where
    C: FnMut() -> bool,
{
    let (output_files, stdout, stderr) = ExternalCommandOutputFiles::create()?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(map_external_seven_zip_io_error)?;

    loop {
        if cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                }
                let (stdout, stderr) = output_files.read()?;
                return Err(map_external_seven_zip_status(
                    &stdout,
                    &stderr,
                    password_provided,
                ));
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(map_external_seven_zip_io_error(error));
            }
        }
    }
}

fn map_external_seven_zip_io_error(error: io::Error) -> ExtractError {
    if error.kind() == io::ErrorKind::NotFound {
        ExtractError::MissingTool("7z")
    } else {
        ExtractError::Other(anyhow!(error).context("Could not run 7z"))
    }
}

fn map_external_seven_zip_status(
    stdout: &[u8],
    stderr: &[u8],
    password_provided: bool,
) -> ExtractError {
    let message = format!(
        "{}\n{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    )
    .to_ascii_lowercase();
    if message.contains("wrong password") || message.contains("can not open encrypted archive") {
        if password_provided {
            ExtractError::BadPassword
        } else {
            ExtractError::PasswordRequired
        }
    } else if !password_provided
        && (message.contains("enter password") || message.contains("encrypted"))
    {
        ExtractError::PasswordRequired
    } else {
        ExtractError::ExternalFailed("7z")
    }
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
    fn parses_external_7z_listing_paths() {
        let output = r#"
Path = sample.rar
Type = Rar5
----------
Path = dir
Folder = +

Path = dir/file.txt
Folder = -
Size = 5
"#;

        assert_eq!(
            parse_external_seven_zip_entries(output),
            vec!["dir".to_string(), "dir/file.txt".to_string()]
        );
    }

    #[test]
    fn rejects_unsafe_external_7z_listing_paths() {
        let dest = Path::new("/tmp/out");

        assert!(validate_external_entry_path(dest, "ok/file.txt").is_ok());
        let error = validate_external_entry_path(dest, "../evil.txt").unwrap_err();
        assert!(matches!(error, ExtractError::UnsafeArchivePath));
        let error = validate_external_entry_path(dest, "/evil.txt").unwrap_err();
        assert!(matches!(error, ExtractError::UnsafeArchivePath));
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
    fn corrupt_zip_does_not_request_password() {
        let root = temp_path("zip-corrupt");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.zip");
        fs::write(&archive_path, b"not a zip").unwrap();

        let plan = plan_extract(&archive_path).unwrap();
        let error = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap_err();

        assert!(matches!(error, ExtractError::Other(_)));
        assert!(!plan.dest_dir.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn encrypted_zip_requires_password() {
        let root = temp_path("zip-encrypted-required");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.zip");
        let password = archive_test_password(&root);
        write_encrypted_zip(&archive_path, &password, ZipEncryption::Deprecated);

        let plan = plan_extract(&archive_path).unwrap();
        let error = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap_err();

        assert!(
            matches!(error, ExtractError::PasswordRequired),
            "expected password required, got {error:?}"
        );
        assert!(!plan.dest_dir.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn encrypted_zip_rejects_wrong_password() {
        let root = temp_path("zip-encrypted-wrong");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.zip");
        let password = archive_test_password(&root);
        write_encrypted_zip(&archive_path, &password, ZipEncryption::Deprecated);

        let plan = plan_extract(&archive_path).unwrap();
        let error = extract_archive_with_password(
            &plan,
            Some(&ArchivePassword::new(format!("{password}-wrong"))),
            |_| {},
            || false,
        )
        .unwrap_err();

        assert!(
            matches!(error, ExtractError::BadPassword),
            "expected bad password, got {error:?}"
        );
        assert!(!plan.dest_dir.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn encrypted_zip_extracts_with_password() {
        let root = temp_path("zip-encrypted-ok");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.zip");
        let password = archive_test_password(&root);
        write_encrypted_zip(&archive_path, &password, ZipEncryption::Deprecated);

        let plan = plan_extract(&archive_path).unwrap();
        let summary = extract_archive_with_password(
            &plan,
            Some(&ArchivePassword::new(password)),
            |_| {},
            || false,
        )
        .unwrap();

        assert_eq!(summary.completed, 1);
        assert_eq!(summary.total, Some(1));
        assert_eq!(
            fs::read_to_string(root.join("sample/file.txt")).unwrap(),
            "hello"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn aes_encrypted_zip_extracts_with_password() {
        let root = temp_path("zip-aes-encrypted-ok");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.zip");
        let password = archive_test_password(&root);
        write_encrypted_zip(&archive_path, &password, ZipEncryption::Aes256);

        let plan = plan_extract(&archive_path).unwrap();
        let summary = extract_archive_with_password(
            &plan,
            Some(&ArchivePassword::new(password)),
            |_| {},
            || false,
        )
        .unwrap();

        assert_eq!(summary.completed, 1);
        assert_eq!(summary.total, Some(1));
        assert_eq!(
            fs::read_to_string(root.join("sample/file.txt")).unwrap(),
            "hello"
        );
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
    fn extracts_rar_with_external_seven_zip_fallback() {
        if !external_seven_zip_available() {
            return;
        }
        let root = temp_path("rar-external-7z");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.rar");
        write_seven_zip(
            &archive_path,
            &[("dir", None), ("dir/file.txt", Some(b"hello"))],
        );

        let plan = plan_extract(&archive_path).unwrap();
        assert_eq!(plan.backend, ExtractBackend::ExternalSevenZip);
        let mut progress = Vec::new();
        let summary =
            extract_archive_with_password(&plan, None, |update| progress.push(update), || false)
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
    fn external_seven_zip_rejects_unsafe_paths_before_extracting() {
        if !external_seven_zip_available() {
            return;
        }
        let root = temp_path("rar-external-7z-slip");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.rar");
        write_seven_zip(&archive_path, &[("../evil.txt", Some(b"bad"))]);

        let plan = plan_extract(&archive_path).unwrap();
        let error = extract_archive_with_password(&plan, None, |_| {}, || false).unwrap_err();

        assert!(matches!(error, ExtractError::UnsafeArchivePath));
        assert!(!root.join("evil.txt").exists());
        assert!(!plan.dest_dir.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg(unix)]
    fn external_seven_zip_command_handles_large_failure_output() {
        let mut command = Command::new("sh");
        command.arg("-c").arg(
            "i=0; while [ $i -lt 5000 ]; do printf 'ERROR: Wrong password\\n' >&2; i=$((i+1)); done; exit 2",
        );

        let error = run_external_seven_zip_command(command, true, &mut || false).unwrap_err();

        assert!(matches!(error, ExtractError::BadPassword));
    }

    #[test]
    fn encrypted_rar_requires_password_before_staging() {
        if !external_seven_zip_available() {
            return;
        }
        let root = temp_path("rar-encrypted-required");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.rar");
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
    fn encrypted_rar_rejects_wrong_password_before_staging() {
        if !external_seven_zip_available() {
            return;
        }
        let root = temp_path("rar-encrypted-wrong");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.rar");
        let password = archive_test_password(&root);
        write_encrypted_seven_zip(
            &archive_path,
            &password,
            &[("dir", None), ("dir/file.txt", Some(b"hello"))],
        );

        let plan = plan_extract(&archive_path).unwrap();
        let error = extract_archive_with_password(
            &plan,
            Some(&ArchivePassword::new(format!("{password}-wrong"))),
            |_| {},
            || false,
        )
        .unwrap_err();

        assert!(matches!(error, ExtractError::BadPassword));
        assert!(!plan.dest_dir.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn encrypted_rar_extracts_with_password() {
        if !external_seven_zip_available() {
            return;
        }
        let root = temp_path("rar-encrypted-ok");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.rar");
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

    fn external_seven_zip_available() -> bool {
        available_external_seven_zip().is_ok()
    }

    enum ZipEncryption {
        Deprecated,
        Aes256,
    }

    fn write_encrypted_zip(archive_path: &Path, password: &str, encryption: ZipEncryption) {
        use zip::unstable::write::FileOptionsExt;

        let file = File::create(archive_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = match encryption {
            ZipEncryption::Deprecated => zip::write::SimpleFileOptions::default()
                .with_deprecated_encryption(password.as_bytes())
                .unwrap(),
            ZipEncryption::Aes256 => zip::write::SimpleFileOptions::default()
                .with_aes_encryption(zip::AesMode::Aes256, password),
        };
        zip.start_file("file.txt", options).unwrap();
        zip.write_all(b"hello").unwrap();
        zip.finish().unwrap();
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
