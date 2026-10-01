use super::{natural_cmp, natural_cmp_for_locale};
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

#[test]
fn natural_cmp_orders_accented_spanish_names_with_their_base_letters() {
    assert_eq!(
        natural_cmp_for_locale("es", "árbol", "elipse"),
        Ordering::Less
    );
}

#[test]
fn natural_cmp_treats_canonically_equivalent_names_as_equal() {
    assert_eq!(
        natural_cmp_for_locale("es", "árbol", "a\u{301}rbol"),
        Ordering::Equal
    );
}

#[test]
fn natural_cmp_orders_equivalent_names_with_numeric_padding_transitively() {
    let nfc = "á1";
    let nfd = "a\u{301}1";
    let nfd_padded = "a\u{301}01";

    assert_eq!(natural_cmp_for_locale("es", nfc, nfd), Ordering::Equal);
    assert_eq!(
        natural_cmp_for_locale("es", nfd, nfd_padded),
        Ordering::Less
    );
    assert_eq!(
        natural_cmp_for_locale("es", nfc, nfd_padded),
        Ordering::Less
    );
}

#[test]
fn hungarian_collation_compares_full_words() {
    let mut names = [
        "ábrahám",
        "aladár",
        "balassagyarmat",
        "édrián",
        "elemér",
        "ubul",
        "údrira",
    ];
    names.sort_by(|left, right| natural_cmp_for_locale("hu", left, right));
    assert_eq!(
        names,
        [
            "ábrahám",
            "aladár",
            "balassagyarmat",
            "édrián",
            "elemér",
            "ubul",
            "údrira",
        ]
    );
}
