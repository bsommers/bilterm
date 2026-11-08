//! `KeyChord`: the one parser/formatter for key chords (`KEY-02`, Phase 63).
//!
//! Grammar: `Token+Token+Key`, case-insensitive. Modifier tokens: `Ctrl Control Alt Option Opt
//! Shift Cmd Super Primary`. Keys: one character, `F1..F24`, `Space Enter Tab BackTab Esc Left
//! Right Up Down Home End PageUp PageDown Insert Delete Backspace`. At most [`MAX_CHORD_LEN`]
//! characters; unknown tokens are rejected. `Primary` stays symbolic until resolved against a
//! [`KeyEnv`], so one config line works on every OS.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::keyenv::{HostOs, KeyEnv, Surface, primary_mods};

/// Longest accepted chord string (config input cap).
pub const MAX_CHORD_LEN: usize = 64;

/// A parsed key chord. `primary` means "the platform primary modifier" (not stored in `mods`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub mods: KeyModifiers,
    pub primary: bool,
    pub code: KeyCode,
}

/// Undoes Shift on digit-row and bracket symbols so `Ctrl+Shift+1` (reported as `!`) is `1`.
pub fn base_key(c: char) -> char {
    match c {
        '!' => '1',
        '@' => '2',
        '#' => '3',
        '$' => '4',
        '%' => '5',
        '^' => '6',
        '&' => '7',
        '*' => '8',
        '(' => '9',
        ')' => '0',
        '{' => '[',
        '}' => ']',
        ':' => ';',
        '"' => '\'',
        '<' => ',',
        '>' => '.',
        '?' => '/',
        '|' => '\\',
        '~' => '`',
        '_' => '-',
        '+' => '=',
        other => other,
    }
}

impl KeyChord {
    /// Chord with explicit modifiers.
    pub const fn new(mods: KeyModifiers, code: KeyCode) -> Self {
        Self { mods, primary: false, code }
    }

    /// `Primary+<code>`.
    pub const fn primary(code: KeyCode) -> Self {
        Self { mods: KeyModifiers::NONE, primary: true, code }
    }

    /// Chord of `key` exactly as reported (no folding); used for legacy string matching.
    pub fn raw(key: KeyEvent) -> Self {
        Self { mods: key.modifiers, primary: false, code: key.code }
    }

    /// Parses a chord string (see module docs).
    pub fn parse(input: &str) -> Result<Self, String> {
        let s = input.trim();
        if s.is_empty() {
            return Err("empty key chord".to_string());
        }
        if s.chars().count() > MAX_CHORD_LEN {
            return Err(format!("key chord longer than {MAX_CHORD_LEN} characters"));
        }
        let (head, key_tok) = if s == "+" {
            ("", "+")
        } else if let Some(h) = s.strip_suffix("++") {
            (h, "+")
        } else {
            match s.rfind('+') {
                Some(i) => (&s[..i], &s[i + 1..]),
                None => ("", s),
            }
        };
        if key_tok.is_empty() {
            return Err(format!("key chord '{s}' has no key"));
        }
        let mut mods = KeyModifiers::NONE;
        let mut primary = false;
        if !head.is_empty() {
            for tok in head.split('+') {
                let (flag, is_primary) = match tok.to_ascii_lowercase().as_str() {
                    "ctrl" | "control" => (KeyModifiers::CONTROL, false),
                    "alt" | "option" | "opt" => (KeyModifiers::ALT, false),
                    "shift" => (KeyModifiers::SHIFT, false),
                    "cmd" | "super" => (KeyModifiers::SUPER, false),
                    "primary" => (KeyModifiers::NONE, true),
                    _ => return Err(format!("unknown modifier '{tok}' in key chord '{s}'")),
                };
                if is_primary {
                    if primary {
                        return Err(format!("duplicate modifier 'Primary' in '{s}'"));
                    }
                    primary = true;
                } else {
                    if mods.contains(flag) {
                        return Err(format!("duplicate modifier '{tok}' in '{s}'"));
                    }
                    mods |= flag;
                }
            }
        }
        if primary && mods.intersects(KeyModifiers::CONTROL | KeyModifiers::SHIFT | KeyModifiers::SUPER) {
            return Err(format!("'Primary' cannot be combined with Ctrl, Shift or Cmd in '{s}'"));
        }
        let code = parse_key(key_tok).ok_or_else(|| format!("unknown key '{key_tok}' in key chord '{s}'"))?;
        Ok(Self { mods, primary, code })
    }

    /// Canonical chord string. `Primary` stays symbolic (config round-trip); `env` only decides
    /// whether the Super modifier is spelled `Cmd` (macOS) or `Super`.
    pub fn format(&self, env: &KeyEnv) -> String {
        self.format_with(if env.os == HostOs::MacOs { "Cmd" } else { "Super" })
    }

    pub(crate) fn format_with(&self, super_name: &str) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.primary {
            parts.push("Primary".to_string());
        }
        if self.mods.contains(KeyModifiers::SUPER) {
            parts.push(super_name.to_string());
        }
        if self.mods.contains(KeyModifiers::CONTROL) {
            parts.push("Ctrl".to_string());
        }
        if self.mods.contains(KeyModifiers::ALT) {
            parts.push("Alt".to_string());
        }
        if self.mods.contains(KeyModifiers::SHIFT) {
            parts.push("Shift".to_string());
        }
        parts.push(key_name(self.code));
        parts.join("+")
    }

    /// Human display with `Primary` resolved against `env`: macOS GUI uses glyphs (`⌘L`, `⌃⌥⇧⌘`
    /// order), everything else spells modifiers out (`Ctrl+Shift+L`).
    pub fn display(&self, env: &KeyEnv) -> String {
        let mut mods = self.mods;
        if self.primary {
            match primary_mods(env) {
                Some(p) => mods |= p,
                None => return self.format(env),
            }
        }
        let key = key_name(self.code);
        if env.os == HostOs::MacOs && env.surface == Surface::Gui {
            let mut s = String::new();
            if mods.contains(KeyModifiers::CONTROL) {
                s.push('⌃');
            }
            if mods.contains(KeyModifiers::ALT) {
                s.push('⌥');
            }
            if mods.contains(KeyModifiers::SHIFT) {
                s.push('⇧');
            }
            if mods.contains(KeyModifiers::SUPER) {
                s.push('⌘');
            }
            s.push_str(&key);
            return s;
        }
        let mut parts: Vec<&str> = Vec::new();
        if mods.contains(KeyModifiers::SUPER) {
            parts.push(if env.os == HostOs::MacOs { "Cmd" } else { "Super" });
        }
        if mods.contains(KeyModifiers::CONTROL) {
            parts.push("Ctrl");
        }
        if mods.contains(KeyModifiers::ALT) {
            parts.push("Alt");
        }
        if mods.contains(KeyModifiers::SHIFT) {
            parts.push("Shift");
        }
        let mut s = parts.join("+");
        if !s.is_empty() {
            s.push('+');
        }
        s.push_str(&key);
        s
    }

    /// Canonical lookup form of a key event: shifted symbols map back to their base key, letters
    /// fold to lowercase, and when the event's modifiers equal the env's `Primary` set the result
    /// is the symbolic `Primary` chord. Shift is dropped for alphabetic keys.
    pub fn from_event(key: KeyEvent, env: &KeyEnv) -> Self {
        let mut mods = key.modifiers & !(KeyModifiers::HYPER | KeyModifiers::META);
        let mut code = key.code;
        if let KeyCode::Char(c) = code {
            let shifted = mods.contains(KeyModifiers::SHIFT);
            let base = if shifted { base_key(c) } else { c };
            code = KeyCode::Char(base.to_ascii_lowercase());
        }
        if code == KeyCode::BackTab {
            mods |= KeyModifiers::SHIFT;
        }
        if let Some(p) = primary_mods(env) {
            let without_shift = |m: KeyModifiers| m & !KeyModifiers::SHIFT;
            let matches = if p.contains(KeyModifiers::SHIFT) { mods == p } else { without_shift(mods) == p };
            if matches {
                return Self { mods: KeyModifiers::NONE, primary: true, code };
            }
        }
        if matches!(code, KeyCode::Char(ch) if ch.is_alphabetic()) {
            mods &= !KeyModifiers::SHIFT;
        }
        Self { mods, primary: false, code }
    }

    /// `true` when this is a `Primary` chord and `env` can deliver Primary chords at all.
    pub fn matches_primary(&self, env: &KeyEnv) -> bool {
        self.primary && primary_mods(env).is_some()
    }

    /// Folded form used for duplicate detection: letters lowercase, Shift dropped on `Char`.
    pub fn folded(&self) -> Self {
        let mut c = *self;
        if let KeyCode::Char(ch) = c.code {
            c.code = KeyCode::Char(ch.to_ascii_lowercase());
            c.mods &= !KeyModifiers::SHIFT;
        }
        c
    }
}

fn key_name(code: KeyCode) -> String {
    match code {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_ascii_uppercase().to_string(),
        KeyCode::F(n) => format!("F{n}"),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "BackTab".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::Insert => "Insert".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        other => format!("{other:?}"),
    }
}

fn parse_key(tok: &str) -> Option<KeyCode> {
    let mut it = tok.chars();
    if let (Some(c), None) = (it.next(), it.clone().next()) {
        return Some(KeyCode::Char(c.to_ascii_lowercase()));
    }
    let lower = tok.to_ascii_lowercase();
    if let Some(n) = lower.strip_prefix('f').and_then(|d| d.parse::<u8>().ok())
        && (1..=24).contains(&n)
    {
        return Some(KeyCode::F(n));
    }
    Some(match lower.as_str() {
        "space" => KeyCode::Char(' '),
        "enter" | "return" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "esc" | "escape" => KeyCode::Esc,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" => KeyCode::PageDown,
        "insert" | "ins" => KeyCode::Insert,
        "delete" | "del" => KeyCode::Delete,
        "backspace" => KeyCode::Backspace,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(os: HostOs, surface: Surface) -> KeyEnv {
        KeyEnv::new(os, surface)
    }

    #[test]
    fn parse_format_round_trips() {
        let e = env(HostOs::MacOs, Surface::Gui);
        for s in ["Primary+L", "Cmd+Alt+Left", "Ctrl+Alt+PageDown", "F13", "Ctrl+Shift+V", "Alt+Space", "Ctrl++"] {
            let c = KeyChord::parse(s).unwrap_or_else(|er| panic!("{s}: {er}"));
            let again = KeyChord::parse(&c.format(&e)).unwrap();
            assert_eq!(c, again, "{s}");
        }
        assert_eq!(
            KeyChord::parse("Cmd+Option+Left").unwrap(),
            KeyChord::new(KeyModifiers::SUPER | KeyModifiers::ALT, KeyCode::Left)
        );
        assert_eq!(KeyChord::parse("F13").unwrap().code, KeyCode::F(13));
        assert_eq!(KeyChord::parse("primary+l").unwrap(), KeyChord::primary(KeyCode::Char('l')));
        assert_eq!(KeyChord::parse("Ctrl++").unwrap().code, KeyCode::Char('+'));
    }

    #[test]
    fn rejects_bad_input() {
        for bad in ["Hyper+X", "", "   ", "Ctrl+", "Primary+Shift+L", "Ctrl+Ctrl+A", "Ctrl+Bogus", "F25", "F0"] {
            assert!(KeyChord::parse(bad).is_err(), "{bad:?} must be rejected");
        }
        let long = format!("Ctrl+{}", "A".repeat(60));
        assert_eq!(long.chars().count(), 65);
        assert!(KeyChord::parse(&long).is_err());
    }

    #[test]
    fn from_event_undoes_shifted_symbols_and_folds_primary() {
        let win_gui = env(HostOs::Windows, Surface::Gui);
        let c = KeyChord::from_event(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::CONTROL | KeyModifiers::SHIFT), &win_gui);
        assert_eq!(c, KeyChord::primary(KeyCode::Char('1')));
        let c = KeyChord::from_event(KeyEvent::new(KeyCode::Char('L'), KeyModifiers::CONTROL | KeyModifiers::SHIFT), &win_gui);
        assert_eq!(c, KeyChord::primary(KeyCode::Char('l')));
        let c = KeyChord::from_event(KeyEvent::new(KeyCode::Char('}'), KeyModifiers::CONTROL | KeyModifiers::SHIFT), &win_gui);
        assert_eq!(c, KeyChord::primary(KeyCode::Char(']')));
        let mac = env(HostOs::MacOs, Surface::Gui);
        let c = KeyChord::from_event(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::SUPER), &mac);
        assert_eq!(c, KeyChord::primary(KeyCode::Char('l')));
        // Legacy TUI: no Primary, Ctrl+Shift+L is just Ctrl+L after the alphabetic Shift fold.
        let tui = env(HostOs::Linux, Surface::Tui);
        let c = KeyChord::from_event(KeyEvent::new(KeyCode::Char('L'), KeyModifiers::CONTROL | KeyModifiers::SHIFT), &tui);
        assert_eq!(c, KeyChord::new(KeyModifiers::CONTROL, KeyCode::Char('l')));
    }

    #[test]
    fn display_uses_glyphs_only_on_macos_gui() {
        let p = KeyChord::primary(KeyCode::Char('l'));
        assert_eq!(p.display(&env(HostOs::MacOs, Surface::Gui)), "⌘L");
        assert_eq!(p.display(&env(HostOs::Linux, Surface::Gui)), "Ctrl+Shift+L");
        assert_eq!(p.display(&env(HostOs::Windows, Surface::Gui)), "Ctrl+Shift+L");
        assert_eq!(p.display(&env(HostOs::Linux, Surface::Tui).with_kitty(true)), "Ctrl+Shift+L");
        assert_eq!(p.display(&env(HostOs::Linux, Surface::Tui)), "Primary+L");
        let c = KeyChord::parse("Cmd+Option+Left").unwrap();
        assert_eq!(c.display(&env(HostOs::MacOs, Surface::Gui)), "⌥⌘Left");
    }
}
