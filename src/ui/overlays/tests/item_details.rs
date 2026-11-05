use super::super::item_details::format_timestamp;
use crate::config::DateFormat;
use chrono::{Datelike, Local, TimeZone};

#[test]
fn timestamp_format_respects_the_selected_date_format() {
    let current_year = Local::now().year();
    let time = Local
        .with_ymd_and_hms(current_year, 11, 1, 17, 11, 0)
        .single()
        .unwrap()
        .into();

    assert_eq!(format_timestamp(time, DateFormat::Absolute), "Nov 01 17:11");
    assert_ne!(format_timestamp(time, DateFormat::Relative), "Nov 01 17:11");
}
