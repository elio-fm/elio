pub(super) struct DesktopEntryCandidate {
    pub(super) name: String,
    pub(super) exec: String,
    pub(super) mime_types: Vec<String>,
    pub(super) terminal: bool,
    /// `OnlyShowIn=` entries (empty means "show everywhere").
    pub(super) only_show_in: Vec<String>,
    /// `NotShowIn=` entries (empty means "never explicitly hidden").
    pub(super) not_show_in: Vec<String>,
}

impl DesktopEntryCandidate {
    /// Returns `false` if this entry should be excluded on the current desktop.
    ///
    /// - Non-empty `only_show_in`: the current desktop must appear in the list.
    /// - Non-empty `not_show_in`: the current desktop must not appear in the list.
    ///
    /// When `desktops` is empty (`XDG_CURRENT_DESKTOP` unset), neither filter
    /// can be evaluated and all apps are allowed through.  This gives the most
    /// permissive behaviour on systems without a recognised desktop environment.
    pub(super) fn is_shown_in(&self, desktops: &[String]) -> bool {
        if desktops.is_empty() {
            return true;
        }
        if !self.only_show_in.is_empty() {
            let shown = self
                .only_show_in
                .iter()
                .any(|d| desktops.iter().any(|c| c.eq_ignore_ascii_case(d)));
            if !shown {
                return false;
            }
        }
        if self
            .not_show_in
            .iter()
            .any(|d| desktops.iter().any(|c| c.eq_ignore_ascii_case(d)))
        {
            return false;
        }
        true
    }
}

/// Returns the desktop-ids from the `[Removed Associations]` section of a
/// mimeapps.list file for the given MIME type.
pub(super) fn parse_mimeapps_removed(contents: &str, mime: &str) -> Vec<String> {
    parse_mimeapps_section(contents, mime, "[Removed Associations]")
}

/// Returns the ordered list of desktop-ids from the `[Default Applications]`
/// section of a mimeapps.list file for the given MIME type.
pub(super) fn parse_mimeapps_defaults(contents: &str, mime: &str) -> Vec<String> {
    parse_mimeapps_section(contents, mime, "[Default Applications]")
}

fn parse_mimeapps_section(contents: &str, mime: &str, section: &str) -> Vec<String> {
    let mut in_section = false;
    let mut result = Vec::new();

    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line == section;
            continue;
        }
        if !in_section || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim() == mime
        {
            result = value
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
        }
    }

    result
}

/// Parses a .desktop file and returns a `DesktopEntryCandidate` if the entry
/// is visible (not Hidden/NoDisplay) and has both `Name` and `Exec`.
pub(super) fn parse_desktop_entry(contents: &str) -> Option<DesktopEntryCandidate> {
    let mut in_entry = false;
    let mut name: Option<String> = None;
    let mut exec: Option<String> = None;
    let mut mime_types: Vec<String> = Vec::new();
    let mut hidden = false;
    let mut no_display = false;
    let mut terminal = false;
    let mut only_show_in: Vec<String> = Vec::new();
    let mut not_show_in: Vec<String> = Vec::new();

    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.starts_with('#') || line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            // Only accept the unlocalized Name= (localized keys have the form
            // Name[de]=…, whose key contains '[').
            "Name" if name.is_none() => {
                name = Some(value.to_string());
            }
            "Exec" => exec = Some(value.to_string()),
            "MimeType" => {
                mime_types = value
                    .split(';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            "Hidden" => hidden = value.eq_ignore_ascii_case("true"),
            "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
            "Terminal" => terminal = value.eq_ignore_ascii_case("true"),
            "OnlyShowIn" => {
                only_show_in = value
                    .split(';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            "NotShowIn" => {
                not_show_in = value
                    .split(';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            _ => {}
        }
    }

    if hidden || no_display {
        return None;
    }

    Some(DesktopEntryCandidate {
        name: name?,
        exec: exec?,
        mime_types,
        terminal,
        only_show_in,
        not_show_in,
    })
}

#[cfg(test)]
mod tests;
