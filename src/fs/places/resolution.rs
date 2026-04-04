use super::devices::mounted_device_items;
use crate::{
    config::{BuiltinPlace, PlaceEntrySpec, PlacesConfig},
    core::{SidebarItem, SidebarItemKind, SidebarRow},
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

const CUSTOM_PLACE_ICON: &str = "󰉋";

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

/// Returns the current user's home directory.
///
/// Delegates to the [`dirs`] crate, which reads `$HOME` on Unix and
/// `%USERPROFILE%` / `{FOLDERID_Profile}` on Windows. Returns `None` only in
/// the unlikely event that none of the relevant system APIs succeed.
pub(crate) fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

pub(crate) fn build_sidebar_rows() -> Vec<SidebarRow> {
    let home = home_dir().unwrap_or_else(|| {
        #[cfg(windows)]
        return PathBuf::from("C:\\");
        #[cfg(not(windows))]
        return PathBuf::from("/");
    });
    let context = system_place_resolution_context(home);
    build_sidebar_rows_with_context(crate::config::places(), &context)
}

pub(super) fn build_sidebar_rows_with_context(
    places: &PlacesConfig,
    context: &PlaceResolutionContext,
) -> Vec<SidebarRow> {
    let pinned_items = build_pinned_sidebar_items(places, context);
    let pinned_paths = pinned_items
        .iter()
        .map(|item| path_identity_key(&item.path))
        .collect::<HashSet<_>>();
    let mut rows = pinned_items
        .into_iter()
        .map(SidebarRow::Item)
        .collect::<Vec<_>>();
    let device_items = if places.show_devices {
        mounted_device_items(&context.home, &pinned_paths)
    } else {
        Vec::new()
    };
    if !device_items.is_empty() {
        rows.push(SidebarRow::Section { title: "Devices" });
        rows.extend(device_items.into_iter().map(SidebarRow::Item));
    }
    rows
}

fn system_place_resolution_context(home: PathBuf) -> PlaceResolutionContext {
    PlaceResolutionContext {
        desktop: dirs::desktop_dir().filter(|path| path.exists()),
        documents: dirs::document_dir().filter(|path| path.exists()),
        downloads: dirs::download_dir().filter(|path| path.exists()),
        pictures: dirs::picture_dir().filter(|path| path.exists()),
        music: dirs::audio_dir().filter(|path| path.exists()),
        videos: dirs::video_dir().filter(|path| path.exists()),
        root: if cfg!(unix) {
            Some(PathBuf::from("/"))
        } else {
            None
        },
        trash: trash_dir(&home),
        home,
    }
}

fn build_pinned_sidebar_items(
    places: &PlacesConfig,
    context: &PlaceResolutionContext,
) -> Vec<SidebarItem> {
    let mut items = Vec::new();
    let mut seen_paths = HashSet::new();

    for entry in &places.entries {
        let Some(item) = resolve_place_entry(entry, context) else {
            continue;
        };
        if seen_paths.insert(path_identity_key(&item.path)) {
            items.push(item);
        }
    }

    items
}

fn resolve_place_entry(
    entry: &PlaceEntrySpec,
    context: &PlaceResolutionContext,
) -> Option<SidebarItem> {
    match entry {
        PlaceEntrySpec::Builtin { place, icon } => {
            resolve_builtin_place(*place, icon.as_deref(), context)
        }
        PlaceEntrySpec::Custom { title, path, icon } => Some(SidebarItem::new(
            SidebarItemKind::Custom,
            title.clone(),
            icon.as_deref().unwrap_or(CUSTOM_PLACE_ICON),
            path.clone(),
        )),
    }
}

fn resolve_builtin_place(
    place: BuiltinPlace,
    icon_override: Option<&str>,
    context: &PlaceResolutionContext,
) -> Option<SidebarItem> {
    match place {
        BuiltinPlace::Home => Some(SidebarItem::new(
            SidebarItemKind::Home,
            "Home",
            icon_override.unwrap_or("󰋜"),
            context.home.clone(),
        )),
        BuiltinPlace::Desktop => context.desktop.clone().map(|path| {
            SidebarItem::new(
                SidebarItemKind::Desktop,
                localized_place_title(&path, "Desktop"),
                icon_override.unwrap_or("󰍹"),
                path,
            )
        }),
        BuiltinPlace::Documents => context.documents.clone().map(|path| {
            SidebarItem::new(
                SidebarItemKind::Documents,
                localized_place_title(&path, "Documents"),
                icon_override.unwrap_or("󰲃"),
                path,
            )
        }),
        BuiltinPlace::Downloads => context.downloads.clone().map(|path| {
            SidebarItem::new(
                SidebarItemKind::Downloads,
                localized_place_title(&path, "Downloads"),
                icon_override.unwrap_or("󰉍"),
                path,
            )
        }),
        BuiltinPlace::Pictures => context.pictures.clone().map(|path| {
            SidebarItem::new(
                SidebarItemKind::Pictures,
                localized_place_title(&path, "Pictures"),
                icon_override.unwrap_or("󰉏"),
                path,
            )
        }),
        BuiltinPlace::Music => context.music.clone().map(|path| {
            SidebarItem::new(
                SidebarItemKind::Music,
                localized_place_title(&path, "Music"),
                icon_override.unwrap_or("󱍙"),
                path,
            )
        }),
        BuiltinPlace::Videos => context.videos.clone().map(|path| {
            SidebarItem::new(
                SidebarItemKind::Videos,
                localized_place_title(&path, videos_label()),
                icon_override.unwrap_or("󰕧"),
                path,
            )
        }),
        BuiltinPlace::Root => context.root.clone().map(|path| {
            SidebarItem::new(
                SidebarItemKind::Root,
                "Root",
                icon_override.unwrap_or("󰋊"),
                path,
            )
        }),
        BuiltinPlace::Trash => context.trash.clone().map(|path| {
            SidebarItem::new(
                SidebarItemKind::Trash,
                "Trash",
                icon_override.unwrap_or("󰩺"),
                path,
            )
        }),
    }
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

fn normalize_absolute_path(path: &Path) -> PathBuf {
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
    // dirs::data_dir() honours $XDG_DATA_HOME on Linux/BSD, returns
    // ~/Library/Application Support on macOS, and %APPDATA% on Windows.
    if let Some(data_dir) = dirs::data_dir() {
        let xdg_trash = data_dir.join("Trash/files");
        if xdg_trash.exists() {
            return Some(xdg_trash);
        }
    }

    // macOS: ~/.Trash (freedesktop path above won't exist there)
    let mac_trash = home.join(".Trash");
    if mac_trash.exists() {
        return Some(mac_trash);
    }

    None
}
