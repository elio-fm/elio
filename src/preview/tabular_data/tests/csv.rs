use super::*;
use std::io::Write;

// ── CSV ───────────────────────────────────────────────────────────────────────

#[test]
fn csv_preview_renders_aligned_table_with_detected_header() {
    let root = temp_path("csv-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("data.csv");
    fs::write(&path, "name,age,city\nAlice,28,New York\nBob,34,London\n")
        .expect("failed to write csv");

    let entry = file_entry(path.clone());
    let preview = build_preview(&entry);
    let text: Vec<String> = preview.lines().iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Data);
    assert_eq!(preview.detail.as_deref(), Some("CSV file"));

    // Header values present
    assert!(text.iter().any(|l| l.contains("name")), "{text:?}");
    assert!(text.iter().any(|l| l.contains("age")), "{text:?}");
    assert!(text.iter().any(|l| l.contains("city")), "{text:?}");
    // Data values present
    assert!(text.iter().any(|l| l.contains("Alice")), "{text:?}");
    assert!(text.iter().any(|l| l.contains("London")), "{text:?}");
    // Footer present
    assert!(text.iter().any(|l| l.contains("rows")), "{text:?}");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn csv_preview_synthesizes_headers_for_all_text_data() {
    let root = temp_path("csv-no-header");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("words.csv");
    // All-text file — ambiguous, should get synthetic col1/col2 headers.
    fs::write(&path, "foo,bar\nbaz,qux\n").expect("failed to write csv");

    let entry = file_entry(path.clone());
    let preview = build_preview(&entry);
    let text: Vec<String> = preview.lines().iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Data);
    assert!(
        text.iter().any(|l| l.contains("col1")),
        "expected synthetic 'col1' header; got: {text:?}"
    );
    assert!(
        text.iter().any(|l| l.contains("col2")),
        "expected synthetic 'col2' header; got: {text:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn tsv_preview_uses_tab_delimiter() {
    let root = temp_path("tsv-basic");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("data.tsv");
    fs::write(&path, "product\tprice\nApple\t1.20\nBanana\t0.50\n").expect("failed to write tsv");

    let entry = file_entry(path.clone());
    let preview = build_preview(&entry);
    let text: Vec<String> = preview.lines().iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Data);
    assert_eq!(preview.detail.as_deref(), Some("TSV file"));
    assert!(text.iter().any(|l| l.contains("product")), "{text:?}");
    assert!(text.iter().any(|l| l.contains("Apple")), "{text:?}");

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn csv_preview_handles_quoted_fields_with_embedded_commas() {
    let root = temp_path("csv-quoted");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("places.csv");
    fs::write(
        &path,
        "city,country\n\"New York, NY\",USA\n\"London, UK\",UK\n",
    )
    .expect("failed to write csv");

    let entry = file_entry(path.clone());
    let preview = build_preview(&entry);
    let text: Vec<String> = preview.lines().iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Data);
    assert!(
        text.iter()
            .any(|l| l.contains("New York, NY") || l.contains("New York")),
        "expected quoted field content; got: {text:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn csv_preview_reports_64kib_truncation_for_large_file_with_few_rows() {
    let root = temp_path("csv-byte-truncated");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("big.csv");

    // Write a CSV with 5 data rows, each row padded to make the file > 64 KiB.
    // The header + 5 fat rows fit above 64 KiB so read_text_preview truncates
    // before row 50, yet the row count never hits the MAX_PREVIEW_ROWS cap.
    let padding = "x".repeat(14_000);
    let mut file = fs::File::create(&path).expect("failed to create csv");
    writeln!(file, "label,value,notes").expect("write header");
    for i in 1..=5u32 {
        writeln!(file, "row{i},{i},{padding}").expect("write row");
    }
    drop(file);

    let entry = file_entry(path.clone());
    let preview = build_preview(&entry);
    let text: Vec<String> = preview.lines().iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Data);
    // Footer must mention the 64 KiB cut, not a false row-cap message.
    assert!(
        text.iter().any(|l| l.contains("truncated at 64 KiB")),
        "expected '64 KiB' truncation note in footer; got: {text:?}"
    );
    assert!(
        !text.iter().any(|l| l.contains("more rows in file")),
        "must not claim 'more rows in file' when the cut was at 64 KiB; got: {text:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn csv_preview_reports_row_cap_truncation_for_file_with_many_short_rows() {
    let root = temp_path("csv-row-truncated");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("many.csv");

    // Write a CSV with 60 short rows — all fit within 64 KiB, but our cap is 50.
    let mut file = fs::File::create(&path).expect("failed to create csv");
    writeln!(file, "id,value").expect("write header");
    for i in 1..=60u32 {
        writeln!(file, "{i},{}", i * 10).expect("write row");
    }
    drop(file);

    let entry = file_entry(path.clone());
    let preview = build_preview(&entry);
    let text: Vec<String> = preview.lines().iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Data);
    // Footer must mention row cap, not 64 KiB.
    assert!(
        text.iter().any(|l| l.contains("more rows in file")),
        "expected 'more rows in file' note; got: {text:?}"
    );
    assert!(
        !text.iter().any(|l| l.contains("64 KiB")),
        "must not claim 64 KiB truncation when file fits in read window; got: {text:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
