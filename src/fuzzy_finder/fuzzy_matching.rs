use super::SearchCandidate;

pub(crate) fn filter_candidates_in<I>(
    candidates: &[SearchCandidate],
    pool: I,
    query: &str,
    limit: usize,
) -> SearchFilterResult
where
    I: IntoIterator<Item = usize>,
{
    if query.trim().is_empty() {
        let pool = pool.into_iter().collect::<Vec<_>>();
        let matches = pool.iter().copied().take(limit).collect();
        return SearchFilterResult { pool, matches };
    }

    let query_key = query.to_lowercase();
    let needle = query_key.as_bytes();
    let mut filtered_pool = Vec::new();
    let mut top = Vec::<(usize, i64, usize)>::with_capacity(limit.min(64));

    for index in pool {
        let candidate = &candidates[index];
        let exact_name_bonus = (candidate.name_key == query_key) as i64 * 220;
        let name_score = fuzzy_score_bytes(needle, candidate.name_key.as_bytes())
            .map(|score| score + 80 + i64::from(candidate.is_dir) * 12 + exact_name_bonus);
        let path_score = fuzzy_score_bytes(needle, candidate.relative_key.as_bytes());
        let score = match (name_score, path_score) {
            (Some(name), Some(path)) => name.max(path),
            (Some(name), None) => name,
            (None, Some(path)) => path,
            (None, None) => continue,
        };

        filtered_pool.push(index);

        let entry = (index, score, candidate.relative.len());
        let insert_at = top
            .binary_search_by(|existing| compare_scored(candidates, existing, &entry))
            .unwrap_or_else(|slot| slot);

        if insert_at >= limit {
            continue;
        }

        top.insert(insert_at, entry);
        if top.len() > limit {
            top.pop();
        }
    }

    let matches = top.into_iter().map(|(index, _, _)| index).collect();
    SearchFilterResult {
        pool: filtered_pool,
        matches,
    }
}

pub(crate) struct SearchFilterResult {
    pub(crate) pool: Vec<usize>,
    pub(crate) matches: Vec<usize>,
}

fn compare_scored(
    candidates: &[SearchCandidate],
    left: &(usize, i64, usize),
    right: &(usize, i64, usize),
) -> std::cmp::Ordering {
    right
        .1
        .cmp(&left.1)
        .then_with(|| left.2.cmp(&right.2))
        .then_with(|| {
            crate::fs::natural_cmp(
                &candidates[left.0].relative_key,
                &candidates[right.0].relative_key,
            )
            .then_with(|| {
                candidates[left.0]
                    .relative
                    .cmp(&candidates[right.0].relative)
            })
        })
}

fn fuzzy_score_bytes(query: &[u8], text: &[u8]) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    if text.is_empty() {
        return None;
    }

    let mut score = 0i64;
    let mut scan_at = 0usize;
    let mut last_match = None;
    let mut streak = 0i64;

    for &byte in query {
        let mut found = None;
        for (index, &candidate) in text.iter().enumerate().skip(scan_at) {
            if candidate == byte {
                found = Some(index);
                break;
            }
        }
        let index = found?;

        if index == 0
            || matches!(
                text[index.saturating_sub(1)],
                b'/' | b'-' | b'_' | b' ' | b'.'
            )
        {
            score += 18;
        }

        if let Some(previous) = last_match {
            if index == previous + 1 {
                streak += 1;
                score += 20 + streak * 6;
            } else {
                streak = 0;
                score -= (index - previous - 1) as i64;
            }
        } else {
            score += 12;
            score -= index as i64;
        }

        score += 10;
        scan_at = index + 1;
        last_match = Some(index);
    }

    score -= (text.len().saturating_sub(scan_at)) as i64 / 3;
    Some(score)
}

#[cfg(test)]
#[path = "tests/fuzzy_matching.rs"]
mod tests;
