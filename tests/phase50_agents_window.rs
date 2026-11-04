//! Phase 50-07: AGENT TRAFFIC zoom window (FLT-01, FLT-02). `window_` = tracer slice,
//! `layout_` = graph placement, `graph_` = selection model, `render_` = frame output.

use bilterm::ui::WorkspaceWidget;
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

/// Two-pane headless fixture.
fn fixture(cols: u16, rows: u16) -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(rows, cols);
    let area = Rect::new(0, 0, cols, rows);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(area).unwrap();
    (wm, area)
}

fn render(wm: &WindowManager, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(wm).render(area, &mut buf);
    buf
}

fn lines(buf: &Buffer) -> Vec<String> {
    let a = buf.area;
    (a.y..a.y + a.height)
        .map(|y| (a.x..a.x + a.width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
        .collect()
}

fn is_braille(sym: &str) -> bool {
    sym.chars().next().is_some_and(|c| ('\u{2801}'..='\u{28FF}').contains(&c))
}

#[test]
fn window_tracer_render_open_close() {
    let (mut wm, area) = fixture(120, 40);
    wm.broadcast_command("echo hi").unwrap();
    wm.open_agents_window(None);
    let rect = wm.agents_window_rect(area).expect("window rect");
    let buf = render(&wm, area);
    let text = lines(&buf).join("\n");

    assert!(text.contains("AGENT TRAFFIC"), "missing window title");
    for label in ["you", "#1", "#2"] {
        assert!(text.contains(label), "missing node label {label}");
    }
    assert!(
        lines(&buf).iter().any(|l| l.contains("echo hi") && l.contains("broadcast")),
        "missing events line for the broadcast"
    );
    let graph_has_braille = (rect.y + 1..rect.y + rect.height - 1)
        .flat_map(|y| (rect.x + 1..rect.x + rect.width / 2).map(move |x| (x, y)))
        .any(|(x, y)| is_braille(buf[(x, y)].symbol()));
    assert!(graph_has_braille, "no braille cell inside the graph pane");

    wm.close_agents_window();
    assert!(!wm.agents_window().active && !wm.agents_window().focused);
    let after = lines(&render(&wm, area)).join("\n");
    assert!(!after.contains("AGENT TRAFFIC"), "window must disappear when closed");
}
