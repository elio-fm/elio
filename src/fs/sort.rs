use std::cmp::Ordering;

/// Compares already-normalized strings using natural ordering.
///
/// Text is compared lexically, but adjacent ASCII digit runs are compared numerically so
/// filenames like `2` sort before `10`.
pub(crate) fn natural_cmp(left: &str, right: &str) -> Ordering {
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    let mut left_index = 0usize;
    let mut right_index = 0usize;

    loop {
        match (
            left_bytes.get(left_index).copied(),
            right_bytes.get(right_index).copied(),
        ) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(left_byte), Some(right_byte))
                if left_byte.is_ascii_digit() && right_byte.is_ascii_digit() =>
            {
                let left_end = digit_run_end(left_bytes, left_index);
                let right_end = digit_run_end(right_bytes, right_index);
                match compare_numeric_runs(
                    &left[left_index..left_end],
                    &right[right_index..right_end],
                ) {
                    Ordering::Equal => {
                        left_index = left_end;
                        right_index = right_end;
                    }
                    order => return order,
                }
            }
            (Some(_), Some(_)) => {
                let left_ch = left[left_index..].chars().next().unwrap_or_default();
                let right_ch = right[right_index..].chars().next().unwrap_or_default();
                match left_ch.cmp(&right_ch) {
                    Ordering::Equal => {
                        left_index += left_ch.len_utf8();
                        right_index += right_ch.len_utf8();
                    }
                    order => return order,
                }
            }
        }
    }
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

#[cfg(test)]
mod tests {
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
}
