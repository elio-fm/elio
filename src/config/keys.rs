use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;
use std::collections::BTreeMap;

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
    RestoreFromTrash,
    CopyPath,
    SearchFolders,
    SearchFiles,
    Zoxide,
    Shell,
    Open,
    OpenWith,
    OpenOrEnter,
    GoTo,
    ToggleSelection,
    CyclePlacesNext,
    CyclePlacesPrevious,
    GoParent,
    PageUp,
    PageDown,
    JumpFirst,
    JumpLast,
    SelectAll,
    HistoryBack,
    HistoryForward,
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
    Space,
    Tab,
    BackTab,
    Backspace,
    PageUp,
    PageDown,
    Home,
    End,
    Function(u8),
}

impl NamedKey {
    fn parse(value: &str) -> Option<Self> {
        let normalized = value.to_ascii_lowercase();

        match normalized.as_str() {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "enter" => Some(Self::Enter),
            "space" => Some(Self::Space),
            "tab" => Some(Self::Tab),
            "backtab" => Some(Self::BackTab),
            "backspace" => Some(Self::Backspace),
            "pageup" => Some(Self::PageUp),
            "pagedown" => Some(Self::PageDown),
            "home" => Some(Self::Home),
            "end" => Some(Self::End),
            value if value.len() >= 2 && value.starts_with('f') => value[1..]
                .parse::<u8>()
                .ok()
                .filter(|number| (1..=12).contains(number))
                .map(Self::Function),
            _ => None,
        }
    }

    fn matches(self, code: KeyCode) -> bool {
        match (self, code) {
            (Self::Left, KeyCode::Left)
            | (Self::Right, KeyCode::Right)
            | (Self::Up, KeyCode::Up)
            | (Self::Down, KeyCode::Down)
            | (Self::Enter, KeyCode::Enter | KeyCode::Char('\n') | KeyCode::Char('\r'))
            | (Self::Space, KeyCode::Char(' '))
            | (Self::Tab, KeyCode::Tab)
            | (Self::BackTab, KeyCode::BackTab)
            | (Self::Backspace, KeyCode::Backspace)
            | (Self::PageUp, KeyCode::PageUp)
            | (Self::PageDown, KeyCode::PageDown)
            | (Self::Home, KeyCode::Home)
            | (Self::End, KeyCode::End) => true,
            (Self::Function(expected), KeyCode::F(actual)) => expected == actual,
            _ => false,
        }
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
            Self::Space => "Space",
            Self::Tab => "Tab",
            Self::BackTab => "Shift+Tab",
            Self::Backspace => "Backspace",
            Self::PageUp => "PageUp",
            Self::PageDown => "PageDown",
            Self::Home => "Home",
            Self::End => "End",
            Self::Function(number) => return write!(f, "F{number}"),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KeyContext {
    Normal,
    Trash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KeyContexts(u8);

impl KeyContexts {
    const NORMAL: Self = Self(0b01);
    const TRASH: Self = Self(0b10);
    const ALL: Self = Self(Self::NORMAL.0 | Self::TRASH.0);

    fn contains(self, context: KeyContext) -> bool {
        let mask = match context {
            KeyContext::Normal => Self::NORMAL,
            KeyContext::Trash => Self::TRASH,
        };
        self.intersects(mask)
    }

    fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl Action {
    fn key_contexts(self) -> KeyContexts {
        match self {
            Self::Rename => KeyContexts::NORMAL,
            Self::RestoreFromTrash => KeyContexts::TRASH,
            _ => KeyContexts::ALL,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct KeyModifierSpec {
    ctrl: bool,
    alt: bool,
    shift: bool,
    other: bool,
}

impl KeyModifierSpec {
    const NONE: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        other: false,
    };

    fn is_empty(self) -> bool {
        !self.ctrl && !self.alt && !self.shift && !self.other
    }

    fn from_event(modifiers: KeyModifiers) -> Self {
        let supported = KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT;
        Self {
            ctrl: modifiers.contains(KeyModifiers::CONTROL),
            alt: modifiers.contains(KeyModifiers::ALT),
            shift: modifiers.contains(KeyModifiers::SHIFT),
            other: modifiers.intersects(!supported),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum KeyCodeSpec {
    Char(char),
    Named(NamedKey),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct KeySpec {
    code: KeyCodeSpec,
    modifiers: KeyModifierSpec,
}

impl KeySpec {
    fn char(c: char) -> Self {
        Self {
            code: KeyCodeSpec::Char(c),
            modifiers: KeyModifierSpec::NONE,
        }
    }

    fn named(named: NamedKey) -> Self {
        Self {
            code: KeyCodeSpec::Named(named),
            modifiers: KeyModifierSpec::NONE,
        }
    }

    fn ctrl_char(c: char) -> Self {
        Self {
            code: KeyCodeSpec::Char(c),
            modifiers: KeyModifierSpec {
                ctrl: true,
                ..KeyModifierSpec::NONE
            },
        }
    }

    fn alt_named(named: NamedKey) -> Self {
        Self {
            code: KeyCodeSpec::Named(named),
            modifiers: KeyModifierSpec {
                alt: true,
                ..KeyModifierSpec::NONE
            },
        }
    }

    fn single_char(self) -> Option<char> {
        match (self.code, self.modifiers.is_empty()) {
            (KeyCodeSpec::Char(c), true) => Some(c),
            _ => None,
        }
    }

    fn matches_event(self, key: KeyEvent) -> bool {
        let (event_code, event_modifiers) = normalize_key_event(key);
        if event_modifiers != self.modifiers {
            return false;
        }

        match self.code {
            KeyCodeSpec::Char(c) => matches!(event_code, KeyCode::Char(actual) if actual == c),
            KeyCodeSpec::Named(named) => named.matches(event_code),
        }
    }
}

fn normalize_key_event(key: KeyEvent) -> (KeyCode, KeyModifierSpec) {
    let mut modifiers = KeyModifierSpec::from_event(key.modifiers);
    let code = match key.code {
        KeyCode::Char(c) if modifiers.ctrl || modifiers.alt => {
            modifiers.shift = false;
            KeyCode::Char(c.to_ascii_lowercase())
        }
        KeyCode::Char(c) => {
            modifiers.shift = false;
            KeyCode::Char(c)
        }
        KeyCode::BackTab => {
            modifiers.shift = false;
            KeyCode::BackTab
        }
        code => code,
    };
    (code, modifiers)
}

impl std::fmt::Display for KeySpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.modifiers.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.modifiers.alt {
            f.write_str("Alt+")?;
        }
        if self.modifiers.shift {
            f.write_str("Shift+")?;
        }

        match self.code {
            KeyCodeSpec::Char(c) if self.modifiers.ctrl || self.modifiers.alt => {
                write!(f, "{}", c.to_ascii_uppercase())
            }
            KeyCodeSpec::Char(c) => write!(f, "{c}"),
            KeyCodeSpec::Named(named) => write!(f, "{named}"),
        }
    }
}

/// One or more key bindings for a browser action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KeyList(Vec<KeySpec>);

impl KeyList {
    fn one(c: char) -> Self {
        Self(vec![KeySpec::char(c)])
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
        self.0.as_slice() == [KeySpec::char(*other)]
    }
}

/// Key bindings for browser actions.
/// All fields default to the built-in keys; set any field in `[keys]` in
/// `config.toml` to override that binding. Values may be either a single
/// string (`open = "o"`) or a list of strings (`open = ["o", "e"]`).
/// Empty lists unbind the action (`open = []`).
/// Character bindings must be one character; named bindings currently support
/// `left`, `right`, `up`, `down`, `enter`, `space`, `tab`, `backtab`,
/// `shift+tab`, `backspace`, `pageup`, `pagedown`, `home`, `end`, and `f1`..`f12`.
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
    pub restore_from_trash: KeyList,
    pub copy_path: KeyList,
    pub search_folders: KeyList,
    pub search_files: KeyList,
    pub zoxide: KeyList,
    pub shell: KeyList,
    pub open: KeyList,
    pub open_with: KeyList,
    pub open_or_enter: KeyList,
    pub go_to: KeyList,
    pub toggle_selection: KeyList,
    pub cycle_places_next: KeyList,
    pub cycle_places_previous: KeyList,
    pub go_parent: KeyList,
    pub page_up: KeyList,
    pub page_down: KeyList,
    pub jump_first: KeyList,
    pub jump_last: KeyList,
    pub select_all: KeyList,
    pub history_back: KeyList,
    pub history_forward: KeyList,
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
    '?', // help
    '+', '=', '-', '_', // grid zoom
];

/// Modified keys that are still hard-wired before configurable browser actions.
const RESERVED_MODIFIED_CHARS: &[char] = &[
    'c', // cancel/clear
    '+', '=', '-', '_', // grid zoom
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
            rename: KeyList(vec![
                KeySpec::char('r'),
                KeySpec::named(NamedKey::Function(2)),
            ]),
            restore_from_trash: KeyList::one('r'),
            copy_path: KeyList::one('c'),
            search_folders: KeyList::one('f'),
            search_files: KeyList(vec![KeySpec::ctrl_char('f')]),
            zoxide: KeyList::one('z'),
            shell: KeyList::one('!'),
            open: KeyList::one('o'),
            open_with: KeyList::one('O'),
            open_or_enter: KeyList(vec![KeySpec::named(NamedKey::Enter)]),
            go_to: KeyList::one('g'),
            toggle_selection: KeyList(vec![KeySpec::named(NamedKey::Space)]),
            cycle_places_next: KeyList(vec![KeySpec::named(NamedKey::Tab)]),
            cycle_places_previous: KeyList(vec![KeySpec::named(NamedKey::BackTab)]),
            go_parent: KeyList(vec![KeySpec::named(NamedKey::Backspace)]),
            page_up: KeyList(vec![KeySpec::named(NamedKey::PageUp)]),
            page_down: KeyList(vec![KeySpec::named(NamedKey::PageDown)]),
            jump_first: KeyList(vec![KeySpec::named(NamedKey::Home)]),
            jump_last: KeyList(vec![KeySpec::char('G'), KeySpec::named(NamedKey::End)]),
            select_all: KeyList(vec![KeySpec::ctrl_char('a')]),
            history_back: KeyList(vec![KeySpec::alt_named(NamedKey::Left)]),
            history_forward: KeyList(vec![KeySpec::alt_named(NamedKey::Right)]),
            sort: KeyList::one('s'),
            toggle_view: KeyList::one('v'),
            toggle_hidden: KeyList::one('.'),
            nav_left: KeyList(vec![KeySpec::char('h'), KeySpec::named(NamedKey::Left)]),
            nav_down: KeyList(vec![KeySpec::char('j'), KeySpec::named(NamedKey::Down)]),
            nav_up: KeyList(vec![KeySpec::char('k'), KeySpec::named(NamedKey::Up)]),
            nav_right: KeyList(vec![KeySpec::char('l'), KeySpec::named(NamedKey::Right)]),
            scroll_preview_left: KeyList::one('H'),
            scroll_preview_right: KeyList::one('L'),
            scroll_preview_up: KeyList(vec![KeySpec::char('K'), KeySpec::char('[')]),
            scroll_preview_down: KeyList(vec![KeySpec::char('J'), KeySpec::char(']')]),
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
    restore_from_trash: Option<KeyConfigOverride>,
    copy_path: Option<KeyConfigOverride>,
    search_folders: Option<KeyConfigOverride>,
    search_files: Option<KeyConfigOverride>,
    zoxide: Option<KeyConfigOverride>,
    shell: Option<KeyConfigOverride>,
    open: Option<KeyConfigOverride>,
    open_with: Option<KeyConfigOverride>,
    open_or_enter: Option<KeyConfigOverride>,
    go_to: Option<KeyConfigOverride>,
    toggle_selection: Option<KeyConfigOverride>,
    cycle_places_next: Option<KeyConfigOverride>,
    cycle_places_previous: Option<KeyConfigOverride>,
    go_parent: Option<KeyConfigOverride>,
    page_up: Option<KeyConfigOverride>,
    page_down: Option<KeyConfigOverride>,
    jump_first: Option<KeyConfigOverride>,
    jump_last: Option<KeyConfigOverride>,
    select_all: Option<KeyConfigOverride>,
    history_back: Option<KeyConfigOverride>,
    history_forward: Option<KeyConfigOverride>,
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
    #[serde(flatten)]
    unknown: BTreeMap<String, toml::Value>,
}

struct RawBinding {
    name: &'static str,
    action: Action,
    override_value: Option<KeyConfigOverride>,
    default: KeyList,
}

fn warn_unknown_key_actions(unknown: &BTreeMap<String, toml::Value>) {
    for key in unknown.keys() {
        eprintln!("{}", unknown_key_action_warning(key));
    }
}

pub(super) fn unknown_key_action_warning(key: &str) -> String {
    format!("elio: keys.{key}: unknown key action; ignoring")
}

impl KeyBindings {
    /// Returns the normal-browser action bound to `key`, if any.
    pub(crate) fn action_for_key(&self, key: KeyEvent) -> Option<Action> {
        self.action_for_key_in_context(key, KeyContext::Normal)
    }

    /// Returns the action bound to `key` in the active browser context, if any.
    pub(crate) fn action_for_key_in_context(
        &self,
        key: KeyEvent,
        context: KeyContext,
    ) -> Option<Action> {
        self.bindings().iter().find_map(|(keys, action)| {
            action
                .key_contexts()
                .contains(context)
                .then_some(())
                .filter(|_| keys.matches_event(key))
                .map(|_| *action)
        })
    }

    fn bindings(&self) -> [(&KeyList, Action); 41] {
        [
            (&self.quit, Action::Quit),
            (&self.quit_without_cd, Action::QuitWithoutCd),
            (&self.yank, Action::Yank),
            (&self.cut, Action::Cut),
            (&self.paste, Action::Paste),
            (&self.trash, Action::Trash),
            (&self.delete_permanently, Action::DeletePermanently),
            (&self.create, Action::Create),
            (&self.rename, Action::Rename),
            (&self.restore_from_trash, Action::RestoreFromTrash),
            (&self.copy_path, Action::CopyPath),
            (&self.search_folders, Action::SearchFolders),
            (&self.search_files, Action::SearchFiles),
            (&self.zoxide, Action::Zoxide),
            (&self.shell, Action::Shell),
            (&self.open, Action::Open),
            (&self.open_with, Action::OpenWith),
            (&self.open_or_enter, Action::OpenOrEnter),
            (&self.go_to, Action::GoTo),
            (&self.toggle_selection, Action::ToggleSelection),
            (&self.cycle_places_next, Action::CyclePlacesNext),
            (&self.cycle_places_previous, Action::CyclePlacesPrevious),
            (&self.go_parent, Action::GoParent),
            (&self.page_up, Action::PageUp),
            (&self.page_down, Action::PageDown),
            (&self.jump_first, Action::JumpFirst),
            (&self.jump_last, Action::JumpLast),
            (&self.select_all, Action::SelectAll),
            (&self.history_back, Action::HistoryBack),
            (&self.history_forward, Action::HistoryForward),
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
        ]
    }

    /// Returns the action bound to `c`, if any.
    #[cfg(test)]
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
        warn_unknown_key_actions(&overrides.unknown);

        let raw = vec![
            RawBinding {
                name: "quit",
                action: Action::Quit,
                override_value: overrides.quit,
                default: defaults.quit.clone(),
            },
            RawBinding {
                name: "quit_without_cd",
                action: Action::QuitWithoutCd,
                override_value: overrides.quit_without_cd,
                default: defaults.quit_without_cd.clone(),
            },
            RawBinding {
                name: "yank",
                action: Action::Yank,
                override_value: overrides.yank,
                default: defaults.yank.clone(),
            },
            RawBinding {
                name: "cut",
                action: Action::Cut,
                override_value: overrides.cut,
                default: defaults.cut.clone(),
            },
            RawBinding {
                name: "paste",
                action: Action::Paste,
                override_value: overrides.paste,
                default: defaults.paste.clone(),
            },
            RawBinding {
                name: "trash",
                action: Action::Trash,
                override_value: overrides.trash,
                default: defaults.trash.clone(),
            },
            RawBinding {
                name: "delete_permanently",
                action: Action::DeletePermanently,
                override_value: overrides.delete_permanently,
                default: defaults.delete_permanently.clone(),
            },
            RawBinding {
                name: "create",
                action: Action::Create,
                override_value: overrides.create,
                default: defaults.create.clone(),
            },
            RawBinding {
                name: "rename",
                action: Action::Rename,
                override_value: overrides.rename,
                default: defaults.rename.clone(),
            },
            RawBinding {
                name: "restore_from_trash",
                action: Action::RestoreFromTrash,
                override_value: overrides.restore_from_trash,
                default: defaults.restore_from_trash.clone(),
            },
            RawBinding {
                name: "copy_path",
                action: Action::CopyPath,
                override_value: overrides.copy_path,
                default: defaults.copy_path.clone(),
            },
            RawBinding {
                name: "search_folders",
                action: Action::SearchFolders,
                override_value: overrides.search_folders,
                default: defaults.search_folders.clone(),
            },
            RawBinding {
                name: "search_files",
                action: Action::SearchFiles,
                override_value: overrides.search_files,
                default: defaults.search_files.clone(),
            },
            RawBinding {
                name: "zoxide",
                action: Action::Zoxide,
                override_value: overrides.zoxide,
                default: defaults.zoxide.clone(),
            },
            RawBinding {
                name: "shell",
                action: Action::Shell,
                override_value: overrides.shell,
                default: defaults.shell.clone(),
            },
            RawBinding {
                name: "open",
                action: Action::Open,
                override_value: overrides.open,
                default: defaults.open.clone(),
            },
            RawBinding {
                name: "open_with",
                action: Action::OpenWith,
                override_value: overrides.open_with,
                default: defaults.open_with.clone(),
            },
            RawBinding {
                name: "open_or_enter",
                action: Action::OpenOrEnter,
                override_value: overrides.open_or_enter,
                default: defaults.open_or_enter.clone(),
            },
            RawBinding {
                name: "go_to",
                action: Action::GoTo,
                override_value: overrides.go_to,
                default: defaults.go_to.clone(),
            },
            RawBinding {
                name: "toggle_selection",
                action: Action::ToggleSelection,
                override_value: overrides.toggle_selection,
                default: defaults.toggle_selection.clone(),
            },
            RawBinding {
                name: "cycle_places_next",
                action: Action::CyclePlacesNext,
                override_value: overrides.cycle_places_next,
                default: defaults.cycle_places_next.clone(),
            },
            RawBinding {
                name: "cycle_places_previous",
                action: Action::CyclePlacesPrevious,
                override_value: overrides.cycle_places_previous,
                default: defaults.cycle_places_previous.clone(),
            },
            RawBinding {
                name: "go_parent",
                action: Action::GoParent,
                override_value: overrides.go_parent,
                default: defaults.go_parent.clone(),
            },
            RawBinding {
                name: "page_up",
                action: Action::PageUp,
                override_value: overrides.page_up,
                default: defaults.page_up.clone(),
            },
            RawBinding {
                name: "page_down",
                action: Action::PageDown,
                override_value: overrides.page_down,
                default: defaults.page_down.clone(),
            },
            RawBinding {
                name: "jump_first",
                action: Action::JumpFirst,
                override_value: overrides.jump_first,
                default: defaults.jump_first.clone(),
            },
            RawBinding {
                name: "jump_last",
                action: Action::JumpLast,
                override_value: overrides.jump_last,
                default: defaults.jump_last.clone(),
            },
            RawBinding {
                name: "select_all",
                action: Action::SelectAll,
                override_value: overrides.select_all,
                default: defaults.select_all.clone(),
            },
            RawBinding {
                name: "history_back",
                action: Action::HistoryBack,
                override_value: overrides.history_back,
                default: defaults.history_back.clone(),
            },
            RawBinding {
                name: "history_forward",
                action: Action::HistoryForward,
                override_value: overrides.history_forward,
                default: defaults.history_forward.clone(),
            },
            RawBinding {
                name: "sort",
                action: Action::Sort,
                override_value: overrides.sort,
                default: defaults.sort.clone(),
            },
            RawBinding {
                name: "toggle_view",
                action: Action::ToggleView,
                override_value: overrides.toggle_view,
                default: defaults.toggle_view.clone(),
            },
            RawBinding {
                name: "toggle_hidden",
                action: Action::ToggleHidden,
                override_value: overrides.toggle_hidden,
                default: defaults.toggle_hidden.clone(),
            },
            RawBinding {
                name: "nav_left",
                action: Action::NavLeft,
                override_value: overrides.nav_left,
                default: defaults.nav_left.clone(),
            },
            RawBinding {
                name: "nav_down",
                action: Action::NavDown,
                override_value: overrides.nav_down,
                default: defaults.nav_down.clone(),
            },
            RawBinding {
                name: "nav_up",
                action: Action::NavUp,
                override_value: overrides.nav_up,
                default: defaults.nav_up.clone(),
            },
            RawBinding {
                name: "nav_right",
                action: Action::NavRight,
                override_value: overrides.nav_right,
                default: defaults.nav_right.clone(),
            },
            RawBinding {
                name: "scroll_preview_left",
                action: Action::ScrollPreviewLeft,
                override_value: overrides.scroll_preview_left,
                default: defaults.scroll_preview_left.clone(),
            },
            RawBinding {
                name: "scroll_preview_right",
                action: Action::ScrollPreviewRight,
                override_value: overrides.scroll_preview_right,
                default: defaults.scroll_preview_right.clone(),
            },
            RawBinding {
                name: "scroll_preview_up",
                action: Action::ScrollPreviewUp,
                override_value: overrides.scroll_preview_up,
                default: defaults.scroll_preview_up.clone(),
            },
            RawBinding {
                name: "scroll_preview_down",
                action: Action::ScrollPreviewDown,
                override_value: overrides.scroll_preview_down,
                default: defaults.scroll_preview_down.clone(),
            },
        ];

        // Step 1: parse each override independently, falling back to default on
        // any format or reserved-char error. Empty lists are valid unbinds.
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
                        .find(|&other_index| {
                            raw[index]
                                .action
                                .key_contexts()
                                .intersects(raw[other_index].action.key_contexts())
                                && candidates[other_index].0.contains(candidate)
                        })
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
            restore_from_trash: resolved(9),
            copy_path: resolved(10),
            search_folders: resolved(11),
            search_files: resolved(12),
            zoxide: resolved(13),
            shell: resolved(14),
            open: resolved(15),
            open_with: resolved(16),
            open_or_enter: resolved(17),
            go_to: resolved(18),
            toggle_selection: resolved(19),
            cycle_places_next: resolved(20),
            cycle_places_previous: resolved(21),
            go_parent: resolved(22),
            page_up: resolved(23),
            page_down: resolved(24),
            jump_first: resolved(25),
            jump_last: resolved(26),
            select_all: resolved(27),
            history_back: resolved(28),
            history_forward: resolved(29),
            sort: resolved(30),
            toggle_view: resolved(31),
            toggle_hidden: resolved(32),
            nav_left: resolved(33),
            nav_down: resolved(34),
            nav_up: resolved(35),
            nav_right: resolved(36),
            scroll_preview_left: resolved(37),
            scroll_preview_right: resolved(38),
            scroll_preview_up: resolved(39),
            scroll_preview_down: resolved(40),
        }
    }
}

fn parse_key_override(name: &str, value: &KeyConfigOverride, default: &KeyList) -> Option<KeyList> {
    let values: Vec<&str> = match value {
        KeyConfigOverride::One(value) => vec![value.as_str()],
        KeyConfigOverride::Many(values) => values.iter().map(String::as_str).collect(),
    };

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
    let (mut modifiers, key_name) = parse_key_modifiers(name, value, default)?;

    if modifiers.shift && key_name.eq_ignore_ascii_case("tab") {
        modifiers.shift = false;
        return validate_key_spec(
            name,
            KeySpec {
                code: KeyCodeSpec::Named(NamedKey::BackTab),
                modifiers,
            },
            default,
        );
    }

    if modifiers.shift && key_name.eq_ignore_ascii_case("backtab") {
        eprintln!(
            "elio: keys.{name}: {value:?} uses shift with backtab; use \"shift+tab\" or \"backtab\" instead; using default '{default}'"
        );
        return None;
    }

    if modifiers.shift && key_name.eq_ignore_ascii_case("space") {
        eprintln!(
            "elio: keys.{name}: {value:?} uses shift with space, which terminals do not report reliably; using default '{default}'"
        );
        return None;
    }

    if modifiers.shift && key_name.len() == 1 {
        let c = key_name.chars().next().expect("single-char key");
        let suggestion = if c.is_ascii_alphabetic() {
            format!("; use {:?} instead", c.to_ascii_uppercase().to_string())
        } else {
            String::new()
        };
        eprintln!(
            "elio: keys.{name}: {value:?} uses shift with a character{suggestion}; using default '{default}'"
        );
        return None;
    }

    if key_name.eq_ignore_ascii_case("shift")
        || key_name.eq_ignore_ascii_case("ctrl")
        || key_name.eq_ignore_ascii_case("alt")
    {
        eprintln!(
            "elio: keys.{name}: {value:?} is missing a key after modifiers; using default '{default}'"
        );
        return None;
    }

    if let Some(named) = NamedKey::parse(key_name) {
        let spec = KeySpec {
            code: KeyCodeSpec::Named(named),
            modifiers,
        };
        return validate_key_spec(name, spec, default);
    }

    let mut chars = key_name.chars();
    let Some(mut c) = chars.next() else {
        eprintln!(
            "elio: keys.{name}: empty strings cannot be used as key bindings; use [] to unbind this action; using default '{default}'"
        );
        return None;
    };
    if chars.next().is_some() {
        eprintln!(
            "elio: keys.{name}: {value:?} is not a single character, modifier binding, or supported named key; using default '{default}'"
        );
        return None;
    }
    if RESERVED_CHARS.contains(&c) && modifiers.is_empty() {
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
    if c == ' ' && modifiers.is_empty() {
        return validate_key_spec(
            name,
            KeySpec {
                code: KeyCodeSpec::Named(NamedKey::Space),
                modifiers,
            },
            default,
        );
    }

    if (modifiers.ctrl || modifiers.alt) && c.is_ascii_alphabetic() {
        c = c.to_ascii_lowercase();
    }

    validate_key_spec(
        name,
        KeySpec {
            code: KeyCodeSpec::Char(c),
            modifiers,
        },
        default,
    )
}

fn validate_key_spec(name: &str, spec: KeySpec, default: &KeyList) -> Option<KeySpec> {
    if is_reserved_key_spec(spec) {
        eprintln!(
            "elio: keys.{name}: '{spec}' is reserved and cannot be rebound; using default '{default}'"
        );
        return None;
    }

    Some(spec)
}

fn is_reserved_key_spec(spec: KeySpec) -> bool {
    match spec.code {
        KeyCodeSpec::Char(c) if spec.modifiers.is_empty() => RESERVED_CHARS.contains(&c),
        KeyCodeSpec::Char(c) if spec.modifiers.ctrl => {
            RESERVED_MODIFIED_CHARS.contains(&c.to_ascii_lowercase())
        }
        _ => false,
    }
}

fn parse_key_modifiers<'a>(
    name: &str,
    value: &'a str,
    default: &KeyList,
) -> Option<(KeyModifierSpec, &'a str)> {
    let mut parts = value.split('+').peekable();
    let mut modifiers = KeyModifierSpec::NONE;
    let mut key = None;

    while let Some(part) = parts.next() {
        if part.is_empty() {
            if value.is_empty() {
                eprintln!(
                    "elio: keys.{name}: empty strings cannot be used as key bindings; use [] to unbind this action; using default '{default}'"
                );
            } else {
                eprintln!(
                    "elio: keys.{name}: {value:?} contains an empty key component; using default '{default}'"
                );
            }
            return None;
        }

        if parts.peek().is_none() {
            key = Some(part);
            break;
        }

        match part.to_ascii_lowercase().as_str() {
            "ctrl" => {
                if modifiers.ctrl {
                    eprintln!(
                        "elio: keys.{name}: duplicate modifier 'ctrl' in {value:?}; using default '{default}'"
                    );
                    return None;
                }
                modifiers.ctrl = true;
            }
            "alt" => {
                if modifiers.alt {
                    eprintln!(
                        "elio: keys.{name}: duplicate modifier 'alt' in {value:?}; using default '{default}'"
                    );
                    return None;
                }
                modifiers.alt = true;
            }
            "shift" => {
                if modifiers.shift {
                    eprintln!(
                        "elio: keys.{name}: duplicate modifier 'shift' in {value:?}; using default '{default}'"
                    );
                    return None;
                }
                modifiers.shift = true;
            }
            _ => {
                eprintln!(
                    "elio: keys.{name}: unknown modifier {part:?} in {value:?}; supported modifiers are ctrl, alt, and shift; using default '{default}'"
                );
                return None;
            }
        }
    }

    let Some(key) = key else {
        eprintln!(
            "elio: keys.{name}: {value:?} is missing a key after modifiers; using default '{default}'"
        );
        return None;
    };

    Some((modifiers, key))
}
