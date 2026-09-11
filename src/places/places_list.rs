use super::devices::mounted_device_items;
use crate::config::{BuiltinPlace, PlaceEntrySpec, PlacesConfig};
#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
use std::{collections::HashMap, ffi::OsString, os::unix::ffi::OsStringExt};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaceItem {
    pub kind: PlaceKind,
    pub title: String,
    pub icon: String,
    /// Canonical comparison key for this path. `path` remains the path to open.
    pub identity_path: PathBuf,
    pub path: PathBuf,
}

impl PlaceItem {
    pub fn new(
        kind: PlaceKind,
        title: impl Into<String>,
        icon: impl Into<String>,
        path: PathBuf,
        identity_path: PathBuf,
    ) -> Self {
        Self {
            kind,
            title: title.into(),
            icon: icon.into(),
            identity_path,
            path,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaceKind {
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    Root,
    Trash,
    Custom,
    Device { removable: bool },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlaceRow {
    Section { title: &'static str },
    Item(PlaceItem),
}

impl PlaceRow {
    pub fn item(&self) -> Option<&PlaceItem> {
        match self {
            Self::Item(item) => Some(item),
            Self::Section { .. } => None,
        }
    }
}

const CUSTOM_PLACE_ICON: &str = "󰉋";
const SYMLINKED_PLACE_ICON: &str = "";
const BROKEN_SYMLINK_PLACE_ICON: &str = "󰌺";

#[derive(Clone, Debug)]
pub(super) struct PlaceResolutionContext {
    pub(super) home: PathBuf,
    pub(super) desktop: Option<PathBuf>,
    pub(super) documents: Option<PathBuf>,
    pub(super) downloads: Option<PathBuf>,
    pub(super) pictures: Option<PathBuf>,
    pub(super) music: Option<PathBuf>,
    pub(super) videos: Option<PathBuf>,
    pub(super) root: Option<PathBuf>,
    pub(super) trash: Option<PathBuf>,
}

pub(crate) fn build_place_rows() -> Vec<PlaceRow> {
    let home = crate::elevated_session::home_dir().unwrap_or_else(|| {
        #[cfg(windows)]
        return PathBuf::from("C:\\");
        #[cfg(not(windows))]
        return PathBuf::from("/");
    });
    let context = system_place_resolution_context(home, dirs::home_dir());
    build_place_rows_with_context(crate::config::places(), &context)
}

pub(super) fn build_place_rows_with_context(
    places: &PlacesConfig,
    context: &PlaceResolutionContext,
) -> Vec<PlaceRow> {
    let pinned_items = build_pinned_sidebar_items(places, context);
    let pinned_paths = pinned_items
        .iter()
        .map(|item| item.identity_path.clone())
        .collect::<HashSet<_>>();
    let mut rows = pinned_items
        .into_iter()
        .map(PlaceRow::Item)
        .collect::<Vec<_>>();
    let device_items = if places.show_devices {
        mounted_device_items(&context.home, &pinned_paths)
    } else {
        Vec::new()
    };
    if !device_items.is_empty() {
        rows.push(PlaceRow::Section { title: "Devices" });
        rows.extend(device_items.into_iter().map(PlaceRow::Item));
    }
    rows
}

fn system_place_resolution_context(
    home: PathBuf,
    process_home: Option<PathBuf>,
) -> PlaceResolutionContext {
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
    let user_dirs = if process_home.as_deref() != Some(&home) {
        read_user_dirs(&home)
    } else {
        Default::default()
    };
    #[cfg(any(not(unix), target_os = "macos", target_os = "ios"))]
    let user_dirs = ();

    let personal_dir = |current, key, fallback| {
        resolve_personal_dir(
            &home,
            process_home.as_deref(),
            current,
            configured_user_dir(&user_dirs, key),
            platform_personal_dir_fallback(fallback),
        )
    };

    PlaceResolutionContext {
        desktop: personal_dir(dirs::desktop_dir(), "DESKTOP", "Desktop"),
        documents: personal_dir(dirs::document_dir(), "DOCUMENTS", "Documents"),
        downloads: personal_dir(dirs::download_dir(), "DOWNLOAD", "Downloads"),
        pictures: personal_dir(dirs::picture_dir(), "PICTURES", "Pictures"),
        music: personal_dir(dirs::audio_dir(), "MUSIC", "Music"),
        videos: personal_dir(
            dirs::video_dir(),
            "VIDEOS",
            if cfg!(target_os = "macos") {
                "Movies"
            } else {
                "Videos"
            },
        ),
        root: if cfg!(unix) {
            Some(PathBuf::from("/"))
        } else {
            None
        },
        trash: crate::elevated_session::trash_home_dir().and_then(|home| trash_dir(&home)),
        home,
    }
}

pub(super) fn resolve_personal_dir(
    home: &Path,
    process_home: Option<&Path>,
    current: Option<PathBuf>,
    configured: Option<PathBuf>,
    fallback: Option<&str>,
) -> Option<PathBuf> {
    if process_home == Some(home) {
        current
    } else {
        configured.or_else(|| fallback.map(|fallback| home.join(fallback)))
    }
    .filter(|path| path.exists())
}

#[cfg(any(not(unix), target_os = "macos", target_os = "ios"))]
fn platform_personal_dir_fallback(fallback: &str) -> Option<&str> {
    Some(fallback)
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
fn platform_personal_dir_fallback(_fallback: &str) -> Option<&str> {
    None
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
fn configured_user_dir(user_dirs: &HashMap<String, PathBuf>, key: &str) -> Option<PathBuf> {
    user_dirs.get(key).cloned()
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
fn read_user_dirs(home: &Path) -> HashMap<String, PathBuf> {
    let Ok(contents) = fs::read(home.join(".config/user-dirs.dirs")) else {
        return HashMap::new();
    };
    contents
        .split(|byte| *byte == b'\n')
        .filter_map(|line| parse_user_dir(line, home))
        .collect()
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
pub(super) fn parse_user_dir(line: &[u8], home: &Path) -> Option<(String, PathBuf)> {
    let separator = line.iter().position(|byte| *byte == b'=')?;
    let (key, value) = (&line[..separator], &line[separator + 1..]);
    let key = trim_ascii(key);
    let key = key.strip_prefix(b"XDG_")?.strip_suffix(b"_DIR")?;
    let key = std::str::from_utf8(key).ok()?.to_string();
    let value = trim_ascii(value).strip_prefix(b"\"")?;
    let (base, value) = if let Some(relative) = value.strip_prefix(b"$HOME/") {
        (Some(home), relative)
    } else if value.starts_with(b"/") {
        (None, value)
    } else {
        return None;
    };
    let value = OsString::from_vec(unescape_quoted_user_dir(value)?);
    if value.is_empty() {
        return None;
    }
    let path = base.map_or_else(|| PathBuf::from(&value), |base| base.join(&value));
    Some((key, path))
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
fn trim_ascii(value: &[u8]) -> &[u8] {
    let start = value
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t' | b'\r'))
        .unwrap_or(value.len());
    let end = value
        .iter()
        .rposition(|byte| !matches!(byte, b' ' | b'\t' | b'\r'))
        .map_or(start, |index| index + 1);
    &value[start..end]
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
fn unescape_quoted_user_dir(value: &[u8]) -> Option<Vec<u8>> {
    let mut unescaped = Vec::with_capacity(value.len());
    let mut bytes = value.iter().copied();
    while let Some(byte) = bytes.next() {
        match byte {
            b'"' => return Some(unescaped),
            b'\\' => unescaped.push(bytes.next()?),
            _ => unescaped.push(byte),
        }
    }
    None
}

#[cfg(any(not(unix), target_os = "macos", target_os = "ios"))]
fn configured_user_dir(_user_dirs: &(), _key: &str) -> Option<PathBuf> {
    None
}

fn build_pinned_sidebar_items(
    places: &PlacesConfig,
    context: &PlaceResolutionContext,
) -> Vec<PlaceItem> {
    let mut items = Vec::new();
    let mut seen_paths = HashSet::new();

    for entry in &places.entries {
        let Some(item) = resolve_place_entry(entry, context) else {
            continue;
        };
        if seen_paths.insert(item.identity_path.clone()) {
            items.push(item);
        }
    }

    items
}

fn resolve_place_entry(
    entry: &PlaceEntrySpec,
    context: &PlaceResolutionContext,
) -> Option<PlaceItem> {
    match entry {
        PlaceEntrySpec::Builtin { place, icon } => {
            resolve_builtin_place(*place, icon.as_deref(), context)
        }
        PlaceEntrySpec::Custom { title, path, icon } => Some(place_item(
            PlaceKind::Custom,
            title.clone(),
            place_icon(path, icon.as_deref(), CUSTOM_PLACE_ICON),
            path.clone(),
        )),
    }
}

fn resolve_builtin_place(
    place: BuiltinPlace,
    icon_override: Option<&str>,
    context: &PlaceResolutionContext,
) -> Option<PlaceItem> {
    match place {
        BuiltinPlace::Home => Some(place_item(
            PlaceKind::Home,
            "Home",
            place_icon(&context.home, icon_override, "󰋜"),
            context.home.clone(),
        )),
        BuiltinPlace::Desktop => context.desktop.clone().map(|path| {
            place_item(
                PlaceKind::Desktop,
                localized_place_title(&path, "Desktop"),
                place_icon(&path, icon_override, "󰍹"),
                path,
            )
        }),
        BuiltinPlace::Documents => context.documents.clone().map(|path| {
            place_item(
                PlaceKind::Documents,
                localized_place_title(&path, "Documents"),
                place_icon(&path, icon_override, "󰲃"),
                path,
            )
        }),
        BuiltinPlace::Downloads => context.downloads.clone().map(|path| {
            place_item(
                PlaceKind::Downloads,
                localized_place_title(&path, "Downloads"),
                place_icon(&path, icon_override, "󰉍"),
                path,
            )
        }),
        BuiltinPlace::Pictures => context.pictures.clone().map(|path| {
            place_item(
                PlaceKind::Pictures,
                localized_place_title(&path, "Pictures"),
                place_icon(&path, icon_override, "󰉏"),
                path,
            )
        }),
        BuiltinPlace::Music => context.music.clone().map(|path| {
            place_item(
                PlaceKind::Music,
                localized_place_title(&path, "Music"),
                place_icon(&path, icon_override, "󱍙"),
                path,
            )
        }),
        BuiltinPlace::Videos => context.videos.clone().map(|path| {
            place_item(
                PlaceKind::Videos,
                localized_place_title(&path, videos_label()),
                place_icon(&path, icon_override, "󰕧"),
                path,
            )
        }),
        BuiltinPlace::Root => context.root.clone().map(|path| {
            place_item(
                PlaceKind::Root,
                "Root",
                place_icon(&path, icon_override, "󰋊"),
                path,
            )
        }),
        BuiltinPlace::Trash => context.trash.clone().map(|path| {
            place_item(
                PlaceKind::Trash,
                "Trash",
                place_icon(&path, icon_override, "󰩺"),
                path,
            )
        }),
    }
}

fn place_icon<'a>(path: &Path, icon_override: Option<&'a str>, default_icon: &'a str) -> &'a str {
    icon_override.unwrap_or_else(|| match place_symlink_state(path) {
        Some(PlaceSymlinkState::Directory) => SYMLINKED_PLACE_ICON,
        Some(PlaceSymlinkState::Broken) => BROKEN_SYMLINK_PLACE_ICON,
        None => default_icon,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlaceSymlinkState {
    Directory,
    Broken,
}

fn place_symlink_state(path: &Path) -> Option<PlaceSymlinkState> {
    // symlink_metadata preserves the symlink bit; metadata follows it to verify a directory target.
    if !fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return None;
    }
    Some(
        if fs::metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
            PlaceSymlinkState::Directory
        } else {
            PlaceSymlinkState::Broken
        },
    )
}

pub(super) fn place_item(
    kind: PlaceKind,
    title: impl Into<String>,
    icon: impl Into<String>,
    path: PathBuf,
) -> PlaceItem {
    let identity_path = path_identity_key(&path);
    PlaceItem::new(kind, title, icon, path, identity_path)
}

fn localized_place_title(path: &Path, fallback: &'static str) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| fallback.to_string())
}

fn videos_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "Movies"
    } else {
        "Videos"
    }
}

pub(super) fn path_identity_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| normalize_absolute_path(path))
}

pub(super) fn normalize_absolute_path(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

/// Returns the path to the user's trash directory, or `None` if it cannot be determined.
///
/// - **Linux / BSD (freedesktop):** `$XDG_DATA_HOME/Trash/files`, falling back to
///   `~/.local/share/Trash/files`. The `files/` subdirectory holds the actual items;
///   the sibling `info/` directory holds `.trashinfo` metadata used for restore.
/// - **macOS:** `~/.Trash`
/// - **Windows:** always returns `None`. The Recycle Bin is a virtual shell folder
///   that is not practically accessible as a regular filesystem path.
pub(crate) fn trash_dir(home: &Path) -> Option<PathBuf> {
    #[cfg(all(unix, not(target_os = "macos")))]
    let data_dir = if crate::elevated_session::trash_home_dir().as_deref() == Some(home) {
        crate::elevated_session::trash_data_dir()
    } else {
        Some(home.join(".local/share"))
    };
    #[cfg(not(unix))]
    let data_dir = dirs::data_dir();

    #[cfg(not(target_os = "macos"))]
    if let Some(data_dir) = data_dir {
        let xdg_trash = data_dir.join("Trash/files");
        if xdg_trash.exists() {
            return Some(xdg_trash);
        }
    }

    // macOS: always use the selected user's ~/.Trash.
    let mac_trash = home.join(".Trash");
    if mac_trash.exists() {
        return Some(mac_trash);
    }

    None
}
