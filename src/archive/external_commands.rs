use super::{
    extraction::{
        ArchivePassword, ExtractError, ExtractPlan, ExtractProgress, ExtractResult, ExtractSummary,
    },
    path_safety::checked_output_name,
};
use anyhow::anyhow;
use std::{
    env,
    fs::{self, File, OpenOptions},
    io,
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

const SEVEN_ZIP_PROGRAMS: &[&str] = &["7z", "7zz", "7za"];
static OUTPUT_COUNTER: AtomicU64 = AtomicU64::new(0);

struct CommandOutputFiles {
    stdout_path: std::path::PathBuf,
    stderr_path: std::path::PathBuf,
}

impl CommandOutputFiles {
    fn create() -> ExtractResult<(Self, File, File)> {
        let temp_dir = env::temp_dir();
        let pid = std::process::id();
        for _ in 0..16 {
            let id = OUTPUT_COUNTER.fetch_add(1, Ordering::Relaxed);
            let base = temp_dir.join(format!("elio-7z-output-{pid}-{id}"));
            let stdout_path = base.with_extension("stdout");
            let stderr_path = base.with_extension("stderr");
            let stdout = match create_command_capture(&stdout_path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(ExtractError::Other(
                        anyhow!(error).context("Could not create 7z stdout capture"),
                    ));
                }
            };
            let stderr = match create_command_capture(&stderr_path) {
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

fn create_command_capture(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

impl Drop for CommandOutputFiles {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.stdout_path);
        let _ = fs::remove_file(&self.stderr_path);
    }
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
    let program = available_seven_zip()?;
    let entries = list_seven_zip_entries(program, &plan.archive_path, password)?;
    for entry in &entries {
        validate_entry_path(&plan.dest_dir, entry)?;
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
            skipped_links: 0,
        });
    }

    let mut command = Command::new(program);
    command.arg("x").arg("-y");
    command.arg(format!("-o{}", plan.dest_dir.display()));
    if let Some(password) = password {
        command.arg(password_arg(password));
    }
    command.arg("--").arg(&plan.archive_path);
    run_seven_zip_command(command, password.is_some(), &mut cancelled)?;

    let completed = entries.len();
    progress(ExtractProgress { completed, total });
    Ok(ExtractSummary {
        dest_dir: plan.dest_dir.clone(),
        completed,
        total,
        skipped_links: 0,
    })
}

fn password_arg(password: &ArchivePassword) -> String {
    // 7z's reliable non-interactive interface takes passwords as -pPASSWORD.
    // Keep stdin closed so archive extraction jobs cannot hang on password prompts.
    format!("-p{}", password.as_str())
}

pub(super) fn available_seven_zip() -> ExtractResult<&'static str> {
    let mut last_error = None;
    for &program in SEVEN_ZIP_PROGRAMS {
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

pub(super) fn preflight(
    archive_path: &Path,
    password: Option<&ArchivePassword>,
) -> ExtractResult<()> {
    let program = available_seven_zip()?;
    list_seven_zip_entries(program, archive_path, password)?;
    Ok(())
}

fn list_seven_zip_entries(
    program: &'static str,
    archive_path: &Path,
    password: Option<&ArchivePassword>,
) -> ExtractResult<Vec<String>> {
    let mut command = Command::new(program);
    command.arg("l").arg("-slt");
    if let Some(password) = password {
        command.arg(password_arg(password));
    }
    command.arg("--").arg(archive_path);
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(map_seven_zip_io_error)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        return Err(map_seven_zip_status(
            &output.stdout,
            &output.stderr,
            password.is_some(),
        ));
    }
    if password.is_none() && listing_has_encrypted_entries(&stdout) {
        return Err(ExtractError::PasswordRequired);
    }
    Ok(parse_seven_zip_entries(&stdout))
}

fn listing_has_encrypted_entries(output: &str) -> bool {
    output
        .lines()
        .any(|line| line.trim_end().eq_ignore_ascii_case("Encrypted = +"))
}

pub(super) fn parse_seven_zip_entries(output: &str) -> Vec<String> {
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

pub(super) fn validate_entry_path(dest_dir: &Path, entry: &str) -> ExtractResult<()> {
    checked_output_name(dest_dir, entry).map_err(|_| ExtractError::UnsafeArchivePath)?;
    Ok(())
}

pub(super) fn run_seven_zip_command<C>(
    mut command: Command,
    password_provided: bool,
    cancelled: &mut C,
) -> ExtractResult<()>
where
    C: FnMut() -> bool,
{
    let (output_files, stdout, stderr) = CommandOutputFiles::create()?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(map_seven_zip_io_error)?;

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
                return Err(map_seven_zip_status(&stdout, &stderr, password_provided));
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(map_seven_zip_io_error(error));
            }
        }
    }
}

fn map_seven_zip_io_error(error: io::Error) -> ExtractError {
    if error.kind() == io::ErrorKind::NotFound {
        ExtractError::MissingTool("7z")
    } else {
        ExtractError::Other(anyhow!(error).context("Could not run 7z"))
    }
}

fn map_seven_zip_status(stdout: &[u8], stderr: &[u8], password_provided: bool) -> ExtractError {
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
