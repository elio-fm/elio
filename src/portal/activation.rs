//! User-local portal descriptor and D-Bus activation management.
//!
//! This module intentionally does not select portal routing or restart the
//! xdg-desktop-portal frontend. It owns only the two generated backend files.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    env,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};
use zbus::{Connection, Proxy, fdo::DBusProxy};

const PORTAL_NAME: &str = "io.github.elio_fm.elio.Portal";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const FILE_CHOOSER_INTERFACE: &str = "org.freedesktop.impl.portal.FileChooser";
const STATE_FILE: &str = "portal-metadata.json";
const STATE_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct EnableResult {
    pub portal_path: PathBuf,
    pub service_path: PathBuf,
    pub launcher: PathBuf,
    pub created_portal: bool,
    pub created_service: bool,
}

#[derive(Clone, Debug)]
pub struct DisableResult {
    pub removed_portal: bool,
    pub removed_service: bool,
}

#[derive(Clone, Debug)]
pub struct Status {
    pub portal_path: PathBuf,
    pub service_path: PathBuf,
    pub state: &'static str,
}

#[derive(Clone, Debug)]
struct Paths {
    portal: PathBuf,
    service: PathBuf,
    state: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MetadataState {
    version: u32,
    phase: Phase,
    launcher: PathBuf,
    resolved_executable: PathBuf,
    portal: ArtifactState,
    service: ArtifactState,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Prepared,
    Committed,
    Removing,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ArtifactState {
    path: PathBuf,
    hash: String,
    created: bool,
}

pub(crate) fn enable() -> Result<EnableResult> {
    let paths = Paths::from_environment()?;
    let (launcher, resolved_executable) = durable_launcher()?;
    let portal_contents = portal_contents();
    let service_contents = service_contents(&launcher)?;
    let previous = load_state(&paths)?;
    if let Some(existing) = previous.as_ref()
        && existing.phase == Phase::Committed
    {
        verify_owned_state(existing, &paths)?;
    }

    // Persist the recovery record before touching either generated artifact.
    let pending = pending_state(
        previous.as_ref(),
        &paths,
        &launcher,
        &resolved_executable,
        portal_contents.as_bytes(),
        service_contents.as_bytes(),
    )?;
    write_state(&paths, &pending)?;
    let created_portal = materialize(
        &paths.portal,
        portal_contents.as_bytes(),
        Some(&pending.portal),
    )?;
    let created_service = materialize(
        &paths.service,
        service_contents.as_bytes(),
        Some(&pending.service),
    )?;

    let next = MetadataState {
        version: STATE_VERSION,
        phase: Phase::Committed,
        launcher: launcher.clone(),
        resolved_executable,
        portal: ArtifactState {
            path: paths.portal.clone(),
            hash: hash(portal_contents.as_bytes()),
            created: pending.portal.created || created_portal,
        },
        service: ArtifactState {
            path: paths.service.clone(),
            hash: hash(service_contents.as_bytes()),
            created: pending.service.created || created_service,
        },
    };
    write_state(&paths, &next)?;
    reload_activate_and_introspect()?;

    Ok(EnableResult {
        portal_path: paths.portal,
        service_path: paths.service,
        launcher,
        created_portal,
        created_service,
    })
}

#[cfg(test)]
#[path = "tests/activation.rs"]
mod tests;

pub(crate) fn disable() -> Result<DisableResult> {
    let paths = Paths::from_environment()?;
    disable_at(&paths, terminate_portal_owner)
}

fn disable_at(paths: &Paths, terminate: impl FnOnce() -> Result<()>) -> Result<DisableResult> {
    let Some(mut state) = load_state(paths)? else {
        return Ok(DisableResult {
            removed_portal: false,
            removed_service: false,
        });
    };
    if state.phase == Phase::Prepared {
        anyhow::bail!(
            "error: portal metadata update is incomplete; rerun `elio portal enable` to recover it"
        )
    }
    terminate()?;
    let resuming_removal = state.phase == Phase::Removing;
    state.phase = Phase::Removing;
    write_state(paths, &state)?;
    let removed_service = remove_owned(&state.service, resuming_removal)?;
    state.service.created = false;
    write_state(paths, &state)?;
    let removed_portal = remove_owned(&state.portal, resuming_removal)?;
    state.portal.created = false;
    write_state(paths, &state)?;
    remove_file_if_regular_owned(&paths.state)?;
    Ok(DisableResult {
        removed_portal,
        removed_service,
    })
}

pub(crate) fn status() -> Result<Status> {
    let paths = Paths::from_environment()?;
    let state = match load_state(&paths)? {
        None => "not installed",
        Some(state) if state.phase == Phase::Committed => {
            verify_owned_state(&state, &paths)?;
            "installed"
        }
        Some(_) => "recovery required",
    };
    Ok(Status {
        portal_path: paths.portal,
        service_path: paths.service,
        state,
    })
}

fn pending_state(
    previous: Option<&MetadataState>,
    paths: &Paths,
    launcher: &Path,
    resolved_executable: &Path,
    portal_contents: &[u8],
    service_contents: &[u8],
) -> Result<MetadataState> {
    let (portal, service) = match previous {
        Some(state) => (state.portal.clone(), state.service.clone()),
        None => (
            pending_artifact(&paths.portal, portal_contents)?,
            pending_artifact(&paths.service, service_contents)?,
        ),
    };
    Ok(MetadataState {
        version: STATE_VERSION,
        phase: Phase::Prepared,
        launcher: launcher.to_path_buf(),
        resolved_executable: resolved_executable.to_path_buf(),
        portal,
        service,
    })
}

fn pending_artifact(path: &Path, contents: &[u8]) -> Result<ArtifactState> {
    match read_regular_owned(path)? {
        Some(existing) if existing == contents => Ok(ArtifactState {
            path: path.to_path_buf(),
            hash: hash(contents),
            created: false,
        }),
        Some(_) => anyhow::bail!(
            "error: refusing to overwrite unmanaged portal metadata {}",
            path.display()
        ),
        None => Ok(ArtifactState {
            path: path.to_path_buf(),
            hash: hash(contents),
            created: true,
        }),
    }
}

impl Paths {
    fn from_environment() -> Result<Self> {
        let data_home = absolute_xdg_home("XDG_DATA_HOME", ".local/share")?;
        let state_home = absolute_xdg_home("XDG_STATE_HOME", ".local/state")?;
        Ok(Self {
            portal: data_home.join("xdg-desktop-portal/portals/elio.portal"),
            service: data_home.join("dbus-1/services/io.github.elio_fm.elio.Portal.service"),
            state: state_home.join("elio").join(STATE_FILE),
        })
    }
}

fn absolute_xdg_home(variable: &str, fallback: &str) -> Result<PathBuf> {
    match env::var_os(variable) {
        Some(path) => {
            let path = PathBuf::from(path);
            if !path.is_absolute() {
                anyhow::bail!("error: ${variable} must be an absolute path")
            }
            Ok(path)
        }
        None => env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .map(|home| home.join(fallback))
            .context(format!(
                "error: could not find ${variable} or an absolute $HOME"
            )),
    }
}

fn durable_launcher() -> Result<(PathBuf, PathBuf)> {
    let resolved =
        fs::canonicalize(env::current_exe().context("error: could not resolve elio executable")?)
            .context("error: could not canonicalize elio executable")?;
    let invoked = env::args_os()
        .next()
        .context("error: could not determine the elio launcher spelling")?;
    let launcher = launcher_candidates(&invoked)?
        .into_iter()
        .find(|candidate| valid_launcher(candidate, &resolved))
        .context("error: could not establish a durable absolute elio launcher; install elio and invoke it through its absolute launcher path")?;
    if launcher.starts_with("/nix/store") {
        anyhow::bail!(
            "error: refusing transient /nix/store launcher; invoke elio through a profile launcher"
        )
    }
    Ok((launcher, resolved))
}

fn launcher_candidates(invoked: &OsString) -> Result<Vec<PathBuf>> {
    let invoked = PathBuf::from(invoked);
    if invoked.is_absolute() {
        return Ok(vec![invoked]);
    }
    let Some(name) = invoked.file_name() else {
        return Ok(Vec::new());
    };
    let Some(path) = env::var_os("PATH") else {
        return Ok(Vec::new());
    };
    Ok(env::split_paths(&path)
        .filter(|entry| entry.is_absolute())
        .map(|entry| entry.join(name))
        .collect())
}

fn valid_launcher(candidate: &Path, resolved: &Path) -> bool {
    let Ok(metadata) = fs::metadata(candidate) else {
        return false;
    };
    metadata.is_file()
        && metadata.permissions().mode() & 0o111 != 0
        && fs::canonicalize(candidate).is_ok_and(|path| path == resolved)
}

fn portal_contents() -> &'static str {
    "[portal]\nDBusName=io.github.elio_fm.elio.Portal\nInterfaces=org.freedesktop.impl.portal.FileChooser;\n"
}

fn service_contents(launcher: &Path) -> Result<String> {
    let launcher = launcher
        .to_str()
        .context("error: durable elio launcher is not valid UTF-8")?;
    if launcher
        .bytes()
        .any(|byte| byte.is_ascii_whitespace() || matches!(byte, b'\'' | b'\"' | b'\\'))
    {
        anyhow::bail!(
            "error: durable elio launcher contains characters unsupported by D-Bus service Exec="
        )
    }
    Ok(format!(
        "[D-BUS Service]\nName={PORTAL_NAME}\nExec={launcher} --portal-service\n"
    ))
}

fn load_state(paths: &Paths) -> Result<Option<MetadataState>> {
    let Some(contents) = read_regular_owned(&paths.state)? else {
        return Ok(None);
    };
    let state = serde_json::from_slice::<MetadataState>(&contents).with_context(|| {
        format!(
            "error: invalid portal metadata state {}",
            paths.state.display()
        )
    })?;
    if state.version != STATE_VERSION {
        anyhow::bail!("error: unsupported portal metadata state version")
    }
    Ok(Some(state))
}

fn write_state(paths: &Paths, state: &MetadataState) -> Result<()> {
    ensure_private_dir(paths.state.parent().expect("state has parent"))?;
    let contents = serde_json::to_vec_pretty(state).expect("metadata state serializes");
    atomic_write(&paths.state, &contents, 0o600)
}

fn materialize(path: &Path, contents: &[u8], owned: Option<&ArtifactState>) -> Result<bool> {
    ensure_parent(path)?;
    match read_regular_owned(path)? {
        Some(existing) => {
            if existing == contents {
                return Ok(false);
            }
            if let Some(owned) = owned
                && owned.created
                && hash(&existing) == owned.hash
            {
                atomic_write(path, contents, 0o644)?;
                return Ok(false);
            }
            anyhow::bail!(
                "error: refusing to overwrite unmanaged portal metadata {}",
                path.display()
            )
        }
        None => {
            if owned.is_some_and(|artifact| !artifact.created) {
                anyhow::bail!(
                    "error: externally managed portal metadata disappeared: {}",
                    path.display()
                )
            }
            atomic_write(path, contents, 0o644)?;
            Ok(true)
        }
    }
}

fn verify_owned_state(state: &MetadataState, paths: &Paths) -> Result<()> {
    if state.portal.path != paths.portal || state.service.path != paths.service {
        anyhow::bail!("error: portal metadata state belongs to different XDG paths")
    }
    verify_artifact(&state.portal)?;
    verify_artifact(&state.service)
}

fn verify_artifact(artifact: &ArtifactState) -> Result<()> {
    let Some(contents) = read_regular_owned(&artifact.path)? else {
        anyhow::bail!(
            "error: missing Elio-owned portal metadata: {}",
            artifact.path.display()
        )
    };
    if hash(&contents) != artifact.hash {
        anyhow::bail!(
            "error: portal metadata was modified outside elio: {}",
            artifact.path.display()
        )
    }
    Ok(())
}

fn remove_owned(artifact: &ArtifactState, allow_missing: bool) -> Result<bool> {
    if !artifact.created {
        return Ok(false);
    }
    if read_regular_owned(&artifact.path)?.is_none() && allow_missing {
        return Ok(false);
    }
    verify_artifact(artifact)?;
    fs::remove_file(&artifact.path)
        .with_context(|| format!("error: failed to remove {}", artifact.path.display()))?;
    sync_parent(artifact.path.parent().expect("artifact has parent"))?;
    Ok(true)
}

fn remove_file_if_regular_owned(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.file_type().is_file() && metadata.uid() == unsafe { libc::geteuid() } =>
        {
            fs::remove_file(path)
                .with_context(|| format!("error: failed to remove {}", path.display()))?;
            sync_parent(path.parent().expect("state has parent"))
        }
        Ok(_) => anyhow::bail!(
            "error: refusing to remove unsafe portal state {}",
            path.display()
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("error: failed to inspect {}", path.display()))
        }
    }
}

fn ensure_parent(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .context("error: metadata path has no parent")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("error: failed to create {}", parent.display()))?;
    let metadata = fs::symlink_metadata(parent)
        .with_context(|| format!("error: failed to inspect {}", parent.display()))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        anyhow::bail!(
            "error: unsafe portal metadata directory {}",
            parent.display()
        )
    }
    Ok(())
}

fn ensure_private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .with_context(|| format!("error: failed to create {}", path.display()))?;
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("error: failed to inspect {}", path.display()))?;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { libc::geteuid() }
    {
        anyhow::bail!("error: unsafe portal state directory {}", path.display())
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("error: failed to secure {}", path.display()))
}

fn read_regular_owned(path: &Path) -> Result<Option<Vec<u8>>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("error: failed to open {}", path.display()));
        }
    };
    let metadata = file
        .metadata()
        .with_context(|| format!("error: failed to inspect {}", path.display()))?;
    if !metadata.is_file() || metadata.uid() != unsafe { libc::geteuid() } {
        anyhow::bail!("error: unsafe portal metadata path {}", path.display())
    }
    let mut contents = Vec::new();
    std::io::Read::read_to_end(&mut std::io::BufReader::new(file), &mut contents)
        .with_context(|| format!("error: failed to read {}", path.display()))?;
    Ok(Some(contents))
}

fn atomic_write(path: &Path, contents: &[u8], mode: u32) -> Result<()> {
    let parent = path
        .parent()
        .context("error: metadata path has no parent")?;
    let name = path
        .file_name()
        .context("error: metadata path has no file name")?
        .to_string_lossy();
    for attempt in 0..100 {
        let temporary = parent.join(format!(".{name}.elio-tmp-{}-{attempt}", std::process::id()));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("error: failed to create {}", temporary.display()));
            }
        };
        let result = (|| -> Result<()> {
            file.write_all(contents)
                .with_context(|| format!("error: failed to write {}", temporary.display()))?;
            file.sync_all()
                .with_context(|| format!("error: failed to sync {}", temporary.display()))?;
            drop(file);
            fs::rename(&temporary, path)
                .with_context(|| format!("error: failed to replace {}", path.display()))?;
            fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
            sync_parent(parent)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        return result;
    }
    anyhow::bail!("error: failed to create a temporary portal metadata file")
}

fn sync_parent(path: &Path) -> Result<()> {
    match File::open(path)?.sync_all() {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::InvalidInput | io::ErrorKind::Unsupported
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn hash(contents: &[u8]) -> String {
    blake3::hash(contents).to_hex().to_string()
}

const OWNER_HANDOFF_TIMEOUT: Duration = Duration::from_secs(5);
const OWNER_HANDOFF_POLL_INTERVAL: Duration = Duration::from_millis(50);
const ACTIVATION_METHOD_TIMEOUT: Duration = Duration::from_secs(5);
#[derive(Debug, Eq, PartialEq)]
enum OwnerHandoff {
    Released,
    StillOwned,
    UnexpectedOwner(String),
}

fn classify_owner_handoff(previous: &str, current: Option<&str>) -> OwnerHandoff {
    match current {
        None => OwnerHandoff::Released,
        Some(current) if current == previous => OwnerHandoff::StillOwned,
        Some(current) => OwnerHandoff::UnexpectedOwner(current.to_string()),
    }
}

fn is_new_owner(previous: Option<&str>, current: Option<&str>) -> bool {
    current.is_some_and(|current| previous != Some(current))
}

fn service_started(reply: zbus::fdo::StartServiceReply) -> bool {
    reply == zbus::fdo::StartServiceReply::Success
}

fn owner_uid_matches(owner_uid: u32, effective_uid: u32) -> bool {
    owner_uid == effective_uid
}

fn portal_owner_name(name: &str) -> zbus::names::BusName<'_> {
    name.try_into().expect("valid D-Bus name")
}

async fn activation_connection(context: &'static str) -> Result<Connection> {
    zbus::connection::Builder::session()
        .method_timeout(ACTIVATION_METHOD_TIMEOUT)
        .build()
        .await
        .context(context)
}

fn get_name_owner_result<T: ToString>(result: zbus::fdo::Result<T>) -> Result<Option<String>> {
    match result {
        Ok(owner) => Ok(Some(owner.to_string())),
        Err(zbus::fdo::Error::NameHasNoOwner(_)) => Ok(None),
        Err(error) => Err(error).context("error: could not read the elio portal owner"),
    }
}

async fn portal_owner(bus: &DBusProxy<'_>) -> Result<Option<String>> {
    if !bus
        .name_has_owner(portal_owner_name(PORTAL_NAME))
        .await
        .context("error: could not query the elio portal owner")?
    {
        return Ok(None);
    }
    get_name_owner_result(bus.get_name_owner(portal_owner_name(PORTAL_NAME)).await)
}

fn terminate_portal_owner() -> Result<()> {
    zbus::block_on(async {
        let connection = activation_connection(
            "error: could not connect to the session D-Bus to stop the elio portal service",
        )
        .await?;
        let bus = DBusProxy::new(&connection).await?;
        terminate_current_portal_owner(&bus).await?;
        Ok(())
    })
}

async fn terminate_current_portal_owner(bus: &DBusProxy<'_>) -> Result<Option<String>> {
    let Some(owner) = portal_owner(bus).await? else {
        return Ok(None);
    };
    let owner_name = portal_owner_name(&owner);
    let owner_uid = bus
        .get_connection_unix_user(owner_name.clone())
        .await
        .context("error: could not read the elio portal owner's Unix UID")?;
    let effective_uid = unsafe { libc::geteuid() };
    if !owner_uid_matches(owner_uid, effective_uid) {
        anyhow::bail!(
            "error: refusing to replace elio portal owner {owner}: Unix UID {owner_uid} does not match effective UID {effective_uid}"
        )
    }
    let pid = bus
        .get_connection_unix_process_id(owner_name)
        .await
        .context("error: could not read the elio portal owner's Unix PID")?;
    if portal_owner(bus).await?.as_deref() != Some(owner.as_str()) {
        anyhow::bail!(
            "error: elio portal owner changed before replacement; rerun `elio portal enable`"
        )
    }
    let pid = libc::pid_t::try_from(pid)
        .context("error: elio portal owner's Unix PID does not fit pid_t")?;
    if pid <= 0 {
        anyhow::bail!("error: elio portal owner reported an invalid Unix PID")
    }
    if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
        return Err(io::Error::last_os_error())
            .context("error: could not SIGTERM the current elio portal owner");
    }
    wait_for_owner_release(bus, &owner).await?;
    Ok(Some(owner))
}

async fn wait_for_owner_release(bus: &DBusProxy<'_>, previous: &str) -> Result<()> {
    let deadline = Instant::now() + OWNER_HANDOFF_TIMEOUT;
    loop {
        match classify_owner_handoff(previous, portal_owner(bus).await?.as_deref()) {
            OwnerHandoff::Released => return Ok(()),
            OwnerHandoff::UnexpectedOwner(owner) => anyhow::bail!(
                "error: unexpected D-Bus owner {owner} took {PORTAL_NAME} while replacing {previous}"
            ),
            OwnerHandoff::StillOwned if Instant::now() >= deadline => anyhow::bail!(
                "error: elio portal owner {previous} did not release {PORTAL_NAME} within {} seconds after SIGTERM",
                OWNER_HANDOFF_TIMEOUT.as_secs()
            ),
            OwnerHandoff::StillOwned => thread::sleep(OWNER_HANDOFF_POLL_INTERVAL),
        }
    }
}

async fn wait_for_new_owner(bus: &DBusProxy<'_>, previous: Option<&str>) -> Result<String> {
    let deadline = Instant::now() + OWNER_HANDOFF_TIMEOUT;
    loop {
        let owner = portal_owner(bus).await?;
        if is_new_owner(previous, owner.as_deref()) {
            return Ok(owner.expect("new owner exists"));
        }
        if Instant::now() >= deadline {
            anyhow::bail!(
                "error: elio portal service did not acquire a new D-Bus owner within {} seconds",
                OWNER_HANDOFF_TIMEOUT.as_secs()
            )
        }
        thread::sleep(OWNER_HANDOFF_POLL_INTERVAL);
    }
}

fn file_chooser_introspection_timed_out(error: &zbus::fdo::Error) -> bool {
    matches!(
        error,
        zbus::fdo::Error::ZBus(zbus::Error::InputOutput(error))
            if error.kind() == io::ErrorKind::TimedOut
    )
}

fn file_chooser_introspection_result(result: zbus::fdo::Result<String>) -> Result<String> {
    match result {
        Ok(xml) => Ok(xml),
        Err(error) if file_chooser_introspection_timed_out(&error) => anyhow::bail!(
            "error: FileChooser introspection timed out after {} seconds",
            ACTIVATION_METHOD_TIMEOUT.as_secs()
        ),
        Err(error) => Err(error).context("error: could not introspect elio portal service"),
    }
}

fn owner_matches(expected: &str, current: Option<&str>) -> bool {
    current == Some(expected)
}

async fn retry_file_chooser_introspection(expected_owner: &str) -> Result<String> {
    let connection = activation_connection(
        "error: could not connect to the session D-Bus for FileChooser introspection retry",
    )
    .await?;
    let bus = DBusProxy::new(&connection).await?;
    let owner = portal_owner(&bus).await?;
    if !owner_matches(expected_owner, owner.as_deref()) {
        anyhow::bail!("error: elio portal owner changed before FileChooser introspection retry")
    }
    let proxy = Proxy::new(
        &connection,
        PORTAL_NAME,
        PORTAL_PATH,
        FILE_CHOOSER_INTERFACE,
    )
    .await?;
    file_chooser_introspection_result(proxy.introspect().await)
}

fn reload_activate_and_introspect() -> Result<()> {
    zbus::block_on(async {
        let connection =
            activation_connection("error: could not connect to the session D-Bus").await?;
        let bus = DBusProxy::new(&connection).await?;
        bus.reload_config()
            .await
            .context("error: D-Bus ReloadConfig failed")?;
        let previous_owner = terminate_current_portal_owner(&bus).await?;
        let reply = bus
            .start_service_by_name(PORTAL_NAME.try_into().expect("valid D-Bus name"), 0)
            .await
            .context("error: could not activate elio portal service")?;
        if !service_started(reply) {
            anyhow::bail!(
                "error: elio portal service was already running after replacement; refusing to use an unexpected owner"
            )
        }
        let owner = wait_for_new_owner(&bus, previous_owner.as_deref()).await?;
        let proxy = Proxy::new(
            &connection,
            PORTAL_NAME,
            PORTAL_PATH,
            FILE_CHOOSER_INTERFACE,
        )
        .await?;
        let xml = match proxy.introspect().await {
            Ok(xml) => xml,
            Err(error) if file_chooser_introspection_timed_out(&error) => {
                retry_file_chooser_introspection(&owner).await?
            }
            Err(error) => {
                return Err(error).context("error: could not introspect elio portal service");
            }
        };
        if !xml.contains(&format!("interface name=\"{FILE_CHOOSER_INTERFACE}\"")) {
            anyhow::bail!(
                "error: activated elio portal service does not expose {FILE_CHOOSER_INTERFACE}"
            )
        }
        Ok(())
    })
}
