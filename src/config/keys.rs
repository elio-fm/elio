use serde::Deserialize;

/// A browser action that can be triggered by a configurable key binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Action {
    Quit,
    Yank,
    Cut,
    Paste,
    Trash,
    Create,
    Rename,
    CopyPath,
    SearchFolders,
    Open,
    OpenWith,
    Sort,
    ToggleView,
    ToggleHidden,
    ScrollPreviewLeft,
    ScrollPreviewRight,
    ScrollPreviewUp,
    ScrollPreviewDown,
}

/// Single-character key bindings for browser actions.
/// All fields default to the built-in keys; set any field in `[keys]` in
/// `config.toml` to override that binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct KeyBindings {
    pub quit: char,
    pub yank: char,
    pub cut: char,
    pub paste: char,
    pub trash: char,
    pub create: char,
    pub rename: char,
    pub copy_path: char,
    pub search_folders: char,
    pub open: char,
    pub open_with: char,
    pub sort: char,
    pub toggle_view: char,
    pub toggle_hidden: char,
    pub scroll_preview_left: char,
    pub scroll_preview_right: char,
    pub scroll_preview_up: char,
    pub scroll_preview_down: char,
}

/// Characters that are hard-wired to non-configurable actions and may not be
/// used as key binding values.
const RESERVED_CHARS: &[char] = &[
    'h', 'j', 'k', 'l', // navigation (vim keys)
    'g', 'G', // go-to overlay / jump to last
    '?', // help
    '[', ']', // page stepping (epub / comic / pdf)
    '+', '=', '-', '_', // grid zoom
    ' ', // toggle selection
];

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            quit: 'q',
            yank: 'y',
            cut: 'x',
            paste: 'p',
            trash: 'd',
            create: 'a',
            rename: 'r',
            copy_path: 'c',
            search_folders: 'f',
            open: 'o',
            open_with: 'O',
            sort: 's',
            toggle_view: 'v',
            toggle_hidden: '.',
            scroll_preview_left: 'H',
            scroll_preview_right: 'L',
            scroll_preview_up: 'K',
            scroll_preview_down: 'J',
        }
    }
}

#[derive(Deserialize, Default)]
pub(super) struct KeysConfigOverride {
    quit: Option<String>,
    yank: Option<String>,
    cut: Option<String>,
    paste: Option<String>,
    trash: Option<String>,
    create: Option<String>,
    rename: Option<String>,
    copy_path: Option<String>,
    search_folders: Option<String>,
    open: Option<String>,
    open_with: Option<String>,
    sort: Option<String>,
    toggle_view: Option<String>,
    toggle_hidden: Option<String>,
    scroll_preview_left: Option<String>,
    scroll_preview_right: Option<String>,
    scroll_preview_up: Option<String>,
    scroll_preview_down: Option<String>,
}

impl KeyBindings {
    /// Returns the action bound to `c`, if any.
    pub(crate) fn action_for(&self, c: char) -> Option<Action> {
        match c {
            _ if c == self.quit => Some(Action::Quit),
            _ if c == self.yank => Some(Action::Yank),
            _ if c == self.cut => Some(Action::Cut),
            _ if c == self.paste => Some(Action::Paste),
            _ if c == self.trash => Some(Action::Trash),
            _ if c == self.create => Some(Action::Create),
            _ if c == self.rename => Some(Action::Rename),
            _ if c == self.copy_path => Some(Action::CopyPath),
            _ if c == self.search_folders => Some(Action::SearchFolders),
            _ if c == self.open => Some(Action::Open),
            _ if c == self.open_with => Some(Action::OpenWith),
            _ if c == self.sort => Some(Action::Sort),
            _ if c == self.toggle_view => Some(Action::ToggleView),
            _ if c == self.toggle_hidden => Some(Action::ToggleHidden),
            _ if c == self.scroll_preview_left => Some(Action::ScrollPreviewLeft),
            _ if c == self.scroll_preview_right => Some(Action::ScrollPreviewRight),
            _ if c == self.scroll_preview_up => Some(Action::ScrollPreviewUp),
            _ if c == self.scroll_preview_down => Some(Action::ScrollPreviewDown),
            _ => None,
        }
    }

    /// Parse a full config TOML string and return only the resolved key
    /// bindings. Falls back to defaults on parse error. Used by integration
    /// tests that need a `KeyBindings` from an override string without going
    /// through the process-wide `OnceLock`.
    #[cfg(test)]
    pub(crate) fn from_toml_str(s: &str) -> Self {
        super::Config::from_str(s)
            .map(|config| config.keys)
            .unwrap_or_else(|_| Self::default())
    }

    pub(super) fn from_override(overrides: KeysConfigOverride, defaults: &Self) -> Self {
        // Each entry: (field_name, user_override_string, default_char)
        let raw: [(&str, Option<String>, char); 18] = [
            ("quit", overrides.quit, defaults.quit),
            ("yank", overrides.yank, defaults.yank),
            ("cut", overrides.cut, defaults.cut),
            ("paste", overrides.paste, defaults.paste),
            ("trash", overrides.trash, defaults.trash),
            ("create", overrides.create, defaults.create),
            ("rename", overrides.rename, defaults.rename),
            ("copy_path", overrides.copy_path, defaults.copy_path),
            (
                "search_folders",
                overrides.search_folders,
                defaults.search_folders,
            ),
            ("open", overrides.open, defaults.open),
            ("open_with", overrides.open_with, defaults.open_with),
            ("sort", overrides.sort, defaults.sort),
            ("toggle_view", overrides.toggle_view, defaults.toggle_view),
            (
                "toggle_hidden",
                overrides.toggle_hidden,
                defaults.toggle_hidden,
            ),
            (
                "scroll_preview_left",
                overrides.scroll_preview_left,
                defaults.scroll_preview_left,
            ),
            (
                "scroll_preview_right",
                overrides.scroll_preview_right,
                defaults.scroll_preview_right,
            ),
            (
                "scroll_preview_up",
                overrides.scroll_preview_up,
                defaults.scroll_preview_up,
            ),
            (
                "scroll_preview_down",
                overrides.scroll_preview_down,
                defaults.scroll_preview_down,
            ),
        ];

        // Step 1: parse each override string independently, falling back to
        // default on any format or reserved-char error.
        // (resolved_char, is_user_set)
        let mut candidates: [(char, bool); 18] = [(' ', false); 18];
        for (index, (name, override_str, default)) in raw.iter().enumerate() {
            candidates[index] = match override_str {
                None => (*default, false),
                Some(value) => {
                    let mut chars = value.chars();
                    match (chars.next(), chars.next()) {
                        (Some(c), None) if RESERVED_CHARS.contains(&c) => {
                            eprintln!(
                                "elio: keys.{name}: '{c}' is reserved and cannot be rebound; \
                                 using default '{default}'"
                            );
                            (*default, false)
                        }
                        (Some(c), None) if c.is_control() => {
                            eprintln!(
                                "elio: keys.{name}: control characters cannot be used as key \
                                 bindings; using default '{default}'"
                            );
                            (*default, false)
                        }
                        (Some(c), None) => (c, true),
                        _ => {
                            eprintln!(
                                "elio: keys.{name}: {value:?} is not a single character; \
                                 using default '{default}'"
                            );
                            (*default, false)
                        }
                    }
                }
            };
        }

        // Step 2: reject user-set bindings that collide with any other binding
        // (user-set or default). Loop until stable so that reverting one
        // binding does not silently leave a conflict with another.
        loop {
            let mut changed = false;
            for index in 0..18 {
                if !candidates[index].1 {
                    continue;
                }
                let candidate = candidates[index].0;
                let collision = (0..18)
                    .filter(|&other_index| other_index != index)
                    .any(|other_index| candidates[other_index].0 == candidate);
                if collision {
                    let (name, _, default) = &raw[index];
                    let other = raw
                        .iter()
                        .enumerate()
                        .filter(|&(other_index, _)| {
                            other_index != index && candidates[other_index].0 == candidate
                        })
                        .map(|(_, (name, _, _))| *name)
                        .next()
                        .unwrap_or("another key");
                    eprintln!(
                        "elio: keys.{name}: '{candidate}' is already bound to {other}; \
                         using default '{default}'"
                    );
                    candidates[index] = (*default, false);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        // Step 3: build from the resolved candidates (order matches `raw`).
        let resolved = |index: usize| candidates[index].0;
        Self {
            quit: resolved(0),
            yank: resolved(1),
            cut: resolved(2),
            paste: resolved(3),
            trash: resolved(4),
            create: resolved(5),
            rename: resolved(6),
            copy_path: resolved(7),
            search_folders: resolved(8),
            open: resolved(9),
            open_with: resolved(10),
            sort: resolved(11),
            toggle_view: resolved(12),
            toggle_hidden: resolved(13),
            scroll_preview_left: resolved(14),
            scroll_preview_right: resolved(15),
            scroll_preview_up: resolved(16),
            scroll_preview_down: resolved(17),
        }
    }
}
