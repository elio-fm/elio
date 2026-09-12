use super::natural_cmp;
use std::cmp::Ordering;

#[test]
fn natural_cmp_orders_numeric_suffixes() {
    assert_eq!(natural_cmp("chapter 2", "chapter 10"), Ordering::Less);
    assert_eq!(natural_cmp("chapter 10", "chapter 2"), Ordering::Greater);
}

#[test]
fn natural_cmp_handles_non_latin_text_around_numbers() {
    assert_eq!(natural_cmp("北斗の拳 2巻", "北斗の拳 10巻"), Ordering::Less);
}

#[test]
fn natural_cmp_keeps_zero_padded_numbers_stable() {
    assert_eq!(natural_cmp("page 1", "page 01"), Ordering::Less);
    assert_eq!(natural_cmp("page 01", "page 001"), Ordering::Less);
}
