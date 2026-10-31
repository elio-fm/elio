use super::renderer::{ResolvedFont, is_private_use_character};
use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};
use windows::{
    Win32::Graphics::DirectWrite::{
        DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_WEIGHT_NORMAL, DWriteCreateFactory, IDWriteFactory, IDWriteLocalFontFileLoader,
    },
    core::{BOOL, Interface, PCWSTR},
};

const PREFERRED_NERD_FONT_FAMILIES: [&str; 2] = ["Symbols Nerd Font Mono", "Symbols Nerd Font"];

pub(super) fn font_for_character(ch: char) -> Option<ResolvedFont> {
    if !is_private_use_character(ch) {
        return None;
    }
    unsafe {
        let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok()?;
        let mut collection = None;
        factory
            .GetSystemFontCollection(&mut collection, false)
            .ok()?;
        let collection = collection?;
        PREFERRED_NERD_FONT_FAMILIES
            .iter()
            .find_map(|family| resolve_family_font(&collection, family, ch))
            .or_else(|| resolve_any_nerd_font(&collection, ch))
    }
}

fn resolve_family_font(
    collection: &windows::Win32::Graphics::DirectWrite::IDWriteFontCollection,
    family_name: &str,
    ch: char,
) -> Option<ResolvedFont> {
    unsafe {
        let name = family_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let mut index = 0;
        let mut exists = BOOL(0);
        collection
            .FindFamilyName(PCWSTR(name.as_ptr()), &mut index, &mut exists)
            .ok()?;
        if !exists.as_bool() {
            return None;
        }
        resolve_font_in_family(collection.GetFontFamily(index).ok()?, ch)
    }
}

fn resolve_any_nerd_font(
    collection: &windows::Win32::Graphics::DirectWrite::IDWriteFontCollection,
    ch: char,
) -> Option<ResolvedFont> {
    unsafe {
        (0..collection.GetFontFamilyCount()).find_map(|index| {
            let family = collection.GetFontFamily(index).ok()?;
            family_name_contains(&family, "Nerd Font")
                .then(|| resolve_font_in_family(family, ch))?
        })
    }
}

fn family_name_contains(
    family: &windows::Win32::Graphics::DirectWrite::IDWriteFontFamily,
    needle: &str,
) -> bool {
    let needle = needle.to_lowercase();
    unsafe {
        let Ok(names) = family.GetFamilyNames() else {
            return false;
        };
        (0..names.GetCount()).any(|index| {
            let Ok(length) = names.GetStringLength(index) else {
                return false;
            };
            let mut name = vec![0; length as usize + 1];
            if names.GetString(index, &mut name).is_err() {
                return false;
            }
            String::from_utf16_lossy(&name[..length as usize])
                .to_lowercase()
                .contains(&needle)
        })
    }
}

fn resolve_font_in_family(
    family: windows::Win32::Graphics::DirectWrite::IDWriteFontFamily,
    ch: char,
) -> Option<ResolvedFont> {
    unsafe {
        let font = family
            .GetFirstMatchingFont(
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
            )
            .ok()?;
        let face = font.CreateFontFace().ok()?;
        let mut glyph_index = 0;
        face.GetGlyphIndices(&(ch as u32), 1, &mut glyph_index)
            .ok()?;
        if glyph_index == 0 {
            return None;
        }
        let mut file_count = 0;
        face.GetFiles(&mut file_count, None).ok()?;
        if file_count != 1 {
            return None;
        }
        let mut files = vec![None; file_count as usize];
        face.GetFiles(&mut file_count, Some(files.as_mut_ptr()))
            .ok()?;
        let file = files.pop().flatten()?;
        let loader: IDWriteLocalFontFileLoader = file.GetLoader().ok()?.cast().ok()?;
        let mut key = std::ptr::null_mut();
        let mut key_size = 0;
        file.GetReferenceKey(&mut key, &mut key_size).ok()?;
        let path_length = loader.GetFilePathLengthFromKey(key, key_size).ok()?;
        let mut path = vec![0; path_length as usize + 1];
        loader.GetFilePathFromKey(key, key_size, &mut path).ok()?;

        Some(ResolvedFont {
            path: PathBuf::from(OsString::from_wide(&path[..path_length as usize])),
            collection_index: face.GetIndex(),
        })
    }
}
