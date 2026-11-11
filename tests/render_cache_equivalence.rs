//! The render cache must never change a frame: for a sequence of workspace states, a frame drawn
//! with the cache on (cold, warming and hot) and in self-checking `Verify` mode equals the frame
//! drawn with the cache off. The states are applied to one long-lived workspace so that stored
//! layers survive from one state to the next; a missing cache-key input shows up as a stale frame.
mod render_fixture;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::ui::canvas_cache::{RenderCacheMode, with_render_cache_mode};
use bilterm::ui::{CompositePaneWidget, WorkspaceWidget};
use bilterm::wm::{WindowManager, WorkspaceLayoutMode};

fn render(mode: RenderCacheMode, wm: &WindowManager, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    with_render_cache_mode(mode, || WorkspaceWidget::new(wm).render(area, &mut buf));
    buf
}

/// Asserts cache-on (three frames: direct, store, hit) and `Verify` equal cache-off for the
/// current state, and returns the cache-off frame.
fn assert_cache_transparent(label: &str, wm: &WindowManager, area: Rect) -> Buffer {
    let reference = render(RenderCacheMode::Off, wm, area);
    for round in 0..3 {
        let cached = render(RenderCacheMode::On, wm, area);
        assert!(cached == reference, "{label}: cache-on frame {round} differs from cache-off frame");
    }
    let verified = render(RenderCacheMode::Verify, wm, area);
    assert!(verified == reference, "{label}: verify-mode frame differs from cache-off frame");
    reference
}

fn colon(wm: &mut WindowManager, cmd: &str) {
    let parsed = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("{cmd}: not a command"))
        .unwrap_or_else(|e| panic!("{cmd}: {e:?}"));
    execute_wm_colon_command(wm, parsed);
}

#[test]
fn workspace_frames_are_identical_with_cache_on_off_and_verify_across_state_changes() {
    let (mut wm, area) = render_fixture::build_workspace(160, 48, 6);
    let ids: Vec<_> = wm.compute_workspace_rects(area).into_iter().map(|(id, _)| id).collect();
    let mut frames: Vec<(String, Buffer)> = Vec::new();
    let mut step = |label: &str, wm: &WindowManager, must_change: bool| {
        let frame = assert_cache_transparent(label, wm, area);
        if must_change && let Some((prev_label, prev)) = frames.last() {
            assert!(*prev != frame, "{label}: expected a visible change after {prev_label}");
        }
        frames.push((label.to_string(), frame));
    };

    step("initial tiled", &wm, false);

    // Grid content: one pane gets output; the others must keep their (cached) content.
    wm.pane_mut(ids[1]).unwrap().process_pty_output(b"\x1b[1;31mNEW OUTPUT\x1b[0m https://example.org/new\r\n");
    step("output in one pane", &wm, true);

    // Scrollback: the generation does not change, only the offset.
    wm.pane_mut(ids[0]).unwrap().vt_mut().scroll_up(7);
    step("scrolled back", &wm, true);
    wm.pane_mut(ids[0]).unwrap().vt_mut().reset_scrollback();
    step("scroll reset", &wm, true);

    // Global theme, then triggers and scanlines.
    colon(&mut wm, ":theme 3279");
    step("global theme", &wm, true);
    wm.config_mut().triggers = vec!["ERROR".to_string(), "warning".to_string()];
    step("triggers", &wm, true);
    wm.config_mut().crt_scanlines = true;
    step("crt scanlines", &wm, true);
    wm.config_mut().crt_scanlines = false;
    wm.config_mut().triggers.clear();
    step("triggers and scanlines off", &wm, true);

    // Per-window colour override on the focused pane only.
    wm.active_pane_chrome_mut().colors.insert("bg".to_string(), "#202040".to_string());
    step("per-window bg", &wm, true);
    wm.active_pane_chrome_mut().colors.insert("font-color".to_string(), "#ffcc00".to_string());
    step("per-window font colour", &wm, true);

    // State drawn above the cached layer: search, copy mode, hints.
    wm.active_pane_mut().process_pty_output(b"needle in https://example.org/haystack at 10.0.0.1\r\n");
    wm.active_pane_mut().set_canvas_search("needle");
    step("canvas search", &wm, true);
    wm.active_pane_mut().clear_canvas_search();
    wm.active_pane_mut().enter_copy_mode();
    step("copy mode", &wm, true);
    wm.active_pane_mut().exit_copy_mode();
    wm.active_pane_mut().enter_hints_mode();
    step("hints mode", &wm, true);
    wm.active_pane_mut().exit_hints_mode();
    step("hints off", &wm, true);

    // Overlays and layout.
    wm.toggle_help();
    step("help modal", &wm, true);
    wm.toggle_help();
    wm.toggle_palette();
    step("command palette", &wm, true);
    wm.toggle_palette();
    wm.toggle_quick_terminal();
    step("quick terminal", &wm, true);
    wm.toggle_quick_terminal();
    wm.set_layout_mode(WorkspaceLayoutMode::Stacked);
    wm.resize_all(area).unwrap();
    step("stacked layout", &wm, true);
    wm.shuffle_deck_next();
    step("stack shuffled", &wm, true);
    wm.set_layout_mode(WorkspaceLayoutMode::Tiled);
    wm.resize_all(area).unwrap();
    wm.toggle_zoom();
    step("zoomed pane", &wm, true);
    wm.toggle_zoom();
    step("unzoomed", &wm, true);

    // Viewport resize: every pane gets a new canvas and grid size.
    let small = Rect::new(0, 0, 120, 36);
    wm.resize_all(small).unwrap();
    let reference = render(RenderCacheMode::Off, &wm, small);
    for _ in 0..3 {
        assert!(render(RenderCacheMode::On, &wm, small) == reference, "resized: cache-on differs");
    }
    assert!(render(RenderCacheMode::Verify, &wm, small) == reference, "resized: verify differs");
}

#[test]
fn a_busy_pane_interleaved_with_idle_frames_never_shows_a_stale_frame() {
    let (mut wm, area) = render_fixture::build_workspace(120, 40, 4);
    let ids: Vec<_> = wm.compute_workspace_rects(area).into_iter().map(|(id, _)| id).collect();
    for i in 0..40usize {
        // Output arrives on some frames only, so layers are stored, hit and invalidated.
        if i % 3 != 1 {
            let id = ids[i % ids.len()];
            let line = format!("\x1b[3{}mframe {i}\x1b[0m \x1b[7m inv \x1b[0m http://localhost/{i}\r\n", i % 8);
            wm.pane_mut(id).unwrap().process_pty_output(line.as_bytes());
        }
        if i % 7 == 0 {
            wm.pane_mut(ids[0]).unwrap().vt_mut().scroll_up(2);
        }
        if i % 11 == 0 {
            wm.pane_mut(ids[0]).unwrap().vt_mut().reset_scrollback();
        }
        let cached = render(RenderCacheMode::On, &wm, area);
        let reference = render(RenderCacheMode::Off, &wm, area);
        assert!(cached == reference, "frame {i}: cache-on differs from cache-off");
    }
}

#[test]
fn a_pane_drawn_at_two_sizes_in_one_frame_stays_correct() {
    // The same pane rendered into two different rectangles (workspace view + a thumbnail) on
    // alternating calls: both views must stay correct while they compete for the pane's cache.
    let (wm, area) = render_fixture::build_workspace(100, 30, 2);
    let pane = wm.active_pane();
    let rects = [Rect::new(0, 0, 100, 30), Rect::new(10, 5, 40, 12), Rect::new(3, 3, 61, 17)];
    let draw = |mode, rect: Rect| {
        let mut buf = Buffer::empty(area);
        with_render_cache_mode(mode, || CompositePaneWidget::with_config(pane, true, wm.config()).render(rect, &mut buf));
        buf
    };
    for round in 0..5 {
        for rect in rects {
            assert!(
                draw(RenderCacheMode::On, rect) == draw(RenderCacheMode::Off, rect),
                "round {round} rect {rect:?}"
            );
        }
    }
}
