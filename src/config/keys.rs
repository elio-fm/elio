use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;

/// A browser action that can be triggered by a configurable key binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Action {
    Quit,
    QuitWithoutCd,
    Yank,
    Cut,
    Paste,
    Trash,
    DeletePermanently,
    Create,
    Rename,
    CopyPath,
    SearchFolders,
    Zoxide,
    Shell,
    Open,
    OpenWith,
    OpenOrEnter,
    Sort,
    ToggleView,
    ToggleHidden,
    NavLeft,
    NavDown,
    NavUp,
    NavRight,
    ScrollPreviewLeft,
    ScrollPreviewRight,
    ScrollPreviewUp,
    ScrollPreviewDown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum NamedKey {
    Left,
    Right,
    Up,
    Down,
    Enter,
}

impl NamedKey {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "enter" => Some(Self::Enter),
            _ => None,
        }
    }

    fn matches(self, code: KeyCode) -> bool {
        matches!(
            (self, code),
            (Self::Left, KeyCode::Left)
                | (Self::Right, KeyCode::Right)
                | (Self::Up, KeyCode::Up)
                | (Self::Down, KeyCode::Down)
                | (
                    Self::Enter,
                    KeyCode::Enter | KeyCode::Char('\n') | KeyCode::Char('\r')
                )
        )
    }
}

impl std::fmt::Display for NamedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Left => "←",
            Self::Right => "→",
            Self::Up => "↑",
            Self::Down => "↓",
            Self::Enter => "Enter",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum KeySpec {
    Char(char),
    Named(NamedKey),
}

impl KeySpec {
    fn single_char(self) -> Option<char> {
        match self {
            Self::Char(c) => Some(c),
            Self::Named(_) => None,
        }
    }

    fn matches_event(self, key: KeyEvent) -> bool {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return false;
        }

        match self {
            Self::Char(c) => matches!(key.code, KeyCode::Char(actual) if actual == c),
            Self::Named(named) => named.matches(key.code),
        }
    }
}

impl std::fmt::Display for KeySpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Char(c) => write!(f, "{c}"),
            Self::Named(named) => write!(f, "{named}"),
        }
    }
}

/// One or more key bindings for a browser action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KeyList(Vec<KeySpec>);

impl KeyList {
    fn one(c: char) -> Self {
        Self(vec![KeySpec::Char(c)])
    }

    fn contains(&self, key: KeySpec) -> bool {
        self.0.contains(&key)
    }

    fn matches_event(&self, key: KeyEvent) -> bool {
        self.0.iter().any(|spec| spec.matches_event(key))
    }

    fn keys(&self) -> impl Iterator<Item = KeySpec> + '_ {
        self.0.iter().copied()
    }

    pub(crate) fn single_char(&self) -> Option<char> {
        match self.0.as_slice() {
            [spec] => spec.single_char(),
            _ => None,
        }
    }
}

impl std::fmt::Display for KeyList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, key) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str("/")?;
            }
            write!(f, "{key}")?;
        }
        Ok(())
    }
}

impl PartialEq<char> for KeyList {
    fn eq(&self, other: &char) -> bool {
        self.0.as_slice() == [KeySpec::Char(*other)]
    }
}

/// Key bindings for browser actions.
/// All fields default to the built-in keys; set any field in `[keys]` in
/// `config.toml` to override that binding. Values may be either a single
/// string (`open = "o"`) or a list of strings (`open = ["o", "e"]`).
/// Character bindings must be one character; named bindings currently support
/// `left`, `right`, `up`, `down`, and `enter`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KeyBindings {
    pub quit: KeyList,
    pub quit_without_cd: KeyList,
    pub yank: KeyList,
    pub cut: KeyList,
    pub paste: KeyList,
    pub trash: KeyList,
    pub delete_permanently: KeyList,
    pub create: KeyList,
    pub rename: KeyList,
    pub copy_path: KeyList,
    pub search_folders: KeyList,
    pub zoxide: KeyList,
    pub shell: KeyList,
    pub open: KeyList,
    pub open_with: KeyList,
    pub open_or_enter: KeyList,
    pub sort: KeyList,
    pub toggle_view: KeyList,
    pub toggle_hidden: KeyList,
    pub nav_left: KeyList,
    pub nav_down: KeyList,
    pub nav_up: KeyList,
    pub nav_right: KeyList,
    pub scroll_preview_left: KeyList,
    pub scroll_preview_right: KeyList,
    pub scroll_preview_up: KeyList,
    pub scroll_preview_down: KeyList,
}

/// Characters that are hard-wired to non-configurable actions and may not be
/// used as key binding values.
const RESERVED_CHARS: &[char] = &[
    'g', 'G', // go-to overlay / jump to last
    '?', // help
    '[', ']', // page stepping (epub / comic / pdf)
    '+', '=', '-', '_', // grid zoom
    ' ', // toggle selection
];

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            quit: KeyList::one('q'),
            quit_without_cd: KeyList::one('Q'),
            yank: KeyList::one('y'),
            cut: KeyList::one('x'),
            paste: KeyList::one('p'),
            trash: KeyList::one('d'),
            delete_permanently: KeyList::one('D'),
            create: KeyList::one('a'),
            rename: KeyList::one('r'),
            copy_path: KeyList::one('c'),
            search_folders: KeyList::one('f'),
            zoxide: KeyList::one('z'),
            shell: KeyList::one('!'),
            open: KeyList::one('o'),
            open_with: KeyList::one('O'),
            open_or_enter: KeyList(vec![KeySpec::Named(NamedKey::Enter)]),
            sort: KeyList::one('s'),
            toggle_view: KeyList::one('v'),
            toggle_hidden: KeyList::one('.'),
            nav_left: KeyList(vec![KeySpec::Char('h'), KeySpec::Named(NamedKey::Left)]),
            nav_down: KeyList(vec![KeySpec::Char('j'), KeySpec::Named(NamedKey::Down)]),
            nav_up: KeyList(vec![KeySpec::Char('k'), KeySpec::Named(NamedKey::Up)]),
            nav_right: KeyList(vec![KeySpec::Char('l'), KeySpec::Named(NamedKey::Right)]),
            scroll_preview_left: KeyList::one('H'),
            scroll_preview_right: KeyList::one('L'),
            scroll_preview_up: KeyList::one('K'),
            scroll_preview_down: KeyList::one('J'),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum KeyConfigOverride {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize, Default)]
pub(super) struct KeysConfigOverride {
    quit: Option<KeyConfigOverride>,
    quit_without_cd: Option<KeyConfigOverride>,
    yank: Option<KeyConfigOverride>,
    cut: Option<KeyConfigOverride>,
    paste: Option<KeyConfigOverride>,
    trash: Option<KeyConfigOverride>,
    delete_permanently: Option<KeyConfigOverride>,
    create: Option<KeyConfigOverride>,
    rename: Option<KeyConfigOverride>,
    copy_path: Option<KeyConfigOverride>,
    search_folders: Option<KeyConfigOverride>,
    zoxide: Option<KeyConfigOverride>,
    shell: Option<KeyConfigOverride>,
    open: Option<KeyConfigOverride>,
    open_with: Option<KeyConfigOverride>,
    open_or_enter: Option<KeyConfigOverride>,
    sort: Option<KeyConfigOverride>,
    toggle_view: Option<KeyConfigOverride>,
    toggle_hidden: Option<KeyConfigOverride>,
    nav_left: Option<KeyConfigOverride>,
    nav_down: Option<KeyConfigOverride>,
    nav_up: Option<KeyConfigOverride>,
    nav_right: Option<KeyConfigOverride>,
    scroll_preview_left: Option<KeyConfigOverride>,
    scroll_preview_right: Option<KeyConfigOverride>,
    scroll_preview_up: Option<KeyConfigOverride>,
    scroll_preview_down: Option<KeyConfigOverride>,
}

struct RawBinding {
    name: &'static str,
    override_value: Option<KeyConfigOverride>,
    default: KeyList,
}

impl KeyBindings {
    /// Returns the action bound to `key`, if any.
    pub(crate) fn action_for_key(&self, key: KeyEvent) -> Option<Action> {
        let bindings = [
            (&self.quit, Action::Quit),
            (&self.quit_without_cd, Action::QuitWithoutCd),
            (&self.yank, Action::Yank),
            (&self.cut, Action::Cut),
            (&self.paste, Action::Paste),
            (&self.trash, Action::Trash),
            (&self.delete_permanently, Action::DeletePermanently),
            (&self.create, Action::Create),
            (&self.rename, Action::Rename),
            (&self.copy_path, Action::CopyPath),
            (&self.search_folders, Action::SearchFolders),
            (&self.zoxide, Action::Zoxide),
            (&self.shell, Action::Shell),
            (&self.open, Action::Open),
            (&self.open_with, Action::OpenWith),
            (&self.open_or_enter, Action::OpenOrEnter),
            (&self.sort, Action::Sort),
            (&self.toggle_view, Action::ToggleView),
            (&self.toggle_hidden, Action::ToggleHidden),
            (&self.nav_left, Action::NavLeft),
            (&self.nav_down, Action::NavDown),
            (&self.nav_up, Action::NavUp),
            (&self.nav_right, Action::NavRight),
            (&self.scroll_preview_left, Action::ScrollPreviewLeft),
            (&self.scroll_preview_right, Action::ScrollPreviewRight),
            (&self.scroll_preview_up, Action::ScrollPreviewUp),
            (&self.scroll_preview_down, Action::ScrollPreviewDown),
        ];

        bindings
            .iter()
            .find_map(|(keys, action)| keys.matches_event(key).then_some(*action))
    }

    /// Returns the action bound to `c`, if any.
    pub(crate) fn action_for(&self, c: char) -> Option<Action> {
        self.action_for_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
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
        let raw = vec![
            RawBinding {
                name: "quit",
                override_value: overrides.quit,
                default: defaults.quit.clone(),
            },
            RawBinding {
                name: "quit_without_cd",
                override_value: overrides.quit_without_cd,
                default: defaults.quit_without_cd.clone(),
            },
            RawBinding {
                name: "yank",
                override_value: overrides.yank,
                default: defaults.yank.clone(),
            },
            RawBinding {
                name: "cut",
                override_value: overrides.cut,
                default: defaults.cut.clone(),
            },
            RawBinding {
                name: "paste",
                override_value: overrides.paste,
                default: defaults.paste.clone(),
            },
            RawBinding {
                name: "trash",
                override_value: overrides.trash,
                default: defaults.trash.clone(),
            },
            RawBinding {
                name: "delete_permanently",
                override_value: overrides.delete_permanently,
                default: defaults.delete_permanently.clone(),
            },
            RawBinding {
                name: "create",
                override_value: overrides.create,
                default: defaults.create.clone(),
            },
            RawBinding {
                name: "rename",
                override_value: overrides.rename,
                default: defaults.rename.clone(),
            },
            RawBinding {
                name: "copy_path",
                override_value: overrides.copy_path,
                default: defaults.copy_path.clone(),
            },
            RawBinding {
                name: "search_folders",
                override_value: overrides.search_folders,
                default: defaults.search_folders.clone(),
            },
            RawBinding {
                name: "zoxide",
                override_value: overrides.zoxide,
                default: defaults.zoxide.clone(),
            },
            RawBinding {
                name: "shell",
                override_value: overrides.shell,
                default: defaults.shell.clone(),
            },
            RawBinding {
                name: "open",
                override_value: overrides.open,
                default: defaults.open.clone(),
            },
            RawBinding {
                name: "open_with",
                override_value: overrides.open_with,
                default: defaults.open_with.clone(),
            },
            RawBinding {
                name: "open_or_enter",
                override_value: overrides.open_or_enter,
                default: defaults.open_or_enter.clone(),
            },
            RawBinding {
                name: "sort",
                override_value: overrides.sort,
                default: defaults.sort.clone(),
            },
            RawBinding {
                name: "toggle_view",
                override_value: overrides.toggle_view,
                default: defaults.toggle_view.clone(),
            },
            RawBinding {
                name: "toggle_hidden",
                override_value: overrides.toggle_hidden,
                default: defaults.toggle_hidden.clone(),
            },
            RawBinding {
                name: "nav_left",
                override_value: overrides.nav_left,
                default: defaults.nav_left.clone(),
            },
            RawBinding {
                name: "nav_down",
                override_value: overrides.nav_down,
                default: defaults.nav_down.clone(),
            },
            RawBinding {
                name: "nav_up",
                override_value: overrides.nav_up,
                default: defaults.nav_up.clone(),
            },
            RawBinding {
                name: "nav_right",
                override_value: overrides.nav_right,
                default: defaults.nav_right.clone(),
            },
            RawBinding {
                name: "scroll_preview_left",
                override_value: overrides.scroll_preview_left,
                default: defaults.scroll_preview_left.clone(),
            },
            RawBinding {
                name: "scroll_preview_right",
                override_value: overrides.scroll_preview_right,
                default: defaults.scroll_preview_right.clone(),
            },
            RawBinding {
                name: "scroll_preview_up",
                override_value: overrides.scroll_preview_up,
                default: defaults.scroll_preview_up.clone(),
            },
            RawBinding {
                name: "scroll_preview_down",
                override_value: overrides.scroll_preview_down,
                default: defaults.scroll_preview_down.clone(),
            },
        ];

        // Step 1: parse each override independently, falling back to default on
        // any format or reserved-char error.
        // (resolved_keys, is_user_set)
        let mut candidates: Vec<(KeyList, bool)> = raw
            .iter()
            .map(|entry| match &entry.override_value {
                None => (entry.default.clone(), false),
                Some(value) => match parse_key_override(entry.name, value, &entry.default) {
                    Some(keys) => (keys, true),
                    None => (entry.default.clone(), false),
                },
            })
            .collect();

        // Step 2: reject user-set bindings that collide with any other binding
        // (user-set or default). Loop until stable so that reverting one
        // binding does not silently leave a conflict with another.
        loop {
            let mut changed = false;
            for index in 0..raw.len() {
                if !candidates[index].1 {
                    continue;
                }
                let collision = candidates[index].0.keys().find_map(|candidate| {
                    (0..raw.len())
                        .filter(|&other_index| other_index != index)
                        .find(|&other_index| candidates[other_index].0.contains(candidate))
                        .map(|other_index| (candidate, other_index))
                });

                if let Some((candidate, other_index)) = collision {
                    eprintln!(
                        "elio: keys.{}: '{}' is already bound to {}; using default '{}'",
                        raw[index].name, candidate, raw[other_index].name, raw[index].default
                    );
                    candidates[index] = (raw[index].default.clone(), false);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        // Step 3: build from the resolved candidates (order matches `raw`).
        let resolved = |index: usize| candidates[index].0.clone();
        Self {
            quit: resolved(0),
            quit_without_cd: resolved(1),
            yank: resolved(2),
            cut: resolved(3),
            paste: resolved(4),
            trash: resolved(5),
            delete_permanently: resolved(6),
            create: resolved(7),
            rename: resolved(8),
            copy_path: resolved(9),
            search_folders: resolved(10),
            zoxide: resolved(11),
            shell: resolved(12),
            open: resolved(13),
            open_with: resolved(14),
            open_or_enter: resolved(15),
            sort: resolved(16),
            toggle_view: resolved(17),
            toggle_hidden: resolved(18),
            nav_left: resolved(19),
            nav_down: resolved(20),
            nav_up: resolved(21),
            nav_right: resolved(22),
            scroll_preview_left: resolved(23),
            scroll_preview_right: resolved(24),
            scroll_preview_up: resolved(25),
            scroll_preview_down: resolved(26),
        }
    }
}

fn parse_key_override(name: &str, value: &KeyConfigOverride, default: &KeyList) -> Option<KeyList> {
    let values: Vec<&str> = match value {
        KeyConfigOverride::One(value) => vec![value.as_str()],
        KeyConfigOverride::Many(values) => values.iter().map(String::as_str).collect(),
    };

    if values.is_empty() {
        eprintln!(
            "elio: keys.{name}: key binding lists cannot be empty; using default '{default}'"
        );
        return None;
    }

    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        let spec = parse_key_spec(name, value, default)?;
        if parsed.contains(&spec) {
            eprintln!(
                "elio: keys.{name}: '{spec}' is listed more than once; using default '{default}'"
            );
            return None;
        }
        parsed.push(spec);
    }

    Some(KeyList(parsed))
}

fn parse_key_spec(name: &str, value: &str, default: &KeyList) -> Option<KeySpec> {
    if let Some(named) = NamedKey::parse(value) {
        return Some(KeySpec::Named(named));
    }

    let mut chars = value.chars();
    let Some(c) = chars.next() else {
        eprintln!(
            "elio: keys.{name}: empty strings cannot be used as key bindings; using default '{default}'"
        );
        return None;
    };
    if chars.next().is_some() {
        eprintln!(
            "elio: keys.{name}: {value:?} is not a single character or supported named key; using default '{default}'"
        );
        return None;
    }
    if RESERVED_CHARS.contains(&c) {
        eprintln!(
            "elio: keys.{name}: '{c}' is reserved and cannot be rebound; using default '{default}'"
        );
        return None;
    }
    if c.is_control() {
        eprintln!(
            "elio: keys.{name}: control characters cannot be used as key bindings; using default '{default}'"
        );
        return None;
    }

    Some(KeySpec::Char(c))
}
