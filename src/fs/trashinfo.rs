use std::path::{Path, PathBuf};

pub(crate) fn parse_original_path(content: &str) -> Option<PathBuf> {
    for line in content.lines() {
        if let Some(encoded) = line.trim().strip_prefix("Path=") {
            return Some(PathBuf::from(percent_decode(encoded)));
        }
    }
    None
}

pub(crate) fn original_basename_from_path_value(encoded_path: &str) -> Option<String> {
    let decoded = percent_decode(encoded_path);
    let name = Path::new(&decoded).file_name()?.to_string_lossy();
    (!name.is_empty()).then(|| name.into_owned())
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_nibble(bytes[i + 1]), hex_nibble(bytes[i + 2]))
        {
            out.push(hi << 4 | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_original_path_from_trashinfo_content() {
        let path = parse_original_path(
            "[Trash Info]\nPath=/home/user/Reports/report%20final.pdf\nDeletionDate=2024-03-15T10:30:00\n",
        )
        .expect("path should parse");

        assert_eq!(path, PathBuf::from("/home/user/Reports/report final.pdf"));
    }

    #[test]
    fn derives_basename_from_encoded_path_value() {
        let name = original_basename_from_path_value("/home/user/Camera/photo%201.jpeg")
            .expect("basename should parse");

        assert_eq!(name, "photo 1.jpeg");
    }
}
