//! Live end-to-end test of the browser canvas against a real headless Chrome (`WEB-01..02`).
//!
//! Opt-in: runs only with `BILTERM_BROWSER_CDP=1` (and a Chromium-family browser installed, or
//! `BILTERM_CHROME_PATH`). It needs no external network: the pages come from a `TcpListener` on
//! 127.0.0.1.
//!
//! `BILTERM_BROWSER_CDP=1 cargo test --test browser_cdp_live -- --nocapture`
#![cfg(unix)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use bilterm::action::{Action, BrowserOp};
use bilterm::colon_command::parse_colon_command;
use bilterm::input::mouse::handle_wm_mouse_event;
use bilterm::input::{handle_wm_key_event, handle_wm_paste_event};
use bilterm::pane::browser_canvas::BrowserCanvasState;
use bilterm::wm::WindowManager;

const PAGE_ONE: &str = "<!doctype html><title>Page One</title><body style='margin:0;background:#fff'>\
<a href='/two' style='display:block;position:fixed;left:0;top:0;width:100%;height:50%;background:#0066cc'>go</a></body>";
const PAGE_TWO: &str = "<!doctype html><title>Page Two</title><body style='margin:0;background:#00aa00'>\
<input autofocus style='position:fixed;left:0;top:0;width:100%;height:100%;font-size:40px' \
oninput=\"document.title='typed:'+this.value\">\
<script>window.onresize=()=>{document.title='w:'+innerWidth}</script></body>";

/// Serves the two pages on 127.0.0.1 and returns the port.
fn serve() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut req = Vec::new();
                let mut buf = [0u8; 1024];
                while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => req.extend_from_slice(&buf[..n]),
                    }
                }
                let line = String::from_utf8_lossy(&req);
                let path = line.split_whitespace().nth(1).unwrap_or("/");
                let (status, body) = match path {
                    "/" => ("200 OK", PAGE_ONE),
                    "/two" => ("200 OK", PAGE_TWO),
                    _ => ("404 Not Found", ""),
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    port
}

fn browser(wm: &WindowManager) -> &BrowserCanvasState {
    wm.active_pane().browser_canvas().expect("browser canvas open")
}

/// Drives the window-manager tick until `cond` holds. The tick is the only thing pulling state.
fn wait_for(wm: &mut WindowManager, what: &str, cond: impl Fn(&WindowManager) -> bool) {
    let start = Instant::now();
    loop {
        wm.poll_all_ptys();
        if cond(wm) {
            eprintln!("  ok  {what} ({} ms)", start.elapsed().as_millis());
            return;
        }
        if start.elapsed() > Duration::from_secs(20) {
            let b = wm.active_pane().browser_canvas();
            panic!(
                "timed out waiting for {what}: backend={:?} url={:?} title={:?} loading={:?} notice={:?}",
                b.map(|b| b.backend_name()),
                b.map(|b| b.current_url().to_string()),
                b.map(|b| b.page_title().to_string()),
                b.map(|b| b.is_loading()),
                b.and_then(|b| b.notice().map(str::to_string)),
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn colon(wm: &mut WindowManager, cmd: &str) {
    let action = parse_colon_command(cmd).expect("colon command").expect("valid syntax");
    wm.apply(action).unwrap();
}

/// RGB of the frame pixel at the given fraction of the frame.
fn pixel(wm: &WindowManager, fx: f32, fy: f32) -> Option<(u8, u8, u8)> {
    let f = browser(wm).cached_frame()?;
    let x = ((f32::from(f.width_px) * fx) as usize).min(usize::from(f.width_px) - 1);
    let y = ((f32::from(f.height_px) * fy) as usize).min(usize::from(f.height_px) - 1);
    let i = (y * usize::from(f.width_px) + x) * 4;
    Some((f.rgba[i], f.rgba[i + 1], f.rgba[i + 2]))
}

fn is_blue(p: Option<(u8, u8, u8)>) -> bool {
    p.is_some_and(|(r, g, b)| r < 60 && (70..140).contains(&g) && b > 160)
}

fn is_white(p: Option<(u8, u8, u8)>) -> bool {
    p.is_some_and(|(r, g, b)| r > 230 && g > 230 && b > 230)
}

fn group_alive(pgid: i32) -> bool {
    // SAFETY: signal 0 only probes whether any process is left in the group.
    unsafe { libc::kill(-pgid, 0) == 0 }
}

fn wait_gone(pgid: i32, dir: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while (group_alive(pgid) || dir.exists()) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!group_alive(pgid), "Chrome process group {pgid} is still alive");
    assert!(!dir.exists(), "profile dir {} was not removed", dir.display());
}

fn click_cell(wm: &mut WindowManager, screen: Rect, column: u16, row: u16) {
    for kind in [MouseEventKind::Down(MouseButton::Left), MouseEventKind::Up(MouseButton::Left)] {
        handle_wm_mouse_event(wm, MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE }, screen).unwrap();
    }
}

#[test]
fn live_chrome_end_to_end() {
    if std::env::var("BILTERM_BROWSER_CDP").as_deref() != Ok("1") {
        eprintln!("skipped: set BILTERM_BROWSER_CDP=1 to run against a real Chrome");
        return;
    }
    let port = serve();
    let root = format!("http://127.0.0.1:{port}/");
    let two = format!("http://127.0.0.1:{port}/two");
    let screen = Rect::new(0, 0, 80, 24);
    let mut wm = WindowManager::new_headless(24, 80);

    eprintln!("open {root}");
    colon(&mut wm, &format!(":browser open {root}"));
    assert!(browser(&wm).backend_name().starts_with("cdp"), "real engine expected, got {}", browser(&wm).backend_name());
    wait_for(&mut wm, "DevTools handshake (backend == cdp)", |wm| browser(wm).backend_name() == "cdp");
    wait_for(&mut wm, "non-placeholder frame (blue link above white body)", |wm| {
        is_blue(pixel(wm, 0.5, 0.25)) && is_white(pixel(wm, 0.5, 0.75))
    });
    wait_for(&mut wm, "title 'Page One'", |wm| browser(wm).page_title() == "Page One");
    wait_for(&mut wm, "load finished", |wm| !browser(wm).is_loading());
    assert_eq!(browser(&wm).current_url(), root);
    let (vw, vh) = browser(&wm).viewport_px();
    let frame = browser(&wm).cached_frame().unwrap();
    assert_eq!((frame.width_px, frame.height_px), (vw, vh), "frame matches the pane viewport");
    eprintln!("  frame {}x{} seq {}", frame.width_px, frame.height_px, frame.seq);

    // Lifecycle facts while it runs.
    let (pgid, profile) = browser(&wm).process_info().expect("real Chrome process");
    assert!(group_alive(pgid));
    let runtime = bilterm::secure_fs::private_runtime_dir().unwrap();
    assert!(profile.starts_with(&runtime), "profile {} is inside the private runtime dir", profile.display());
    let name = profile.file_name().unwrap().to_string_lossy().to_string();
    assert!(name.starts_with(&format!("profile-{}-", std::process::id())), "{name}");
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(&profile).unwrap().permissions().mode() & 0o777, 0o700);

    eprintln!("click the link through the mouse path");
    let canvas = wm.active_pane().canvas_rect(screen);
    let prompt = wm.active_pane().prompt_rect(screen);
    let body = Rect::new(canvas.x, canvas.y + 1, canvas.width, canvas.height - 1);
    // A cell in the upper half of the page (the link) that the floating prompt does not cover.
    let row = (body.y..body.y + body.height / 2).rev().find(|r| *r < prompt.y || *r >= prompt.bottom()).expect("free row");
    let col = body.x + body.width / 2;
    click_cell(&mut wm, screen, col, row);
    wait_for(&mut wm, "URL changed to /two", |wm| browser(wm).current_url() == two);
    wait_for(&mut wm, "title 'Page Two'", |wm| browser(wm).page_title() == "Page Two");
    wait_for(&mut wm, "history has a back entry", |wm| browser(wm).can_go_back() && !browser(wm).can_go_forward());
    assert!(browser(&wm).viewport_focused, "clicking the page focused it");

    eprintln!("type into the input through the key path");
    wait_for(&mut wm, "page two loaded and painted (input replaces the blue link)", |wm| {
        !browser(wm).is_loading() && is_white(pixel(wm, 0.5, 0.25))
    });
    click_cell(&mut wm, screen, col, row);
    for (code, mods) in [
        (KeyCode::Char('h'), KeyModifiers::NONE),
        (KeyCode::Char('i'), KeyModifiers::NONE),
        (KeyCode::Char('!'), KeyModifiers::SHIFT),
    ] {
        handle_wm_key_event(&mut wm, KeyEvent::new(code, mods)).unwrap();
    }
    wait_for(&mut wm, "typed text read back via title 'typed:hi!'", |wm| browser(wm).page_title() == "typed:hi!");
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)).unwrap();
    wait_for(&mut wm, "Backspace edits the input ('typed:hi')", |wm| browser(wm).page_title() == "typed:hi");
    handle_wm_paste_event(&mut wm, " there").unwrap();
    wait_for(&mut wm, "paste inserted ('typed:hi there')", |wm| browser(wm).page_title() == "typed:hi there");
    assert!(wm.active_pane().prompt().text().is_empty(), "nothing leaked into the prompt");

    eprintln!("escape hatch, back/forward, URL policy");
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char(']'), KeyModifiers::CONTROL)).unwrap();
    assert!(!browser(&wm).viewport_focused, "Ctrl+] returned the keyboard");
    colon(&mut wm, ":browser back");
    wait_for(&mut wm, "back to Page One", |wm| browser(wm).current_url() == root && browser(wm).page_title() == "Page One");
    wait_for(&mut wm, "forward entry available", |wm| browser(wm).can_go_forward() && !browser(wm).can_go_back());
    colon(&mut wm, ":browser forward");
    wait_for(&mut wm, "forward to /two", |wm| browser(wm).current_url() == two);
    wm.apply(Action::Browser(BrowserOp::Open { url: "javascript:document.title='pwned'".into(), pane_id: None })).unwrap();
    assert_eq!(browser(&wm).current_url(), two, "javascript: URL never reached Chrome");
    assert!(browser(&wm).notice().is_some_and(|n| n.contains("blocked URL")));

    eprintln!("resize and zoom");
    // Forward restores the page from the back/forward cache, typed text and title included.
    wait_for(&mut wm, "restored page keeps its title", |wm| browser(wm).page_title() == "typed:hi there");
    let id = wm.active_pane_id();
    wm.pane_mut(id).unwrap().resize_viewport(Rect::new(0, 0, 100, 30)).unwrap();
    let (rw, rh) = browser(&wm).viewport_px();
    assert_ne!((rw, rh), (vw, vh), "pane resize changed the page size");
    wait_for(&mut wm, "page saw the resize (title w:<css width>)", |wm| browser(wm).page_title() == format!("w:{rw}"));
    wait_for(&mut wm, "frame follows the resize", |wm| {
        browser(wm).cached_frame().is_some_and(|f| (f.width_px, f.height_px) == (rw, rh))
    });
    wm.apply(Action::Browser(BrowserOp::Zoom { delta_pct: 100, pane_id: None })).unwrap();
    assert_eq!(browser(&wm).zoom_pct, 200);
    wait_for(&mut wm, "zoom 200% halves the CSS width", |wm| browser(wm).page_title() == format!("w:{}", rw / 2));
    // Chrome captures the screencast at CSS-pixel resolution, so a zoomed-in page arrives as a
    // smaller frame that the pane stretches; it never exceeds the size that was asked for.
    wait_for(&mut wm, "zoomed frame arrives within the requested size", |wm| {
        browser(wm).cached_frame().is_some_and(|f| f.width_px == rw / 2 || (f.width_px, f.height_px) == (rw, rh))
    });
    let f = browser(&wm).cached_frame().unwrap();
    eprintln!("  frame after zoom {}x{}", f.width_px, f.height_px);

    eprintln!("crash detection and reload");
    // SAFETY: kills only the Chrome process group this test started.
    unsafe { libc::kill(-pgid, libc::SIGKILL) };
    wait_for(&mut wm, "crash detected", |wm| browser(wm).is_crashed());
    assert_eq!(browser(&wm).backend_name(), "cdp (crashed)");
    assert!(browser(&wm).overlay_message().is_some_and(|m| m.ends_with("press r to reload")));
    assert!(browser(&wm).cached_frame().is_none(), "no frozen frame after the crash");
    colon(&mut wm, ":browser reload");
    wait_for(&mut wm, "relaunched after reload", |wm| browser(wm).backend_name() == "cdp" && browser(wm).cached_frame().is_some());
    wait_for(&mut wm, "page back after relaunch", |wm| browser(wm).page_title() == "Page Two");
    assert_eq!(browser(&wm).zoom_pct, 200, "zoom survives the relaunch");
    wait_gone(pgid, &profile);
    let (pgid2, profile2) = browser(&wm).process_info().expect("relaunched Chrome");
    assert_ne!(profile2, profile, "a fresh profile dir per launch");

    eprintln!("close: Chrome gone and profile removed");
    colon(&mut wm, ":browser close");
    assert!(wm.active_pane().browser_canvas().is_none());
    wait_gone(pgid2, &profile2);
    eprintln!("  ok  process group {pgid2} gone, {} removed", profile2.display());
}
