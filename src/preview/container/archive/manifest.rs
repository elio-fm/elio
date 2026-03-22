use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct ZipManifestMetadata {
    pub(super) title: Option<String>,
    pub(super) version: Option<String>,
    pub(super) main_class: Option<String>,
    pub(super) created_by: Option<String>,
    pub(super) automatic_module: Option<String>,
}

pub(super) fn parse_zip_manifest(contents: &str) -> ZipManifestMetadata {
    let mut fields = BTreeMap::<String, String>::new();
    let mut current_key: Option<String> = None;

    for line in contents.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix(' ') {
            if let Some(key) = &current_key
                && let Some(value) = fields.get_mut(key)
            {
                value.push_str(rest);
            }
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            current_key = None;
            continue;
        };
        let key = key.trim().to_string();
        let value = value.trim().to_string();
        current_key = Some(key.clone());
        fields.insert(key, value);
    }

    ZipManifestMetadata {
        title: fields
            .get("Implementation-Title")
            .cloned()
            .or_else(|| fields.get("Bundle-Name").cloned()),
        version: fields
            .get("Implementation-Version")
            .cloned()
            .or_else(|| fields.get("Bundle-Version").cloned()),
        main_class: fields.get("Main-Class").cloned(),
        created_by: fields.get("Created-By").cloned(),
        automatic_module: fields.get("Automatic-Module-Name").cloned(),
    }
}

pub(super) fn zip_manifest_sections(
    manifest: &ZipManifestMetadata,
) -> Vec<(&'static str, Vec<(&'static str, String)>)> {
    if manifest.is_empty() {
        return Vec::new();
    }

    let mut fields = Vec::new();
    if let Some(value) = &manifest.title {
        fields.push(("Title", value.clone()));
    }
    if let Some(value) = &manifest.version {
        fields.push(("Version", value.clone()));
    }
    if let Some(value) = &manifest.main_class {
        fields.push(("Main-Class", value.clone()));
    }
    if let Some(value) = &manifest.automatic_module {
        fields.push(("Module", value.clone()));
    }
    if let Some(value) = &manifest.created_by {
        fields.push(("Created By", value.clone()));
    }
    vec![("Manifest", fields)]
}

impl ZipManifestMetadata {
    pub(super) fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.version.is_none()
            && self.main_class.is_none()
            && self.created_by.is_none()
            && self.automatic_module.is_none()
    }
}

#[cfg(test)]
mod tests {
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
}
