//! `:clipboard` grammar end to end on a headless workspace: every verb and alias parses, executes
//! through `WindowManager::apply`, and the workspace still renders afterwards; malformed arguments
//! are answered with the usage text instead of being accepted or panicking.
//!
//! (`persist` / `save` are left to the persistence suites: they write the clipboard store to disk.)

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use bilterm::colon_command::parse_colon_command;
use bilterm::ui::WorkspaceWidget;
use bilterm::wm::WindowManager;

const HEADS: &[&str] = &[":clipboard", ":clips", ":clip", ":cb", ":buffers", ":yank-history", ":yank-ring"];

/// Every well-formed argument list of the grammar, in an order that exercises real state changes
/// (entries exist before they are selected, pinned, pasted and deleted).
const VERBS: &[&str] = &[
    "open", "copy first clip", "add second clip with spaces", "push third", "copy https://example.com/a?b=c", "show", "list", "ls",
    "all", "session", "sess", "session cycle", "sess next", "session active", "session current", "session reset", "session work",
    "window", "win", "pane", "window cycle", "win next", "window active", "pane reset", "window 1", "window #1", "window 999",
    "pinned", "pins", "media", "images", "video", "tab next", "tab prev", "all",
    "filter clip", "search second clip", "find https", "filter", "search clear", "find off", "filter reset",
    "first", "home", "down", "up", "prev", "step 2", "step -5", "step 0", "last", "end", "page-down", "page-up",
    "select 1", "select 2", "select 0", "select 999", "scroll 3", "scroll -3", "preview-scroll 2", "preview-scroll -9",
    "pin-entry", "pin-entry 1", "pin-entry #2 on", "pin-entry 2 off", "pin-entry 2 toggle", "pin-entry 1 pin", "pin-entry 1 unpin", "pin-entry 424242",
    "yank", "pop", "cycle", "next", "paste", "paste 1", "paste #2", "paste 999",
    "move 3 2", "move -50 -50", "move 500 500", "resize 4 3", "resize -100 -100", "resize 300 300", "rect 2 1 60 18", "rect 0 0 1 1",
    "rect 65535 65535 65535 65535", "size auto", "size reset", "scale +", "scale up", "scale in", "scale -", "scale down", "scale out", "scale 25",
    "scale -40%", "scale 0", "pin", "pin on", "pin true", "pin 1", "unpin", "pin off", "pin false", "pin 0", "pin toggle",
    "dock", "dock on", "dock true", "dock 1", "float", "undock", "dock off", "dock false", "dock 0", "dock toggle", "blur", "toggle", "toggle",
    "delete", "del", "rm", "delete 1", "del #2", "rm 424242", "clear", "reset", "dismiss", "close", "hide", "",
];

/// Argument lists the grammar must refuse.
const MALFORMED: &[&str] = &[
    "bogus", "open extra", "pin maybe", "dock sideways", "window abc", "window -1", "paste abc", "paste -1", "pin-entry abc",
    "pin-entry 1 maybe", "pin-entry 1 on extra", "delete abc", "delete 1 2", "step", "step x", "step 1 2", "select", "select x",
    "select -1", "scroll", "scroll x", "preview-scroll x", "move 1", "move a b", "move 1 2 3", "move 40000 0", "resize 1", "resize a b",
    "rect 1 2 3", "rect a b c d", "rect -1 0 1 1", "rect 70000 0 1 1", "size", "size huge", "scale", "scale x", "scale 99999", "tab",
    "tab sideways", "copy", "add", "push", "session a b", "persist maybe",
];

fn render(wm: &WindowManager) -> Buffer {
    let area = Rect::new(0, 0, 100, 30);
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(wm).render(area, &mut buf);
    buf
}

#[test]
fn every_verb_and_alias_parses_executes_and_renders() {
    let mut wm = WindowManager::new_headless(100, 30);
    let mut executed = 0;
    for (i, verb) in VERBS.iter().enumerate() {
        let head = HEADS[i % HEADS.len()];
        let line = format!("{head} {verb}");
        let action = match parse_colon_command(line.trim_end()) {
            Some(Ok(a)) => a,
            other => panic!("{line:?} must parse, got {other:?}"),
        };
        wm.apply(action).unwrap_or_else(|e| panic!("{line:?} failed to execute: {e}"));
        executed += 1;
        let buf = render(&wm);
        assert_eq!(buf.area, Rect::new(0, 0, 100, 30), "{line:?} corrupted the frame");
    }
    assert_eq!(executed, VERBS.len());
    // The bare head is a toggle: closed -> open -> closed.
    wm.apply(parse_colon_command(":clipboard close").unwrap().unwrap()).unwrap();
    assert!(!wm.clipboard_window().active);
    wm.apply(parse_colon_command(":clipboard").unwrap().unwrap()).unwrap();
    assert!(wm.clipboard_window().active);
    wm.apply(parse_colon_command(":clips toggle").unwrap().unwrap()).unwrap();
    assert!(!wm.clipboard_window().active);
}

#[test]
fn copied_text_is_recorded_listed_and_removable() {
    let mut wm = WindowManager::new_headless(100, 30);
    let run = |wm: &mut WindowManager, line: &str| {
        wm.apply(parse_colon_command(line).unwrap().unwrap()).unwrap();
    };
    assert!(wm.clipboard_registry().entries().is_empty());
    run(&mut wm, ":clipboard copy alpha one");
    run(&mut wm, ":clip add beta two");
    let texts: Vec<String> = wm.clipboard_registry().entries().iter().map(|e| e.content.clone()).collect();
    assert!(texts.iter().any(|t| t == "alpha one") && texts.iter().any(|t| t == "beta two"), "{texts:?}");

    run(&mut wm, ":clipboard open");
    assert!(wm.clipboard_window().active);
    let frame = render(&wm);
    let shown: String = frame.content.iter().map(|c| c.symbol()).collect();
    assert!(shown.contains("alpha one") || shown.contains("beta two"), "the open window lists the entries");

    // Pinned entries survive `clear`; unpinned ones do not.
    let keep = wm.clipboard_registry().entries().iter().find(|e| e.content == "alpha one").unwrap().id;
    run(&mut wm, &format!(":clipboard pin-entry {keep} on"));
    run(&mut wm, ":clipboard clear");
    let left: Vec<String> = wm.clipboard_registry().entries().iter().map(|e| e.content.clone()).collect();
    assert_eq!(left, vec!["alpha one".to_string()], "clear removes only unpinned entries");
    run(&mut wm, &format!(":clipboard delete {keep}"));
    assert!(wm.clipboard_registry().entries().is_empty());
}

#[test]
fn malformed_arguments_get_the_usage_text() {
    for bad in MALFORMED {
        for head in [":clipboard", ":cb"] {
            let line = format!("{head} {bad}");
            match parse_colon_command(&line) {
                Some(Err(msg)) => assert!(msg.contains("Usage: :clipboard"), "{line:?}: {msg}"),
                other => panic!("{line:?} must be rejected with usage, got {other:?}"),
            }
        }
    }
}
