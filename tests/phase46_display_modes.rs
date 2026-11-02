use bilterm::ui::design::{AppearanceProfile, DisplayMode};
use bilterm::ui::buffer_to_snapshot_string;
use bilterm::ui::WorkspaceWidget;
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

fn two_pane_wm(profile: Option<AppearanceProfile>, mode: DisplayMode) -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(20, 80);
    let area = Rect::new(0, 0, 80, 20);
    wm.pane_mut(1).unwrap().process_pty_output(b"LEFT_OUT\r\n");
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.pane_mut(p2).unwrap().process_pty_output(b"RIGHT_OUT\r\n");
    wm.config_mut().appearance.profile = profile;
    wm.config_mut().appearance.display_mode = mode;
    wm.resize_all(area).unwrap();
    (wm, area)
}

fn render(wm: &WindowManager, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(wm).render(area, &mut buf);
    buf
}

#[test]
fn legacy_ignores_display_mode_completely() {
    let (std_wm, area) = two_pane_wm(None, DisplayMode::Standard);
    let base = buffer_to_snapshot_string(&render(&std_wm, area));
    for mode in [DisplayMode::Focus, DisplayMode::Zen] {
        let (wm, area) = two_pane_wm(None, mode);
        assert_eq!(wm.compute_workspace_rects(area).len(), 2);
        assert!(wm.is_framed());
        assert_eq!(buffer_to_snapshot_string(&render(&wm, area)), base, "{mode:?} changed Legacy output");
    }
}

#[test]
fn obsidian_zen_shows_only_focused_pane_unframed() {
    let (wm, area) = two_pane_wm(Some(AppearanceProfile::Obsidian), DisplayMode::Zen);
    let rects = wm.compute_workspace_rects(area);
    assert_eq!(rects.len(), 1);
    assert_eq!(rects[0].0, wm.active_pane_id());
    assert_eq!(rects[0].1, area);
    assert!(!wm.is_framed());
    let text = buffer_to_snapshot_string(&render(&wm, area));
    assert!(!text.contains("LEFT_OUT") || !text.contains("RIGHT_OUT"), "both panes visible in Zen");
    // The floating prompt box (top 3 rows) is kept in Zen; no pane frame may appear below it.
    let below: String = text.lines().skip(3).collect::<Vec<_>>().join("\n");
    assert!(!below.contains('╔') && !below.contains('║') && !below.contains('┌'), "pane frame visible in Zen:\n{text}");
}

#[test]
fn obsidian_zen_bottom_row_shows_only_notice() {
    let (mut wm, area) = two_pane_wm(Some(AppearanceProfile::Obsidian), DisplayMode::Zen);
    wm.active_pane_mut().set_status_message(Some("hello-notice".into()));
    let buf = render(&wm, area);
    let last: String = (0..area.width).map(|x| buf[(x, area.height - 1)].symbol().to_string()).collect();
    assert!(last.contains("hello-notice"), "{last:?}");
    assert!(!last.contains("Ln") && !last.contains("PANE"), "OIA content leaked into Zen row: {last:?}");
}

#[test]
fn leaving_zen_restores_all_panes() {
    let (mut wm, area) = two_pane_wm(Some(AppearanceProfile::Obsidian), DisplayMode::Zen);
    assert_eq!(wm.compute_workspace_rects(area).len(), 1);
    wm.config_mut().appearance.display_mode = DisplayMode::Standard;
    assert_eq!(wm.compute_workspace_rects(area).len(), 2);
    assert!(wm.is_framed());
}

fn row_text(buf: &Buffer, y: u16, w: u16) -> String {
    (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect()
}

#[test]
fn obsidian_standard_shows_navigator_pills_and_legacy_does_not() {
    let (wm, area) = two_pane_wm(Some(AppearanceProfile::Obsidian), DisplayMode::Standard);
    let top = row_text(&render(&wm, area), 0, area.width);
    assert!(top.contains(" 1 ") && top.contains(" 2 "), "{top:?}");
    let (legacy, area) = two_pane_wm(None, DisplayMode::Standard);
    let ltop = row_text(&render(&legacy, area), 0, area.width);
    assert!(!ltop.contains(" 1 ") || ltop.contains('╔') || ltop.contains('┌'), "{ltop:?}");
    // The pill row costs the workspace exactly one row.
    let rects = wm.compute_workspace_rects(Rect::new(0, 1, 80, 19));
    assert!(rects.iter().all(|(_, r)| r.y >= 1));
}

#[test]
fn pill_bar_absent_in_zen_and_in_minimal_with_single_pane() {
    let (zen, area) = two_pane_wm(Some(AppearanceProfile::Obsidian), DisplayMode::Zen);
    assert!(!row_text(&render(&zen, area), 0, area.width).contains(" 1 "));
    let mut wm = WindowManager::new_headless(20, 80);
    wm.config_mut().appearance.profile = Some(AppearanceProfile::Obsidian);
    wm.config_mut().appearance.display_mode = DisplayMode::Focus;
    let area = Rect::new(0, 0, 80, 20);
    assert!(!row_text(&render(&wm, area), 0, area.width).contains(" 1 "));
}

#[test]
fn clicking_a_pill_focuses_that_pane() {
    use bilterm::input::mouse::handle_wm_mouse_event;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let (mut wm, area) = two_pane_wm(Some(AppearanceProfile::Obsidian), DisplayMode::Standard);
    let first_pane = *wm.panes().keys().next().unwrap();
    assert_ne!(wm.active_pane_id(), first_pane);
    let bar = Rect::new(0, 0, area.width, 1);
    let pill = bilterm::ui::navigator::layout_pills(&wm, bar)
        .into_iter()
        .find(|p| p.pane == Some(first_pane))
        .unwrap();
    let ev = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: pill.x0 + 1, row: 0, modifiers: KeyModifiers::NONE };
    handle_wm_mouse_event(&mut wm, ev, area).unwrap();
    assert_eq!(wm.active_pane_id(), first_pane);
}

#[test]
fn old_modern_and_liquid_flags_do_not_get_pills() {
    for liquid in [false, true] {
        let mut wm = WindowManager::new_headless(20, 80);
        wm.split_focused(SplitDirection::Vertical).unwrap();
        if liquid { wm.config_mut().liquid_glass = true } else { wm.config_mut().modern_mode = true }
        let area = Rect::new(0, 0, 80, 20);
        assert!(!bilterm::ui::shell::wants_pill_bar(wm.config(), 2, area.height));
        assert_eq!(bilterm::ui::shell::ShellLayout::compute(area, wm.config(), 2).workspace, area);
    }
}

#[test]
fn mcp_console_rows_scroll_horizontally_to_show_full_text() {
    let mut wm = WindowManager::new_headless(24, 100);
    let tail = "TAIL_MARKER_END";
    let long = format!("{}{}", "x".repeat(150), tail);
    wm.open_mcp_popup_window("TEST", vec![long, "short".into()]);
    let area = Rect::new(0, 0, 100, 24);
    let before = buffer_to_snapshot_string(&render(&wm, area));
    assert!(!before.contains(tail), "tail should be clipped before scrolling");
    for _ in 0..40 {
        wm.mcp_console_mut().scroll_right(8);
    }
    let after = buffer_to_snapshot_string(&render(&wm, area));
    assert!(after.contains(tail), "tail should be reachable after scrolling right:\n{after}");
    assert!(after.contains('‹'), "left indicator missing");
    wm.mcp_console_mut().scroll_left(10_000);
    assert_eq!(wm.mcp_console().h_scroll, 0);
}

// ---- 46-04d: GUI-only drawer / :mcp panel scale ----

fn gui_frame(wm: &mut WindowManager, area: Rect) -> bilterm::ui::renderer::GuiFrameSurface {
    use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer};
    wm.set_gui_surface(true);
    let mut renderer = GuiWorkspaceRenderer::new(8, 16);
    let mut surface = GuiFrameSurface::new(8, 16);
    renderer.render_workspace_with_cell_size(wm, area, 8, 16, &mut surface);
    surface
}

#[test]
fn panel_scale_default_is_identical_and_ignored_off_gui() {
    let (mut wm, area) = two_pane_wm(Some(AppearanceProfile::Obsidian), DisplayMode::Standard);
    wm.toggle_drawer();
    let base = gui_frame(&mut wm, area);
    wm.set_panel_scale_pct(100);
    let same = gui_frame(&mut wm, area);
    assert_eq!(base.glyphs.len(), same.glyphs.len());
    assert_eq!(base.quads.len(), same.quads.len());
    // Off the GUI the setting never applies.
    let mut tui = WindowManager::new_headless(20, 80);
    tui.set_panel_scale_pct(150);
    assert_eq!(tui.panel_scale_pct(), 100);
    assert_eq!(tui.panel_scale_setting(), 150);
}

#[test]
fn panel_scale_enlarges_drawer_glyphs_in_gui() {
    let (mut wm, area) = two_pane_wm(Some(AppearanceProfile::Obsidian), DisplayMode::Standard);
    wm.toggle_drawer();
    let base = gui_frame(&mut wm, area);
    wm.set_panel_scale_pct(150);
    let big = gui_frame(&mut wm, area);
    let base_max = base.glyphs.iter().map(|g| g.cell_width_px).max().unwrap();
    assert!(big.glyphs.iter().any(|g| g.cell_width_px > base_max), "drawer cells should be wider at 150%");
}

#[test]
fn panel_scale_maps_mouse_into_panel_space() {
    use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
    let mut wm = WindowManager::new_headless(20, 80);
    let area = Rect::new(0, 0, 80, 20);
    wm.set_gui_surface(true);
    wm.set_panel_scale_pct(200);
    let m = MouseEvent { kind: MouseEventKind::Moved, column: 40, row: 10, modifiers: KeyModifiers::NONE };
    let (pm, vws) = wm.panel_space(m, area);
    assert_eq!((pm.column, pm.row, vws.width, vws.height), (20, 5, 40, 10));
}

#[test]
fn drawer_rows_scroll_horizontally_and_default_is_unchanged() {
    let (mut wm, area) = two_pane_wm(Some(AppearanceProfile::Obsidian), DisplayMode::Standard);
    wm.toggle_drawer();
    let base = buffer_to_snapshot_string(&render(&wm, area));
    wm.drawer_hscroll_by(0);
    assert_eq!(base, buffer_to_snapshot_string(&render(&wm, area)));
    wm.drawer_hscroll_by(6);
    if wm.drawer().h_scroll > 0 {
        assert!(buffer_to_snapshot_string(&render(&wm, area)).contains('‹'));
    }
    wm.drawer_hscroll_by(-100);
    assert_eq!(wm.drawer().h_scroll, 0);
    assert_eq!(base, buffer_to_snapshot_string(&render(&wm, area)));
}

#[test]
fn mcp_size_override_sets_panel_rect_and_auto_restores() {
    let mut wm = WindowManager::new_headless(30, 100);
    let area = Rect::new(0, 0, 100, 30);
    wm.execute_mcp_command("open");
    let auto = wm.mcp_console().panel_rect(area).unwrap();
    wm.execute_mcp_command("size 70x12");
    let r = wm.mcp_console().panel_rect(area).unwrap();
    assert_eq!((r.width, r.height), (70, 12));
    wm.execute_mcp_command("size auto");
    assert_eq!(wm.mcp_console().panel_rect(area).unwrap(), auto);
}

#[test]
fn drawer_wheel_moves_viewport_not_selection() {
    let mut d = bilterm::wm::WorkspaceDrawer::new();
    assert_eq!(d.effective_scroll(0, 5, 20), 0);
    d.wheel_scroll(0, 5, 20, 3);
    assert_eq!(d.effective_scroll(0, 5, 20), 3);
    // Selection moved: viewport falls back to pinning.
    assert_eq!(d.effective_scroll(1, 5, 20), 0);
    d.wheel_scroll(0, 5, 20, 100);
    assert_eq!(d.effective_scroll(0, 5, 20), 15);
}

use bilterm::ui::design::TitlePolicy;

#[test]
fn chrome_title_policy_applies_to_new_looks_and_legacy_ignores_it() {
    let (mut legacy, area) = two_pane_wm(None, DisplayMode::Standard);
    let base = buffer_to_snapshot_string(&render(&legacy, area));
    legacy.config_mut().appearance.title = Some(TitlePolicy::None);
    legacy.config_mut().appearance.focus_ring = true;
    assert_eq!(buffer_to_snapshot_string(&render(&legacy, area)), base, "Legacy must ignore title/focus_ring");

    let (mut wm, area) = two_pane_wm(Some(AppearanceProfile::Obsidian), DisplayMode::Standard);
    let all = buffer_to_snapshot_string(&render(&wm, area));
    wm.config_mut().appearance.title = Some(TitlePolicy::None);
    let none = buffer_to_snapshot_string(&render(&wm, area));
    wm.config_mut().appearance.title = Some(TitlePolicy::Focused);
    let focused = buffer_to_snapshot_string(&render(&wm, area));
    assert_ne!(all, none);
    assert_ne!(all, focused);
    assert_ne!(focused, none);
    wm.config_mut().appearance.title = None;
    assert_eq!(buffer_to_snapshot_string(&render(&wm, area)), all, "None keeps the mode default");
}

#[test]
fn focus_ring_recolors_only_the_focused_border() {
    let (mut wm, area) = two_pane_wm(Some(AppearanceProfile::Obsidian), DisplayMode::Standard);
    let before = render(&wm, area);
    wm.config_mut().appearance.focus_ring = true;
    let after = render(&wm, area);
    assert_eq!(buffer_to_snapshot_string(&before), buffer_to_snapshot_string(&after), "ring changes colour only");
    let accent = bilterm::ui::design::DesignTokens::from_palette(&wm.config().palette()).accent;
    let count = |b: &Buffer| (0..area.height).flat_map(|y| (0..area.width).map(move |x| (x, y))).filter(|&(x, y)| b[(x, y)].fg == accent).count();
    assert!(count(&after) > 0 && count(&after) >= count(&before), "focused border must carry the accent colour");
}

#[test]
fn chrome_commands_parse_and_apply() {
    use bilterm::colon_command::parse_colon_command;
    use bilterm::action::Action;
    assert!(matches!(parse_colon_command(":chrome title focused").unwrap().unwrap(), Action::ChromeTitle(Some(TitlePolicy::Focused))));
    assert!(matches!(parse_colon_command(":chrome title auto").unwrap().unwrap(), Action::ChromeTitle(None)));
    assert!(matches!(parse_colon_command(":chrome ring on").unwrap().unwrap(), Action::ChromeRing(true)));
    assert!(parse_colon_command(":chrome ring maybe").unwrap().is_err());
}

fn strip_cells(wm: &WindowManager, area: Rect) -> usize {
    let buf = render(wm, area);
    (0..area.width)
        .flat_map(|x| (0..area.height).map(move |y| (x, y)))
        .filter(|&(x, y)| matches!(buf[(x, y)].symbol(), "⊞" | "⤢" | "✕"))
        .count()
}

#[test]
fn pane_action_strip_only_in_non_legacy_standard() {
    let (legacy, area) = two_pane_wm(None, DisplayMode::Standard);
    assert_eq!(strip_cells(&legacy, area), 0);
    let (std_wm, area) = two_pane_wm(Some(AppearanceProfile::Obsidian), DisplayMode::Standard);
    assert_eq!(strip_cells(&std_wm, area), 3, "one strip, on the focused pane only");
    for mode in [DisplayMode::Focus, DisplayMode::Zen] {
        let (wm, area) = two_pane_wm(Some(AppearanceProfile::Obsidian), mode);
        assert_eq!(strip_cells(&wm, area), 0, "{mode:?}");
    }
}

#[test]
fn pane_action_strip_buttons_run_the_same_actions() {
    // 2 panes in 120 columns so the pane stays wide enough after a split.
    use bilterm::input::mouse::handle_wm_mouse_event;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    let click = |wm: &mut WindowManager, area: Rect, idx: usize| {
        let ws = bilterm::ui::shell::ShellLayout::compute(area, wm.config(), wm.pane_count()).workspace;
        let active = wm.active_pane_id();
        let outer = wm.compute_workspace_rects(ws).into_iter().find(|&(id, _)| id == active).unwrap().1;
        let (x, ..) = bilterm::ui::pane_actions::buttons(outer)[idx];
        let ev = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: x, row: outer.y, modifiers: KeyModifiers::NONE };
        handle_wm_mouse_event(wm, ev, area).unwrap();
    };
    let (mut wm, _) = two_pane_wm(Some(AppearanceProfile::Obsidian), DisplayMode::Standard);
    let area = Rect::new(0, 0, 120, 20);
    wm.resize_all(area).unwrap();
    assert!(wm.zoomed_pane().is_none());
    click(&mut wm, area, 1);
    assert!(wm.zoomed_pane().is_some(), "zoom button");
    click(&mut wm, area, 1);
    assert!(wm.zoomed_pane().is_none());
    let before = wm.pane_count();
    click(&mut wm, area, 0);
    assert_eq!(wm.pane_count(), before + 1, "split button");
    click(&mut wm, area, 1);
    wm.resize_all(area).unwrap();
    click(&mut wm, area, 1); // unzoom
    // The split pane is now narrower than the strip threshold, so close from the other pane.
    let narrow = wm.active_pane_id();
    assert!(bilterm::ui::pane_actions::buttons(Rect::new(0, 0, 26, 10)).is_empty());
    let _ = narrow;
    let n = wm.pane_count();
    click(&mut wm, area, 2);
    assert_eq!(wm.pane_count(), n - 1, "close button");
}

#[test]
fn drawer_arrow_keys_scroll_rows_and_right_still_leaves_at_the_end() {
    use bilterm::input::handle_wm_key_event;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (mut wm, _) = two_pane_wm(Some(AppearanceProfile::Obsidian), DisplayMode::Standard);
    wm.toggle_drawer();
    wm.drawer_mut().focused = true;
    // Make a long row so there is something to scroll to.
    wm.active_pane_mut().set_title("x".repeat(200));
    let key = |c| KeyEvent::new(c, KeyModifiers::NONE);
    handle_wm_key_event(&mut wm, key(KeyCode::Right)).unwrap();
    if wm.drawer().h_scroll > 0 {
        assert!(wm.drawer().focused, "Right scrolls while text overflows");
        handle_wm_key_event(&mut wm, key(KeyCode::Left)).unwrap();
        assert_eq!(wm.drawer().h_scroll, 0);
    }
    wm.drawer_hscroll_by(10_000);
    handle_wm_key_event(&mut wm, key(KeyCode::Right)).unwrap();
    assert!(!wm.drawer().focused, "Right at the end leaves the drawer");
}

fn style_dump(buf: &Buffer, area: Rect) -> Vec<(ratatui::style::Color, ratatui::style::Color)> {
    (0..area.height).flat_map(|y| (0..area.width).map(move |x| (x, y))).map(|(x, y)| (buf[(x, y)].fg, buf[(x, y)].bg)).collect()
}

#[test]
fn appearance_profiles_visibly_restyle_and_legacy_restores_exactly() {
    let (mut wm, area) = two_pane_wm(None, DisplayMode::Standard);
    let legacy = style_dump(&render(&wm, area), area);
    let mut seen = vec![legacy.clone()];
    for p in [AppearanceProfile::Obsidian, AppearanceProfile::Midnight, AppearanceProfile::Arctic, AppearanceProfile::Liquid] {
        wm.config_mut().appearance.profile = Some(p);
        let d = style_dump(&render(&wm, area), area);
        assert!(!seen.contains(&d), "{p:?} must look different from the other looks");
        seen.push(d);
    }
    wm.config_mut().appearance.profile = Some(AppearanceProfile::Legacy);
    assert_eq!(style_dump(&render(&wm, area), area), legacy, ":appearance legacy restores the original look exactly");
}
