//! Plan 51.2-03: SCRIPT STUDIO window test suite (WIN-03).
//!
//! Covers:
//! - `:script open`, `:script toggle`, `:studio open`, `:studio toggle`
//! - Script execution (`RunAll`) and status reflection
//! - Single-step execution (`StepNext`) with comment skipping
//! - Live action recording while `[● Rec]` is active
//! - Save, load, and list in isolated temp directory
//! - In-line text editing (`InsertChar`, `Backspace`, `NewLine`, `DeleteLine`)
//! - Window geometry (`Move`, `Resize`, `SetRect`, `Dock`, `Pin`)
//! - TUI rendering with `TestBackend`
//! - Keyboard & mouse routing and parity

use std::time::Instant;

use bilterm::action::Action;
use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::input::script_studio::handle_script_studio_mouse;
use bilterm::input::script_studio_keys::handle_script_studio_key;
use bilterm::ui::WorkspaceWidget;
use bilterm::wm::script_studio::{ScriptStudioOp, scripts_dir};
use bilterm::wm::WindowManager;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

const COLS: u16 = 120;
const ROWS: u16 = 40;

fn area() -> Rect {
    Rect::new(0, 0, COLS, ROWS)
}

fn fixture() -> WindowManager {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.resize_all(area()).unwrap();
    wm.update_agent_states(Instant::now());
    wm
}

fn render_to_buffer(wm: &WindowManager) -> Buffer {
    let mut buf = Buffer::empty(area());
    WorkspaceWidget::new(wm).render(area(), &mut buf);
    buf
}

fn buffer_text(buf: &Buffer) -> String {
    let a = buf.area;
    (a.y..a.y + a.height)
        .map(|y| {
            (a.x..a.x + a.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn exec(wm: &mut WindowManager, colon_cmd: &str) {
    let cmd = parse_colon_command(colon_cmd)
        .expect("must parse")
        .expect("must be Ok");
    let _ = execute_wm_colon_command(wm, cmd);
}

// ---------------------------------------------------------------------------
// 1. Script Open, Toggle and Studio Alias
// ---------------------------------------------------------------------------
#[test]
fn test_script_open_toggle_and_studio_alias() {
    let mut wm = fixture();
    assert!(!wm.script_studio().active);

    exec(&mut wm, ":script open");
    assert!(wm.script_studio().active);
    assert!(wm.script_studio().focused);

    exec(&mut wm, ":script toggle");
    assert!(!wm.script_studio().active);

    // Studio alias
    exec(&mut wm, ":studio open");
    assert!(wm.script_studio().active);
    assert!(wm.script_studio().focused);

    exec(&mut wm, ":studio toggle");
    assert!(!wm.script_studio().active);
}

// ---------------------------------------------------------------------------
// 2. Add Commands and Run All
// ---------------------------------------------------------------------------
#[test]
fn test_script_add_and_run_all() {
    let mut wm = fixture();
    exec(&mut wm, ":script open");
    exec(&mut wm, ":script clear");
    exec(&mut wm, ":script add :font vt323");
    exec(&mut wm, ":script add :font size 16");

    assert_eq!(wm.script_studio().lines.len(), 2);

    exec(&mut wm, ":script run");

    // Focused pane font options
    let opts = wm.active_pane().font_options();
    assert_eq!(opts.family.display_name(), "VT323");
    assert_eq!(opts.effective_size_pt(wm.config()), 16);

    // Run log verification
    let logs = &wm.script_studio().run_log;
    assert_eq!(logs.len(), 2);
    assert!(logs[0].ok);
    assert!(logs[1].ok);
    assert_eq!(logs[0].source, ":font vt323");
    assert_eq!(logs[1].source, ":font size 16");
}

// ---------------------------------------------------------------------------
// 3. Step Next with Comment Skipping
// ---------------------------------------------------------------------------
#[test]
fn test_script_step_next_skips_comments() {
    let mut wm = fixture();
    exec(&mut wm, ":script open");
    exec(&mut wm, ":script clear");
    exec(&mut wm, ":script add # Section header");
    exec(&mut wm, ":script add :font vt323");
    exec(&mut wm, ":script add # Interstitial comment");
    exec(&mut wm, ":script add :font size 18");

    assert_eq!(wm.script_studio().step_pc, 0);

    // First step: should skip line 0 (# Section header) and execute line 1 (:font vt323)
    exec(&mut wm, ":script step");
    assert_eq!(wm.script_studio().run_log.len(), 1);
    assert_eq!(wm.script_studio().run_log[0].source, ":font vt323");
    assert_eq!(wm.active_pane().font_options().family.display_name(), "VT323");
    assert_eq!(wm.script_studio().cursor_line, 1);
    assert_eq!(wm.script_studio().step_pc, 3); // advances past line 2 (#) to line 3

    // Second step: should execute line 3 (:font size 18)
    exec(&mut wm, ":script step");
    assert_eq!(wm.script_studio().run_log.len(), 2);
    assert_eq!(wm.script_studio().run_log[1].source, ":font size 18");
    assert_eq!(wm.active_pane().font_options().effective_size_pt(wm.config()), 18);
    assert_eq!(wm.script_studio().cursor_line, 3);
}

// ---------------------------------------------------------------------------
// 4. Live Recording
// ---------------------------------------------------------------------------
#[test]
fn test_script_live_recording() {
    let mut wm = fixture();
    exec(&mut wm, ":script open");
    exec(&mut wm, ":script clear");
    assert_eq!(wm.script_studio().lines.len(), 0); // empty after clear

    exec(&mut wm, ":script record on");
    assert!(wm.script_studio().recording);

    // Executing :font lazenby on WindowManager should be recorded
    exec(&mut wm, ":font lazenby");
    assert!(wm.script_studio().lines.iter().any(|l| l.contains("lazenby")));

    // Commands to :script itself should not be recorded
    exec(&mut wm, ":script status");
    assert!(!wm.script_studio().lines.iter().any(|l| l.starts_with(":script")));

    // Turning off recording
    exec(&mut wm, ":script record off");
    assert!(!wm.script_studio().recording);

    let count_before = wm.script_studio().lines.len();
    exec(&mut wm, ":font vt323");
    assert_eq!(wm.script_studio().lines.len(), count_before);
}

// ---------------------------------------------------------------------------
// 5. Save and Load Scripts
// ---------------------------------------------------------------------------
#[test]
fn test_script_save_and_load() {
    let temp_dir = std::env::temp_dir().join("bt_test_scripts_512");
    let _ = std::fs::remove_dir_all(&temp_dir);
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &temp_dir);
    }

    let mut wm = fixture();
    exec(&mut wm, ":script open");
    exec(&mut wm, ":script clear");
    exec(&mut wm, ":script add :font vt323");
    exec(&mut wm, ":script add :font size 20");

    // Save script
    exec(&mut wm, ":script save test_demo");
    assert_eq!(wm.script_studio().script_name, "test_demo.bt");

    let saved_file = scripts_dir().join("test_demo.bt");
    assert!(saved_file.exists());

    // Clear and reload
    exec(&mut wm, ":script clear");
    assert_eq!(wm.script_studio().lines.len(), 0);

    exec(&mut wm, ":script load test_demo");
    assert_eq!(wm.script_studio().lines.len(), 2);
    assert_eq!(wm.script_studio().lines[0], ":font vt323");
    assert_eq!(wm.script_studio().lines[1], ":font size 20");
    assert_eq!(wm.script_studio().script_name, "test_demo.bt");

    // List saved
    let list_msg = wm.script_studio_apply(&ScriptStudioOp::ListSaved).unwrap();
    assert!(list_msg.contains("test_demo.bt"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ---------------------------------------------------------------------------
// 6. Editing Operations
// ---------------------------------------------------------------------------
#[test]
fn test_script_editing_operations() {
    let mut wm = fixture();
    exec(&mut wm, ":script open");
    exec(&mut wm, ":script clear");

    // InsertChar
    wm.script_studio_apply(&ScriptStudioOp::InsertChar('a'));
    wm.script_studio_apply(&ScriptStudioOp::InsertChar('b'));
    wm.script_studio_apply(&ScriptStudioOp::InsertChar('c'));
    assert_eq!(wm.script_studio().lines[0], "abc");
    assert_eq!(wm.script_studio().cursor_col, 3);

    // Backspace
    wm.script_studio_apply(&ScriptStudioOp::Backspace);
    assert_eq!(wm.script_studio().lines[0], "ab");
    assert_eq!(wm.script_studio().cursor_col, 2);

    // NewLine
    wm.script_studio_apply(&ScriptStudioOp::NewLine);
    assert_eq!(wm.script_studio().lines.len(), 2);
    assert_eq!(wm.script_studio().cursor_line, 1);
    assert_eq!(wm.script_studio().cursor_col, 0);

    // Insert on second line
    wm.script_studio_apply(&ScriptStudioOp::InsertChar('z'));
    assert_eq!(wm.script_studio().lines[1], "z");

    // DeleteLine
    wm.script_studio_apply(&ScriptStudioOp::DeleteLine);
    assert_eq!(wm.script_studio().lines.len(), 1);
    assert_eq!(wm.script_studio().lines[0], "ab");
}

// ---------------------------------------------------------------------------
// 7. Move and Resize Geometry
// ---------------------------------------------------------------------------
#[test]
fn test_script_move_resize_set_rect() {
    let mut wm = fixture();
    exec(&mut wm, ":script open");

    // Move
    wm.script_studio_apply(&ScriptStudioOp::Move { dx: 4, dy: 2 });
    let pos = wm.script_studio().float_pos.unwrap();
    assert_eq!(pos, (14, 7));

    // Resize
    wm.script_studio_apply(&ScriptStudioOp::Resize { dw: 10, dh: 5 });
    let sz = wm.script_studio().float_size.unwrap();
    assert_eq!(sz, (76, 25));

    // SetRect
    wm.script_studio_apply(&ScriptStudioOp::SetRect { x: 5, y: 5, w: 60, h: 22 });
    let rect = wm.script_studio_rect(area()).unwrap();
    assert_eq!(rect.x, 5);
    assert_eq!(rect.y, 5);
    assert_eq!(rect.width, 60);
    assert_eq!(rect.height, 22);
}

// ---------------------------------------------------------------------------
// 8. Rendering with TestBackend
// ---------------------------------------------------------------------------
#[test]
fn test_script_rendering_test_backend() {
    let mut wm = fixture();
    exec(&mut wm, ":script open");
    exec(&mut wm, ":script add :font status");
    exec(&mut wm, ":script run");

    let buf = render_to_buffer(&wm);
    let text = buffer_text(&buf);

    assert!(text.contains("SCRIPT STUDIO"), "must render window title");
    assert!(text.contains("RUN LOG"), "must render run log separator");
    assert!(text.contains("Run(F5)"), "must render toolbar");
}

// ---------------------------------------------------------------------------
// 9. Pin and Dock Toggles
// ---------------------------------------------------------------------------
#[test]
fn test_script_pin_and_dock() {
    let mut wm = fixture();
    exec(&mut wm, ":script open");

    // Default opened state is pinned
    assert!(wm.script_studio().pinned);

    // When pinned, Dismiss only blurs
    wm.script_studio_apply(&ScriptStudioOp::Dismiss);
    assert!(wm.script_studio().active);
    assert!(!wm.script_studio().focused);

    // Unpin
    exec(&mut wm, ":script pin off");
    assert!(!wm.script_studio().pinned);

    // When unpinned, Dismiss closes
    wm.script_studio_apply(&ScriptStudioOp::Dismiss);
    assert!(!wm.script_studio().active);

    // Docking
    exec(&mut wm, ":script open");
    exec(&mut wm, ":script dock on");
    assert!(wm.script_studio().docked);
    let r = wm.script_studio_rect(area()).unwrap();
    assert_eq!(r.x + r.width, area().width); // docked right
}

// ---------------------------------------------------------------------------
// 10. Selection and Navigation
// ---------------------------------------------------------------------------
#[test]
fn test_script_selection_and_navigation() {
    let mut wm = fixture();
    exec(&mut wm, ":script open");
    exec(&mut wm, ":script clear");
    exec(&mut wm, ":script add line0");
    exec(&mut wm, ":script add line1");
    exec(&mut wm, ":script add line2");

    wm.script_studio_apply(&ScriptStudioOp::SelectLine(1));
    assert_eq!(wm.script_studio().cursor_line, 1);

    wm.script_studio_apply(&ScriptStudioOp::CursorDown);
    assert_eq!(wm.script_studio().cursor_line, 2);

    wm.script_studio_apply(&ScriptStudioOp::CursorUp);
    assert_eq!(wm.script_studio().cursor_line, 1);

    wm.script_studio_apply(&ScriptStudioOp::CursorRight);
    assert_eq!(wm.script_studio().cursor_col, 1);

    wm.script_studio_apply(&ScriptStudioOp::CursorLeft);
    assert_eq!(wm.script_studio().cursor_col, 0);
}

// ---------------------------------------------------------------------------
// 11. Reset Step and Clear Log
// ---------------------------------------------------------------------------
#[test]
fn test_script_reset_step_and_clear_log() {
    let mut wm = fixture();
    exec(&mut wm, ":script open");
    exec(&mut wm, ":script add :font vt323");
    exec(&mut wm, ":script run");
    assert!(!wm.script_studio().run_log.is_empty());

    exec(&mut wm, ":script reset");
    assert_eq!(wm.script_studio().step_pc, 0);
    assert!(wm.script_studio().run_log.is_empty());
}

// ---------------------------------------------------------------------------
// 12. Keyboard Handling in Edit Mode vs Nav Mode
// ---------------------------------------------------------------------------
#[test]
fn test_script_keyboard_handling() {
    let mut wm = fixture();
    exec(&mut wm, ":script open");
    exec(&mut wm, ":script clear");

    // Nav mode: press 'i' to enter edit mode
    let key_i = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
    handle_script_studio_key(&mut wm, key_i);
    assert!(wm.script_studio().edit_mode);

    // Edit mode: press 'H', 'i' to insert chars
    let key_h = KeyEvent::new(KeyCode::Char('H'), KeyModifiers::SHIFT);
    let key_i2 = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
    handle_script_studio_key(&mut wm, key_h);
    handle_script_studio_key(&mut wm, key_i2);
    assert_eq!(wm.script_studio().lines[0], "Hi");

    // Press Esc to exit edit mode
    let key_esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    handle_script_studio_key(&mut wm, key_esc);
    assert!(!wm.script_studio().edit_mode);

    // Nav mode: press 'r' to reset
    let key_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
    handle_script_studio_key(&mut wm, key_r);
    assert_eq!(wm.script_studio().step_pc, 0);
}

// ---------------------------------------------------------------------------
// 13. Mouse Handling
// ---------------------------------------------------------------------------
#[test]
fn test_script_mouse_handling() {
    let mut wm = fixture();
    exec(&mut wm, ":script open");
    let rect = wm.script_studio_rect(area()).unwrap();

    // Click outside blurs the window
    let mouse_out = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    };
    handle_script_studio_mouse(&mut wm, mouse_out, area());
    assert!(!wm.script_studio().focused);

    // Click inside focuses the window
    let mouse_in = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x + 5,
        row: rect.y + 5,
        modifiers: KeyModifiers::NONE,
    };
    handle_script_studio_mouse(&mut wm, mouse_in, area());
    assert!(wm.script_studio().focused);
}

// ---------------------------------------------------------------------------
// 14. Non-Colon Shell Input
// ---------------------------------------------------------------------------
#[test]
fn test_script_shell_command_input() {
    let mut wm = fixture();
    exec(&mut wm, ":script open");
    exec(&mut wm, ":script clear");
    exec(&mut wm, ":script add !echo hello");

    exec(&mut wm, ":script run");
    let logs = &wm.script_studio().run_log;
    assert_eq!(logs.len(), 1);
    assert!(logs[0].ok);
    assert_eq!(logs[0].output, "sent to focused pane");
    assert!(logs[0].source.contains("echo hello"));
}

// ---------------------------------------------------------------------------
// 15. Action Roundtrip and Colon Form
// ---------------------------------------------------------------------------
#[test]
fn test_action_roundtrip_and_colon_forms() {
    let ops = vec![
        ScriptStudioOp::Open,
        ScriptStudioOp::Close,
        ScriptStudioOp::Toggle,
        ScriptStudioOp::Dismiss,
        ScriptStudioOp::Blur,
        ScriptStudioOp::RunAll,
        ScriptStudioOp::StepNext,
        ScriptStudioOp::ResetStep,
        ScriptStudioOp::Record(Some(true)),
        ScriptStudioOp::Record(Some(false)),
        ScriptStudioOp::Record(None),
        ScriptStudioOp::EditMode(Some(true)),
        ScriptStudioOp::EditMode(Some(false)),
        ScriptStudioOp::EditMode(None),
        ScriptStudioOp::ClearScript,
        ScriptStudioOp::ClearLog,
        ScriptStudioOp::Save(Some("foo".to_string())),
        ScriptStudioOp::Save(None),
        ScriptStudioOp::Load("bar".to_string()),
        ScriptStudioOp::ListSaved,
        ScriptStudioOp::Pin(Some(true)),
        ScriptStudioOp::Dock(Some(true)),
    ];

    for op in ops {
        let colon = op.colon_form();
        let parsed = parse_colon_command(&colon)
            .unwrap_or_else(|| panic!("must parse '{colon}'"))
            .unwrap_or_else(|e| panic!("must be Ok for '{colon}': {e}"));
        assert_eq!(parsed, Action::ScriptStudio(op), "mismatch for {colon}");
    }
}
