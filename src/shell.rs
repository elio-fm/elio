use std::{
    ffi::OsString,
    io::{self, ErrorKind, Write},
    path::Path,
    process::Command,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShellInvocation {
    pub(crate) program: OsString,
    pub(crate) args: Vec<OsString>,
}

impl ShellInvocation {
    fn label(&self) -> String {
        if self.args.is_empty() {
            self.program.to_string_lossy().into_owned()
        } else {
            let args = self
                .args
                .iter()
                .map(|arg| arg.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");
            format!("{} {args}", self.program.to_string_lossy())
        }
    }
}

pub(crate) fn run_in_current_terminal(cwd: &Path) -> Result<(), String> {
    ensure_cwd_exists(cwd)?;

    let invocations = shell_invocations();
    let tried: Vec<String> = invocations.iter().map(ShellInvocation::label).collect();

    print_shell_banner(cwd)?;
    for invocation in invocations {
        match Command::new(&invocation.program)
            .args(&invocation.args)
            .current_dir(cwd)
            .env("ELIO_SHELL", "1")
            .env(
                "ELIO_LEVEL",
                next_shell_level(std::env::var_os("ELIO_LEVEL")),
            )
            .status()
        {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                ensure_cwd_exists(cwd)?;
            }
            Err(error) => {
                return Err(format!(
                    "Could not open shell in {}: {error}",
                    cwd.display()
                ));
            }
        }
    }

    Err(format!(
        "Could not find a shell to open in {} (tried {})",
        cwd.display(),
        tried.join(", ")
    ))
}

fn print_shell_banner(cwd: &Path) -> Result<(), String> {
    let mut stdout = io::stdout();
    let (label_style, value_style, dim_style, reset_style) = if shell_banner_color_enabled() {
        ("\x1b[1;36m", "\x1b[1m", "\x1b[2m", "\x1b[0m")
    } else {
        ("", "", "", "")
    };

    writeln!(
        stdout,
        "{label_style}elio:{reset_style} opened shell in {value_style}{}{reset_style}",
        cwd.display()
    )
    .and_then(|()| {
        writeln!(
            stdout,
            "{dim_style}return:{reset_style} {}",
            shell_return_hint()
        )
    })
    .and_then(|()| writeln!(stdout))
    .and_then(|()| stdout.flush())
    .map_err(|error| format!("Could not prepare shell in {}: {error}", cwd.display()))
}

fn shell_banner_color_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none()
        && std::env::var_os("TERM").is_none_or(|term| term != "dumb")
}

fn shell_return_hint() -> &'static str {
    #[cfg(windows)]
    {
        "exit"
    }

    #[cfg(not(windows))]
    {
        "exit or Ctrl-D"
    }
}

pub(crate) fn shell_invocations() -> Vec<ShellInvocation> {
    #[cfg(windows)]
    {
        windows_shell_invocations(std::env::var_os("COMSPEC"))
    }

    #[cfg(not(windows))]
    {
        unix_shell_invocations(std::env::var_os("SHELL"))
    }
}

#[cfg(any(not(windows), test))]
fn unix_shell_invocations(shell: Option<OsString>) -> Vec<ShellInvocation> {
    let fallback = ShellInvocation {
        program: OsString::from("/bin/sh"),
        args: Vec::new(),
    };

    let Some(program) = non_empty_env_value(shell) else {
        return vec![fallback];
    };

    let configured = ShellInvocation {
        program,
        args: Vec::new(),
    };
    if configured.program == fallback.program {
        vec![configured]
    } else {
        vec![configured, fallback]
    }
}

#[cfg(any(windows, test))]
fn windows_shell_invocations(comspec: Option<OsString>) -> Vec<ShellInvocation> {
    let mut invocations = Vec::new();
    if let Some(program) = non_empty_env_value(comspec) {
        invocations.push(ShellInvocation {
            program,
            args: Vec::new(),
        });
    }

    invocations.extend([
        ShellInvocation {
            program: OsString::from("pwsh"),
            args: vec![OsString::from("-NoLogo")],
        },
        ShellInvocation {
            program: OsString::from("powershell"),
            args: vec![OsString::from("-NoLogo")],
        },
        ShellInvocation {
            program: OsString::from("cmd"),
            args: Vec::new(),
        },
    ]);
    invocations
}

fn non_empty_env_value(value: Option<OsString>) -> Option<OsString> {
    let value = value?;
    (!value.to_string_lossy().trim().is_empty()).then_some(value)
}

fn next_shell_level(current: Option<OsString>) -> OsString {
    let Some(current) = current else {
        return OsString::from("1");
    };
    let Ok(level) = current.to_string_lossy().trim().parse::<u32>() else {
        return OsString::from("1");
    };
    OsString::from(level.saturating_add(1).to_string())
}

fn ensure_cwd_exists(cwd: &Path) -> Result<(), String> {
    match cwd.try_exists() {
        Ok(true) => Ok(()),
        Ok(false) => Err(format!(
            "Cannot open shell in {}: folder no longer exists",
            cwd.display()
        )),
        Err(error) => Err(format!("Cannot open shell in {}: {error}", cwd.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_shell_uses_shell_env_then_sh_fallback() {
        assert_eq!(
            unix_shell_invocations(Some(OsString::from("/bin/bash"))),
            vec![
                ShellInvocation {
                    program: OsString::from("/bin/bash"),
                    args: Vec::new(),
                },
                ShellInvocation {
                    program: OsString::from("/bin/sh"),
                    args: Vec::new(),
                },
            ]
        );
    }

    #[test]
    fn unix_shell_uses_only_sh_when_shell_is_empty() {
        assert_eq!(
            unix_shell_invocations(Some(OsString::from(""))),
            vec![ShellInvocation {
                program: OsString::from("/bin/sh"),
                args: Vec::new(),
            }]
        );
    }

    #[test]
    fn unix_shell_does_not_duplicate_sh_fallback() {
        assert_eq!(
            unix_shell_invocations(Some(OsString::from("/bin/sh"))),
            vec![ShellInvocation {
                program: OsString::from("/bin/sh"),
                args: Vec::new(),
            }]
        );
    }

    #[test]
    fn windows_shell_uses_comspec_before_powershell_fallbacks() {
        assert_eq!(
            windows_shell_invocations(Some(OsString::from(r"C:\Windows\System32\cmd.exe"))),
            vec![
                ShellInvocation {
                    program: OsString::from(r"C:\Windows\System32\cmd.exe"),
                    args: Vec::new(),
                },
                ShellInvocation {
                    program: OsString::from("pwsh"),
                    args: vec![OsString::from("-NoLogo")],
                },
                ShellInvocation {
                    program: OsString::from("powershell"),
                    args: vec![OsString::from("-NoLogo")],
                },
                ShellInvocation {
                    program: OsString::from("cmd"),
                    args: Vec::new(),
                },
            ]
        );
    }

    #[test]
    fn windows_shell_falls_back_when_comspec_is_empty() {
        assert_eq!(
            windows_shell_invocations(Some(OsString::from(" "))),
            vec![
                ShellInvocation {
                    program: OsString::from("pwsh"),
                    args: vec![OsString::from("-NoLogo")],
                },
                ShellInvocation {
                    program: OsString::from("powershell"),
                    args: vec![OsString::from("-NoLogo")],
                },
                ShellInvocation {
                    program: OsString::from("cmd"),
                    args: Vec::new(),
                },
            ]
        );
    }

    #[test]
    fn shell_level_starts_at_one() {
        assert_eq!(next_shell_level(None), OsString::from("1"));
        assert_eq!(
            next_shell_level(Some(OsString::from(""))),
            OsString::from("1")
        );
        assert_eq!(
            next_shell_level(Some(OsString::from("not-a-number"))),
            OsString::from("1")
        );
    }

    #[test]
    fn shell_level_increments_existing_level() {
        assert_eq!(
            next_shell_level(Some(OsString::from("1"))),
            OsString::from("2")
        );
        assert_eq!(
            next_shell_level(Some(OsString::from(" 41 "))),
            OsString::from("42")
        );
    }

    #[test]
    fn cwd_check_reports_deleted_folder() {
        let missing = std::env::temp_dir().join(format!(
            "elio-missing-shell-cwd-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock should be after unix epoch")
                .as_nanos()
        ));

        let error = ensure_cwd_exists(&missing).expect_err("missing cwd should fail");

        assert!(
            error.contains("folder no longer exists"),
            "unexpected error: {error}"
        );
    }
}
