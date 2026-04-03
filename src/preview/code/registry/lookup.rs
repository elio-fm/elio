use super::{
    data,
    types::{RegisteredLanguage, RegistryEntry},
};

pub(crate) fn language_for_extension(ext: &str) -> Option<RegisteredLanguage> {
    language_for_alias(ext, |entry| entry.extensions)
}

pub(crate) fn language_for_exact_name(name: &str) -> Option<RegisteredLanguage> {
    let normalized = normalize(name);
    if is_env_name(&normalized) {
        return language_for_code_syntax("dotenv");
    }
    data::all_languages()
        .find(|entry| contains(entry.exact_filenames, &normalized))
        .map(|entry| entry.language)
}

pub(crate) fn language_for_shebang(interpreter: &str) -> Option<RegisteredLanguage> {
    language_for_alias(interpreter, |entry| entry.shebang_interpreters)
}

pub(crate) fn language_for_modeline(token: &str) -> Option<RegisteredLanguage> {
    language_for_alias(token, |entry| entry.modelines)
}

pub(crate) fn language_for_markdown_fence(token: &str) -> Option<RegisteredLanguage> {
    language_for_alias(token, |entry| entry.markdown_fences)
}

pub(crate) fn language_for_code_syntax(code_syntax: &str) -> Option<RegisteredLanguage> {
    let normalized = normalize(code_syntax);
    data::all_languages()
        .find(|entry| entry.language.canonical_id == normalized)
        .map(|entry| entry.language)
}

pub(crate) fn display_label_for_code_syntax(code_syntax: &str) -> Option<&'static str> {
    language_for_code_syntax(code_syntax).map(|language| language.display_label)
}

fn language_for_alias(
    value: &str,
    aliases: impl Fn(&RegistryEntry) -> &'static [&'static str],
) -> Option<RegisteredLanguage> {
    let normalized = normalize(value);
    data::all_languages()
        .find(|entry| contains(aliases(entry), &normalized))
        .map(|entry| entry.language)
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn contains(values: &[&str], needle: &str) -> bool {
    values.contains(&needle)
}

fn is_env_name(name: &str) -> bool {
    name == ".env" || name.starts_with(".env.")
}
