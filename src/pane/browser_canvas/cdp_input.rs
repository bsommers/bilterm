//! Terminal key and mouse events to CDP `Input.*` commands (`WEB-02`).

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton};
use serde_json::{Map, Value, json};

use super::cdp_proto::{Class, Cmd, Scope, Tag, insert_text};
use super::{MediaMouseEvent, MediaMouseEventKind};

/// CDP modifier bit field: Alt=1, Ctrl=2, Meta=4, Shift=8.
pub fn cdp_modifiers(m: KeyModifiers) -> u8 {
    let mut bits = 0;
    if m.contains(KeyModifiers::ALT) {
        bits |= 1;
    }
    if m.contains(KeyModifiers::CONTROL) {
        bits |= 2;
    }
    if m.intersects(KeyModifiers::SUPER | KeyModifiers::META) {
        bits |= 4;
    }
    if m.contains(KeyModifiers::SHIFT) {
        bits |= 8;
    }
    bits
}

/// DOM `key`, DOM `code`, Windows virtual key code and text for one key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeySpec {
    pub key: String,
    pub code: String,
    pub vk: u32,
    pub text: Option<String>,
}

fn named(key: &str, code: &str, vk: u32, text: Option<&str>) -> KeySpec {
    KeySpec { key: key.to_string(), code: code.to_string(), vk, text: text.map(str::to_string) }
}

/// `(code, virtual key)` for an ASCII character on a US layout.
fn us_layout(c: char) -> Option<(String, u32)> {
    let lower = c.to_ascii_lowercase();
    if lower.is_ascii_lowercase() {
        return Some((format!("Key{}", lower.to_ascii_uppercase()), lower.to_ascii_uppercase() as u32));
    }
    if c.is_ascii_digit() {
        return Some((format!("Digit{c}"), c as u32));
    }
    let (code, vk) = match c {
        ' ' => ("Space", 32),
        ')' => ("Digit0", 48),
        '!' => ("Digit1", 49),
        '@' => ("Digit2", 50),
        '#' => ("Digit3", 51),
        '$' => ("Digit4", 52),
        '%' => ("Digit5", 53),
        '^' => ("Digit6", 54),
        '&' => ("Digit7", 55),
        '*' => ("Digit8", 56),
        '(' => ("Digit9", 57),
        ';' | ':' => ("Semicolon", 186),
        '=' | '+' => ("Equal", 187),
        ',' | '<' => ("Comma", 188),
        '-' | '_' => ("Minus", 189),
        '.' | '>' => ("Period", 190),
        '/' | '?' => ("Slash", 191),
        '`' | '~' => ("Backquote", 192),
        '[' | '{' => ("BracketLeft", 219),
        '\\' | '|' => ("Backslash", 220),
        ']' | '}' => ("BracketRight", 221),
        '\'' | '"' => ("Quote", 222),
        _ => return None,
    };
    Some((code.to_string(), vk))
}

/// Maps a terminal key to its DOM description. `None` for keys with no browser meaning.
pub fn key_spec(code: KeyCode, mods: KeyModifiers) -> Option<KeySpec> {
    let spec = match code {
        KeyCode::Enter => named("Enter", "Enter", 13, Some("\r")),
        KeyCode::Tab => named("Tab", "Tab", 9, None),
        KeyCode::BackTab => named("Tab", "Tab", 9, None),
        KeyCode::Backspace => named("Backspace", "Backspace", 8, None),
        KeyCode::Delete => named("Delete", "Delete", 46, None),
        KeyCode::Insert => named("Insert", "Insert", 45, None),
        KeyCode::Esc => named("Escape", "Escape", 27, None),
        KeyCode::Left => named("ArrowLeft", "ArrowLeft", 37, None),
        KeyCode::Up => named("ArrowUp", "ArrowUp", 38, None),
        KeyCode::Right => named("ArrowRight", "ArrowRight", 39, None),
        KeyCode::Down => named("ArrowDown", "ArrowDown", 40, None),
        KeyCode::Home => named("Home", "Home", 36, None),
        KeyCode::End => named("End", "End", 35, None),
        KeyCode::PageUp => named("PageUp", "PageUp", 33, None),
        KeyCode::PageDown => named("PageDown", "PageDown", 34, None),
        KeyCode::F(n @ 1..=12) => named(&format!("F{n}"), &format!("F{n}"), 111 + u32::from(n), None),
        KeyCode::Char(c) => {
            let (dom_code, vk) = us_layout(c)?;
            let chord = mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER | KeyModifiers::META);
            KeySpec { key: c.to_string(), code: dom_code, vk, text: (!chord).then(|| c.to_string()) }
        }
        _ => return None,
    };
    Some(spec)
}

/// Editing commands Chrome expects alongside Ctrl chords (headless has no native key bindings).
fn editing_command(code: KeyCode, mods: KeyModifiers) -> Option<&'static str> {
    if !mods.contains(KeyModifiers::CONTROL) || mods.contains(KeyModifiers::ALT) {
        return None;
    }
    match code {
        KeyCode::Char('a') => Some("selectAll"),
        KeyCode::Char('c') => Some("copy"),
        KeyCode::Char('v') => Some("paste"),
        KeyCode::Char('z') => Some("undo"),
        KeyCode::Char('y') => Some("redo"),
        _ => None,
    }
}

fn key_cmd(kind: &str, spec: &KeySpec, modifiers: u8, with_text: bool, command: Option<&str>) -> Cmd {
    let mut p = Map::new();
    p.insert("type".into(), json!(kind));
    p.insert("key".into(), json!(spec.key));
    p.insert("code".into(), json!(spec.code));
    p.insert("windowsVirtualKeyCode".into(), json!(spec.vk));
    p.insert("nativeVirtualKeyCode".into(), json!(spec.vk));
    p.insert("modifiers".into(), json!(modifiers));
    if with_text && let Some(t) = &spec.text {
        p.insert("text".into(), json!(t));
        p.insert("unmodifiedText".into(), json!(t));
    }
    if let Some(c) = command {
        p.insert("commands".into(), json!([c]));
    }
    Cmd { method: "Input.dispatchKeyEvent", params: Value::Object(p), scope: Scope::Page, tag: Tag::None, class: Class::Normal }
}

/// Commands for one terminal key press: `keyDown` (with text when printable) then `keyUp`.
///
/// Key releases produce nothing (the press already sent both halves). Characters outside the US
/// layout table (accents, CJK, emoji) are sent with `Input.insertText`.
pub fn key_commands(key: KeyEvent) -> Vec<Cmd> {
    if key.kind == KeyEventKind::Release {
        return Vec::new();
    }
    let mut mods = key.modifiers;
    if key.code == KeyCode::BackTab {
        mods |= KeyModifiers::SHIFT;
    }
    let bits = cdp_modifiers(mods);
    let Some(spec) = key_spec(key.code, mods) else {
        return match key.code {
            KeyCode::Char(c) if !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                vec![insert_text(&c.to_string())]
            }
            _ => Vec::new(),
        };
    };
    let down_kind = if spec.text.is_some() { "keyDown" } else { "rawKeyDown" };
    vec![
        key_cmd(down_kind, &spec, bits, true, editing_command(key.code, mods)),
        key_cmd("keyUp", &spec, bits, false, None),
    ]
}

fn button_name(b: MouseButton) -> (&'static str, u8) {
    match b {
        MouseButton::Left => ("left", 1),
        MouseButton::Right => ("right", 2),
        MouseButton::Middle => ("middle", 4),
    }
}

/// Builds `Input.dispatchMouseEvent` for a mouse event at CSS pixel `(x, y)`.
pub fn mouse_command(event: &MediaMouseEvent, x_css: f64, y_css: f64, click_count: u8) -> Cmd {
    let mut p = Map::new();
    p.insert("x".into(), json!(x_css));
    p.insert("y".into(), json!(y_css));
    p.insert("modifiers".into(), json!(cdp_modifiers(event.modifiers)));
    let mut class = Class::Normal;
    let kind = match event.kind {
        MediaMouseEventKind::Down(b) | MediaMouseEventKind::Up(b) => {
            let (name, mask) = button_name(b);
            let down = matches!(event.kind, MediaMouseEventKind::Down(_));
            p.insert("button".into(), json!(name));
            p.insert("buttons".into(), json!(if down { mask } else { 0 }));
            p.insert("clickCount".into(), json!(click_count.max(1)));
            if down { "mousePressed" } else { "mouseReleased" }
        }
        MediaMouseEventKind::Drag(b) => {
            let (name, mask) = button_name(b);
            p.insert("button".into(), json!(name));
            p.insert("buttons".into(), json!(mask));
            class = Class::MouseMove;
            "mouseMoved"
        }
        MediaMouseEventKind::Move => {
            p.insert("button".into(), json!("none"));
            p.insert("buttons".into(), json!(0));
            class = Class::MouseMove;
            "mouseMoved"
        }
        MediaMouseEventKind::Wheel { delta_x, delta_y } => {
            p.insert("deltaX".into(), json!(delta_x));
            p.insert("deltaY".into(), json!(delta_y));
            "mouseWheel"
        }
    };
    p.insert("type".into(), json!(kind));
    Cmd { method: "Input.dispatchMouseEvent", params: Value::Object(p), scope: Scope::Page, tag: Tag::None, class }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode, mods: KeyModifiers) -> Vec<Cmd> {
        key_commands(KeyEvent::new(code, mods))
    }

    #[test]
    fn printable_letter_sends_keydown_with_text_then_keyup() {
        let cmds = press(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(cmds.len(), 2);
        let d = &cmds[0].params;
        assert_eq!(d["type"], "keyDown");
        assert_eq!(d["key"], "a");
        assert_eq!(d["code"], "KeyA");
        assert_eq!(d["windowsVirtualKeyCode"], 65);
        assert_eq!(d["text"], "a");
        assert_eq!(d["modifiers"], 0);
        assert_eq!(cmds[1].params["type"], "keyUp");
        assert!(cmds[1].params.get("text").is_none());
    }

    #[test]
    fn shifted_and_punctuation_keys_map_to_us_layout() {
        let up = press(KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert_eq!(up[0].params["key"], "A");
        assert_eq!(up[0].params["code"], "KeyA");
        assert_eq!(up[0].params["modifiers"], 8);
        assert_eq!(up[0].params["text"], "A");
        let q = press(KeyCode::Char('?'), KeyModifiers::SHIFT);
        assert_eq!(q[0].params["code"], "Slash");
        assert_eq!(q[0].params["windowsVirtualKeyCode"], 191);
        let sp = press(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(sp[0].params["code"], "Space");
        assert_eq!(sp[0].params["text"], " ");
        let d7 = press(KeyCode::Char('7'), KeyModifiers::NONE);
        assert_eq!(d7[0].params["code"], "Digit7");
        assert_eq!(d7[0].params["windowsVirtualKeyCode"], 55);
    }

    #[test]
    fn special_keys_use_raw_key_down_except_enter() {
        let enter = press(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(enter[0].params["type"], "keyDown");
        assert_eq!(enter[0].params["key"], "Enter");
        assert_eq!(enter[0].params["windowsVirtualKeyCode"], 13);
        assert_eq!(enter[0].params["text"], "\r");
        for (code, key, vk) in [
            (KeyCode::Backspace, "Backspace", 8),
            (KeyCode::Tab, "Tab", 9),
            (KeyCode::Esc, "Escape", 27),
            (KeyCode::Left, "ArrowLeft", 37),
            (KeyCode::Down, "ArrowDown", 40),
            (KeyCode::PageDown, "PageDown", 34),
            (KeyCode::Delete, "Delete", 46),
            (KeyCode::F(5), "F5", 116),
        ] {
            let c = press(code, KeyModifiers::NONE);
            assert_eq!(c[0].params["type"], "rawKeyDown", "{key}");
            assert_eq!(c[0].params["key"], key);
            assert_eq!(c[0].params["windowsVirtualKeyCode"], vk);
            assert!(c[0].params.get("text").is_none());
        }
        let bt = press(KeyCode::BackTab, KeyModifiers::NONE);
        assert_eq!(bt[0].params["key"], "Tab");
        assert_eq!(bt[0].params["modifiers"], 8, "BackTab is Shift+Tab");
    }

    #[test]
    fn ctrl_chords_have_no_text_and_carry_editing_commands() {
        let a = press(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(a[0].params["type"], "rawKeyDown");
        assert_eq!(a[0].params["modifiers"], 2);
        assert!(a[0].params.get("text").is_none());
        assert_eq!(a[0].params["commands"], json!(["selectAll"]));
        let k = press(KeyCode::Char('k'), KeyModifiers::CONTROL | KeyModifiers::ALT);
        assert_eq!(k[0].params["modifiers"], 3);
        assert!(k[0].params.get("commands").is_none());
    }

    #[test]
    fn non_ascii_text_uses_insert_text_and_releases_are_dropped() {
        let e = press(KeyCode::Char('é'), KeyModifiers::NONE);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].method, "Input.insertText");
        assert_eq!(e[0].params["text"], "é");
        assert!(press(KeyCode::Char('é'), KeyModifiers::CONTROL).is_empty());
        let mut rel = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        rel.kind = KeyEventKind::Release;
        assert!(key_commands(rel).is_empty());
        assert!(press(KeyCode::CapsLock, KeyModifiers::NONE).is_empty());
    }

    #[test]
    fn mouse_events_map_to_dispatch_mouse_event() {
        let ev = |kind| MediaMouseEvent { kind, x_px: 0, y_px: 0, modifiers: KeyModifiers::SHIFT };
        let down = mouse_command(&ev(MediaMouseEventKind::Down(MouseButton::Left)), 12.5, 40.0, 2);
        assert_eq!(down.method, "Input.dispatchMouseEvent");
        assert_eq!(down.params["type"], "mousePressed");
        assert_eq!(down.params["button"], "left");
        assert_eq!(down.params["buttons"], 1);
        assert_eq!(down.params["clickCount"], 2);
        assert_eq!(down.params["x"], 12.5);
        assert_eq!(down.params["modifiers"], 8);
        let up = mouse_command(&ev(MediaMouseEventKind::Up(MouseButton::Right)), 1.0, 2.0, 1);
        assert_eq!(up.params["type"], "mouseReleased");
        assert_eq!(up.params["button"], "right");
        assert_eq!(up.params["buttons"], 0);
        let mv = mouse_command(&ev(MediaMouseEventKind::Move), 1.0, 2.0, 0);
        assert_eq!(mv.params["type"], "mouseMoved");
        assert_eq!(mv.class, Class::MouseMove);
        let drag = mouse_command(&ev(MediaMouseEventKind::Drag(MouseButton::Middle)), 1.0, 2.0, 0);
        assert_eq!(drag.params["buttons"], 4);
        assert_eq!(drag.class, Class::MouseMove);
        let wheel = mouse_command(&ev(MediaMouseEventKind::Wheel { delta_x: 0, delta_y: -53 }), 5.0, 6.0, 0);
        assert_eq!(wheel.params["type"], "mouseWheel");
        assert_eq!(wheel.params["deltaY"], -53);
        assert_eq!(wheel.class, Class::Normal);
    }
}
