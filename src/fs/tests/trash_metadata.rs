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
