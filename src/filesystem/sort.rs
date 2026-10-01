use icu_collator::preferences::CollationNumericOrdering;
use icu_collator::{Collator, CollatorBorrowed, CollatorPreferences};
use icu_locale_core::Locale;
use icu_normalizer::{ComposingNormalizer, ComposingNormalizerBorrowed};
use std::{cmp::Ordering, sync::OnceLock};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortMode {
    Name,
    Modified,
    Size,
}

impl SortMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::Name => Self::Modified,
            Self::Modified => Self::Size,
            Self::Size => Self::Name,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Modified => "Modified",
            Self::Size => "Size",
        }
    }
}

static FILE_NAME_COLLATOR: OnceLock<CollatorBorrowed<'static>> = OnceLock::new();
static NFC_NORMALIZER: ComposingNormalizerBorrowed<'static> = ComposingNormalizer::new_nfc();

/// Compares filenames using the user's system locale and numeric collation.
///
/// The collator normalizes canonically equivalent Unicode forms and orders digit runs by numeric
/// value, so `file2` sorts before `file10`.
pub(crate) fn natural_cmp(left: &str, right: &str) -> Ordering {
    natural_cmp_with_collator(file_name_collator(), left, right)
}

fn natural_cmp_with_collator(
    collator: &CollatorBorrowed<'static>,
    left: &str,
    right: &str,
) -> Ordering {
    let order = collator.compare(left, right);
    if order.is_eq() {
        let left_normalized = NFC_NORMALIZER.normalize(left);
        let right_normalized = NFC_NORMALIZER.normalize(right);
        numeric_padding_cmp(left_normalized.as_ref(), right_normalized.as_ref())
    } else {
        order
    }
}

fn numeric_padding_cmp(left: &str, right: &str) -> Ordering {
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    let mut left_index = 0;
    let mut right_index = 0;

    while let (Some(&left_byte), Some(&right_byte)) =
        (left_bytes.get(left_index), right_bytes.get(right_index))
    {
        if left_byte.is_ascii_digit() && right_byte.is_ascii_digit() {
            let left_end = digit_run_end(left_bytes, left_index);
            let right_end = digit_run_end(right_bytes, right_index);
            let order =
                compare_numeric_runs(&left[left_index..left_end], &right[right_index..right_end]);
            if !order.is_eq() {
                return order;
            }
            left_index = left_end;
            right_index = right_end;
            continue;
        }
        if left_byte != right_byte {
            return Ordering::Equal;
        }
        left_index += 1;
        right_index += 1;
    }

    Ordering::Equal
}

fn digit_run_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    index
}

fn compare_numeric_runs(left: &str, right: &str) -> Ordering {
    let left_trimmed = left.trim_start_matches('0');
    let right_trimmed = right.trim_start_matches('0');
    let left_normalized = if left_trimmed.is_empty() {
        "0"
    } else {
        left_trimmed
    };
    let right_normalized = if right_trimmed.is_empty() {
        "0"
    } else {
        right_trimmed
    };

    match left_normalized.len().cmp(&right_normalized.len()) {
        Ordering::Equal => match left_normalized.cmp(right_normalized) {
            Ordering::Equal => left.len().cmp(&right.len()),
            order => order,
        },
        order => order,
    }
}

fn file_name_collator() -> &'static CollatorBorrowed<'static> {
    FILE_NAME_COLLATOR.get_or_init(|| {
        let locale = sys_locale::get_locale().unwrap_or_else(|| "und".to_string());
        collator_for_locale(&locale)
    })
}

fn collator_for_locale(locale: &str) -> CollatorBorrowed<'static> {
    let locale = locale
        .parse::<Locale>()
        .unwrap_or_else(|_| "und".parse().expect("und must be a valid locale"));
    let mut preferences = CollatorPreferences::from(locale);
    preferences.numeric_ordering = Some(CollationNumericOrdering::True);
    Collator::try_new(preferences, Default::default()).unwrap_or_else(|_| {
        Collator::try_new(Default::default(), Default::default())
            .expect("root collation data must be available")
    })
}

#[cfg(test)]
pub(super) fn natural_cmp_for_locale(locale: &str, left: &str, right: &str) -> Ordering {
    natural_cmp_with_collator(&collator_for_locale(locale), left, right)
}

#[cfg(test)]
#[path = "tests/sort.rs"]
mod tests;
