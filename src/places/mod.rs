mod devices;
#[cfg(target_os = "linux")]
mod linux_devices;
mod places_list;
#[cfg(test)]
mod tests;

pub use self::places_list::{PlaceItem, PlaceKind, PlaceRow};
pub(crate) use self::places_list::{PlacesState, trash_dir};

pub(crate) fn path_is_trash(path: &std::path::Path) -> bool {
    crate::elevated_session::trash_home_dir()
        .and_then(|home| trash_dir(&home))
        .is_some_and(|trash| path == trash)
}

pub(crate) fn path_is_inside_trash(path: &std::path::Path) -> bool {
    crate::elevated_session::trash_home_dir()
        .and_then(|home| trash_dir(&home))
        .is_some_and(|trash| path.starts_with(&trash))
}
