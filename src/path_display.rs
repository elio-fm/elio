use std::path::Path;

pub(crate) fn user_facing(path: &Path) -> String {
    strip_windows_verbatim_prefix(&path.display().to_string())
}

fn strip_windows_verbatim_prefix(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::strip_windows_verbatim_prefix;

    #[test]
    fn removes_windows_drive_verbatim_prefix() {
        assert_eq!(
            strip_windows_verbatim_prefix(r"\\?\C:\Users\migue\AppData\Roaming"),
            r"C:\Users\migue\AppData\Roaming"
        );
    }

    #[test]
    fn removes_windows_unc_verbatim_prefix() {
        assert_eq!(
            strip_windows_verbatim_prefix(r"\\?\UNC\server\share\folder"),
            r"\\server\share\folder"
        );
    }

    #[test]
    fn leaves_regular_paths_unchanged() {
        assert_eq!(
            strip_windows_verbatim_prefix("/home/user/project"),
            "/home/user/project"
        );
        assert_eq!(
            strip_windows_verbatim_prefix(r"C:\Users\migue"),
            r"C:\Users\migue"
        );
    }
}
