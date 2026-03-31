use super::*;
use rusqlite::Connection;
use std::{fs, io::Write};

// ── SQLite ────────────────────────────────────────────────────────────────────

#[test]
fn sqlite_preview_shows_header_and_tables() {
    let root = temp_path("sqlite-basic");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("app.sqlite");

    let conn = Connection::open(&path).expect("failed to open sqlite db");
    conn.execute_batch(
        "CREATE TABLE accounts (
             id    INTEGER PRIMARY KEY,
             name  TEXT NOT NULL,
             email TEXT
         );
         CREATE TABLE posts (
             id         INTEGER PRIMARY KEY,
             account_id INTEGER NOT NULL,
             body       TEXT
         );",
    )
    .expect("failed to create tables");
    // Insert into `accounts`, which sorts before `posts` alphabetically and will
    // be the first table shown, triggering sample-row rendering.
    conn.execute(
        "INSERT INTO accounts (name, email) VALUES (?1, ?2)",
        ["Alice", "alice@example.com"],
    )
    .expect("failed to insert row");
    drop(conn);

    let entry = file_entry(path.clone());
    let preview = build_preview(&entry);

    let text: Vec<String> = preview.lines().iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Data);
    assert_eq!(preview.detail.as_deref(), Some("SQLite database"));

    // Summary section header
    assert!(
        text.iter().any(|l| l.contains("SQLite 3")),
        "expected 'SQLite 3' section header; got: {text:?}"
    );
    // Page size field
    assert!(
        text.iter().any(|l| l.contains("Page size")),
        "expected 'Page size' field; got: {text:?}"
    );
    // Both tables listed
    assert!(
        text.iter().any(|l| l.contains("accounts")),
        "expected 'accounts' table; got: {text:?}"
    );
    assert!(
        text.iter().any(|l| l.contains("posts")),
        "expected 'posts' table; got: {text:?}"
    );
    // Column names for accounts table
    assert!(
        text.iter().any(|l| l.contains("name")),
        "expected 'name' column; got: {text:?}"
    );
    assert!(
        text.iter().any(|l| l.contains("email")),
        "expected 'email' column; got: {text:?}"
    );
    // Sample row value
    assert!(
        text.iter().any(|l| l.contains("Alice")),
        "expected sample row with 'Alice'; got: {text:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sqlite_preview_shows_views() {
    let root = temp_path("sqlite-view");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("app.sqlite");

    let conn = Connection::open(&path).expect("failed to open sqlite db");
    conn.execute_batch(
        "CREATE TABLE items (id INTEGER PRIMARY KEY, value TEXT);
         CREATE VIEW active_items AS SELECT * FROM items WHERE value IS NOT NULL;",
    )
    .expect("failed to create schema");
    drop(conn);

    let entry = file_entry(path.clone());
    let preview = build_preview(&entry);
    let text: Vec<String> = preview.lines().iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Data);
    assert!(
        text.iter().any(|l| l.contains("active_items")),
        "expected view name; got: {text:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn non_sqlite_db_file_falls_through_to_binary_preview() {
    let root = temp_path("sqlite-not-sqlite");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("custom.db");
    // Write a file that is clearly not SQLite (no magic bytes).
    fs::write(&path, b"\x00\x01\x02\x03not sqlite at all\x00").expect("failed to write file");

    let entry = file_entry(path.clone());
    let preview = build_preview(&entry);

    // Must NOT produce a Data/SQLite preview.
    assert_ne!(
        preview.kind,
        PreviewKind::Data,
        "non-SQLite .db file should not get a Data preview"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn sqlite_preview_shows_generated_columns() {
    let root = temp_path("sqlite-generated");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("calc.sqlite");

    let conn = Connection::open(&path).expect("failed to open sqlite db");
    conn.execute_batch(
        "CREATE TABLE products (
             id        INTEGER PRIMARY KEY,
             price     REAL NOT NULL,
             tax_rate  REAL NOT NULL DEFAULT 0.2,
             -- VIRTUAL generated column (hidden = 2 in table_xinfo)
             price_inc REAL GENERATED ALWAYS AS (price * (1 + tax_rate)) VIRTUAL
         );",
    )
    .expect("failed to create table with generated column");
    drop(conn);

    let entry = file_entry(path.clone());
    let preview = build_preview(&entry);
    let text: Vec<String> = preview.lines().iter().map(line_text).collect();

    assert_eq!(preview.kind, PreviewKind::Data);
    assert!(
        text.iter().any(|l| l.contains("price_inc")),
        "generated column 'price_inc' should be visible; got: {text:?}"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

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
