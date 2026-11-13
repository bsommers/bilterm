//! `:copy off|exit|clear|stop|close` and `:select off|clear` must remove every canvas highlight
//! source (selection, yank flash, copy cursor, search matches) and leave nothing to repaint.

use bilterm::colon_command::parse_colon_command;
use bilterm::wm::WindowManager;

fn run(wm: &mut WindowManager, cmd: &str) {
    let c = parse_colon_command(cmd).unwrap().unwrap();
    wm.apply(c).unwrap();
}

fn any_highlight(wm: &WindowManager) -> bool {
    let p = wm.active_pane();
    let cm = p.copy_mode();
    cm.active
        || cm.has_selection()
        || cm.flash_region.is_some()
        || cm.rect_mode
        || p.canvas_search().active
        || (0..24).any(|r| (0..80).any(|c| cm.is_cell_selected(r, c)))
}

fn setup() -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.active_pane_mut()
        .process_pty_output(b"ALPHA_111_END\r\nBETA__222_END\r\nGAMMA_333_END\r\n");
    wm
}

#[test]
fn copy_off_aliases_clear_every_highlight_source() {
    for cmd in [
        ":copy off", ":copy exit", ":copy clear", ":copy stop", ":copy close", ":select off",
        ":select clear", ":rect off", ":rect clear",
    ] {
        let mut wm = setup();
        run(&mut wm, ":rect 1 1 3 5 highlight");
        assert!(any_highlight(&wm), "precondition for {cmd}");
        run(&mut wm, cmd);
        assert!(!any_highlight(&wm), "{cmd} left a highlight behind");
    }
}

#[test]
fn copy_off_clears_yank_flash_that_has_no_toast_ttl() {
    let mut wm = setup();
    // Yank leaves a flash region; a later plain status message cancels the toast TTL that would
    // otherwise have cleared it, orphaning the flash forever.
    run(&mut wm, ":rect 1 1 3 5 highlight");
    run(&mut wm, ":copy yank");
    assert!(wm.active_pane().copy_mode().flash_region.is_some());
    wm.active_pane_mut().set_status_message(Some("something else".into()));
    wm.active_pane_mut().tick_toast(10_000_000);
    assert!(
        wm.active_pane().copy_mode().flash_region.is_none(),
        "cancelling the toast must not orphan the yank flash"
    );
    // A flash still pending when `:copy off` runs is also cleared.
    run(&mut wm, ":rect 1 1 3 5 highlight");
    run(&mut wm, ":copy yank");
    assert!(wm.active_pane().copy_mode().flash_region.is_some());
    run(&mut wm, ":copy off");
    assert!(!any_highlight(&wm));
}

#[test]
fn copy_off_clears_search_matches() {
    let mut wm = setup();
    run(&mut wm, ":search BETA");
    assert!(wm.active_pane().canvas_search().active);
    run(&mut wm, ":rect 1 1 2 2 highlight");
    run(&mut wm, ":copy off");
    assert!(!any_highlight(&wm));
}
