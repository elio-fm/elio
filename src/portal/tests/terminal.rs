use super::*;

fn child() -> Vec<OsString> {
    [
        "/usr/bin/elio",
        "--portal-chooser",
        "--socket",
        "/tmp/request.sock",
    ]
    .map(OsString::from)
    .to_vec()
}

#[test]
fn chooser_args_give_portal_windows_a_stable_identity() {
    let cases = [
        (
            TerminalAdapter::Kitty,
            &[
                "--title",
                "elio File Chooser",
                "--app-id",
                "io.github.elio_fm.elio.filechooser",
            ][..],
        ),
        (
            TerminalAdapter::Ghostty,
            &[
                "--title=elio File Chooser",
                "--class=io.github.elio_fm.elio.filechooser",
                "-e",
            ][..],
        ),
        (
            TerminalAdapter::Foot,
            &[
                "--title=elio File Chooser",
                "--app-id=io.github.elio_fm.elio.filechooser",
            ][..],
        ),
        (
            TerminalAdapter::WezTerm,
            &[
                "start",
                "--class",
                "io.github.elio_fm.elio.filechooser",
                "--",
            ][..],
        ),
        (
            TerminalAdapter::Alacritty,
            &[
                "--title",
                "elio File Chooser",
                "--class",
                "io.github.elio_fm.elio.filechooser",
                "--command",
            ][..],
        ),
        (
            TerminalAdapter::Rio,
            &["--title-placeholder", "elio File Chooser", "--command"][..],
        ),
        (
            TerminalAdapter::Konsole,
            &["-p", "tabtitle=elio File Chooser", "-e"][..],
        ),
        (
            TerminalAdapter::Gnome,
            &[
                "--title",
                "elio File Chooser",
                "--class=io.github.elio_fm.elio.filechooser",
                "--",
            ][..],
        ),
        (
            TerminalAdapter::Xterm,
            &[
                "-T",
                "elio File Chooser",
                "-class",
                "io.github.elio_fm.elio.filechooser",
                "-e",
            ][..],
        ),
    ];

    for (terminal, prefix) in cases {
        assert_eq!(
            terminal.chooser_args(&child()),
            prefix
                .iter()
                .copied()
                .chain([
                    "/usr/bin/elio",
                    "--portal-chooser",
                    "--socket",
                    "/tmp/request.sock"
                ])
                .map(OsString::from)
                .collect::<Vec<_>>(),
        );
    }
}

#[test]
fn supported_config_names_round_trip() {
    for name in [
        "kitty",
        "ghostty",
        "foot",
        "wezterm",
        "alacritty",
        "rio",
        "konsole",
        "gnome-terminal",
        "xterm",
    ] {
        assert_eq!(TerminalAdapter::from_name(name).unwrap().name(), name);
    }
}
