mod data;
mod lookup;
#[cfg(test)]
mod tests;
mod types;

pub(crate) use self::lookup::{
    display_label_for_code_syntax, language_for_code_syntax, language_for_exact_name,
    language_for_extension, language_for_markdown_fence, language_for_modeline,
    language_for_shebang,
};
pub(crate) use self::types::RegisteredLanguage;
