use super::parse_zip_manifest;

#[test]
fn parse_zip_manifest_supports_bundle_fallback_and_continuations() {
    let manifest = parse_zip_manifest(concat!(
        "Bundle-Name: Elio Runtime\n",
        "Bundle-Version: 2.0.0\n",
        "Main-Class: io.elio.Main\n",
        "Automatic-Module-Name: io.elio.\n",
        " core\n",
    ));

    assert_eq!(manifest.title.as_deref(), Some("Elio Runtime"));
    assert_eq!(manifest.version.as_deref(), Some("2.0.0"));
    assert_eq!(manifest.main_class.as_deref(), Some("io.elio.Main"));
    assert_eq!(manifest.automatic_module.as_deref(), Some("io.elio.core"));
    assert!(manifest.created_by.is_none());
}
