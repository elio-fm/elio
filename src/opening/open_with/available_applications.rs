#[cfg(test)]
use std::cell::RefCell;
use std::path::Path;

#[cfg(all(unix, not(target_os = "macos")))]
use crate::filesystem::Entry;
use crate::{
    file_classification::{FileClass, PreviewKind, inspect_path},
    filesystem::EntryKind,
};

#[derive(Clone, Debug)]
pub(crate) struct OpenWithApplication {
    pub(crate) display_name: String,
    // Desktop-file ID on Linux/BSD or bundle ID on macOS. Reserved for a
    // future "set as default" action; not yet read at launch time.
    #[allow(dead_code)]
    pub(crate) application_id: Option<String>,
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) is_default: bool,
    /// True when the application must run inside a terminal emulator rather
    /// than being launched detached.
    pub(crate) requires_terminal: bool,
}

#[cfg(all(test, unix, not(target_os = "macos")))]
thread_local! {
    static TEST_DEFAULT_OPEN_WITH_APP: RefCell<Option<OpenWithApplication>> = const { RefCell::new(None) };
    static TEST_OPEN_WITH_APPS_FOUND: RefCell<Option<bool>> = const { RefCell::new(None) };
    static TEST_EDITOR_FALLBACK_APP: RefCell<Option<OpenWithApplication>> = const { RefCell::new(None) };
}

#[cfg(test)]
thread_local! {
    static TEST_DISCOVER_OPEN_WITH_APPS: RefCell<Option<Vec<OpenWithApplication>>> = const { RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn set_applications_for_test(apps: Option<Vec<OpenWithApplication>>) {
    TEST_DISCOVER_OPEN_WITH_APPS.with(|slot| *slot.borrow_mut() = apps);
}

#[cfg(test)]
pub(crate) fn applications_for_test() -> Option<Vec<OpenWithApplication>> {
    TEST_DISCOVER_OPEN_WITH_APPS.with(|slot| slot.borrow().clone())
}

pub(crate) fn applications_for(entry: &crate::filesystem::Entry) -> Vec<OpenWithApplication> {
    #[cfg(target_os = "macos")]
    {
        super::macos::applications_for(&entry.path)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        super::linux_bsd::applications_for(&entry.path, Some(entry.name.as_str()), true)
    }

    #[cfg(not(unix))]
    {
        let _ = entry;
        vec![]
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn default_application_for(entry: &Entry) -> Option<OpenWithApplication> {
    #[cfg(test)]
    {
        let _ = entry;
        TEST_DEFAULT_OPEN_WITH_APP.with(|slot| slot.borrow().clone())
    }

    #[cfg(not(test))]
    super::linux_bsd::desktop_applications_for(&entry.path, Some(entry.name.as_str()))
        .into_iter()
        .find(|app| app.is_default)
}

#[cfg(all(test, unix, not(target_os = "macos")))]
pub(crate) fn set_default_application_for_test(app: Option<OpenWithApplication>) {
    TEST_DEFAULT_OPEN_WITH_APP.with(|slot| *slot.borrow_mut() = app);
}

#[cfg(all(test, unix, not(target_os = "macos")))]
pub(crate) fn set_applications_found_for_test(found: Option<bool>) {
    TEST_OPEN_WITH_APPS_FOUND.with(|slot| *slot.borrow_mut() = found);
}

#[cfg(all(test, unix, not(target_os = "macos")))]
pub(crate) fn set_editor_fallback_for_test(app: Option<OpenWithApplication>) {
    TEST_EDITOR_FALLBACK_APP.with(|slot| *slot.borrow_mut() = app);
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn has_applications_for(entry: &Entry) -> bool {
    #[cfg(test)]
    {
        let _ = entry;
        TEST_OPEN_WITH_APPS_FOUND.with(|slot| slot.borrow().unwrap_or(true))
    }

    #[cfg(not(test))]
    {
        !super::linux_bsd::desktop_applications_for(&entry.path, Some(entry.name.as_str()))
            .is_empty()
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn editor_fallback_for(entry: &Entry) -> Option<OpenWithApplication> {
    #[cfg(test)]
    {
        let _ = entry;
        TEST_EDITOR_FALLBACK_APP.with(|slot| slot.borrow().clone())
    }

    #[cfg(not(test))]
    {
        super::terminal_editors::environment_editor_for(&entry.path)
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn is_editor_compatible(path: &Path) -> bool {
    let facts = inspect_path(path, EntryKind::File);

    match facts.preview.kind {
        PreviewKind::Markdown | PreviewKind::Csv => true,
        // Source previews are usually a good editor fit, but image formats like
        // SVG should still behave like images in "Open With".
        PreviewKind::Source => facts.builtin_class != FileClass::Image,
        // Plain-text previews cover both true text files and some binary
        // document/image categories that render metadata as text. Only treat
        // them as editor-friendly when they are not one of those richer types.
        PreviewKind::PlainText => {
            facts.preview.document_format.is_none()
                && !matches!(
                    facts.builtin_class,
                    FileClass::Image
                        | FileClass::Audio
                        | FileClass::Video
                        | FileClass::Archive
                        | FileClass::Font
                )
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "tests/available_applications.rs"]
mod tests;
