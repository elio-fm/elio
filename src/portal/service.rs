//! Runtime-only XDG FileChooser backend.
//!
//! The D-Bus executor only awaits a completion future. All private Unix-socket
//! work runs on a dedicated worker thread, so a slow terminal or chooser never
//! blocks D-Bus dispatch.

use super::{
    chooser_protocol::{
        ChooserRequest, ChooserRequestMode, Message, SelectionKind, ServiceEndpoint,
    },
    terminal::TerminalAdapter,
};
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    env, fs,
    future::Future,
    os::unix::fs::PermissionsExt,
    path::Path,
    pin::Pin,
    process::{Child, Command},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context as TaskContext, Poll, Waker},
    thread,
    time::{Duration, Instant},
};
use zbus::{
    Connection,
    zvariant::{OwnedObjectPath, OwnedValue},
};

const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_NAME: &str = "io.github.elio_fm.elio.Portal";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_OPTIONS: usize = 16;
const MAX_TEXT_BYTES: usize = 4096;
const MAX_FILTERS: usize = 64;
const MAX_FILTER_RULES: usize = 64;
const OPEN_OPTIONS: &[&str] = &[
    "accept_label",
    "modal",
    "multiple",
    "directory",
    "filters",
    "current_filter",
    "current_folder",
];
const SAVE_OPTIONS: &[&str] = &[
    "accept_label",
    "modal",
    "filters",
    "current_filter",
    "current_folder",
    "current_name",
    "current_file",
];

type Filter = (String, Vec<(u32, String)>);

pub(crate) fn run() -> Result<()> {
    crate::config::initialize(None)?;
    let terminal = crate::config::portal_terminal()
        .context("[portal].terminal must be configured before starting the portal service")?;
    let chooser_executable = chooser_executable()?;
    zbus::block_on(run_async(terminal, chooser_executable))
}

async fn run_async(
    terminal: TerminalAdapter,
    chooser_executable: std::path::PathBuf,
) -> Result<()> {
    let connection = Connection::session().await?;
    connection.request_name(PORTAL_NAME).await?;
    connection
        .object_server()
        .at(
            PORTAL_PATH,
            FileChooser {
                terminal,
                chooser_executable,
            },
        )
        .await?;
    connection.closed().await;
    Ok(())
}

struct FileChooser {
    terminal: TerminalAdapter,
    chooser_executable: std::path::PathBuf,
}

#[zbus::interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooser {
    async fn open_file(
        &self,
        handle: OwnedObjectPath,
        _app_id: String,
        _parent_window: String,
        _title: String,
        options: HashMap<String, OwnedValue>,
        #[zbus(connection)] connection: &Connection,
    ) -> (u32, HashMap<String, OwnedValue>) {
        if !has_only_options(&options, OPEN_OPTIONS) || !valid_ignored_options(&options) {
            return failed();
        }
        let Some(multiple) = option_bool(&options, "multiple") else {
            return failed();
        };
        let Some(initial_path) = option_path(&options, "current_folder") else {
            return failed();
        };
        let Some(directory) = option_bool(&options, "directory") else {
            return failed();
        };
        let kind = if directory {
            SelectionKind::Directory
        } else {
            SelectionKind::File
        };
        self.start_request(
            handle,
            ChooserRequest {
                mode: ChooserRequestMode::Open { kind, multiple },
                initial_path,
            },
            connection,
        )
        .await
    }

    async fn save_file(
        &self,
        handle: OwnedObjectPath,
        _app_id: String,
        _parent_window: String,
        _title: String,
        options: HashMap<String, OwnedValue>,
        #[zbus(connection)] connection: &Connection,
    ) -> (u32, HashMap<String, OwnedValue>) {
        if !has_only_options(&options, SAVE_OPTIONS) || !valid_ignored_options(&options) {
            return failed();
        }
        let Some((initial_path, initial_name)) = save_startup(&options) else {
            return failed();
        };
        self.start_request(
            handle,
            ChooserRequest {
                mode: ChooserRequestMode::SaveFile { initial_name },
                initial_path,
            },
            connection,
        )
        .await
    }

    async fn save_files(
        &self,
        _handle: OwnedObjectPath,
        _app_id: String,
        _parent_window: String,
        _title: String,
        _options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        failed()
    }
}

impl FileChooser {
    async fn start_request(
        &self,
        handle: OwnedObjectPath,
        request: ChooserRequest,
        connection: &Connection,
    ) -> (u32, HashMap<String, OwnedValue>) {
        let path = handle.to_string();
        let state = Arc::new(RequestState::default());
        let request_object = Request {
            state: Arc::clone(&state),
        };
        if connection
            .object_server()
            .at(path.as_str(), request_object)
            .await
            .is_err()
        {
            return failed();
        }

        let worker_state = Arc::clone(&state);
        let terminal = self.terminal;
        let chooser_executable = self.chooser_executable.clone();
        thread::spawn(move || {
            let response = run_request(
                terminal,
                chooser_executable,
                request,
                Arc::clone(&worker_state),
            );
            worker_state.complete(response);
        });
        let response = CompletionFuture {
            state: Arc::clone(&state),
        }
        .await;
        let _ = connection
            .object_server()
            .remove::<Request, _>(path.as_str())
            .await;
        response
    }
}

struct Request {
    state: Arc<RequestState>,
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Request")]
impl Request {
    fn close(&self) {
        self.state.cancel();
        self.state.complete(cancelled());
    }
}

#[derive(Default)]
struct Completion {
    state: Mutex<CompletionState>,
}

#[derive(Default)]
struct CompletionState {
    value: Option<(u32, HashMap<String, OwnedValue>)>,
    waker: Option<Waker>,
}

impl Completion {
    /// The request outcome is immutable. Close, child completion, and a socket
    /// failure may race; the first terminal transition wins.
    fn complete(&self, value: (u32, HashMap<String, OwnedValue>)) {
        let waker = {
            let mut state = self.state.lock().expect("completion lock poisoned");
            if state.value.is_some() {
                return;
            }
            state.value = Some(value);
            state.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    fn poll(&self, waker: &Waker) -> Poll<(u32, HashMap<String, OwnedValue>)> {
        let mut state = self.state.lock().expect("completion lock poisoned");
        if let Some(value) = state.value.take() {
            Poll::Ready(value)
        } else {
            state.waker = Some(waker.clone());
            Poll::Pending
        }
    }
}

#[derive(Default)]
struct RequestState {
    cancelled: AtomicBool,
    child: Mutex<Option<Child>>,
    cancellation: Mutex<Option<super::chooser_protocol::ServiceCancellation>>,
    completion: Completion,
}

impl RequestState {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        if let Ok(mut cancellation) = self.cancellation.lock()
            && let Some(mut cancellation) = cancellation.take()
        {
            let _ = cancellation.cancel_and_disconnect();
        }
        if let Ok(mut child) = self.child.lock()
            && let Some(mut child) = child.take()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn complete(&self, value: (u32, HashMap<String, OwnedValue>)) {
        self.completion.complete(value);
    }

    fn reap_child(&self) {
        let child = self
            .child
            .lock()
            .expect("request child lock poisoned")
            .take();
        if let Some(mut child) = child {
            thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}

struct CompletionFuture {
    state: Arc<RequestState>,
}

impl Future for CompletionFuture {
    type Output = (u32, HashMap<String, OwnedValue>);

    fn poll(self: Pin<&mut Self>, context: &mut TaskContext<'_>) -> Poll<Self::Output> {
        self.state.completion.poll(context.waker())
    }
}

fn run_request(
    terminal: TerminalAdapter,
    chooser_executable: std::path::PathBuf,
    request: ChooserRequest,
    state: Arc<RequestState>,
) -> (u32, HashMap<String, OwnedValue>) {
    if state.cancelled() {
        return cancelled();
    }
    let endpoint = match ServiceEndpoint::bind() {
        Ok(endpoint) => endpoint,
        Err(_) => return failed(),
    };
    let child_argv = [
        chooser_executable.into_os_string(),
        "--portal-chooser".into(),
        "--socket".into(),
        endpoint.socket_path().as_os_str().to_owned(),
    ];
    let terminal_executable = match terminal.resolve_executable() {
        Some(path) => path,
        None => return failed(),
    };
    let child = match Command::new(terminal_executable)
        .args(terminal.chooser_args(&child_argv))
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return failed(),
    };
    *state.child.lock().expect("request child lock poisoned") = Some(child);
    if state.cancelled() {
        state.cancel();
        return cancelled();
    }

    let mut connection = match endpoint.accept_until(Instant::now() + HANDSHAKE_TIMEOUT) {
        Ok(connection) => connection,
        Err(_) => {
            return finish(
                &state,
                if state.cancelled() {
                    cancelled()
                } else {
                    failed()
                },
            );
        }
    };
    if state.cancelled() {
        return finish(&state, cancelled());
    }
    if connection.set_timeout(Some(HANDSHAKE_TIMEOUT)).is_err()
        || connection.send_request(&request).is_err()
        || connection.receive_ready().is_err()
        || connection.set_timeout(None).is_err()
    {
        return finish(
            &state,
            if state.cancelled() {
                cancelled()
            } else {
                failed()
            },
        );
    }
    match connection.cancellation_handle() {
        Ok(cancellation) => {
            *state
                .cancellation
                .lock()
                .expect("request cancellation lock poisoned") = Some(cancellation);
        }
        Err(_) => return finish(&state, failed()),
    }
    if state.cancelled() {
        state.cancel();
        return finish(&state, cancelled());
    }
    let result = connection.receive_result();
    if state.cancelled() {
        return finish(&state, cancelled());
    }
    finish(
        &state,
        match result {
            Ok(Message::Accepted { paths }) => accepted(paths, &request),
            Ok(Message::Cancelled) => cancelled(),
            Ok(Message::Error { .. }) | Err(_) => failed(),
            Ok(_) => failed(),
        },
    )
}

/// Keep the service's original launcher spelling so an executable replacement
/// cannot turn `/proc/self/exe` into a path ending in ` (deleted)`.
fn chooser_executable() -> Result<std::path::PathBuf> {
    chooser_executable_with_fallback(env::args_os().next(), || {
        env::current_exe()
            .context("error: could not determine an executable for launching the portal chooser")
    })
}

fn chooser_executable_with_fallback(
    invoked: Option<std::ffi::OsString>,
    fallback: impl FnOnce() -> Result<std::path::PathBuf>,
) -> Result<std::path::PathBuf> {
    invocation_launcher(invoked)
        .map(Ok)
        .unwrap_or_else(fallback)
}

fn invocation_launcher(invoked: Option<std::ffi::OsString>) -> Option<std::path::PathBuf> {
    let launcher = std::path::PathBuf::from(invoked?);
    (launcher.is_absolute() && is_executable_file(&launcher)).then_some(launcher)
}

fn is_executable_file(path: &Path) -> bool {
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

fn finish(
    state: &RequestState,
    response: (u32, HashMap<String, OwnedValue>),
) -> (u32, HashMap<String, OwnedValue>) {
    if response.0 == 0 {
        state.reap_child();
    } else {
        state.cancel();
    }
    response
}

fn accepted(paths: Vec<Vec<u8>>, request: &ChooserRequest) -> (u32, HashMap<String, OwnedValue>) {
    let multiple = matches!(
        request.mode,
        ChooserRequestMode::Open { multiple: true, .. }
    );
    if paths.is_empty()
        || (!multiple && paths.len() != 1)
        || paths.iter().any(|path| !matches_selection(path, request))
    {
        return failed();
    }
    let Some(uris) = paths
        .iter()
        .map(|path| file_uri(path))
        .collect::<Option<Vec<_>>>()
    else {
        return failed();
    };
    let mut results = HashMap::new();
    results.insert(
        "uris".to_owned(),
        OwnedValue::try_from(zbus::zvariant::Value::from(uris))
            .expect("a string array is a valid D-Bus value"),
    );
    (0, results)
}

fn matches_selection(path: &[u8], request: &ChooserRequest) -> bool {
    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt;
    #[cfg(unix)]
    let path = Path::new(std::ffi::OsStr::from_bytes(path));
    #[cfg(not(unix))]
    let path = Path::new(match std::str::from_utf8(path) {
        Ok(path) => path,
        Err(_) => return false,
    });
    match &request.mode {
        ChooserRequestMode::Open { kind, .. } => match std::fs::metadata(path) {
            Ok(metadata) => match kind {
                SelectionKind::File => metadata.is_file(),
                SelectionKind::Directory => metadata.is_dir(),
                SelectionKind::FileOrDirectory => metadata.is_file() || metadata.is_dir(),
            },
            Err(_) => false,
        },
        ChooserRequestMode::SaveFile { .. } => path.is_absolute(),
    }
}

fn option_bool(options: &HashMap<String, OwnedValue>, key: &str) -> Option<bool> {
    match options.get(key) {
        Some(value) => bool::try_from(value).ok(),
        None => Some(false),
    }
}

fn valid_ignored_options(options: &HashMap<String, OwnedValue>) -> bool {
    option_bool(options, "modal").is_some()
        && option_string(options, "accept_label").is_some()
        && option_filters(options, "filters").is_some()
        && option_filter(options, "current_filter").is_some()
}

fn option_filters(options: &HashMap<String, OwnedValue>, key: &str) -> Option<()> {
    let Some(value) = options.get(key) else {
        return Some(());
    };
    let filters = Vec::<Filter>::try_from(value.try_clone().ok()?).ok()?;
    (filters.len() <= MAX_FILTERS && filters.iter().all(valid_filter)).then_some(())
}

fn option_filter(options: &HashMap<String, OwnedValue>, key: &str) -> Option<()> {
    let Some(value) = options.get(key) else {
        return Some(());
    };
    let filter = Filter::try_from(value.try_clone().ok()?).ok()?;
    valid_filter(&filter).then_some(())
}

fn valid_filter((name, rules): &Filter) -> bool {
    name.len() <= MAX_TEXT_BYTES
        && rules.len() <= MAX_FILTER_RULES
        && rules
            .iter()
            .all(|(kind, value)| matches!(kind, 0 | 1) && value.len() <= MAX_TEXT_BYTES)
}

fn option_string(options: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    match options.get(key) {
        Some(value) => {
            let value = String::try_from(value.try_clone().ok()?).ok()?;
            (value.len() <= MAX_TEXT_BYTES).then_some(value)
        }
        None => Some(String::new()),
    }
}

fn option_path(options: &HashMap<String, OwnedValue>, key: &str) -> Option<Option<Vec<u8>>> {
    let Some(value) = options.get(key) else {
        return Some(None);
    };
    let mut bytes = Vec::<u8>::try_from(value.try_clone().ok()?).ok()?;
    if bytes.len() < 2 || bytes.len() > MAX_TEXT_BYTES || bytes.pop() != Some(0) {
        return None;
    }
    (!bytes.contains(&0) && bytes.starts_with(b"/")).then_some(Some(bytes))
}

fn save_startup(options: &HashMap<String, OwnedValue>) -> Option<(Option<Vec<u8>>, String)> {
    let current_folder = option_path(options, "current_folder")?;
    let current_name = option_string(options, "current_name")?;
    let current_file = option_path(options, "current_file")?;
    let current_file_parts = current_file.as_deref().and_then(current_file_parts);
    let initial_path = current_folder.or_else(|| {
        current_file_parts
            .as_ref()
            .map(|(parent, _)| parent.clone())
    });
    let initial_name = if current_name.is_empty() {
        current_file_parts
            .and_then(|(_, name)| std::str::from_utf8(name).ok().map(str::to_owned))
            .unwrap_or_default()
    } else {
        current_name
    };
    Some((initial_path, initial_name))
}

fn current_file_parts(path: &[u8]) -> Option<(Vec<u8>, &[u8])> {
    let slash = path.iter().rposition(|byte| *byte == b'/')?;
    let name = path.get(slash + 1..)?;
    (!name.is_empty()).then(|| {
        let parent = if slash == 0 {
            b"/".to_vec()
        } else {
            path[..slash].to_vec()
        };
        (parent, name)
    })
}

fn has_only_options(options: &HashMap<String, OwnedValue>, allowed: &[&str]) -> bool {
    options.len() <= MAX_OPTIONS && options.keys().all(|key| allowed.contains(&key.as_str()))
}

fn file_uri(path: &[u8]) -> Option<String> {
    if !path.starts_with(b"/") || path.contains(&0) {
        return None;
    }
    let mut uri = String::from("file://");
    for &byte in path {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'.' | b'_' | b'~') {
            uri.push(byte as char);
        } else {
            use std::fmt::Write;
            write!(uri, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    Some(uri)
}

fn failed() -> (u32, HashMap<String, OwnedValue>) {
    (2, HashMap::new())
}

fn cancelled() -> (u32, HashMap<String, OwnedValue>) {
    (1, HashMap::new())
}

#[cfg(test)]
#[path = "tests/service.rs"]
mod tests;
