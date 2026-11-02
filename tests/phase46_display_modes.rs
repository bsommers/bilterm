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
