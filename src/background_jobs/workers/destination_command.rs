use super::*;
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_COMMAND_OUTPUT_BYTES: u64 = 8 * 1024;
const COMMAND_FAILED: &str = "command failed";
const COMMAND_COULD_NOT_START: &str = "command could not start";
const COMMAND_TIMED_OUT: &str = "command timed out";
const INVALID_DIRECTORY: &str = "command did not return a valid directory";

pub(in crate::background_jobs) struct DestinationCommandPool {
    request_tx: Option<mpsc::Sender<DestinationCommandRequest>>,
    pending: Arc<AtomicUsize>,
    worker: Option<thread::JoinHandle<()>>,
}

impl DestinationCommandPool {
    pub(in crate::background_jobs) fn new(result_tx: mpsc::Sender<JobResult>) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<DestinationCommandRequest>();
        let pending = Arc::new(AtomicUsize::new(0));
        let worker_pending = Arc::clone(&pending);
        let worker = thread::spawn(move || {
            while let Ok(request) = request_rx.recv() {
                let result = resolve_command_destination(&request.command, &request.cwd);
                worker_pending.fetch_sub(1, Ordering::Relaxed);
                if result_tx
                    .send(JobResult::DestinationCommand(DestinationCommandBuild {
                        token: request.token,
                        recipient: request.recipient,
                        label: request.label,
                        result,
                    }))
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            request_tx: Some(request_tx),
            pending,
            worker: Some(worker),
        }
    }

    pub(in crate::background_jobs) fn submit(&self, request: DestinationCommandRequest) -> bool {
        let Some(request_tx) = &self.request_tx else {
            return false;
        };
        self.pending.fetch_add(1, Ordering::Relaxed);
        if request_tx.send(request).is_ok() {
            true
        } else {
            self.pending.fetch_sub(1, Ordering::Relaxed);
            false
        }
    }

    pub(in crate::background_jobs) fn has_pending_work(&self) -> bool {
        self.pending.load(Ordering::Relaxed) > 0
    }
}

impl Drop for DestinationCommandPool {
    fn drop(&mut self) {
        self.request_tx = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub(in crate::background_jobs) fn resolve_command_destination(
    command: &str,
    cwd: &Path,
) -> Result<PathBuf, &'static str> {
    let tokens = crate::opening::tokenize_command(command);
    let Some((program, args)) = tokens.split_first() else {
        return Err(COMMAND_COULD_NOT_START);
    };

    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| command_start_error(&error))?;
    let stdout = child
        .stdout
        .take()
        .expect("stdout is piped when starting a destination command");
    let reader = thread::spawn(move || {
        let mut output = Vec::new();
        stdout
            .take(MAX_COMMAND_OUTPUT_BYTES + 1)
            .read_to_end(&mut output)
            .map(|_| output)
    });

    let started_at = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|_| COMMAND_FAILED)? {
            break status;
        }
        if started_at.elapsed() >= COMMAND_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(COMMAND_TIMED_OUT);
        }
        thread::sleep(Duration::from_millis(10));
    };
    let output = reader
        .join()
        .map_err(|_| INVALID_DIRECTORY)?
        .map_err(|_| INVALID_DIRECTORY)?;

    if !status.success() {
        return Err(COMMAND_FAILED);
    }
    if output.len() as u64 > MAX_COMMAND_OUTPUT_BYTES {
        return Err(INVALID_DIRECTORY);
    }
    parse_command_output(&output)
}

fn command_start_error(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::NotFound => "command not found",
        io::ErrorKind::PermissionDenied => "command permission denied",
        _ => COMMAND_COULD_NOT_START,
    }
}

fn parse_command_output(output: &[u8]) -> Result<PathBuf, &'static str> {
    let output = std::str::from_utf8(output).map_err(|_| INVALID_DIRECTORY)?;
    let output = output.strip_suffix('\n').unwrap_or(output);
    let output = output.strip_suffix('\r').unwrap_or(output);
    if output.is_empty() {
        return Err(INVALID_DIRECTORY);
    }
    if output.contains(['\n', '\r', '\0']) {
        return Err(INVALID_DIRECTORY);
    }

    let path = PathBuf::from(output);
    if !path.is_absolute() || !path.is_dir() {
        return Err(INVALID_DIRECTORY);
    }
    Ok(path)
}

#[cfg(test)]
#[path = "tests/destination_command.rs"]
mod tests;
