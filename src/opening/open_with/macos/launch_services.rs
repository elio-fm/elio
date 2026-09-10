use std::ffi::{CStr, c_void};

use objc2_foundation::{NSBundle, NSString, NSURL};

type CFTypeRef = *const c_void;
type CFURLRef = *const c_void;
type CFArrayRef = *const c_void;
type CFStringRef = *const c_void;
type CFIndex = isize;
type CFStringEncoding = u32;
type Boolean = u8;

const ROLES_ALL: u32 = 0xFFFF_FFFF;
pub(super) const ROLES_VIEWER: u32 = 1 << 1;
pub(super) const ROLES_EDITOR: u32 = 1 << 2;
const CF_URL_POSIX_PATH_STYLE: CFIndex = 0;
const CF_STRING_ENCODING_UTF8: CFStringEncoding = 0x0800_0100;

pub(super) struct FileHandler {
    pub(super) path: String,
    pub(super) bundle_identifier: Option<String>,
}

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn LSCopyApplicationURLsForURL(url: CFURLRef, role_mask: u32) -> CFArrayRef;
    fn LSCopyDefaultApplicationURLForURL(
        url: CFURLRef,
        role_mask: u32,
        error: *mut CFTypeRef,
    ) -> CFURLRef;
    fn LSCopyAllRoleHandlersForContentType(content_type: CFStringRef, role_mask: u32)
    -> CFArrayRef;
    fn LSCopyDefaultRoleHandlerForContentType(
        content_type: CFStringRef,
        role_mask: u32,
    ) -> CFStringRef;
    fn LSCopyApplicationURLsForBundleIdentifier(
        bundle_id: CFStringRef,
        out_app_urls: *mut CFArrayRef,
    ) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFArrayGetCount(array: CFArrayRef) -> CFIndex;
    fn CFArrayGetValueAtIndex(array: CFArrayRef, idx: CFIndex) -> *const c_void;
    fn CFURLCopyFileSystemPath(url: CFURLRef, path_style: CFIndex) -> CFStringRef;
    fn CFStringGetLength(value: CFStringRef) -> CFIndex;
    fn CFStringGetMaximumSizeForEncoding(len: CFIndex, encoding: CFStringEncoding) -> CFIndex;
    fn CFStringGetCString(
        value: CFStringRef,
        buffer: *mut i8,
        buffer_size: CFIndex,
        encoding: CFStringEncoding,
    ) -> Boolean;
    fn CFRelease(value: CFTypeRef);
}

pub(super) fn file_handlers(path: &str) -> (Vec<FileHandler>, Option<String>) {
    let ns_path = NSString::from_str(path);
    let file_url = NSURL::fileURLWithPath(&ns_path);
    let cf_file_url: CFURLRef = (&*file_url) as *const NSURL as *const c_void;
    let applications = unsafe { LSCopyApplicationURLsForURL(cf_file_url, ROLES_ALL) };
    if applications.is_null() {
        return (Vec::new(), None);
    }

    let default_path = {
        let default = unsafe {
            LSCopyDefaultApplicationURLForURL(cf_file_url, ROLES_ALL, std::ptr::null_mut())
        };
        if default.is_null() {
            None
        } else {
            let path = cf_url_to_path(default);
            unsafe { CFRelease(default) };
            path
        }
    };

    let count = unsafe { CFArrayGetCount(applications) };
    let mut handlers = Vec::with_capacity(count as usize);
    for index in 0..count {
        let application_url = unsafe { CFArrayGetValueAtIndex(applications, index) } as CFURLRef;
        let Some(path) = cf_url_to_path(application_url) else {
            continue;
        };
        let ns_application_url = unsafe { &*(application_url as *const NSURL) };
        let bundle_identifier = NSBundle::bundleWithURL(ns_application_url)
            .and_then(|bundle| bundle.bundleIdentifier())
            .map(|identifier| identifier.to_string());
        handlers.push(FileHandler {
            path,
            bundle_identifier,
        });
    }
    unsafe { CFRelease(applications) };

    (handlers, default_path)
}

pub(super) fn role_handlers_for_content_type(content_type: &str, role_mask: u32) -> Vec<String> {
    let ns_content_type = NSString::from_str(content_type);
    let cf_content_type = (&*ns_content_type) as *const NSString as CFStringRef;
    let handlers = unsafe { LSCopyAllRoleHandlersForContentType(cf_content_type, role_mask) };
    let values = cf_array_to_strings(handlers);
    if !handlers.is_null() {
        unsafe { CFRelease(handlers) };
    }
    values
}

pub(super) fn default_role_handler_for_content_type(
    content_type: &str,
    role_mask: u32,
) -> Option<String> {
    let ns_content_type = NSString::from_str(content_type);
    let cf_content_type = (&*ns_content_type) as *const NSString as CFStringRef;
    let handler = unsafe { LSCopyDefaultRoleHandlerForContentType(cf_content_type, role_mask) };
    if handler.is_null() {
        return None;
    }
    let value = cf_string_to_string(handler);
    unsafe { CFRelease(handler) };
    value
}

pub(super) fn application_paths_for_bundle_identifier(bundle_identifier: &str) -> Vec<String> {
    let ns_bundle_identifier = NSString::from_str(bundle_identifier);
    let cf_bundle_identifier = (&*ns_bundle_identifier) as *const NSString as CFStringRef;
    let mut application_urls = std::ptr::null();
    let status = unsafe {
        LSCopyApplicationURLsForBundleIdentifier(cf_bundle_identifier, &mut application_urls)
    };
    if status != 0 || application_urls.is_null() {
        return Vec::new();
    }

    let paths = cf_array_to_paths(application_urls);
    unsafe { CFRelease(application_urls) };
    paths
}

fn cf_array_to_strings(array: CFArrayRef) -> Vec<String> {
    if array.is_null() {
        return Vec::new();
    }
    let count = unsafe { CFArrayGetCount(array) };
    let mut values = Vec::with_capacity(count as usize);
    for index in 0..count {
        let value = unsafe { CFArrayGetValueAtIndex(array, index) } as CFStringRef;
        if let Some(value) = cf_string_to_string(value) {
            values.push(value);
        }
    }
    values
}

fn cf_array_to_paths(array: CFArrayRef) -> Vec<String> {
    if array.is_null() {
        return Vec::new();
    }
    let count = unsafe { CFArrayGetCount(array) };
    let mut paths = Vec::with_capacity(count as usize);
    for index in 0..count {
        let url = unsafe { CFArrayGetValueAtIndex(array, index) } as CFURLRef;
        if let Some(path) = cf_url_to_path(url) {
            paths.push(path);
        }
    }
    paths
}

fn cf_url_to_path(url: CFURLRef) -> Option<String> {
    if url.is_null() {
        return None;
    }
    let value = unsafe { CFURLCopyFileSystemPath(url, CF_URL_POSIX_PATH_STYLE) };
    if value.is_null() {
        return None;
    }
    let result = cf_string_to_string(value);
    unsafe { CFRelease(value) };
    result
}

fn cf_string_to_string(value: CFStringRef) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let len = unsafe { CFStringGetLength(value) };
    let max_size = unsafe { CFStringGetMaximumSizeForEncoding(len, CF_STRING_ENCODING_UTF8) } + 1;
    let mut buffer = vec![0_i8; max_size as usize];
    let converted = unsafe {
        CFStringGetCString(
            value,
            buffer.as_mut_ptr(),
            max_size,
            CF_STRING_ENCODING_UTF8,
        )
    };
    if converted == 0 {
        return None;
    }
    unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_str()
        .ok()
        .map(str::to_string)
}

#[cfg(test)]
#[path = "tests/launch_services.rs"]
mod tests;
