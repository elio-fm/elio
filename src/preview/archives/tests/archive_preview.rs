use super::{ArchiveFormat, ZIP_BUILT_IN_READER_MAX_BYTES, build_zip_archive_preview};
use std::{
    fs,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[test]
fn oversized_zip_skips_built_in_reader() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "elio-oversized-zip-reader-skip-{unique}-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("huge.zip");
    let file = fs::File::create(&path).expect("failed to create sparse zip fixture");
    file.set_len(ZIP_BUILT_IN_READER_MAX_BYTES + 1)
        .expect("failed to size sparse zip fixture");

    let started_at = Instant::now();
    let preview = build_zip_archive_preview(&path, ArchiveFormat::Zip, None, &|| false);

    assert!(preview.is_none());
    assert!(
        started_at.elapsed().as_millis() < 100,
        "oversized ZIP files should skip the uncancellable zip reader"
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
