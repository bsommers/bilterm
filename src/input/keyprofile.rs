//! Per-OS key profiles and the effective keymap (`KEY-02`, `KEY-03`, Phase 63). API FROZEN by 63-01.
//!
//! [`KeyProfile::resolve`] turns `(KeysConfig, KeyEnv)` into an [`EffectiveKeymap`]: a cheap value
//! (no table copy) that filters the static legacy tables on the fly, adds the `Primary` and macOS
//! rows, and answers lookups, reverse lookups (`keys_for_action`, `hint_for_action`) and integrity
//! checks. The `legacy` profile is the pre-Phase-63 behaviour bit-for-bit.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::config::keys::{KeyProfileName, KeysConfig, LegacyAlt};
use crate::wm::key_mode::KeyMode;

use super::chord::KeyChord;
use super::keyenv::{HostOs, KeyEnv, primary_mods};
use super::keymap::{DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS, DEFAULT_PANE_BINDINGS};
use super::keyprofile_tables::{
    ALLOWED_SHADOWS, LEADER_OVERRIDES, MAC_EXTRA, NON_LEGACY_REMOVED, PRIMARY_BINDINGS,
    PROMPT_EDIT_ALT_KEYS,
};

/// One table row: modifiers, code, action.
pub type Row = (KeyModifiers, KeyCode, &'static Action);

/// Namespace for profile resolution.
pub struct KeyProfile;

/// The active key bindings for one `(profile, env)`; see module docs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectiveKeymap {
    profile: KeyProfileName,
    env: KeyEnv,
    legacy_alt: bool,
    unbound: Vec<KeyChord>,
}

/// Two different actions bound to one folded chord that is not in [`ALLOWED_SHADOWS`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicateChord {
    pub chord: String,
    pub table: &'static str,
    pub actions: Vec<Action>,
}

impl KeyProfile {
    /// The concrete (never `Auto`) profile for `name` on `os`.
    pub fn concrete(name: KeyProfileName, os: HostOs) -> KeyProfileName {
        match name {
            KeyProfileName::Auto => match os {
                HostOs::MacOs => KeyProfileName::Mac,
                HostOs::Windows => KeyProfileName::Windows,
                HostOs::Linux => KeyProfileName::Linux,
            },
            other => other,
        }
    }

    /// Builds the effective keymap for `cfg` in `env`. Unparseable `unbind` entries are ignored
    /// (they are rejected at load by `KeysConfig::validate`).
    pub fn resolve(cfg: &KeysConfig, env: &KeyEnv) -> EffectiveKeymap {
        let profile = Self::concrete(cfg.profile, env.os);
        let legacy_alt = match cfg.legacy_alt {
            LegacyAlt::On | LegacyAlt::Auto => true,
            LegacyAlt::Off => false,
        };
        let unbound = cfg.unbind.iter().filter_map(|c| KeyChord::parse(c).ok()).map(|c| c.folded()).collect();
        EffectiveKeymap { profile, env: *env, legacy_alt, unbound }
    }
}

fn is_alt_printable(mods: KeyModifiers, code: KeyCode) -> bool {
    mods.contains(KeyModifiers::ALT)
        && !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER)
        && matches!(code, KeyCode::Char(c) if !c.is_control())
}

/// Global lookup over `rows`: exact modifiers first, then Shift-folded for rows without SHIFT.
pub(crate) fn lookup_global<'a, I>(rows: I, key: KeyEvent) -> Option<Action>
where
    I: Iterator<Item = (KeyModifiers, KeyCode, &'a Action)> + Clone,
{
    let lookup_mods = match key.code {
        KeyCode::Char(_) => key.modifiers - KeyModifiers::SHIFT,
        _ => key.modifiers,
    };
    if let Some((_, _, a)) = rows.clone().find(|(m, c, _)| *c == key.code && *m == key.modifiers) {
        return Some(a.clone());
    }
    rows.into_iter()
        .find(|(m, c, _)| *c == key.code && *m == lookup_mods && !m.contains(KeyModifiers::SHIFT))
        .map(|(_, _, a)| a.clone())
}

/// Pane lookup over `rows`: Shift-folded for `Char` keys, exact otherwise.
pub(crate) fn lookup_pane<'a, I>(rows: I, key: KeyEvent) -> Option<Action>
where
    I: Iterator<Item = (KeyModifiers, KeyCode, &'a Action)>,
{
    let lookup_mods = match key.code {
        KeyCode::Char(_) => key.modifiers - KeyModifiers::SHIFT,
        _ => key.modifiers,
    };
    rows.into_iter().find(|(m, c, _)| *m == lookup_mods && *c == key.code).map(|(_, _, a)| a.clone())
}

impl EffectiveKeymap {
    /// The keymap the headless workspace and bare panes start with: `legacy` on a deterministic env.
    pub fn legacy_default() -> Self {
        Self { profile: KeyProfileName::Legacy, env: KeyEnv::headless(), legacy_alt: true, unbound: Vec::new() }
    }

    pub fn profile(&self) -> KeyProfileName {
        self.profile
    }

    pub fn env(&self) -> KeyEnv {
        self.env
    }

    /// Whether the `Alt+<printable>` layer (and the hard-coded pane Alt hotkeys) is active.
    pub fn alt_layer(&self) -> bool {
        match self.profile {
            KeyProfileName::Legacy => true,
            KeyProfileName::Mac | KeyProfileName::Portable => false,
            _ => self.legacy_alt,
        }
    }

    /// Modifier set of the hard-coded pane-focus arrow chord (`Alt+Arrow`; `Cmd+Option+Arrow`
    /// on macOS), or `None` on `portable`.
    pub fn focus_arrow_mods(&self) -> Option<KeyModifiers> {
        match self.profile {
            KeyProfileName::Mac => Some(KeyModifiers::SUPER | KeyModifiers::ALT),
            KeyProfileName::Portable => None,
            _ => Some(KeyModifiers::ALT),
        }
    }

    /// `true` when `Primary` chords exist for this profile and env.
    pub fn primary_enabled(&self) -> bool {
        !matches!(self.profile, KeyProfileName::Legacy | KeyProfileName::Portable) && primary_mods(&self.env).is_some()
    }

    /// `true` when this env can feed macOS Option-composed characters into the Alt normalizer.
    pub fn normalizes_macos_option(&self) -> bool {
        self.env.os == HostOs::MacOs && self.env.surface == super::keyenv::Surface::Tui
    }

    /// `true` when an `Alt+key` chord must be delivered as a chord (not composed text) by a GUI
    /// runner: prompt-editing keys, or any chord this profile binds.
    pub fn option_chord_wanted(&self, key: KeyEvent) -> bool {
        PROMPT_EDIT_ALT_KEYS.contains(&key.code)
            || self.global_action(key, false).is_some()
            || self.pane_action(key).is_some()
    }

    fn unbound_chord(&self, mods: KeyModifiers, code: KeyCode) -> bool {
        !self.unbound.is_empty() && self.unbound.contains(&KeyChord::new(mods, code).folded())
    }

    fn row_active(&self, mods: KeyModifiers, code: KeyCode) -> bool {
        if self.unbound_chord(mods, code) {
            return false;
        }
        match self.profile {
            KeyProfileName::Legacy => true,
            profile => {
                if NON_LEGACY_REMOVED.iter().any(|(m, c)| *m == mods && *c == code) {
                    return false;
                }
                if profile == KeyProfileName::Portable && mods.contains(KeyModifiers::ALT) {
                    return false;
                }
                !is_alt_printable(mods, code) || self.alt_layer()
            }
        }
    }

    /// Active global rows (legacy table filtered by profile, plus macOS extras).
    pub fn global_rows(&self) -> impl Iterator<Item = Row> + Clone + '_ {
        let base = DEFAULT_GLOBAL_BINDINGS.iter().map(|(m, c, a)| (*m, *c, a)).filter(|(m, c, _)| self.row_active(*m, *c));
        let extra = MAC_EXTRA
            .iter()
            .map(|(m, c, a)| (*m, *c, a))
            .filter(|(m, c, _)| self.profile == KeyProfileName::Mac && !self.unbound_chord(*m, *c));
        base.chain(extra)
    }

    /// Active pane rows (legacy table filtered by profile).
    pub fn pane_rows(&self) -> impl Iterator<Item = Row> + Clone + '_ {
        DEFAULT_PANE_BINDINGS.iter().map(|(m, c, a)| (*m, *c, a)).filter(|(m, c, _)| self.row_active(*m, *c))
    }

    /// Active leader rows (`w`/`m` differ outside `legacy`).
    pub fn leader_rows(&self) -> Vec<(KeyCode, Action)> {
        let legacy = self.profile == KeyProfileName::Legacy;
        DEFAULT_LEADER_BINDINGS
            .iter()
            .map(|(code, action)| {
                let over = (!legacy).then(|| LEADER_OVERRIDES.iter().find(|(c, _)| c == code)).flatten();
                (*code, over.map_or_else(|| action.clone(), |(_, a)| a.clone()))
            })
            .collect()
    }

    /// Global shortcut lookup (`Palette`, `Quake`, F-keys, Alt chords, Ctrl+Alt launchers).
    pub fn global_action(&self, key: KeyEvent, fuzzy_search_active: bool) -> Option<Action> {
        if fuzzy_search_active && key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('p') {
            return None;
        }
        lookup_global(self.global_rows(), key)
    }

    /// Intra-pane shortcut lookup (`F3`, `Alt+<key>` in command-prompt mode).
    pub fn pane_action(&self, key: KeyEvent) -> Option<Action> {
        lookup_pane(self.pane_rows(), key)
    }

    /// Leader (`<Leader> + key`) lookup.
    pub fn leader_action(&self, key: KeyEvent) -> Option<Action> {
        let legacy = self.profile == KeyProfileName::Legacy;
        if !legacy && let Some((_, a)) = LEADER_OVERRIDES.iter().find(|(c, _)| *c == key.code) {
            return Some(a.clone());
        }
        super::keymap::default_leader_action(key)
    }

    /// `Primary + key` lookup for this env (`None` when Primary is unavailable or not bound).
    pub fn primary_action(&self, key: KeyEvent) -> Option<Action> {
        if !self.primary_enabled() {
            return None;
        }
        let chord = KeyChord::from_event(key, &self.env);
        if !chord.primary || self.unbound.contains(&chord.folded()) {
            return None;
        }
        PRIMARY_BINDINGS.iter().find(|(c, _)| *c == chord.code).map(|(_, a)| a.clone())
    }

    /// Every chord bound to `action` in this profile (Primary first, then global, then pane rows).
    pub fn keys_for_action(&self, action: &Action) -> Vec<KeyChord> {
        let mut out: Vec<KeyChord> = Vec::new();
        let mut push = |c: KeyChord| {
            if !out.iter().any(|o| o.folded() == c.folded()) {
                out.push(c);
            }
        };
        if self.primary_enabled() {
            for (code, a) in PRIMARY_BINDINGS {
                if a == action && !self.unbound.contains(&KeyChord::primary(*code).folded()) {
                    push(KeyChord::primary(*code));
                }
            }
        }
        for (m, c, a) in self.global_rows().chain(self.pane_rows()) {
            if a == action {
                push(KeyChord::new(m, c));
            }
        }
        out
    }

    /// Leader sequences (`Ctrl+X M`) bound to `action`, shortest key first.
    pub fn leader_keys_for_action(&self, action: &Action) -> Vec<KeyCode> {
        let mut keys: Vec<KeyCode> = self.leader_rows().into_iter().filter(|(_, a)| a == action).map(|(c, _)| c).collect();
        keys.sort_by_key(|c| match c {
            KeyCode::Char(ch) if ch.is_ascii_alphabetic() => 0,
            KeyCode::Char(_) => 1,
            _ => 2,
        });
        keys
    }

    /// Accelerator text for `action`: Primary chord > leader sequence > Window-mode sequence >
    /// any other bound chord. `leader_label` is the configured leader (default `Ctrl+X`).
    pub fn hint_for_action_with_leader(&self, action: &Action, leader_label: &str) -> Option<String> {
        if self.primary_enabled()
            && let Some(c) = self.keys_for_action(action).into_iter().find(|c| c.primary)
        {
            return Some(c.display(&self.env));
        }
        if let Some(code) = self.leader_keys_for_action(action).first() {
            return Some(format!("{leader_label} {}", leader_key_label(*code)));
        }
        // The sequence starts with the leader key that enters Window mode, which only exists
        // outside the `legacy` profile (there leader `w` still closes the pane).
        if !self.leader_keys_for_action(&Action::WindowMode(crate::action::WindowModeOp::Enter)).is_empty()
            && let Some(seq) = super::keyhint_window::window_mode_sequence(action)
        {
            return Some(format!("{leader_label} {seq}"));
        }
        self.keys_for_action(action).into_iter().next().map(|c| c.display(&self.env))
    }

    /// [`Self::hint_for_action_with_leader`] with the default `Ctrl+X` leader.
    pub fn hint_for_action(&self, action: &Action) -> Option<String> {
        self.hint_for_action_with_leader(action, "Ctrl+X")
    }

    /// Internal integrity check: folded chords that two different actions share and that are not
    /// in [`ALLOWED_SHADOWS`]. Covers global+pane+Primary+macOS rows together, and the leader table.
    pub fn unexpected_duplicates(&self) -> Vec<DuplicateChord> {
        let mut seen: Vec<(KeyChord, Action)> = Vec::new();
        let mut out: Vec<DuplicateChord> = Vec::new();
        let rows = self
            .global_rows()
            .chain(self.pane_rows())
            .map(|(m, c, a)| (KeyChord::new(m, c), a.clone()))
            .chain(
                PRIMARY_BINDINGS
                    .iter()
                    .filter(|_| self.primary_enabled())
                    .map(|(c, a)| (KeyChord::primary(*c), a.clone())),
            );
        for (chord, action) in rows {
            let folded = chord.folded();
            for (other, other_action) in seen.iter().filter(|(o, _)| *o == folded) {
                if *other_action != action {
                    let name = folded.format_with("Super");
                    if !ALLOWED_SHADOWS.iter().any(|(c, _)| c.eq_ignore_ascii_case(&name)) {
                        out.push(DuplicateChord { chord: name, table: "chords", actions: vec![other_action.clone(), action.clone()] });
                    }
                    let _ = other;
                }
            }
            seen.push((folded, action));
        }
        let mut lead: Vec<(KeyCode, Action)> = Vec::new();
        for (code, action) in self.leader_rows() {
            if let Some((_, other)) = lead.iter().find(|(c, _)| *c == code)
                && *other != action
            {
                out.push(DuplicateChord { chord: leader_key_label(code), table: "leader", actions: vec![other.clone(), action.clone()] });
            }
            lead.push((code, action));
        }
        out.sort_by(|a, b| (a.table, &a.chord).cmp(&(b.table, &b.chord)));
        out.dedup();
        out
    }
}

/// Command bound to `key` by the user: `[keys.bind]` first (accepts `Primary`/`Cmd` tokens and
/// matches after Shift-folding), then the legacy `[keybindings]` table (exact chord string).
pub(crate) fn user_bound_command(cfg: &crate::config::BiltermConfig, key: KeyEvent, env: &KeyEnv) -> Option<String> {
    if !cfg.keys.bind.is_empty() {
        let ev = KeyChord::from_event(key, env).folded();
        let plain_env = KeyEnv { surface: super::keyenv::Surface::Tui, kitty_kb: false, ..*env };
        let plain = KeyChord::from_event(key, &plain_env).folded();
        for (chord, command) in &cfg.keys.bind {
            if let Ok(c) = KeyChord::parse(chord) {
                let c = c.folded();
                if c == ev || c == plain {
                    return Some(command.clone());
                }
            }
        }
    }
    if !cfg.keybindings.is_empty()
        && let Some(chord) = super::keymap::format_key_chord(key)
    {
        return cfg.keybindings.iter().find(|(k, _)| k.eq_ignore_ascii_case(&chord)).map(|(_, v)| v.clone());
    }
    None
}

/// Leader key label as typed after the leader (`M`, `Space`, `F5`).
pub fn leader_key_label(code: KeyCode) -> String {
    match code {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::F(n) => format!("F{n}"),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "BackTab".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        other => format!("{other:?}"),
    }
}

/// Accelerator text for `action` under the auto-selected profile of `env` with default config.
/// Used by menus and help where no workspace is at hand; workspaces call
/// [`crate::wm::WindowManager::hint_for_action`] to honour `[keys]` overrides.
pub fn hint_for_action(action: &Action, env: &KeyEnv) -> Option<String> {
    KeyProfile::resolve(&KeysConfig::default(), env).hint_for_action(action)
}

/// `Ctrl+X M` -> `^X M` for compact bars.
pub fn compact_hint(hint: &str) -> String {
    hint.replacen("Ctrl+", "^", 1)
}

/// One row of the which-key overlay (`KEY-01`); generated by `ui/which_key_ui.rs`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhichKeyRow {
    pub chord: String,
    pub label: String,
    pub group: &'static str,
}

impl EffectiveKeymap {
    /// Leader footer rows generated from this keymap's live leader table
    /// (see `ui::which_key_ui`, the single generator).
    pub fn leader_footer_rows(&self) -> Vec<WhichKeyRow> {
        crate::ui::which_key_ui::leader_footer_rows(self)
    }

    /// Single status line shown while the leader awaits its next key, built from the live table.
    pub fn leader_status_line(&self, leader_label: &str) -> String {
        crate::ui::which_key_ui::footer_line(&KeyMode::LeaderPending, self, leader_label)
    }
}

/// Which-key rows for `mode` under the auto profile of `env` with the default `[keys]` config
/// (leader and Window mode; generated in `ui/which_key_ui.rs`).
pub fn which_key_rows(mode: &KeyMode, env: &KeyEnv) -> Vec<WhichKeyRow> {
    crate::ui::which_key_ui::which_key_rows(mode, env)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::keyenv::Surface;

    fn km(profile: KeyProfileName, env: KeyEnv) -> EffectiveKeymap {
        let cfg = KeysConfig { profile, ..KeysConfig::default() };
        KeyProfile::resolve(&cfg, &env)
    }

    #[test]
    fn auto_resolves_by_os() {
        assert_eq!(KeyProfile::concrete(KeyProfileName::Auto, HostOs::MacOs), KeyProfileName::Mac);
        assert_eq!(KeyProfile::concrete(KeyProfileName::Auto, HostOs::Windows), KeyProfileName::Windows);
        assert_eq!(KeyProfile::concrete(KeyProfileName::Auto, HostOs::Linux), KeyProfileName::Linux);
        assert_eq!(KeyProfile::concrete(KeyProfileName::Legacy, HostOs::MacOs), KeyProfileName::Legacy);
    }

    #[test]
    fn legacy_rows_are_the_unfiltered_tables() {
        let k = EffectiveKeymap::legacy_default();
        assert_eq!(k.global_rows().count(), DEFAULT_GLOBAL_BINDINGS.len());
        assert_eq!(k.pane_rows().count(), DEFAULT_PANE_BINDINGS.len());
        assert_eq!(k.leader_rows().len(), DEFAULT_LEADER_BINDINGS.len());
        let alt_m = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT);
        assert_eq!(k.global_action(alt_m, false), Some(Action::LayoutCycle));
        assert_eq!(k.leader_action(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE)), Some(Action::ClosePane));
        assert!(!k.primary_enabled());
    }

    #[test]
    fn mac_unbinds_alt_m_and_reaches_layout_cycle_via_primary() {
        let env = KeyEnv::new(HostOs::MacOs, Surface::Gui);
        let k = km(KeyProfileName::Mac, env);
        let alt_m = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT);
        assert_eq!(k.global_action(alt_m, false), None);
        assert_eq!(k.primary_action(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::SUPER)), Some(Action::LayoutCycle));
        assert_eq!(k.hint_for_action(&Action::LayoutCycle).as_deref(), Some("⌘L"));
    }

    #[test]
    fn hint_falls_back_to_leader_without_primary() {
        let env = KeyEnv::new(HostOs::Linux, Surface::Tui);
        assert_eq!(hint_for_action(&Action::LayoutCycle, &env).as_deref(), Some("Ctrl+X M"));
        let gui = KeyEnv::new(HostOs::Linux, Surface::Gui);
        assert_eq!(hint_for_action(&Action::LayoutCycle, &gui).as_deref(), Some("Ctrl+Shift+L"));
        assert_eq!(compact_hint("Ctrl+X M"), "^X M");
    }

    #[test]
    fn unbind_removes_shipped_chords() {
        let cfg = KeysConfig { profile: KeyProfileName::Linux, unbind: vec!["Alt+V".into()], ..KeysConfig::default() };
        let k = KeyProfile::resolve(&cfg, &KeyEnv::headless());
        assert_eq!(k.global_action(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::ALT), false), None);
        assert_eq!(
            k.global_action(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT), false),
            Some(Action::SplitHorizontal)
        );
    }

    #[test]
    fn leader_status_line_comes_from_the_table() {
        let line = km(KeyProfileName::Linux, KeyEnv::headless()).leader_status_line("Ctrl+X");
        assert!(line.starts_with("⌨ LEADER [Ctrl+X]:"), "{line}");
        assert!(line.contains("Zoom") && line.contains("Window") && line.contains("Menu"), "{line}");
    }
}
