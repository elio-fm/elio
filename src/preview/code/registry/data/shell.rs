use super::super::types::{RegistryEntry, entry, language};
use crate::file_info::CodeBackend;

pub(super) const LANGUAGES: &[RegistryEntry] = &[
    entry(
        language("sh", "Shell", CodeBackend::Syntect, None),
        &["sh"],
        &[".profile", ".xprofile", ".xsessionrc", ".envrc"],
        &["sh"],
        &["sh", "shell"],
        &["sh", "shell"],
    ),
    entry(
        language("bash", "Bash", CodeBackend::Syntect, None),
        &["bash"],
        &[
            ".bashrc",
            ".bash_profile",
            ".bash_login",
            ".bash_logout",
            ".bash_aliases",
            "pkgbuild",
        ],
        &["bash"],
        &["bash"],
        &["bash"],
    ),
    entry(
        language("zsh", "Zsh", CodeBackend::Syntect, None),
        &["zsh"],
        &[".zshrc", ".zprofile", ".zshenv", ".zlogin", ".zlogout"],
        &["zsh"],
        &["zsh"],
        &["zsh"],
    ),
    entry(
        language("ksh", "KornShell", CodeBackend::Syntect, None),
        &["ksh"],
        &[".kshrc", ".mkshrc"],
        &["ksh"],
        &["ksh"],
        &["ksh"],
    ),
    entry(
        language("fish", "Fish", CodeBackend::Syntect, None),
        &["fish"],
        &[],
        &["fish"],
        &["fish"],
        &["fish"],
    ),
    entry(
        language("powershell", "PowerShell", CodeBackend::Syntect, None),
        &["ps1", "psm1", "psd1"],
        &[],
        &["pwsh", "powershell"],
        &["powershell", "pwsh", "ps1"],
        &["powershell", "pwsh", "ps1"],
    ),
];
