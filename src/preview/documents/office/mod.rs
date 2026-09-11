mod apple_pages;
mod legacy_word;
mod microsoft_office;
mod open_document;

pub(super) use self::{
    apple_pages::extract_apple_pages_metadata, legacy_word::extract_legacy_word_metadata,
    microsoft_office::extract_microsoft_office_metadata,
    open_document::extract_open_document_metadata,
};
