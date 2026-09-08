use std::{env, io, io::IsTerminal};

const SHELL_INTEGRATION_DOCS: &str = "https://elio-fm.github.io/docs/shell-integration/";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HelpTopic {
    Root,
    Shell,
    ShellInit,
    ShellInstall,
    ShellUninstall,
}

pub(super) fn print_help(topic: HelpTopic) {
    print!("{}", text(topic, style_enabled()));
}

#[derive(Clone, Copy)]
struct HelpStyle {
    heading: &'static str,
    literal: &'static str,
    link: &'static str,
    reset: &'static str,
}

impl HelpStyle {
    const PLAIN: Self = Self {
        heading: "",
        literal: "",
        link: "",
        reset: "",
    };

    const ANSI: Self = Self {
        heading: "\x1b[1m",
        literal: "\x1b[36m",
        link: "\x1b[4;36m",
        reset: "\x1b[0m",
    };
}

fn style_enabled() -> bool {
    io::stdout().is_terminal()
        && env::var_os("NO_COLOR").is_none()
        && env::var_os("TERM").is_none_or(|term| term != "dumb")
}

fn text(topic: HelpTopic, styled: bool) -> String {
    let style = if styled {
        HelpStyle::ANSI
    } else {
        HelpStyle::PLAIN
    };
    match topic {
        HelpTopic::Root => root_text(style),
        HelpTopic::Shell => shell_text(style),
        HelpTopic::ShellInit => shell_command_text(
            style,
            "init",
            "<SHELL>",
            "Target shell: bash, zsh, fish, or nu",
        ),
        HelpTopic::ShellInstall => shell_command_text(
            style,
            "install",
            "[SHELL]",
            "Target shell (bash, zsh, fish, or nu); detected when omitted",
        ),
        HelpTopic::ShellUninstall => shell_command_text(
            style,
            "uninstall",
            "[SHELL]",
            "Target shell (bash, zsh, fish, or nu); detected when omitted",
        ),
    }
}

fn root_text(style: HelpStyle) -> String {
    let HelpStyle {
        heading,
        literal,
        link,
        reset,
    } = style;
    format!(
        concat!(
            "{heading}Usage:{reset} {literal}elio{reset} [OPTIONS] [PATH]\n",
            "\n",
            "{heading}Arguments:{reset}\n",
            "  [PATH]  Start in a directory, or focus a file in its parent directory\n",
            "\n",
            "{heading}Options:{reset}\n",
            "      {literal}--chooser-file{reset} <FILE>     Write selected paths to FILE; use \"-\" for stdout\n",
            "      {literal}--config{reset} <FILE>           Load configuration from FILE\n",
            "      {literal}--cwd-file{reset} <FILE>         Write the final directory to FILE on exit\n",
            "      {literal}--theme{reset} <FILE>            Load a theme from FILE\n",
            "  {literal}-h, --help{reset}                    Print help\n",
            "  {literal}-V, --version{reset}                 Print version\n",
            "\n",
            "{heading}Shell integration:{reset}\n",
            "  {literal}elio shell init{reset} <SHELL>       Print shell integration code\n",
            "  {literal}elio shell install{reset} [SHELL]    Install integration; detect shell when omitted\n",
            "  {literal}elio shell uninstall{reset} [SHELL]  Remove integration; detect shell when omitted\n",
            "\n",
            "  Supported shells: bash, zsh, fish, nu\n",
            "\n",
            "{heading}CLI documentation:{reset} {link}https://elio-fm.github.io/docs/cli/{reset}\n",
        ),
        heading = heading,
        literal = literal,
        link = link,
        reset = reset,
    )
}

fn shell_text(style: HelpStyle) -> String {
    let HelpStyle {
        heading,
        literal,
        link,
        reset,
    } = style;
    format!(
        concat!(
            "{heading}Usage:{reset} {literal}elio shell{reset} <COMMAND>\n",
            "\n",
            "{heading}Commands:{reset}\n",
            "  {literal}init{reset} <SHELL>       Print shell integration code\n",
            "  {literal}install{reset} [SHELL]    Install integration; detect shell when omitted\n",
            "  {literal}uninstall{reset} [SHELL]  Remove integration; detect shell when omitted\n",
            "\n",
            "{heading}Options:{reset}\n",
            "  {literal}-h, --help{reset}  Print help\n",
            "\n",
            "{heading}Shell integration documentation:{reset} {link}{docs}{reset}\n",
        ),
        heading = heading,
        literal = literal,
        link = link,
        docs = SHELL_INTEGRATION_DOCS,
        reset = reset,
    )
}

fn shell_command_text(
    style: HelpStyle,
    command: &str,
    shell_argument: &str,
    argument_description: &str,
) -> String {
    let HelpStyle {
        heading,
        literal,
        link,
        reset,
    } = style;
    format!(
        concat!(
            "{heading}Usage:{reset} {literal}elio shell {command}{reset} {shell_argument}\n",
            "\n",
            "{heading}Arguments:{reset}\n",
            "  {shell_argument}  {argument_description}\n",
            "\n",
            "{heading}Options:{reset}\n",
            "  {literal}-h, --help{reset}  Print help\n",
            "\n",
            "{heading}Shell integration documentation:{reset} {link}{docs}{reset}\n",
        ),
        heading = heading,
        literal = literal,
        command = command,
        shell_argument = shell_argument,
        argument_description = argument_description,
        link = link,
        docs = SHELL_INTEGRATION_DOCS,
        reset = reset,
    )
}

#[cfg(test)]
#[path = "tests/help_output.rs"]
mod tests;
