//! Session Introspection & Regression Test Suite (`docs/LESSONS_LEARNED.md`).
//!
//! Covers every architectural, platform, typography, input, and UX lesson discovered
//! across live debugging and the 29-phase development of Bilterm:
//! 1. macOS `Option` Unicode dead-key normalization & held-`Ctrl` leader chords
//! 2. Powerline (`U+E0B0`, `U+E0A0`) & PUA glyph preservation and geometric rasterization
//! 3. Dual-plane (`FloatingCommandPrompt` <-> `OutputCanvas`) selection, clipboard & focus switching
//! 4. Toggleable version badge (`:version`) in 3270-OIA & `<F1>` Help without snapshot pollution
//! 5. Persistent, scrollable `herdr`-style Sessions/Terminals drawer (`F7` / `Ctrl+X d`)
//! 6. 3-signal `!ECHO LOCK` password detection vs. normal Readline/ZLE `!ECHO`
//! 7. Enforced `<= 800`-line module ceiling (`ARCH-R04`) and `<= 80`-line `handle_wm_key_event` (`ARCH-R01`)

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::pty::LineDisciplineState;
use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer, WorkspaceFrameRenderer};
use bilterm::ui::{CompositePaneWidget, WorkspaceWidget};
use bilterm::wm::{SplitDirection, WindowManager};

fn render_wm_text(wm: &WindowManager, area: Rect) -> String {
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(wm).render(area, &mut buf);
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn lesson_1_macos_option_dead_keys_and_held_ctrl_leader_chords() {
    let mut wm = WindowManager::new_headless(24, 80);
    assert_eq!(wm.pane_count(), 1);

    // On macOS, Option+V emits '√' with KeyModifiers::NONE. Verify it normalizes to Alt+V (split vertical).
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('√'), KeyModifiers::NONE),
    )
    .unwrap();
    assert_eq!(
        wm.pane_count(),
        2,
        "macOS Option+V ('√') should normalize to Alt+V and split the focused pane vertically"
    );

    // Option+C emits 'ç' on macOS -> should normalize to Alt+C and toggle CopyMode on the active pane
    assert!(!wm.active_pane().copy_mode().active);
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('ç'), KeyModifiers::NONE),
    )
    .unwrap();
    assert!(
        wm.active_pane().copy_mode().active,
        "macOS Option+C ('ç') should normalize to Alt+C and focus OutputCanvas (CopyMode)"
    );
    // Exit copy mode via Esc
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
    assert!(!wm.active_pane().copy_mode().active);

    // Held-Ctrl leader chord: Ctrl+X followed by Ctrl+D (user keeps Control held) should toggle the drawer
    assert!(!wm.drawer_is_open());
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
    )
    .unwrap();
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert!(
        wm.drawer_is_open(),
        "Ctrl+X followed by held Ctrl+D should normalize and open the herdr drawer"
    );
}

#[test]
fn lesson_2_powerline_and_nerd_font_pua_glyphs_preserve_codepoints_and_rasterize_cleanly() {
    let mut wm = WindowManager::new_headless(12, 80);
    let viewport = Rect::new(0, 0, 80, 12);
    wm.resize_all(viewport).unwrap();

    // Feed the exact Powerline prompt reported in this session: " bill@bigmac  ~ "
    // plus branch icon (U+E0A0 '') and right-pointing/left-pointing Powerline dividers
    let sample = "\r\n\r\n\r\n bill@bigmac \u{E0B0} ~ \u{E0B0} \u{E0A0}main \u{E0B2}\r\n";
    wm.active_pane_mut().process_pty_output(sample.as_bytes());

    let line3 = wm.active_pane().vt().viewport_line_text(3);
    assert!(
        line3.contains('\u{E0B0}') && line3.contains('\u{E0A0}') && line3.contains('\u{E0B2}'),
        "VT viewport must preserve full Unicode PUA scalars without truncating to 0xB0 ('°') or '?': {line3:?}"
    );
    assert!(
        !line3.contains('?'),
        "Powerline prompt must never degrade into '?' replacement chars: {line3:?}"
    );

    // Verify bitmap_font geometric rasterizer produces crisp non-empty pixels for every Powerline codepoint
    for &glyph in &[
        '\u{E0A0}', '\u{E0A2}', '\u{E0B0}', '\u{E0B1}', '\u{E0B2}', '\u{E0B3}', '\u{E0B4}',
        '\u{E0B6}',
    ] {
        let mut fb = vec![0u8; 10 * 20 * 4];
        bilterm::ui::bitmap_font::rasterize_glyph_cell(
            &mut fb,
            10,
            20,
            0,
            0,
            10,
            20,
            glyph,
            [51, 255, 51, 255],
            false,
        );
        let lit_pixels = fb.as_chunks::<4>().0.iter().filter(|px| px[3] > 0).count();
        assert!(
            lit_pixels >= 12,
            "expected Powerline glyph U+{:04X} to rasterize >= 12 geometric foreground pixels, got {lit_pixels}",
            glyph as u32
        );
    }

    // Verify full GuiWorkspaceRenderer scene includes the Powerline glyph runs and rasterizes cleanly
    let mut gui = GuiWorkspaceRenderer::new(8, 16);
    let mut surface = GuiFrameSurface::new(8, 16);
    gui.render_workspace(&wm, viewport, &mut surface);
    assert!(
        surface.glyphs.iter().any(|g| g.text.contains('\u{E0B0}')),
        "GuiFrameSurface should emit positioned glyph runs for Powerline U+E0B0"
    );
    let (w, h, rgba) = surface.rasterize_rgba();
    assert!(w > 0 && h > 0 && !rgba.is_empty());
}

#[test]
fn lesson_3_dual_plane_prompt_and_canvas_selection_cut_copy_paste_and_focus_switching() {
    let mut wm = WindowManager::new_headless(14, 80);
    let viewport = Rect::new(0, 0, 80, 14);
    wm.resize_all(viewport).unwrap();

    // Part A: Type into FloatingCommandPrompt, select with Shift+Left, Cut/Paste, and word-jump with Alt+Left
    for ch in "echo hello_world".chars() {
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }
    assert_eq!(wm.active_pane().prompt().text(), "echo hello_world");

    // Select "world" (5 chars) with Shift+Left
    for _ in 0..5 {
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT),
        )
        .unwrap();
    }
    assert_eq!(
        wm.active_pane().prompt().selected_text().as_deref(),
        Some("world")
    );

    // Cut selection via Shift+Delete
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Delete, KeyModifiers::SHIFT),
    )
    .unwrap();
    assert_eq!(wm.active_pane().prompt().text(), "echo hello_");

    // Part B: Populate OutputCanvas, switch focus from FloatingCommandPrompt to OutputCanvas via F8,
    // select text on the canvas with Shift+Right, and yank directly into the prompt via 'x'.
    wm.active_pane_mut()
        .process_pty_output(b"\r\n\r\n\r\nCANVAS_TOKEN_42\r\n");

    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE)).unwrap();
    assert!(
        wm.active_pane().copy_mode().active,
        "F8 should switch focus from FloatingCommandPrompt to OutputCanvas (CopyMode)"
    );

    // Move cursor to row 3 col 0 and select "CANVAS_TOKEN_42" (15 chars)
    {
        let cm = wm.active_pane_mut().copy_mode_mut();
        cm.cursor_row = 3;
        cm.cursor_col = 0;
        cm.anchor = Some((3, 0));
        cm.selection_kind = bilterm::pane::CopySelectionKind::Char;
        cm.cursor_col = 14;
    }
    // Press 'x' in CopyMode to copy selection and insert directly into the command prompt
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
    )
    .unwrap();
    assert!(
        !wm.active_pane().copy_mode().active,
        "'x' should yank canvas selection into prompt and return focus to FloatingCommandPrompt"
    );
    assert!(
        wm.active_pane()
            .prompt()
            .text()
            .contains("CANVAS_TOKEN_42"),
        "expected yanked canvas token inside command prompt, got {:?}",
        wm.active_pane().prompt().text()
    );
}

#[test]
fn lesson_4_toggleable_version_badge_in_oia_and_help_modal_without_snapshot_pollution() {
    let mut wm = WindowManager::new_headless(20, 80);
    let viewport = Rect::new(0, 0, 80, 20);
    wm.resize_all(viewport).unwrap();

    // Headless panes default to show_version_badge == false so golden snapshots stay invariant
    assert!(!wm.active_pane().show_version_badge());

    // Toggle on via :version
    let cmd = parse_colon_command(":version on").unwrap().unwrap();
    execute_wm_colon_command(&mut wm, cmd);
    assert!(wm.active_pane().show_version_badge());

    let mut pane_buf = Buffer::empty(viewport);
    CompositePaneWidget::with_config(wm.active_pane(), true, wm.config())
        .render(viewport, &mut pane_buf);
    let status_row = viewport.height - 1;
    let oia_line: String = (0..viewport.width)
        .map(|x| pane_buf[(x, status_row)].symbol())
        .collect();
    let expected_ver = concat!("v", env!("CARGO_PKG_VERSION"));
    assert!(
        oia_line.contains(expected_ver),
        "expected {expected_ver} in 3270-OIA status bar when :version on is active:\n{oia_line}"
    );

    // Open <F1> Help modal and verify version number in the title frame
    wm.toggle_help();
    let help_text = render_wm_text(&wm, viewport);
    assert!(
        help_text.contains(expected_ver),
        "expected {expected_ver} in <F1> Help modal header:\n{help_text}"
    );
}

#[test]
fn lesson_5_persistent_scrollable_herdr_drawer_stays_open_on_enter_and_click() {
    let mut wm = WindowManager::new_headless(14, 90);
    let viewport = Rect::new(0, 0, 90, 14);
    for _ in 0..7 {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.resize_all(viewport).unwrap();

    // Open the herdr drawer via F7
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE)).unwrap();
    assert!(wm.drawer_is_open());

    // Navigate down multiple rows and press Enter -> drawer MUST remain open in-place!
    for _ in 0..4 {
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)).unwrap();
    }
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert!(
        wm.drawer_is_open(),
        "pressing Enter to focus a session/terminal in the drawer must NOT auto-close the drawer"
    );

    // Mouse-wheel scroll inside the drawer
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 4,
            row: 5,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    assert!(wm.drawer_is_open());

    // Click a pane row inside the drawer -> drawer MUST still remain open
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 6,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    assert!(
        wm.drawer_is_open(),
        "clicking a pane row in the herdr drawer must switch focus without auto-closing the drawer"
    );

    // Pressing F7 again explicitly closes the drawer
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE)).unwrap();
    assert!(!wm.drawer_is_open());
}

#[test]
fn lesson_6_three_signal_echo_lock_protects_sudo_without_locking_normal_shell_readline() {
    let mut wm = WindowManager::new_headless(12, 80);
    let pane = wm.active_pane_mut();

    // Case 1: Normal zsh/bash/fish Readline prompt disables termios ECHO while waiting for input,
    // with no foreground subcommand and a normal shell prompt line. Must NOT lock!
    pane.process_pty_output(b"bill@bigmac ~ % ");
    *pane.line_discipline_mut() = LineDisciplineState {
        canonical_mode: false,
        echo_enabled: false,
        ..Default::default()
    };
    assert!(
        !pane.is_password_prompt(),
        "normal shell Readline/ZLE (!ECHO without subcommand or password prompt) must not trigger !ECHO LOCK"
    );

    // Case 2: sudo password prompt on the VT canvas with !ECHO -> MUST immediately trigger !ECHO LOCK
    pane.process_pty_output(b"\r\n[sudo] password for bill: ");
    *pane.line_discipline_mut() = LineDisciplineState {
        canonical_mode: true,
        echo_enabled: false,
        ..Default::default()
    };
    assert!(
        pane.is_password_prompt(),
        "sudo password prompt with !ECHO must engage 3-signal !ECHO LOCK"
    );
}

fn collect_rs_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_rs_files(&p, out);
            } else if p.extension().and_then(|s| s.to_str()) == Some("rs") {
                out.push(p);
            }
        }
    }
}

#[test]
fn lesson_7_architectural_complexity_ceilings_and_file_line_budgets() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let mut files = Vec::new();
    collect_rs_files(&src_dir, &mut files);
    assert!(!files.is_empty());

    for file in files {
        let content = std::fs::read_to_string(&file).expect("read source file");
        let line_count = content.lines().count();
        assert!(
            line_count <= 800,
            "ARCH-R04 violation: {} has {} lines (must be <= 800 lines)",
            file.display(),
            line_count
        );
    }

    // Verify handle_wm_key_event in src/input/mod.rs remains <= 80 lines (ARCH-R01)
    let input_mod = std::fs::read_to_string(src_dir.join("input/mod.rs")).unwrap();
    let lines: Vec<&str> = input_mod.lines().collect();
    let start_idx = lines
        .iter()
        .position(|l| l.contains("pub fn handle_wm_key_event("))
        .expect("locate handle_wm_key_event");
    let mut brace_depth = 0isize;
    let mut opened = false;
    let mut end_idx = start_idx;
    for (idx, line) in lines.iter().enumerate().skip(start_idx) {
        for ch in line.chars() {
            if ch == '{' {
                brace_depth += 1;
                opened = true;
            } else if ch == '}' {
                brace_depth -= 1;
            }
        }
        if opened && brace_depth == 0 {
            end_idx = idx;
            break;
        }
    }
    let fn_lines = end_idx - start_idx + 1;
    assert!(
        fn_lines <= 80,
        "ARCH-R01 violation: handle_wm_key_event is {fn_lines} lines (must be <= 80 lines)"
    );
}
