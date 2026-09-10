use std::ffi::c_void;

use objc2_foundation::{NSString, NSURL};

use super::cf_url_to_path;

#[test]
fn null_url_has_no_path() {
    assert!(cf_url_to_path(std::ptr::null()).is_none());
}

#[test]
fn url_path_round_trips() {
    let path = NSString::from_str("/Applications");
    let url = NSURL::fileURLWithPath(&path);
    let cf_url = (&*url) as *const NSURL as *const c_void;

    assert_eq!(cf_url_to_path(cf_url).as_deref(), Some("/Applications"));
}
