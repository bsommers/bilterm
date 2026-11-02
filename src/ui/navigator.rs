//! Workspace navigator: the compact top-bar "pills" (one per pane/window) for non-Legacy profiles.
//!
//! Read-only view over `WindowManager` (no new state). Clicking a pill calls the existing
//! `WindowManager::focus_and_raise`. Layout and hit-testing share `layout_pills` so they cannot disagree.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::ui::design::{ColorRole, DesignTokens};
use crate::ui::theme::ThemePalette;
use crate::pane::PaneId;
use crate::wm::WindowManager;

/// Activity state shown as a dot on the pill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavState {
    Active,
    Busy,
    Idle,
}

/// One positioned pill. `x0..x1` are absolute columns (x1 exclusive).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pill {
    pub pane: Option<PaneId>,
    pub label: String,
    pub state: NavState,
    pub x0: u16,
    pub x1: u16,
}

fn nav_state(wm: &WindowManager, id: PaneId) -> NavState {
    let Some(p) = wm.pane(id) else { return NavState::Idle };
    if wm.active_pane_id() == id {
        NavState::Active
    } else if p.has_foreground_subcommand() {
        NavState::Busy
    } else {
        NavState::Idle
    }
}

/// Positions pills inside `bar`. Pills that do not fit collapse into a trailing `+n` pill (`pane: None`).
pub fn layout_pills(wm: &WindowManager, bar: Rect) -> Vec<Pill> {
    let mut out = Vec::new();
    let mut x = bar.x.saturating_add(1);
    let end = bar.x.saturating_add(bar.width);
    let ids: Vec<PaneId> = wm.panes().keys().copied().collect();
    for (n, id) in ids.iter().enumerate() {
        let title = wm.pane(*id).map(|p| p.title().to_string()).unwrap_or_default();
        let short: String = title.chars().take(18).collect();
        let label = format!(" {} {} ", id, short.trim());
        let w = label.chars().count() as u16;
        let remaining = ids.len() - n;
        // Reserve room for a "+n" pill unless this is the last one.
        let reserve = if remaining > 1 { 5 } else { 0 };
        if x + w + reserve > end {
            let more = format!(" +{remaining} ");
            let mw = more.chars().count() as u16;
            if x + mw <= end {
                out.push(Pill { pane: None, label: more, state: NavState::Idle, x0: x, x1: x + mw });
            }
            return out;
        }
        out.push(Pill { pane: Some(*id), label, state: nav_state(wm, *id), x0: x, x1: x + w });
        x += w + 1;
    }
    out
}

/// Pane under absolute column `col` on the bar, if a real pill is there.
pub fn pill_at(wm: &WindowManager, bar: Rect, col: u16) -> Option<PaneId> {
    layout_pills(wm, bar).into_iter().find(|p| col >= p.x0 && col < p.x1).and_then(|p| p.pane)
}

/// Draws the bar: pills on the left over the canvas colour.
pub fn render_navigator(wm: &WindowManager, bar: Rect, palette: &ThemePalette, buf: &mut Buffer) {
    if bar.width == 0 || bar.height == 0 {
        return;
    }
    let t = DesignTokens::from_palette(palette);
    let base = Style::default().fg(t.get(ColorRole::TextMuted)).bg(t.get(ColorRole::Canvas));
    for x in bar.x..bar.x + bar.width {
        if let Some(c) = buf.cell_mut((x, bar.y)) {
            c.set_symbol(" ").set_style(base);
        }
    }
    for pill in layout_pills(wm, bar) {
        let style = match pill.state {
            NavState::Active => Style::default()
                .fg(t.get(ColorRole::AccentOn))
                .bg(t.get(ColorRole::Accent))
                .add_modifier(Modifier::BOLD),
            NavState::Busy => Style::default().fg(t.get(ColorRole::Warning)).bg(t.get(ColorRole::Surface)),
            NavState::Idle => Style::default().fg(t.get(ColorRole::TextMuted)).bg(t.get(ColorRole::Surface)),
        };
        buf.set_stringn(pill.x0, bar.y, &pill.label, usize::from(pill.x1 - pill.x0), style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wm::split_tree::SplitDirection;

    fn wm_with(n: usize) -> WindowManager {
        let mut wm = WindowManager::new_headless(20, 100);
        for _ in 1..n {
            wm.split_focused(SplitDirection::Vertical).unwrap();
        }
        wm
    }

    #[test]
    fn one_pill_per_pane_and_hit_testing_agrees_with_layout() {
        let wm = wm_with(3);
        let bar = Rect::new(0, 0, 100, 1);
        let pills = layout_pills(&wm, bar);
        assert_eq!(pills.len(), 3);
        for p in &pills {
            assert_eq!(pill_at(&wm, bar, p.x0), p.pane);
            assert_eq!(pill_at(&wm, bar, p.x1 - 1), p.pane);
        }
        assert_eq!(pill_at(&wm, bar, 99), None);
        assert_eq!(pills.iter().filter(|p| p.state == NavState::Active).count(), 1);
    }

    #[test]
    fn narrow_bar_collapses_overflow_into_plus_n() {
        let wm = wm_with(6);
        let bar = Rect::new(0, 0, 30, 1);
        let pills = layout_pills(&wm, bar);
        let last = pills.last().unwrap();
        assert_eq!(last.pane, None);
        assert!(last.label.contains('+'));
        assert!(pills.iter().all(|p| p.x1 <= bar.x + bar.width));
    }
}
