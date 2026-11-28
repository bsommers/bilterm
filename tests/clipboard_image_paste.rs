//! Image paste (path delivery): deterministic headless tests with a mocked clipboard image source.
//!
//! Covers smart `Ctrl+V`, `:paste image`, the leader chord, the menu item, RPC / MCP (gated),
//! passthrough fallback, and the resource bounds (size, dimensions, disk LRU, memory reservation).

use std::sync::{Arc, Mutex};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use bilterm::action::Action;
use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::clipboard::{ClipboardMimeProbe, set_test_clipboard_mime_override};
use bilterm::clipboard_image::{self, ImageDelivery, ImageError, ImageLimits, ImageSource};
use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::input::handle_wm_key_event;
use bilterm::input::keymap::default_leader_action;
use bilterm::ui::modals::{PALETTE_CATALOG, menubar_category_item_count, menubar_item_command};
use bilterm::wm::WindowManager;
use bilterm::wm::clipboard_window::ClipboardWindowOp;

static LOCK: Mutex<()> = Mutex::new(());

struct Fixed(Mutex<Result<Vec<u8>, ImageError>>);

impl ImageSource for Fixed {
    fn read(&self, _l: &ImageLimits) -> Result<Vec<u8>, ImageError> {
        self.0.lock().unwrap().clone()
    }
}

fn png(w: u32, h: u32, pad: usize) -> Vec<u8> {
    let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    v.extend_from_slice(&w.to_be_bytes());
    v.extend_from_slice(&h.to_be_bytes());
    v.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    v.extend(std::iter::repeat_n(0xAB, pad));
    v
}

fn setup(tag: &str, src: Result<Vec<u8>, ImageError>) -> std::sync::MutexGuard<'static, ()> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("imgpaste-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    clipboard_image::set_test_store_dir(Some(dir));
    clipboard_image::set_test_image_source(Some(Arc::new(Fixed(Mutex::new(src)))));
    set_test_clipboard_mime_override(Some(ClipboardMimeProbe::Media { mime: "image/png".into(), bytes_len: 0 }));
    g
}

fn set_src(src: Result<Vec<u8>, ImageError>) {
    clipboard_image::set_test_image_source(Some(Arc::new(Fixed(Mutex::new(src)))));
}

fn raw_tui_wm() -> WindowManager {
    let mut wm = WindowManager::new_headless(25, 80);
    wm.active_pane_mut().process_pty_output(b"\x1b[?1049h\x1b[?2004h");
    wm.active_pane_mut().write_to_pty(b"").unwrap();
    wm
}

fn ctrl_v(wm: &mut WindowManager) {
    handle_wm_key_event(wm, KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL)).unwrap();
}

fn last_write(wm: &WindowManager) -> String {
    String::from_utf8_lossy(wm.active_pane().last_pty_write()).into_owned()
}

fn pasted_path(write: &str) -> String {
    write.strip_prefix("\x1b[200~").and_then(|s| s.strip_suffix("\x1b[201~")).expect("bracketed paste").to_string()
}

#[test]
fn ctrl_v_with_image_pastes_a_private_file_path_bracketed() {
    let _g = setup("ctrlv", Ok(png(64, 48, 100)));
    let mut wm = raw_tui_wm();
    ctrl_v(&mut wm);
    let path = pasted_path(&last_write(&wm));
    let bytes = std::fs::read(&path).expect("stored file exists");
    assert!(bytes.starts_with(b"\x89PNG"));
    assert!(path.contains("img-") && path.ends_with(".png"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }
    assert!(wm.active_pane().status_message().unwrap_or("").contains("Pasted image path"));
    // Same image again: deduped to the same path.
    wm.active_pane_mut().write_to_pty(b"").unwrap();
    ctrl_v(&mut wm);
    assert_eq!(pasted_path(&last_write(&wm)), path);
}

#[test]
fn passthrough_delivery_and_disabled_keep_the_raw_ctrl_v_behaviour() {
    let _g = setup("passthrough", Ok(png(8, 8, 0)));
    let mut wm = raw_tui_wm();
    wm.config_mut().clipboard.images.delivery = ImageDelivery::Passthrough;
    ctrl_v(&mut wm);
    assert_eq!(wm.active_pane().last_pty_write(), &[0x16]);

    let mut wm = raw_tui_wm();
    wm.config_mut().clipboard.images.enabled = false;
    ctrl_v(&mut wm);
    assert_eq!(wm.active_pane().last_pty_write(), &[0x16]);
}

#[test]
fn no_usable_image_falls_back_to_raw_ctrl_v_but_limit_errors_do_not() {
    let _g = setup("fallback", Err(ImageError::NoImage));
    let mut wm = raw_tui_wm();
    ctrl_v(&mut wm);
    assert_eq!(wm.active_pane().last_pty_write(), &[0x16], "no image / missing tool -> app reads clipboard itself");

    for err in [ImageError::TooLarge, ImageError::NotAnImage("x".into())] {
        set_src(Err(err));
        let mut wm = raw_tui_wm();
        ctrl_v(&mut wm);
        assert!(wm.active_pane().last_pty_write().is_empty(), "limit errors must not forward ^V");
        assert!(!wm.active_pane().status_message().unwrap_or("").is_empty());
    }
}

#[test]
fn oversize_dimensions_and_size_caps_write_nothing() {
    let _g = setup("caps", Ok(png(20_000, 20, 0)));
    let mut wm = raw_tui_wm();
    ctrl_v(&mut wm);
    assert!(wm.active_pane().last_pty_write().is_empty());
    assert!(wm.active_pane().status_message().unwrap().contains("dimension"));

    set_src(Ok(png(10, 10, 2 * 1024 * 1024)));
    wm.config_mut().clipboard.images.max_image_mib = 1;
    ctrl_v(&mut wm);
    assert!(wm.active_pane().status_message().unwrap().contains("size limit"));
    assert_eq!(clipboard_image::in_flight_bytes(), 0);
}

#[test]
fn disk_budget_evicts_least_recently_used_files() {
    let _g = setup("lru", Ok(png(4, 4, 600 * 1024)));
    let mut wm = raw_tui_wm();
    wm.config_mut().clipboard.images.max_image_mib = 1;
    wm.config_mut().clipboard.images.max_disk_mib = 1;
    let mut paths = Vec::new();
    for n in 0..3usize {
        set_src(Ok(png(4, 4, 600 * 1024 + n)));
        wm.active_pane_mut().write_to_pty(b"").unwrap();
        ctrl_v(&mut wm);
        paths.push(pasted_path(&last_write(&wm)));
    }
    let alive = paths.iter().filter(|p| std::path::Path::new(p).is_file()).count();
    assert_eq!(alive, 1, "only one ~600 KiB image fits in 1 MiB: {paths:?}");
    assert!(std::path::Path::new(&paths[2]).is_file(), "newest survives");
}

#[test]
fn every_surface_lowers_to_the_same_action_and_effect() {
    let _g = setup("surfaces", Ok(png(16, 16, 10)));
    let want = Action::ClipboardWindow(ClipboardWindowOp::PasteImage);
    assert_eq!(parse_colon_command(":paste image").unwrap().unwrap(), want);
    assert_eq!(parse_colon_command(":clipboard paste-image").unwrap().unwrap(), want);
    assert_eq!(default_leader_action(KeyEvent::new(KeyCode::Char('('), KeyModifiers::NONE)), Some(want.clone()));
    assert_eq!(ClipboardWindowOp::PasteImage.colon_form(), ":paste image");
    assert!(PALETTE_CATALOG.iter().any(|e| e.0 == ":paste image"), "palette entry");
    let menu = (0..9).find_map(|c| (0..menubar_category_item_count(c)).find_map(|i| (menubar_item_command(c, i) == Some(":paste image")).then_some((c, i))));
    assert!(menu.is_some(), "menu bar item (mouse path)");

    // Effect through colon, leader Action and direct apply is identical.
    let mut outs = Vec::new();
    for how in 0..3 {
        let mut wm = raw_tui_wm();
        match how {
            0 => {
                execute_wm_colon_command(&mut wm, parse_colon_command(":paste image").unwrap().unwrap());
            }
            1 => {
                wm.apply(default_leader_action(KeyEvent::new(KeyCode::Char('('), KeyModifiers::NONE)).unwrap()).unwrap();
            }
            _ => {
                wm.apply(want.clone()).unwrap();
            }
        }
        outs.push(last_write(&wm));
    }
    assert!(outs.iter().all(|o| o == &outs[0] && o.starts_with("\x1b[200~")), "{outs:?}");
}

fn params(kv: &[(&str, JsonValue)]) -> JsonValue {
    JsonValue::obj(kv.iter().map(|(k, v)| (*k, v.clone())))
}

#[test]
fn rpc_and_mcp_image_paste_are_gated_by_allow_clipboard_image() {
    let _g = setup("rpc", Ok(png(16, 16, 10)));
    let mut cp = AutomationControlPlane::new_headless(25, 80);
    cp.wm.active_pane_mut().process_pty_output(b"\x1b[?1049h\x1b[?2004h");
    let p = params(&[("image", JsonValue::Bool(true))]);

    let err = cp.dispatch_method("clipboard.paste", &p).unwrap_err();
    assert!(err.contains("allow_clipboard_image"), "{err}");
    assert!(cp.wm.active_pane().last_pty_write().is_empty());
    let mcp = cp
        .handle_mcp_line(r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"bilterm_clipboard","arguments":{"action":"paste","image":true}}}"#)
        .unwrap();
    assert!(mcp.contains("allow_clipboard_image"), "{mcp}");
    let wm_cmd = cp.dispatch_method("wm.command", &params(&[("command", JsonValue::String(":paste image".into()))]));
    assert!(wm_cmd.is_err() && wm_cmd.unwrap_err().contains("allow_clipboard_image"));

    cp.wm.config_mut().api.allow_clipboard_image = true;
    let ok = cp.dispatch_method("clipboard.paste", &p).unwrap();
    assert_eq!(ok.get("status").and_then(JsonValue::as_str), Some("pasted_image"));
    let image = ok.get("image").unwrap();
    assert_eq!(image.get("mime").and_then(JsonValue::as_str), Some("image/png"));
    assert_eq!(image.get("width").and_then(JsonValue::as_i64), Some(16));
    let path = image.get("path").and_then(JsonValue::as_str).unwrap().to_string();
    assert!(std::path::Path::new(&path).is_file());
    assert!(String::from_utf8_lossy(cp.wm.active_pane().last_pty_write()).contains(&path));

    let mcp = cp
        .handle_mcp_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"bilterm_clipboard","arguments":{"action":"paste","image":true}}}"#)
        .unwrap();
    assert!(mcp.contains("\"isError\":false") && mcp.contains("pasted_image"), "{mcp}");

    // Source errors surface as RPC errors rather than a fake success.
    set_src(Err(ImageError::NoImage));
    let e = cp.dispatch_method("clipboard.paste", &p).unwrap_err();
    assert!(e.contains("No image"), "{e}");
}

#[test]
fn local_surfaces_are_never_gated() {
    let _g = setup("local", Ok(png(16, 16, 10)));
    let mut wm = raw_tui_wm();
    assert!(!wm.config().api.allow_clipboard_image);
    wm.apply(Action::ClipboardWindow(ClipboardWindowOp::PasteImage)).unwrap();
    assert!(last_write(&wm).starts_with("\x1b[200~"));
}
