use crate::file_info::{CodeBackend, PreviewSpec, StructuredFormat};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RegisteredLanguage {
    pub canonical_id: &'static str,
    pub display_label: &'static str,
    pub backend: CodeBackend,
    pub structured_format: Option<StructuredFormat>,
}

impl RegisteredLanguage {
    pub(crate) const fn preview_spec(self) -> PreviewSpec {
        PreviewSpec::code(self.canonical_id, self.backend, self.structured_format)
    }
}

#[derive(Clone, Copy)]
pub(super) struct RegistryEntry {
    pub(super) language: RegisteredLanguage,
    pub(super) extensions: &'static [&'static str],
    pub(super) exact_filenames: &'static [&'static str],
    pub(super) shebang_interpreters: &'static [&'static str],
    pub(super) modelines: &'static [&'static str],
    pub(super) markdown_fences: &'static [&'static str],
}

pub(super) const fn language(
    canonical_id: &'static str,
    display_label: &'static str,
    backend: CodeBackend,
    structured_format: Option<StructuredFormat>,
) -> RegisteredLanguage {
    RegisteredLanguage {
        canonical_id,
        display_label,
        backend,
        structured_format,
    }
}

pub(super) const fn entry(
    language: RegisteredLanguage,
    extensions: &'static [&'static str],
    exact_filenames: &'static [&'static str],
    shebang_interpreters: &'static [&'static str],
    modelines: &'static [&'static str],
    markdown_fences: &'static [&'static str],
) -> RegistryEntry {
    RegistryEntry {
        language,
        extensions,
        exact_filenames,
        shebang_interpreters,
        modelines,
        markdown_fences,
    }
}
