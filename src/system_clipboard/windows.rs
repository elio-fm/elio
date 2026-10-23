//! Native Windows clipboard access through Win32.

use super::{ClipboardError, FileClipboard, FileClipboardOperation};
use std::{
    ffi::OsString,
    mem::size_of,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::PathBuf,
    ptr, thread,
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{GlobalFree, HANDLE, HGLOBAL},
    System::{
        Console::GetConsoleWindow,
        DataExchange::{
            CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard,
            RegisterClipboardFormatW, SetClipboardData,
        },
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
        Ole::{CF_HDROP, CF_UNICODETEXT, DROPEFFECT_COPY, DROPEFFECT_MOVE},
    },
    UI::Shell::DROPFILES,
};

const PREFERRED_DROP_EFFECT: &[u16] = &[
    b'P' as u16,
    b'r' as u16,
    b'e' as u16,
    b'f' as u16,
    b'e' as u16,
    b'r' as u16,
    b'r' as u16,
    b'e' as u16,
    b'd' as u16,
    b' ' as u16,
    b'D' as u16,
    b'r' as u16,
    b'o' as u16,
    b'p' as u16,
    b'E' as u16,
    b'f' as u16,
    b'f' as u16,
    b'e' as u16,
    b'c' as u16,
    b't' as u16,
    0,
];

pub(super) fn read_files() -> Option<FileClipboard> {
    let _clipboard = Clipboard::open()?;
    let handle = unsafe { GetClipboardData(CF_HDROP as u32) };
    let paths = drop_files_from_global(handle)?;
    (!paths.is_empty()).then_some(FileClipboard {
        paths,
        operation: preferred_drop_effect().unwrap_or(FileClipboardOperation::Copy),
    })
}

pub(super) fn read_text() -> Option<String> {
    let _clipboard = Clipboard::open()?;
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT as u32) };
    text_from_global(handle)
}

pub(super) fn write_files(paths: &[PathBuf]) -> Result<(), ClipboardError> {
    let files = file_drop_data(paths).ok_or(ClipboardError::WriteFailed)?;
    let clipboard = Clipboard::open().ok_or(ClipboardError::Unavailable)?;
    replace_contents(&clipboard)?;
    set_global_data(CF_HDROP as u32, &files)?;
    set_global_data(
        preferred_drop_effect_format()?,
        &DROPEFFECT_COPY.to_le_bytes(),
    )
}

pub(super) fn write_text(text: &str) -> Result<(), ClipboardError> {
    let clipboard = Clipboard::open().ok_or(ClipboardError::Unavailable)?;
    replace_contents(&clipboard)?;
    set_global_data(CF_UNICODETEXT as u32, &utf16_nul_bytes(text))
}

/// Holds the clipboard open only for the explicit user action in progress.
struct Clipboard;

impl Clipboard {
    fn open() -> Option<Self> {
        // A console process needs a non-null owner when replacing clipboard data.
        // Windows Terminal provides a message-only console window for this purpose.
        let owner = unsafe { GetConsoleWindow() };
        for _ in 0..5 {
            if unsafe { OpenClipboard(owner) } != 0 {
                return Some(Self);
            }
            thread::sleep(Duration::from_millis(10));
        }
        None
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}

fn replace_contents(_clipboard: &Clipboard) -> Result<(), ClipboardError> {
    (unsafe { EmptyClipboard() } != 0)
        .then_some(())
        .ok_or(ClipboardError::WriteFailed)
}

fn set_global_data(format: u32, bytes: &[u8]) -> Result<(), ClipboardError> {
    let memory = global_memory(bytes).ok_or(ClipboardError::WriteFailed)?;
    if unsafe { SetClipboardData(format, memory.cast()) }.is_null() {
        unsafe {
            GlobalFree(memory);
        }
        return Err(ClipboardError::WriteFailed);
    }
    Ok(())
}

fn global_memory(bytes: &[u8]) -> Option<HGLOBAL> {
    let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) };
    if memory.is_null() {
        return None;
    }
    let destination = unsafe { GlobalLock(memory) };
    if destination.is_null() {
        unsafe {
            GlobalFree(memory);
        }
        return None;
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), destination.cast(), bytes.len());
        GlobalUnlock(memory);
    }
    Some(memory)
}

fn global_bytes(handle: HANDLE) -> Option<Vec<u8>> {
    if handle.is_null() {
        return None;
    }
    let memory = handle.cast::<std::ffi::c_void>();
    let size = unsafe { GlobalSize(memory) };
    if size == 0 {
        return None;
    }
    let source = unsafe { GlobalLock(memory) };
    if source.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(source.cast::<u8>(), size).to_vec() };
    unsafe {
        GlobalUnlock(memory);
    }
    Some(bytes)
}

fn preferred_drop_effect() -> Option<FileClipboardOperation> {
    let format = preferred_drop_effect_format().ok()?;
    let handle = unsafe { GetClipboardData(format) };
    let bytes = global_bytes(handle)?;
    let effect = u32::from_le_bytes(bytes.get(..size_of::<u32>())?.try_into().ok()?);
    Some(if effect & DROPEFFECT_MOVE != 0 {
        FileClipboardOperation::Cut
    } else {
        FileClipboardOperation::Copy
    })
}

fn preferred_drop_effect_format() -> Result<u32, ClipboardError> {
    let format = unsafe { RegisterClipboardFormatW(PREFERRED_DROP_EFFECT.as_ptr()) };
    (format != 0)
        .then_some(format)
        .ok_or(ClipboardError::WriteFailed)
}

fn drop_files_from_global(handle: HANDLE) -> Option<Vec<PathBuf>> {
    drop_files_from_bytes(&global_bytes(handle)?)
}

fn drop_files_from_bytes(bytes: &[u8]) -> Option<Vec<PathBuf>> {
    let header_size = size_of::<DROPFILES>();
    if bytes.len() < header_size || header_size != 20 {
        return None;
    }
    let offset = u32::from_le_bytes(bytes[..4].try_into().ok()?) as usize;
    let wide = i32::from_le_bytes(bytes[16..20].try_into().ok()?);
    if wide == 0 || offset < header_size || offset > bytes.len() {
        return None;
    }
    let units = utf16_units(&bytes[offset..])?;
    let mut paths = Vec::new();
    let mut path = Vec::new();
    for unit in units {
        if unit == 0 {
            if path.is_empty() {
                return Some(paths);
            }
            paths.push(PathBuf::from(OsString::from_wide(&path)));
            path.clear();
        } else {
            path.push(unit);
        }
    }
    None
}

fn file_drop_data(paths: &[PathBuf]) -> Option<Vec<u8>> {
    if paths.is_empty() || size_of::<DROPFILES>() != 20 {
        return None;
    }
    let mut units = Vec::new();
    for path in paths {
        let path = path.as_os_str().encode_wide().collect::<Vec<_>>();
        if path.is_empty() || path.contains(&0) {
            return None;
        }
        units.extend(path);
        units.push(0);
    }
    units.push(0);
    let header = DROPFILES {
        pFiles: size_of::<DROPFILES>() as u32,
        fWide: 1,
        ..Default::default()
    };
    let mut bytes = unsafe {
        std::slice::from_raw_parts((&raw const header).cast::<u8>(), size_of::<DROPFILES>())
            .to_vec()
    };
    for unit in units {
        bytes.extend(unit.to_le_bytes());
    }
    Some(bytes)
}

fn text_from_global(handle: HANDLE) -> Option<String> {
    let units = utf16_units(&global_bytes(handle)?)?
        .into_iter()
        .take_while(|unit| *unit != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&units).ok()
}

fn utf16_units(bytes: &[u8]) -> Option<Vec<u16>> {
    let (chunks, remainder) = bytes.as_chunks::<2>();
    remainder.is_empty().then(|| {
        chunks
            .iter()
            .map(|unit| u16::from_le_bytes(*unit))
            .collect()
    })
}

fn utf16_nul_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(test)]
pub(super) fn file_drop_data_for_test(paths: &[PathBuf]) -> Option<Vec<u8>> {
    file_drop_data(paths)
}

#[cfg(test)]
pub(super) fn drop_files_from_bytes_for_test(bytes: &[u8]) -> Option<Vec<PathBuf>> {
    drop_files_from_bytes(bytes)
}

#[cfg(test)]
pub(super) fn operation_from_drop_effect_for_test(effect: u32) -> FileClipboardOperation {
    if effect & DROPEFFECT_MOVE != 0 {
        FileClipboardOperation::Cut
    } else {
        FileClipboardOperation::Copy
    }
}

#[cfg(test)]
pub(super) fn text_from_utf16_bytes_for_test(bytes: &[u8]) -> Option<String> {
    let units = utf16_units(bytes)?
        .into_iter()
        .take_while(|unit| *unit != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&units).ok()
}
