//! Native macOS clipboard access through AppKit.

use super::{ClipboardError, FileClipboard, FileClipboardOperation};
use objc2::{
    ClassType,
    rc::autoreleasepool,
    runtime::{AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSApplication, NSPasteboard, NSPasteboardItem, NSPasteboardTypeFileURL, NSPasteboardTypeString,
    NSPasteboardURLReadingFileURLsOnlyKey, NSPasteboardWriting,
};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString, NSURL};
use std::path::PathBuf;

pub(super) fn read_files() -> Option<FileClipboard> {
    with_appkit(|| read_files_from(&NSPasteboard::generalPasteboard()))?
}

pub(super) fn read_text() -> Option<String> {
    with_appkit(|| read_text_from(&NSPasteboard::generalPasteboard()))?
}

pub(super) fn write_files(paths: &[PathBuf]) -> Result<(), ClipboardError> {
    with_appkit(|| write_files_to(&NSPasteboard::generalPasteboard(), paths))
        .unwrap_or(Err(ClipboardError::Unavailable))
}

pub(super) fn write_text(text: &str) -> Result<(), ClipboardError> {
    with_appkit(|| write_text_to(&NSPasteboard::generalPasteboard(), text))
        .unwrap_or(Err(ClipboardError::Unavailable))
}

fn with_appkit<T>(operation: impl FnOnce() -> T) -> Option<T> {
    // elio runs as a terminal process, so it must initialize AppKit itself
    // before accessing the pasteboard.
    autoreleasepool(|_| NSApplication::load().then(operation))
}

fn read_files_from(pasteboard: &NSPasteboard) -> Option<FileClipboard> {
    let classes = NSArray::from_slice(&[NSURL::class()]);
    let options = NSDictionary::<NSString, AnyObject>::from_retained_objects(
        &[file_urls_only_key()],
        &[NSNumber::new_bool(true).into_super().into()],
    );
    // NSURL is the only requested class, so every returned object is an NSURL.
    let urls = unsafe { pasteboard.readObjectsForClasses_options(&classes, Some(&options)) }?;
    let paths = urls
        .iter()
        .filter_map(|object| {
            let url = object.downcast_ref::<NSURL>()?;
            local_file_path(url)
        })
        .collect::<Vec<_>>();
    (!paths.is_empty()).then_some(FileClipboard {
        paths,
        operation: FileClipboardOperation::Copy,
    })
}

fn read_text_from(pasteboard: &NSPasteboard) -> Option<String> {
    let texts = pasteboard.pasteboardItems().map_or_else(Vec::new, |items| {
        items
            .iter()
            .filter_map(|item| item.stringForType(text_type()))
            .map(|text| text.to_string())
            .collect::<Vec<_>>()
    });
    (!texts.is_empty()).then(|| texts.join("\n")).or_else(|| {
        pasteboard
            .stringForType(text_type())
            .map(|text| text.to_string())
    })
}

fn write_files_to(pasteboard: &NSPasteboard, paths: &[PathBuf]) -> Result<(), ClipboardError> {
    let objects = paths
        .iter()
        .map(file_pasteboard_item)
        .collect::<Result<Vec<_>, _>>()?;
    let objects = objects
        .into_iter()
        .map(ProtocolObject::<dyn NSPasteboardWriting>::from_retained)
        .collect::<Vec<_>>();
    let objects = NSArray::from_retained_slice(&objects);
    pasteboard.clearContents();
    pasteboard
        .writeObjects(&objects)
        .then_some(())
        .ok_or(ClipboardError::WriteFailed)
}

fn file_pasteboard_item(
    path: &PathBuf,
) -> Result<objc2::rc::Retained<NSPasteboardItem>, ClipboardError> {
    let url = if path.is_dir() {
        NSURL::from_directory_path(path)
    } else {
        NSURL::from_file_path(path)
    }
    .ok_or(ClipboardError::WriteFailed)?;
    let file_url = url.absoluteString().ok_or(ClipboardError::WriteFailed)?;
    let path_text = url.path().ok_or(ClipboardError::WriteFailed)?;
    let utf16_type = utf16_text_type();
    let item = NSPasteboardItem::new();

    (item.setString_forType(&file_url, file_url_type())
        && item.setString_forType(&path_text, text_type())
        && item.setString_forType(&path_text, &utf16_type))
    .then_some(item)
    .ok_or(ClipboardError::WriteFailed)
}

fn write_text_to(pasteboard: &NSPasteboard, text: &str) -> Result<(), ClipboardError> {
    pasteboard.clearContents();
    pasteboard
        .setString_forType(&NSString::from_str(text), text_type())
        .then_some(())
        .ok_or(ClipboardError::WriteFailed)
}

fn text_type() -> &'static NSString {
    // AppKit exports this pasteboard type as an extern static.
    unsafe { NSPasteboardTypeString }
}

fn file_url_type() -> &'static NSString {
    // AppKit exports this pasteboard type as an extern static.
    unsafe { NSPasteboardTypeFileURL }
}

fn utf16_text_type() -> objc2::rc::Retained<NSString> {
    NSString::from_str("public.utf16-external-plain-text")
}

fn file_urls_only_key() -> &'static NSString {
    // AppKit exports this reading-option key as an extern static.
    unsafe { NSPasteboardURLReadingFileURLsOnlyKey }
}

fn local_file_path(url: &NSURL) -> Option<PathBuf> {
    if !url.isFileURL() {
        return None;
    }
    let host = url.host().map(|host| host.to_string());
    if host
        .as_deref()
        .is_some_and(|host| !host.is_empty() && !host.eq_ignore_ascii_case("localhost"))
    {
        return None;
    }
    url.to_file_path()
}

#[cfg(test)]
pub(super) fn read_files_for_test(pasteboard: &NSPasteboard) -> Option<FileClipboard> {
    with_appkit(|| read_files_from(pasteboard))?
}

#[cfg(test)]
pub(super) fn read_text_for_test(pasteboard: &NSPasteboard) -> Option<String> {
    with_appkit(|| read_text_from(pasteboard))?
}

#[cfg(test)]
pub(super) fn write_files_for_test(
    pasteboard: &NSPasteboard,
    paths: &[PathBuf],
) -> Result<(), ClipboardError> {
    with_appkit(|| write_files_to(pasteboard, paths)).unwrap_or(Err(ClipboardError::Unavailable))
}

#[cfg(test)]
pub(super) fn write_text_for_test(
    pasteboard: &NSPasteboard,
    text: &str,
) -> Result<(), ClipboardError> {
    with_appkit(|| write_text_to(pasteboard, text)).unwrap_or(Err(ClipboardError::Unavailable))
}

#[cfg(test)]
pub(super) fn unique_pasteboard_for_test() -> objc2::rc::Retained<NSPasteboard> {
    with_appkit(NSPasteboard::pasteboardWithUniqueName)
        .expect("AppKit must initialize for macOS clipboard tests")
}

#[cfg(test)]
pub(super) fn write_urls_for_test(
    pasteboard: &NSPasteboard,
    urls: Vec<objc2::rc::Retained<NSURL>>,
) -> Result<(), ClipboardError> {
    with_appkit(|| {
        let objects = urls
            .into_iter()
            .map(ProtocolObject::<dyn NSPasteboardWriting>::from_retained)
            .collect::<Vec<_>>();
        let objects = NSArray::from_retained_slice(&objects);
        pasteboard.clearContents();
        pasteboard
            .writeObjects(&objects)
            .then_some(())
            .ok_or(ClipboardError::WriteFailed)
    })
    .unwrap_or(Err(ClipboardError::Unavailable))
}
