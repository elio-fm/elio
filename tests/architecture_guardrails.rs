use std::{fs, path::Path, path::PathBuf};

#[test]
fn preview_does_not_depend_on_app() {
    assert_tree_has_no_pattern("src/preview", "app::", &[]);
}

#[test]
fn preview_theme_access_stays_behind_preview_appearance_adapter() {
    assert_tree_has_no_pattern("src/preview", "ui::theme", &["src/preview/appearance.rs"]);
}

#[test]
fn fs_does_not_depend_on_app() {
    assert_tree_has_no_pattern("src/fs", "app::", &[]);
}

#[test]
fn file_info_does_not_depend_on_app() {
    assert_tree_has_no_pattern("src/file_info", "app::", &[]);
}

fn assert_tree_has_no_pattern(root: &str, forbidden: &str, allowed_files: &[&str]) {
    let mut files = Vec::new();
    collect_rust_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join(root),
        &mut files,
    );
    files.sort();

    let violations = files
        .into_iter()
        .filter_map(|path| {
            let relative = relative_path(&path);
            if allowed_files.contains(&relative.as_str()) {
                return None;
            }

            let contents = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
            strip_comments_and_strings(&contents)
                .contains(forbidden)
                .then_some(relative)
        })
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "found forbidden `{forbidden}` references:\n{}",
        violations.join("\n")
    );
}

fn collect_rust_files(root: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(root)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()));
    let mut children = entries
        .map(|entry| entry.expect("directory entry should be readable").path())
        .collect::<Vec<_>>();
    children.sort();

    for child in children {
        if child.is_dir() {
            collect_rust_files(&child, files);
        } else if child.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(child);
        }
    }
}

fn relative_path(path: &Path) -> String {
    path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .expect("source path should live under workspace root")
        .to_string_lossy()
        .replace('\\', "/")
}

fn strip_comments_and_strings(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = String::with_capacity(source.len());
    let mut index = 0usize;

    while index < bytes.len() {
        if let Some((prefix_len, hash_count)) = raw_string_prefix(&bytes[index..]) {
            output.push_str(&" ".repeat(prefix_len));
            index += prefix_len;

            while index < bytes.len() {
                let byte = bytes[index];
                output.push(if byte == b'\n' { '\n' } else { ' ' });
                index += 1;

                if byte == b'"' && has_n_hashes(bytes, index, hash_count) {
                    output.push_str(&" ".repeat(hash_count));
                    index += hash_count;
                    break;
                }
            }
            continue;
        }

        if bytes[index..].starts_with(b"//") {
            output.push_str("  ");
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                output.push(' ');
                index += 1;
            }
            continue;
        }

        if bytes[index..].starts_with(b"/*") {
            output.push_str("  ");
            index += 2;
            let mut depth = 1usize;
            while index < bytes.len() && depth > 0 {
                if bytes[index..].starts_with(b"/*") {
                    output.push_str("  ");
                    index += 2;
                    depth += 1;
                    continue;
                }
                if bytes[index..].starts_with(b"*/") {
                    output.push_str("  ");
                    index += 2;
                    depth -= 1;
                    continue;
                }
                output.push(if bytes[index] == b'\n' { '\n' } else { ' ' });
                index += 1;
            }
            continue;
        }

        if bytes[index..].starts_with(b"b\"") || bytes[index] == b'"' {
            let prefix_len = usize::from(bytes[index] == b'b') + 1;
            output.push_str(&" ".repeat(prefix_len));
            index += prefix_len;
            let mut escaped = false;
            while index < bytes.len() {
                let byte = bytes[index];
                output.push(if byte == b'\n' { '\n' } else { ' ' });
                index += 1;

                if escaped {
                    escaped = false;
                    continue;
                }
                if byte == b'\\' {
                    escaped = true;
                    continue;
                }
                if byte == b'"' {
                    break;
                }
            }
            continue;
        }

        output.push(bytes[index] as char);
        index += 1;
    }

    output
}

fn raw_string_prefix(bytes: &[u8]) -> Option<(usize, usize)> {
    if bytes.is_empty() {
        return None;
    }

    let offset = if bytes.starts_with(b"br") || bytes.starts_with(b"rb") {
        2
    } else if bytes[0] == b'r' {
        1
    } else {
        return None;
    };

    let mut index = offset;
    while bytes.get(index) == Some(&b'#') {
        index += 1;
    }
    (bytes.get(index) == Some(&b'"')).then_some((index + 1, index - offset))
}

fn has_n_hashes(bytes: &[u8], start: usize, count: usize) -> bool {
    bytes
        .get(start..start + count)
        .is_some_and(|slice| slice.iter().all(|&byte| byte == b'#'))
}
