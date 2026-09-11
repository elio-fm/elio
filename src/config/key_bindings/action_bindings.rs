use super::{binding_parsing::KeysConfigOverride, binding_validation::resolve_key_overrides};
use crossterm::event::{KeyCode, KeyEvent, KeyEventState, KeyModifiers};

/// A browser action that can be triggered by a configurable key binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Action {
    Quit,
    QuitWithoutCd,
    Yank,
    Cut,
    Paste,
    CreateArchive,
    ExtractArchive,
    SymlinkAbsolute,
    SymlinkRelative,
    Trash,
    DeletePermanently,
    Create,
    Rename,
    RenameInEditor,
    RestoreFromTrash,
    CopyPath,
    SearchFolders,
    SearchFiles,
    FilterDirectory,
    FindDuplicates,
    Zoxide,
    ShellHere,
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
    TogglePreview,
    FullscreenPreview,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChooserKeyAction {
    Choose,
    Cancel,
    Normal(Action),
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
    Delete,
    PageUp,
    PageDown,
    Home,
    End,
    Function(u8),
}

impl NamedKey {
    pub(super) fn parse(value: &str) -> Option<Self> {
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
            "delete" | "del" => Some(Self::Delete),
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
            | (Self::Delete, KeyCode::Delete)
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
            Self::Delete => "Del",
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
pub(super) struct KeyContexts(u8);

impl KeyContexts {
    const NORMAL: Self = Self(0b01);
    const TRASH: Self = Self(0b10);
    const ALL: Self = Self(Self::NORMAL.0 | Self::TRASH.0);

    pub(super) fn contains(self, context: KeyContext) -> bool {
        let mask = match context {
            KeyContext::Normal => Self::NORMAL,
            KeyContext::Trash => Self::TRASH,
        };
        self.intersects(mask)
    }

    pub(super) fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl Action {
    pub(super) fn key_contexts(self) -> KeyContexts {
        match self {
            Self::Rename | Self::RenameInEditor | Self::SymlinkAbsolute | Self::SymlinkRelative => {
                KeyContexts::NORMAL
            }
            Self::RestoreFromTrash => KeyContexts::TRASH,
            _ => KeyContexts::ALL,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct KeyModifierSpec {
    pub(super) ctrl: bool,
    pub(super) alt: bool,
    pub(super) shift: bool,
    pub(super) other: bool,
}

impl KeyModifierSpec {
    pub(super) const NONE: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        other: false,
    };

    pub(super) fn is_empty(self) -> bool {
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
pub(super) enum KeyCodeSpec {
    Char(char),
    Named(NamedKey),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct KeySpec {
    pub(super) code: KeyCodeSpec,
    pub(super) modifiers: KeyModifierSpec,
}

pub(crate) fn normalized_plain_key_char(key: KeyEvent) -> Option<char> {
    let (code, modifiers) = normalize_key_event(key);
    match (code, modifiers.is_empty()) {
        (KeyCode::Char(ch), true) => Some(ch),
        _ => None,
    }
}

impl KeySpec {
    pub(super) fn char(c: char) -> Self {
        Self {
            code: KeyCodeSpec::Char(c),
            modifiers: KeyModifierSpec::NONE,
        }
    }

    pub(super) fn named(named: NamedKey) -> Self {
        Self {
            code: KeyCodeSpec::Named(named),
            modifiers: KeyModifierSpec::NONE,
        }
    }

    pub(super) fn ctrl_char(c: char) -> Self {
        Self {
            code: KeyCodeSpec::Char(c),
            modifiers: KeyModifierSpec {
                ctrl: true,
                ..KeyModifierSpec::NONE
            },
        }
    }

    pub(super) fn alt_char(c: char) -> Self {
        Self {
            code: KeyCodeSpec::Char(c),
            modifiers: KeyModifierSpec {
                alt: true,
                ..KeyModifierSpec::NONE
            },
        }
    }

    pub(super) fn alt_named(named: NamedKey) -> Self {
        Self {
            code: KeyCodeSpec::Named(named),
            modifiers: KeyModifierSpec {
                alt: true,
                ..KeyModifierSpec::NONE
            },
        }
    }

    pub(super) fn shift_named(named: NamedKey) -> Self {
        Self {
            code: KeyCodeSpec::Named(named),
            modifiers: KeyModifierSpec {
                shift: true,
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
    let caps_lock = key.state.contains(KeyEventState::CAPS_LOCK);
    let code = match key.code {
        KeyCode::Char(c) if modifiers.ctrl || modifiers.alt => {
            modifiers.shift = false;
            KeyCode::Char(c.to_ascii_lowercase())
        }
        KeyCode::Char(mut c) => {
            if caps_lock {
                c = if modifiers.shift {
                    single_case_mapping(c.to_lowercase()).unwrap_or(c)
                } else {
                    single_case_mapping(c.to_uppercase()).unwrap_or(c)
                };
            }
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

fn single_case_mapping(mut mapping: impl Iterator<Item = char>) -> Option<char> {
    let mapped = mapping.next()?;
    mapping.next().is_none().then_some(mapped)
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
pub(crate) struct KeyList(pub(super) Vec<KeySpec>);

impl KeyList {
    pub(super) fn one(c: char) -> Self {
        Self(vec![KeySpec::char(c)])
    }

    pub(super) fn contains(&self, key: KeySpec) -> bool {
        self.0.contains(&key)
    }

    pub(super) fn matches_event(&self, key: KeyEvent) -> bool {
        self.0.iter().any(|spec| spec.matches_event(key))
    }

    pub(super) fn keys(&self) -> impl Iterator<Item = KeySpec> + '_ {
        self.0.iter().copied()
    }

    pub(crate) fn without(&self, shadowed: &Self) -> Self {
        Self(
            self.0
                .iter()
                .copied()
                .filter(|key| !shadowed.contains(*key))
                .collect(),
        )
    }

    pub(crate) fn single_chars(&self) -> impl Iterator<Item = char> + '_ {
        self.0
            .iter()
            .filter_map(|spec| spec.single_char().map(|ch| ch.to_ascii_lowercase()))
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
/// `shift+tab`, `backspace`, `delete`, `del`, `pageup`, `pagedown`, `home`,
/// `end`, and `f1`..`f12`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KeyBindings {
    pub choose: KeyList,
    pub quit: KeyList,
    pub quit_without_cd: KeyList,
    pub yank: KeyList,
    pub cut: KeyList,
    pub paste: KeyList,
    pub create_archive: KeyList,
    pub extract_archive: KeyList,
    pub symlink_absolute: KeyList,
    pub symlink_relative: KeyList,
    pub trash: KeyList,
    pub delete_permanently: KeyList,
    pub create: KeyList,
    pub rename: KeyList,
    pub rename_in_editor: KeyList,
    pub restore_from_trash: KeyList,
    pub copy_path: KeyList,
    pub search_folders: KeyList,
    pub search_files: KeyList,
    pub filter_directory: KeyList,
    pub find_duplicates: KeyList,
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
    pub toggle_preview: KeyList,
    pub fullscreen_preview: KeyList,
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

    pub(crate) fn chooser_action_for_key(
        &self,
        key: KeyEvent,
        context: KeyContext,
    ) -> Option<ChooserKeyAction> {
        if self.choose.matches_event(key) {
            return Some(ChooserKeyAction::Choose);
        }
        if self.quit.matches_event(key) || self.quit_without_cd.matches_event(key) {
            return Some(ChooserKeyAction::Cancel);
        }
        self.action_for_key_in_context(key, context)
            .map(ChooserKeyAction::Normal)
    }

    pub(crate) fn open_with_reserved_shortcuts(&self) -> Vec<char> {
        self.nav_down
            .single_chars()
            .chain(self.nav_up.single_chars())
            .chain(self.open_or_enter.single_chars())
            .collect()
    }

    fn bindings(&self) -> [(&KeyList, Action); 50] {
        [
            (&self.quit, Action::Quit),
            (&self.quit_without_cd, Action::QuitWithoutCd),
            (&self.yank, Action::Yank),
            (&self.cut, Action::Cut),
            (&self.paste, Action::Paste),
            (&self.create_archive, Action::CreateArchive),
            (&self.extract_archive, Action::ExtractArchive),
            (&self.symlink_absolute, Action::SymlinkAbsolute),
            (&self.symlink_relative, Action::SymlinkRelative),
            (&self.trash, Action::Trash),
            (&self.delete_permanently, Action::DeletePermanently),
            (&self.create, Action::Create),
            (&self.rename, Action::Rename),
            (&self.rename_in_editor, Action::RenameInEditor),
            (&self.restore_from_trash, Action::RestoreFromTrash),
            (&self.copy_path, Action::CopyPath),
            (&self.search_folders, Action::SearchFolders),
            (&self.search_files, Action::SearchFiles),
            (&self.filter_directory, Action::FilterDirectory),
            (&self.find_duplicates, Action::FindDuplicates),
            (&self.zoxide, Action::Zoxide),
            (&self.shell, Action::ShellHere),
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
            (&self.toggle_preview, Action::TogglePreview),
            (&self.fullscreen_preview, Action::FullscreenPreview),
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
        crate::config::loading::Config::from_str(s)
            .map(|config| config.keys)
            .unwrap_or_else(|_| Self::default())
    }

    pub(in crate::config) fn from_override(overrides: KeysConfigOverride, defaults: &Self) -> Self {
        resolve_key_overrides(overrides, defaults)
    }
}
