//! TUI rasterization, prompt occlusion fix, and interactive URL bar editing for the browser canvas.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;

use bilterm::input::handle_wm_key_event;
use bilterm::input::mouse::handle_wm_mouse_event;
use bilterm::pane::PromptAnchor;
use bilterm::ui::CompositePaneWidget;
use bilterm::wm::WindowManager;

fn synth() {
    // SAFETY: process-local test env var
    unsafe { std::env::set_var("BILTERM_BROWSER_SYNTHETIC", "1") };
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn row_string(buf: &Buffer, r: Rect, y: u16) -> String {
    (r.x..r.right()).map(|c| buf[(c, y)].symbol().to_string()).collect()
}

fn setup() -> (WindowManager, usize, Rect) {
    synth();
    let mut wm = WindowManager::new_headless(24, 80);
    let id = wm.active_pane_id();
    wm.pane_mut(id).unwrap().open_browser("https://example.com");
    let screen = Rect::new(0, 0, 80, 24);
    (wm, id, screen)
}

#[test]
fn tui_renders_nav_bar_and_halfblock_frame() {
    let (wm, id, screen) = setup();
    let pane = wm.pane(id).unwrap();
    assert!(matches!(pane.prompt().anchor, PromptAnchor::Top));
    let canvas = pane.canvas_rect(screen);
    let mut buf = Buffer::empty(screen);
    CompositePaneWidget::new(pane, true).render(screen, &mut buf);

    let nav = row_string(&buf, canvas, canvas.y);
    assert!(nav.contains("[◄]"), "nav bar must not be occluded by Top prompt: {nav:?}");
    assert!(nav.contains("example.com"));

    let mut halfblocks = 0;
    for y in canvas.y + 1..canvas.bottom() {
        for x in canvas.x..canvas.right() {
            let c = &buf[(x, y)];
            if c.symbol() == "▀" && matches!(c.fg, Color::Rgb(..)) && matches!(c.bg, Color::Rgb(..)) {
                halfblocks += 1;
            }
        }
    }
    assert!(halfblocks > 100, "expected rasterized half-block cells, got {halfblocks}");
}

#[test]
fn prompt_rect_sits_below_browser_nav_bar() {
    let (mut wm, id, screen) = setup();
    let pane = wm.pane(id).unwrap();
    let canvas = pane.canvas_rect(screen);
    assert_eq!(pane.prompt_rect(screen).y, canvas.y + 1);
    wm.pane_mut(id).unwrap().close_browser();
    let pane = wm.pane(id).unwrap();
    assert_eq!(pane.prompt_rect(screen).y, canvas.y, "no offset without browser");
}

#[test]
fn url_bar_click_enters_edit_and_enter_navigates_with_scheme() {
    let (mut wm, id, screen) = setup();
    let canvas = wm.pane(id).unwrap().canvas_rect(screen);
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: canvas.x + 20,
        row: canvas.y,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, click, screen).unwrap();
    assert!(wm.pane(id).unwrap().browser_canvas().unwrap().is_editing_url());

    // highlighted while editing
    let mut buf = Buffer::empty(screen);
    CompositePaneWidget::new(wm.pane(id).unwrap(), true).render(screen, &mut buf);
    assert!(row_string(&buf, canvas, canvas.y).contains("example.com"));

    // Ctrl+U clears, type new host
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL)).unwrap();
    for ch in "rust-lang.org".chars() {
        handle_wm_key_event(&mut wm, key(KeyCode::Char(ch))).unwrap();
    }
    handle_wm_key_event(&mut wm, key(KeyCode::Backspace)).unwrap();
    handle_wm_key_event(&mut wm, key(KeyCode::Char('g'))).unwrap();
    handle_wm_key_event(&mut wm, key(KeyCode::Home)).unwrap();
    handle_wm_key_event(&mut wm, key(KeyCode::End)).unwrap();
    handle_wm_key_event(&mut wm, key(KeyCode::Left)).unwrap();
    handle_wm_key_event(&mut wm, key(KeyCode::Right)).unwrap();
    let mut buf = Buffer::empty(screen);
    CompositePaneWidget::new(wm.pane(id).unwrap(), true).render(screen, &mut buf);
    assert!(row_string(&buf, canvas, canvas.y).contains("rust-lang.org"));

    handle_wm_key_event(&mut wm, key(KeyCode::Enter)).unwrap();
    let b = wm.pane(id).unwrap().browser_canvas().unwrap();
    assert!(!b.is_editing_url());
    assert_eq!(b.current_url(), "https://rust-lang.org");
}

#[test]
fn ctrl_l_edits_and_esc_cancels() {
    let (mut wm, id, _screen) = setup();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL)).unwrap();
    assert!(wm.pane(id).unwrap().browser_canvas().unwrap().is_editing_url());
    handle_wm_key_event(&mut wm, key(KeyCode::Char('x'))).unwrap();
    handle_wm_key_event(&mut wm, key(KeyCode::Esc)).unwrap();
    let b = wm.pane(id).unwrap().browser_canvas().unwrap();
    assert!(!b.is_editing_url());
    assert_eq!(b.current_url(), "https://example.com");
}
