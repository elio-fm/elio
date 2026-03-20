use std::error::Error;
use std::path::PathBuf;
use syntect::dumps::dump_to_uncompressed_file;
use syntect::parsing::SyntaxSet;

mod syntect_manifest {
    include!("src/preview/code/backends/syntect_manifest.rs");
}

use syntect_manifest::{CURATED_SYNTAXES, CuratedSyntax};

fn main() {
    if let Err(error) = generate_curated_syntax_bundle() {
        panic!("failed to generate curated syntect bundle: {error}");
    }
}

fn generate_curated_syntax_bundle() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/preview/code/backends/syntect_manifest.rs");
    println!("cargo:rerun-if-changed=assets/syntaxes");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    let syntax_dir = manifest_dir.join("assets/syntaxes");
    let bundle_path = out_dir.join("elio-curated-syntaxes.packdump");

    for entry in std::fs::read_dir(&syntax_dir)? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "sublime-syntax")
        {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    let mut curated_builder = SyntaxSet::load_defaults_newlines().into_builder();
    curated_builder.add_from_folder(&syntax_dir, true)?;

    let curated_syntax_set = curated_builder.build();
    let unresolved = curated_syntax_set.find_unlinked_contexts();
    if !unresolved.is_empty() {
        let message = unresolved.into_iter().collect::<Vec<_>>().join("\n");
        return Err(format!("unresolved curated syntax references:\n{message}").into());
    }

    for syntax in CURATED_SYNTAXES {
        if find_curated_syntax(&curated_syntax_set, syntax).is_none() {
            return Err(format!(
                "curated syntax {:?} with lookup token {:?} did not build",
                syntax.canonical_id, syntax.lookup_token
            )
            .into());
        }
    }

    dump_to_uncompressed_file(&curated_syntax_set, &bundle_path)?;
    Ok(())
}

fn find_curated_syntax<'a>(
    syntax_set: &'a SyntaxSet,
    syntax: &CuratedSyntax,
) -> Option<&'a syntect::parsing::SyntaxReference> {
    syntax_set
        .find_syntax_by_token(syntax.lookup_token)
        .or_else(|| syntax_set.find_syntax_by_extension(syntax.lookup_token))
}
