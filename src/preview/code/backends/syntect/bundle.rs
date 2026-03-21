use crate::preview::code::syntax_manifest::curated_syntax;
use std::sync::OnceLock;
use syntect::{
    dumps::from_uncompressed_data,
    parsing::{SyntaxReference, SyntaxSet},
};

pub(super) fn syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(|| {
        from_uncompressed_data(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/elio-curated-syntaxes.packdump"
        )))
        .expect("embedded curated syntect syntax dump should deserialize")
    })
}

pub(super) fn find_syntax<'a>(
    syntax_set: &'a SyntaxSet,
    code_syntax: &str,
) -> Option<&'a SyntaxReference> {
    let lookup_token = curated_syntax(code_syntax)?.lookup_token;
    syntax_set
        .find_syntax_by_token(lookup_token)
        .or_else(|| syntax_set.find_syntax_by_extension(lookup_token))
}
