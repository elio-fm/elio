use super::{HelpTopic, text};

#[test]
fn styled_help_applies_semantic_terminal_styles() {
    let help = text(HelpTopic::Root, true);

    assert!(help.contains("\x1b[1mUsage:\x1b[0m \x1b[36melio\x1b[0m [OPTIONS] [PATH]"));
    assert!(help.contains("\x1b[36m--chooser-file\x1b[0m"));
    assert!(help.contains("\x1b[36m--chooser-file\x1b[0m <FILE>"));
    assert!(help.contains("\x1b[4;36mhttps://elio-fm.github.io/docs/cli/\x1b[0m"));

    let shell_help = text(HelpTopic::ShellInstall, true);
    assert!(shell_help.contains("\x1b[36melio shell install\x1b[0m [SHELL]"));
    assert!(
        shell_help.contains("\x1b[4;36mhttps://elio-fm.github.io/docs/shell-integration/\x1b[0m")
    );
}
