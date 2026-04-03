mod languages;
mod shell;
mod structured;
mod tooling;
mod web;

use super::types::RegistryEntry;

const GROUPS: &[&[RegistryEntry]] = &[
    structured::LANGUAGES,
    web::LANGUAGES,
    tooling::LANGUAGES,
    languages::LANGUAGES,
    shell::LANGUAGES,
];

pub(super) fn all_languages() -> impl Iterator<Item = &'static RegistryEntry> {
    GROUPS.iter().flat_map(|group| group.iter())
}
