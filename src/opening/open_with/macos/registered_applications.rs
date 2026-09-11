// This module is only compiled on macOS. Launch Services supplies the same
// registered handlers shown by Finder's Open With menu. Applications launch
// through `open`, preserving macOS sandboxing and document handoff behavior.

use std::{collections::HashSet, path::Path};

use objc2_foundation::{NSFileManager, NSString};

use super::{
    super::{OpenWithApplication, available_applications, terminal_editors},
    launch_services,
};

pub(in crate::opening::open_with) fn applications_for(path: &Path) -> Vec<OpenWithApplication> {
    let Some(path_str) = path.to_str() else {
        return Vec::new();
    };

    let mut applications = file_url_handlers(path_str);
    if available_applications::is_editor_compatible(path) {
        merge_unique(
            &mut applications,
            generic_editor_applications(path_str, path),
        );
        merge_unique(
            &mut applications,
            terminal_editors::macos_applications(path),
        );
    }
    sort_applications(&mut applications);
    applications
}

fn file_url_handlers(path: &str) -> Vec<OpenWithApplication> {
    let (handlers, default_path) = launch_services::file_handlers(path);
    let file_manager = NSFileManager::defaultManager();
    let mut applications = Vec::with_capacity(handlers.len());

    for handler in handlers {
        let application_path = NSString::from_str(&handler.path);
        let display_name = file_manager
            .displayNameAtPath(&application_path)
            .to_string();
        let is_default = default_path.as_deref() == Some(handler.path.as_str());
        applications.push(OpenWithApplication {
            display_name,
            application_id: handler.bundle_identifier,
            program: "open".to_string(),
            args: vec!["-a".to_string(), handler.path, path.to_string()],
            is_default,
            requires_terminal: false,
        });
    }

    applications.sort_unstable_by(|left, right| {
        right.is_default.cmp(&left.is_default).then_with(|| {
            left.display_name
                .to_ascii_lowercase()
                .cmp(&right.display_name.to_ascii_lowercase())
        })
    });
    applications
}

fn generic_editor_applications(path_str: &str, path: &Path) -> Vec<OpenWithApplication> {
    let file_manager = NSFileManager::defaultManager();
    let mut applications = Vec::new();

    for content_type in generic_editor_content_types(path) {
        let role_mask = launch_services::ROLES_VIEWER | launch_services::ROLES_EDITOR;
        let default_bundle_identifier =
            launch_services::default_role_handler_for_content_type(content_type, role_mask);
        let mut bundle_identifiers =
            launch_services::role_handlers_for_content_type(content_type, role_mask);

        if let Some(default) = default_bundle_identifier.as_ref()
            && !bundle_identifiers
                .iter()
                .any(|identifier| identifier == default)
        {
            bundle_identifiers.insert(0, default.clone());
        }

        for bundle_identifier in bundle_identifiers {
            let application_paths =
                launch_services::application_paths_for_bundle_identifier(&bundle_identifier);
            let display_name = application_paths
                .iter()
                .find_map(|application_path| {
                    let application_path = NSString::from_str(application_path);
                    let display_name = file_manager
                        .displayNameAtPath(&application_path)
                        .to_string();
                    (!display_name.is_empty()).then_some(display_name)
                })
                .unwrap_or_else(|| bundle_identifier.clone());

            applications.push(OpenWithApplication {
                display_name,
                application_id: Some(bundle_identifier.clone()),
                program: "open".to_string(),
                args: vec![
                    "-b".to_string(),
                    bundle_identifier.clone(),
                    path_str.to_string(),
                ],
                is_default: default_bundle_identifier.as_deref()
                    == Some(bundle_identifier.as_str()),
                requires_terminal: false,
            });
        }
    }
    applications
}

fn generic_editor_content_types(path: &Path) -> &'static [&'static str] {
    use crate::{file_classification::PreviewKind, fs::EntryKind};

    match crate::file_classification::inspect_path(path, EntryKind::File)
        .preview
        .kind
    {
        PreviewKind::Source => &["public.source-code", "public.plain-text"],
        PreviewKind::Markdown => &["net.daringfireball.markdown", "public.plain-text"],
        PreviewKind::PlainText | PreviewKind::Csv => &["public.plain-text"],
        _ => &[],
    }
}

fn merge_unique(target: &mut Vec<OpenWithApplication>, applications: Vec<OpenWithApplication>) {
    let mut seen = target.iter().map(identity_key).collect::<HashSet<_>>();
    for application in applications {
        if seen.insert(identity_key(&application)) {
            target.push(application);
        }
    }
}

fn sort_applications(applications: &mut [OpenWithApplication]) {
    applications.sort_unstable_by(|left, right| {
        right
            .is_default
            .cmp(&left.is_default)
            .then_with(|| is_environment_editor(right).cmp(&is_environment_editor(left)))
            .then_with(|| left.requires_terminal.cmp(&right.requires_terminal))
            .then_with(|| {
                left.display_name
                    .to_ascii_lowercase()
                    .cmp(&right.display_name.to_ascii_lowercase())
            })
    });
}

fn is_environment_editor(application: &OpenWithApplication) -> bool {
    application.display_name.contains("($VISUAL)") || application.display_name.contains("($EDITOR)")
}

fn identity_key(application: &OpenWithApplication) -> String {
    if let Some(identifier) = application.application_id.as_ref() {
        return format!("bundle:{identifier}");
    }
    format!(
        "program:{}:{}",
        application.program,
        if application.requires_terminal {
            "terminal"
        } else {
            "gui"
        }
    )
}

#[cfg(test)]
#[path = "tests/registered_applications.rs"]
mod tests;
