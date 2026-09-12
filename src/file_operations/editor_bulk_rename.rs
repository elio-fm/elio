use super::FileOperationsState;
use super::bulk_rename::BulkRenameItem;
#[cfg(unix)]
use anyhow::Context;
use anyhow::{Result, bail};
use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg(unix)]
pub(crate) struct BulkRenameEditorSession {
    pub(crate) root: PathBuf,
    pub(crate) temp_path: PathBuf,
    pub(crate) expected_temp_owner: Option<libc::uid_t>,
    pub(crate) items: Vec<BulkRenameItem>,
}

pub(crate) struct EditorRenameConfirmOverlay {
    pub(crate) items: Vec<BulkRenameItem>,
    pub(crate) new_names: Vec<String>,
    pub(crate) root: PathBuf,
    pub(crate) scroll: usize,
    pub(crate) confirmed: bool,
}
#[cfg(unix)]
use std::{
    env,
    io::{Read, Write},
    os::unix::{fs::OpenOptionsExt, io::AsRawFd},
};

#[cfg(unix)]
const MAX_EDITOR_RENAME_BYTES: u64 = 1024 * 1024;

#[cfg(unix)]
pub(crate) struct EditorBulkRenameLaunch {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) session: BulkRenameEditorSession,
}

#[cfg(unix)]
pub(crate) enum EditorRenameReview {
    Ready,
    Status(String),
}

pub(crate) enum BulkRenameConfirmation {
    None,
    Status(String),
    Applied(BulkRenameCompletion),
}

pub(crate) struct BulkRenameCompletion {
    pub(crate) changed_old_paths: Vec<PathBuf>,
    pub(crate) duplicate_rename_pairs: Vec<(PathBuf, PathBuf)>,
    pub(crate) reselect_path: Option<PathBuf>,
    pub(crate) status: String,
}

impl FileOperationsState {
    pub(crate) fn editor_rename_confirm_overlay_mut(
        &mut self,
    ) -> Option<&mut EditorRenameConfirmOverlay> {
        self.editor_rename_confirm.as_mut()
    }

    #[cfg(unix)]
    pub(crate) fn prepare_editor_bulk_rename(
        &mut self,
        selected_paths: Vec<PathBuf>,
    ) -> Result<EditorBulkRenameLaunch> {
        let root = common_root(&selected_paths);
        let rows = selected_paths
            .iter()
            .map(|path| {
                path.strip_prefix(&root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();
        let invoking_user = editor_temp_owner(crate::elevated_session::context())?;
        let expected_temp_owner = invoking_user.map(|(uid, _)| uid);
        let temp_path = create_temp_file(&rows, invoking_user)?;
        let (program, mut args) = editor_command();
        args.push(temp_path.to_string_lossy().into_owned());

        self.close_rename_overlays();
        Ok(EditorBulkRenameLaunch {
            program,
            args,
            session: BulkRenameEditorSession {
                root,
                temp_path,
                expected_temp_owner,
                items: selected_paths
                    .into_iter()
                    .map(bulk_rename_item_from_path)
                    .collect(),
            },
        })
    }

    #[cfg(unix)]
    pub(crate) fn finish_editor_bulk_rename(
        &mut self,
        session: BulkRenameEditorSession,
        launch_result: std::io::Result<std::process::ExitStatus>,
    ) -> Result<EditorRenameReview> {
        let BulkRenameEditorSession {
            root,
            temp_path,
            expected_temp_owner,
            items,
        } = session;

        let result = (|| -> Result<EditorRenameReview> {
            match launch_result {
                Ok(status) if status.success() => {}
                Ok(status) => {
                    return Ok(EditorRenameReview::Status(format!(
                        "Editor exited with {status}"
                    )));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(EditorRenameReview::Status("Editor not found".to_string()));
                }
                Err(error) => {
                    return Ok(EditorRenameReview::Status(format!(
                        "Could not run editor: {error}"
                    )));
                }
            }

            let edited = read_editor_rename_file(&temp_path, expected_temp_owner)
                .with_context(|| format!("failed to read {}", temp_path.display()))?;
            let mut new_rows: Vec<String> = edited.lines().map(str::to_owned).collect();
            if edited.ends_with('\n') && new_rows.last().is_some_and(String::is_empty) {
                new_rows.pop();
            }

            if new_rows.len() != items.len() {
                return Ok(EditorRenameReview::Status(format!(
                    "Editor rename aborted: expected {} line{}, got {}",
                    items.len(),
                    if items.len() == 1 { "" } else { "s" },
                    new_rows.len()
                )));
            }

            let original_rows = items
                .iter()
                .map(|item| {
                    item.path
                        .strip_prefix(&root)
                        .unwrap_or(&item.path)
                        .to_string_lossy()
                        .into_owned()
                })
                .collect::<Vec<_>>();
            if new_rows == original_rows {
                return Ok(EditorRenameReview::Status("No files renamed".to_string()));
            }

            match build_rename_plan(&items, &new_rows, Some(&root)) {
                Ok(plan) if plan.is_empty() => {
                    Ok(EditorRenameReview::Status("No files renamed".to_string()))
                }
                Ok(_) => {
                    self.editor_rename_confirm = Some(EditorRenameConfirmOverlay {
                        items,
                        new_names: new_rows,
                        root,
                        scroll: 0,
                        confirmed: true,
                    });
                    Ok(EditorRenameReview::Ready)
                }
                Err(errors) => Ok(EditorRenameReview::Status(editor_validation_status(
                    &errors,
                ))),
            }
        })();

        let _ = fs::remove_file(&temp_path);
        result
    }

    pub(crate) fn confirm_editor_rename(&mut self) -> BulkRenameConfirmation {
        let Some(overlay) = &self.editor_rename_confirm else {
            return BulkRenameConfirmation::None;
        };
        let plan = match build_rename_plan(&overlay.items, &overlay.new_names, Some(&overlay.root))
        {
            Ok(plan) => plan,
            Err(errors) => {
                return BulkRenameConfirmation::Status(editor_validation_status(&errors));
            }
        };
        match apply_rename_plan(&plan) {
            Ok(completion) => {
                self.editor_rename_confirm = None;
                BulkRenameConfirmation::Applied(completion)
            }
            Err(error) => BulkRenameConfirmation::Status(error.to_string()),
        }
    }

    #[cfg(unix)]
    fn close_rename_overlays(&mut self) {
        self.create = None;
        self.rename = None;
        self.trash = None;
        self.restore = None;
        self.bulk_rename = None;
        self.editor_rename_confirm = None;
    }
}

pub(super) fn confirm_bulk_rename_overlay(
    overlay: &mut Option<super::bulk_rename::BulkRenameOverlay>,
) -> BulkRenameConfirmation {
    let Some(rename) = overlay else {
        return BulkRenameConfirmation::None;
    };
    let plan = build_rename_plan(&rename.items, &rename.new_names, rename.root.as_deref());
    let ops = match plan {
        Ok(ops) => ops,
        Err(errors) => {
            if let Some(error_line) = errors.iter().position(Option::is_some) {
                rename.line_errors = errors;
                rename.cursor_line = error_line;
                rename.cursor_col = rename
                    .cursor_col
                    .min(rename.new_names[error_line].chars().count());
                rename.preferred_col = rename.cursor_col;
            }
            return BulkRenameConfirmation::None;
        }
    };
    match apply_rename_plan(&ops) {
        Ok(completion) => {
            *overlay = None;
            BulkRenameConfirmation::Applied(completion)
        }
        Err(error) => BulkRenameConfirmation::Status(error.to_string()),
    }
}

fn apply_rename_plan(ops: &[RenameOp]) -> Result<BulkRenameCompletion> {
    let changed_old_paths = ops.iter().map(|op| op.old_path.clone()).collect();
    let duplicate_rename_pairs = ops
        .iter()
        .map(|op| (op.old_path.clone(), op.new_path.clone()))
        .collect();
    let applied = apply_rename_ops(ops)?;
    Ok(BulkRenameCompletion {
        changed_old_paths,
        duplicate_rename_pairs,
        reselect_path: applied.last_new_path.clone(),
        status: rename_status(ops, &applied),
    })
}

fn editor_validation_status(errors: &[Option<String>]) -> String {
    errors
        .iter()
        .enumerate()
        .find_map(|(index, error)| {
            error
                .as_ref()
                .map(|error| format!("Editor rename aborted: line {}: {}", index + 1, error))
        })
        .unwrap_or_else(|| "Editor rename aborted".to_string())
}

#[derive(Clone, Debug)]
struct RenameOp {
    old_path: PathBuf,
    original_label: String,
    new_label: String,
    new_path: PathBuf,
}

#[derive(Debug, Default)]
struct AppliedRenames {
    renamed: usize,
    last_new_path: Option<PathBuf>,
}

struct StagedRename<'a> {
    op: &'a RenameOp,
    temp_path: PathBuf,
    temp_dir: PathBuf,
}

fn build_rename_plan(
    items: &[BulkRenameItem],
    new_names: &[String],
    root: Option<&Path>,
) -> std::result::Result<Vec<RenameOp>, Vec<Option<String>>> {
    let count = items.len();
    let mut errors = vec![None; count];
    if new_names.len() != count {
        if !errors.is_empty() {
            errors[0] = Some(format!(
                "Expected {} name{}, got {}",
                count,
                if count == 1 { "" } else { "s" },
                new_names.len()
            ));
        }
        return Err(errors);
    }

    let renaming_paths: HashSet<&Path> = items.iter().map(|item| item.path.as_path()).collect();
    let mut seen_new_paths = HashSet::new();
    let mut first_error = None;

    for (index, (item, new_name)) in items.iter().zip(new_names.iter()).enumerate() {
        let new_name = normalized_new_name(new_name, root);
        let target = match target_path(item, new_name, root) {
            Ok(path) => path,
            Err(message) => {
                errors[index] = Some(message);
                first_error.get_or_insert(index);
                continue;
            }
        };

        let err = if !seen_new_paths.insert(target.clone()) {
            Some(format!("\"{}\" appears more than once", new_name))
        } else if item.is_dir && target.starts_with(&item.path) && target != item.path {
            Some("Cannot move a folder inside itself".to_string())
        } else if target.parent().is_some_and(|parent| !parent.is_dir()) {
            Some("Destination folder does not exist".to_string())
        } else if target.exists() && !renaming_paths.contains(target.as_path()) {
            Some(format!("\"{}\" already exists", new_name))
        } else {
            None
        };

        if let Some(message) = err {
            errors[index] = Some(message);
            first_error.get_or_insert(index);
        }
    }

    if first_error.is_some() {
        return Err(errors);
    }

    Ok(items
        .iter()
        .zip(new_names.iter())
        .filter_map(|(item, new_name)| {
            let new_name = normalized_new_name(new_name, root);
            let new_path = target_path(item, new_name, root).ok()?;
            (new_path != item.path).then(|| RenameOp {
                old_path: item.path.clone(),
                original_label: display_label(item, root),
                new_label: new_name.to_string(),
                new_path,
            })
        })
        .collect())
}

fn normalized_new_name<'a>(new_name: &'a str, root: Option<&Path>) -> &'a str {
    if root.is_some() {
        new_name
    } else {
        new_name.trim()
    }
}

fn target_path(
    item: &BulkRenameItem,
    new_name: &str,
    root: Option<&Path>,
) -> std::result::Result<PathBuf, String> {
    if new_name.is_empty() {
        return Err("Name cannot be empty".to_string());
    }

    if let Some(root) = root {
        validate_relative_path(new_name)?;
        return Ok(root.join(new_name));
    }

    if new_name.contains('/') {
        return Err("Name cannot contain /".to_string());
    }
    Ok(renamed_path(&item.path, new_name))
}

fn validate_relative_path(value: &str) -> std::result::Result<(), String> {
    let path = Path::new(value);
    if path.is_absolute() {
        return Err("Path must be relative".to_string());
    }
    let mut saw_component = false;
    for component in path.components() {
        match component {
            Component::Normal(part) if !part.is_empty() => saw_component = true,
            _ => return Err("Path cannot contain . or ..".to_string()),
        }
    }
    if saw_component {
        Ok(())
    } else {
        Err("Name cannot be empty".to_string())
    }
}

fn apply_rename_ops(ops: &[RenameOp]) -> Result<AppliedRenames> {
    if ops.is_empty() {
        return Ok(AppliedRenames::default());
    }

    let mut staged = Vec::with_capacity(ops.len());
    for (index, op) in ops.iter().enumerate() {
        let temp_dir = unique_temp_sibling_dir(&op.old_path, index)?;
        let temp_path = temp_dir.join(path_name(&op.old_path));
        if let Err(error) = fs::rename(&op.old_path, &temp_path) {
            rollback_staged(&staged);
            let _ = fs::remove_dir(&temp_dir);
            bail!("Could not rename \"{}\": {error}", op.original_label);
        }
        staged.push(StagedRename {
            op,
            temp_path,
            temp_dir,
        });
    }

    let mut applied = AppliedRenames::default();
    let mut applied_ops: Vec<&RenameOp> = Vec::with_capacity(ops.len());
    for staged_rename in &staged {
        let op = staged_rename.op;
        if let Err(error) = fs::rename(&staged_rename.temp_path, &op.new_path) {
            rollback_applied(&applied_ops);
            rollback_staged_remaining(&staged, applied_ops.len());
            bail!("Could not rename \"{}\": {error}", op.original_label);
        }
        let _ = fs::remove_dir(&staged_rename.temp_dir);
        applied_ops.push(op);
        applied.renamed += 1;
        applied.last_new_path = Some(op.new_path.clone());
    }
    Ok(applied)
}

fn rollback_staged(staged: &[StagedRename<'_>]) {
    for staged_rename in staged.iter().rev() {
        let _ = fs::rename(&staged_rename.temp_path, &staged_rename.op.old_path);
        let _ = fs::remove_dir(&staged_rename.temp_dir);
    }
}

fn rollback_staged_remaining(staged: &[StagedRename<'_>], start: usize) {
    for staged_rename in staged.iter().skip(start).rev() {
        let _ = fs::rename(&staged_rename.temp_path, &staged_rename.op.old_path);
        let _ = fs::remove_dir(&staged_rename.temp_dir);
    }
}

fn rollback_applied(applied: &[&RenameOp]) {
    for op in applied.iter().rev() {
        let _ = fs::rename(&op.new_path, &op.old_path);
    }
}

fn unique_temp_sibling_dir(path: &Path, index: usize) -> Result<PathBuf> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    for attempt in 0..1000usize {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let candidate = parent.join(format!(
            ".elio-rename-{}-{now}-{index}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&candidate) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&candidate, fs::Permissions::from_mode(0o700));
                }
                return Ok(candidate);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    bail!(
        "Could not create temporary rename path for {}",
        path.display()
    )
}

fn rename_status(ops: &[RenameOp], applied: &AppliedRenames) -> String {
    match applied.renamed {
        0 => "No files renamed".to_string(),
        1 => {
            let op = ops.first().expect("single rename op should exist");
            format!("Renamed \"{}\" → \"{}\"", op.original_label, op.new_label)
        }
        n => format!("Renamed {} items", n),
    }
}

fn display_label(item: &BulkRenameItem, root: Option<&Path>) -> String {
    root.and_then(|root| item.path.strip_prefix(root).ok())
        .unwrap_or_else(|| Path::new(&item.original_name))
        .to_string_lossy()
        .into_owned()
}

#[cfg(unix)]
fn bulk_rename_item_from_path(path: PathBuf) -> BulkRenameItem {
    let original_name = path_name(&path);
    let is_dir = path.is_dir();
    BulkRenameItem {
        path,
        original_name,
        is_dir,
    }
}

fn path_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

fn renamed_path(path: &Path, new_name: &str) -> PathBuf {
    path.parent()
        .map(|parent| parent.join(new_name))
        .unwrap_or_else(|| PathBuf::from(new_name))
}

#[cfg(any(test, unix))]
fn common_root(paths: &[PathBuf]) -> PathBuf {
    let mut components: Vec<_> = paths
        .first()
        .and_then(|path| path.parent())
        .map(|path| path.components().collect::<Vec<_>>())
        .unwrap_or_default();

    for path in paths.iter().skip(1) {
        let parent_components: Vec<_> = path
            .parent()
            .map(|parent| parent.components().collect())
            .unwrap_or_default();
        let common_len = components
            .iter()
            .zip(parent_components.iter())
            .take_while(|(a, b)| a == b)
            .count();
        components.truncate(common_len);
    }

    let mut root = PathBuf::new();
    for component in components {
        root.push(component.as_os_str());
    }
    if root.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        root
    }
}

#[cfg(unix)]
fn editor_temp_owner(
    context: &crate::elevated_session::InvocationContext,
) -> Result<Option<(libc::uid_t, libc::gid_t)>> {
    match context {
        crate::elevated_session::InvocationContext::Normal
        | crate::elevated_session::InvocationContext::RootSession => Ok(None),
        crate::elevated_session::InvocationContext::Elevated(user) => {
            Ok(Some((user.uid, user.gid)))
        }
        crate::elevated_session::InvocationContext::ElevatedUnresolved => {
            bail!("could not resolve invoking user")
        }
    }
}

#[cfg(unix)]
fn create_temp_file(
    rows: &[String],
    invoking_user: Option<(libc::uid_t, libc::gid_t)>,
) -> Result<PathBuf> {
    let base = if invoking_user.is_some() {
        PathBuf::from("/tmp")
    } else {
        env::temp_dir()
    };
    for attempt in 0..1000u32 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = base.join(format!(
            "elio-bulk-rename-{}-{now}-{attempt}.txt",
            std::process::id()
        ));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        if invoking_user.is_some() {
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(mut file) => {
                let result = (|| -> Result<()> {
                    file.write_all(rows.join("\n").as_bytes())?;
                    file.write_all(b"\n")?;
                    if let Some((uid, gid)) = invoking_user {
                        if unsafe { libc::fchmod(file.as_raw_fd(), 0o600) } != 0 {
                            return Err(std::io::Error::last_os_error().into());
                        }
                        if unsafe { libc::fchown(file.as_raw_fd(), uid, gid) } != 0 {
                            return Err(std::io::Error::last_os_error().into());
                        }
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(error);
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    bail!("Could not create temporary rename file")
}

#[cfg(unix)]
fn read_editor_rename_file(path: &Path, expected_owner: Option<libc::uid_t>) -> Result<String> {
    let Some(expected_owner) = expected_owner else {
        return Ok(fs::read_to_string(path)?);
    };

    use std::os::unix::fs::MetadataExt;

    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        bail!("edited rename document is not a regular file");
    }
    if metadata.uid() != expected_owner {
        bail!("edited rename document has unexpected owner");
    }
    if metadata.mode() & 0o022 != 0 {
        bail!("edited rename document is writable by another user");
    }
    if metadata.len() > MAX_EDITOR_RENAME_BYTES {
        bail!("edited rename document is too large");
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_EDITOR_RENAME_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_EDITOR_RENAME_BYTES {
        bail!("edited rename document is too large");
    }
    String::from_utf8(bytes).context("edited rename document is not valid UTF-8")
}

#[cfg(unix)]
fn editor_command() -> (String, Vec<String>) {
    for key in ["VISUAL", "EDITOR"] {
        if let Some(value) =
            crate::elevated_session::env_var(key).and_then(|value| value.into_string().ok())
        {
            let tokens = crate::opening::tokenize_command(&value);
            if let Some((program, args)) = split_program_args(tokens) {
                return (program, args);
            }
        }
    }
    ("vi".to_string(), Vec::new())
}

#[cfg(unix)]
fn split_program_args(tokens: Vec<String>) -> Option<(String, Vec<String>)> {
    let mut tokens = tokens.into_iter();
    let program = tokens.next()?;
    Some((program, tokens.collect()))
}

#[cfg(test)]
#[path = "tests/editor_bulk_rename_unit.rs"]
mod tests;

impl FileOperationsState {
    pub fn editor_rename_confirm_is_open(&self) -> bool {
        self.editor_rename_confirm.is_some()
    }

    pub fn editor_rename_confirm_count(&self) -> usize {
        self.editor_rename_confirm
            .as_ref()
            .map_or(0, |overlay| overlay.items.len())
    }

    pub fn editor_rename_confirm_scroll(&self) -> usize {
        self.editor_rename_confirm
            .as_ref()
            .map_or(0, |overlay| overlay.scroll)
    }

    pub fn editor_rename_confirm_row(&self, index: usize) -> Option<(String, String)> {
        let overlay = self.editor_rename_confirm.as_ref()?;
        let item = overlay.items.get(index)?;
        let old = item
            .path
            .strip_prefix(&overlay.root)
            .unwrap_or(&item.path)
            .to_string_lossy()
            .into_owned();
        let new = overlay.new_names.get(index)?.clone();
        Some((old, new))
    }

    pub fn editor_rename_confirm_title(&self) -> String {
        match self.editor_rename_confirm_count() {
            1 => "Confirm rename?".to_string(),
            count => format!("Confirm {count} renames?"),
        }
    }

    pub fn editor_rename_confirmed(&self) -> bool {
        self.editor_rename_confirm
            .as_ref()
            .is_some_and(|overlay| overlay.confirmed)
    }

    pub(crate) fn dismiss_editor_rename_confirm(&mut self) {
        self.editor_rename_confirm = None;
    }

    pub(crate) fn scroll_editor_rename_confirm(&mut self, delta: isize) {
        if let Some(overlay) = &mut self.editor_rename_confirm {
            let max_scroll = overlay.items.len().saturating_sub(1);
            overlay.scroll = overlay.scroll.saturating_add_signed(delta).min(max_scroll);
        }
    }
}
