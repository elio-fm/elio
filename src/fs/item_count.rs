use std::{fs, io, path::Path};

pub(crate) fn count_directory_items(dir: &Path, show_hidden: bool) -> io::Result<usize> {
    let mut count = 0usize;
    for item in fs::read_dir(dir)? {
        let item = match item {
            Ok(item) => item,
            Err(_) => continue,
        };
        if !show_hidden && super::is_hidden(item.file_name().as_os_str()) {
            continue;
        }
        count += 1;
    }
    Ok(count)
}
