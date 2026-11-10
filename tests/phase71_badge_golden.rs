//! Phase 71: golden for the compact coordinate badge on two stacked windows.

use bilterm::ui::{WorkspaceWidget, buffer_to_snapshot_string};
use bilterm::wm::WindowManager;
use bilterm::wm::coordinates::CoordDisplayStyle;
use bilterm::wm::split_tree::SplitDirection;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

#[test]
fn compact_badge_two_stacked_windows() {
    let area = Rect::new(0, 0, 120, 30);
    let mut wm = WindowManager::new_headless(30, 120);
    wm.resize_all(area).unwrap();
    wm.split_focused(SplitDirection::Horizontal).unwrap();
    wm.resize_all(area).unwrap();
    wm.set_coord_style(CoordDisplayStyle::Compact);
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(&wm).render(area, &mut buf);
    insta::assert_snapshot!(buffer_to_snapshot_string(&buf));
}
