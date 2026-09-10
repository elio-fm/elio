use super::{fzf_default_options, fzf_options_with};

#[test]
fn fzf_options_use_invoking_user_values() {
    let options = fzf_options_with(|name| match name {
        "FZF_DEFAULT_OPTS" => Some("--height=40%".to_string()),
        "ELIO_ZOXIDE_OPTS" => Some("--no-mouse".to_string()),
        _ => None,
    });

    assert!(options.starts_with("--height=40% "));
    assert!(options.ends_with(" --no-mouse"));
    assert!(options.contains("--exact"));
}

#[test]
fn fzf_options_include_base_picker_flags() {
    let options = fzf_default_options();
    assert!(options.contains(&"--exact"));
    assert!(options.contains(&"--exit-0"));
}

#[cfg(target_os = "linux")]
#[test]
fn linux_fzf_options_include_colored_preview() {
    let options = fzf_default_options();
    assert!(options.contains(&"--preview-window=down,30%,sharp"));
    assert!(
        options
            .iter()
            .any(|option| option.contains("--color=always --group-directories-first"))
    );
}

#[cfg(all(unix, not(target_os = "linux")))]
#[test]
fn non_linux_unix_fzf_options_include_plain_preview() {
    let options = fzf_default_options();
    assert!(options.contains(&"--preview-window=down,30%,sharp"));
    assert!(options.contains(&"--preview='\\command -p ls -Cp {2..}'"));
}
