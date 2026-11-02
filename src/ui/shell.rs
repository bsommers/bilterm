//! Shell layout: where the top bar, workspace area and bottom bar sit inside a viewport.
//!
//! Single source of truth for the offset math that the TUI renderer, the GUI renderer and mouse
//! hit-testing must agree on. Phase 46 adds display modes here; for the Legacy profile the result is
//! exactly the pre-Phase-46 behaviour (a 1-row menubar and/or 1-row F-key bar controlled by config flags).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::config::BiltermConfig;
use crate::pane::CompositePane;
use crate::ui::design::{AppearanceProfile, ColorRole, DesignTokens, DisplayMode};
use crate::ui::theme::ThemePalette;

/// The display mode that actually applies: Legacy ignores `[appearance].display_mode` entirely.
pub fn effective_display_mode(cfg: &BiltermConfig) -> DisplayMode {
    if AppearanceProfile::resolve(cfg) == AppearanceProfile::Legacy {
        DisplayMode::Standard
    } else {
        cfg.appearance.display_mode
    }
}

/// Rectangles for the fixed rows around the workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellLayout {
    /// One-row bar at the top of the viewport, when shown.
    pub top_bar: Option<Rect>,
    /// Area handed to the tiled/stacked panes and overlays.
    pub workspace: Rect,
    /// One-row bar at the bottom of the viewport, when shown.
    pub bottom_bar: Option<Rect>,
}

impl ShellLayout {
    /// Computes the layout for `area` under `cfg`.
    pub fn compute(area: Rect, cfg: &BiltermConfig) -> Self {
        let top = u16::from(cfg.show_top_menubar && area.height > 3);
        let bottom = u16::from(cfg.show_dos_fkey_bar && area.height > 2 + top);
        let workspace = Rect {
            x: area.x,
            y: area.y + top,
            width: area.width,
            height: area.height.saturating_sub(top + bottom).max(1),
        };
        Self {
            top_bar: (top > 0).then_some(Rect { x: area.x, y: area.y, width: area.width, height: 1 }),
            workspace,
            bottom_bar: (bottom > 0)
                .then(|| Rect { x: area.x, y: area.y + area.height - 1, width: area.width, height: 1 }),
        }
    }

    /// Rows consumed above the workspace (0 or 1).
    pub fn top_offset(&self) -> u16 {
        u16::from(self.top_bar.is_some())
    }
}

/// Zen's bottom row: blank canvas with only the pane's transient notice, if any.
pub fn render_zen_row(pane: &CompositePane, rect: Rect, palette: &ThemePalette, buf: &mut Buffer) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let tokens = DesignTokens::from_palette(palette);
    let base = Style::default().fg(tokens.get(ColorRole::TextMuted)).bg(tokens.get(ColorRole::Canvas));
    for x in rect.x..rect.x + rect.width {
        if let Some(cell) = buf.cell_mut((x, rect.y)) {
            cell.set_symbol(" ").set_style(base);
        }
    }
    if let Some(msg) = pane.status_message() {
        let style = Style::default().fg(tokens.get(ColorRole::Accent)).bg(tokens.get(ColorRole::Canvas));
        buf.set_stringn(rect.x + 1, rect.y, msg, usize::from(rect.width.saturating_sub(2)), style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The formula as it was written inline before Phase 46 (kept verbatim as the parity oracle).
    fn legacy_reference(area: Rect, menubar: bool, fkey: bool) -> (u16, u16, Rect) {
        let top_offset = u16::from(menubar && area.height > 3);
        let bottom_offset = u16::from(fkey && area.height > 2 + top_offset);
        let ws = Rect {
            x: area.x,
            y: area.y + top_offset,
            width: area.width,
            height: area.height.saturating_sub(top_offset + bottom_offset).max(1),
        };
        (top_offset, bottom_offset, ws)
    }

    #[test]
    fn matches_pre_phase46_formula_for_every_size_and_flag_combo() {
        for menubar in [false, true] {
            for fkey in [false, true] {
                let cfg = BiltermConfig {
                    show_top_menubar: menubar,
                    show_dos_fkey_bar: fkey,
                    ..BiltermConfig::default()
                };
                for h in 0..=40u16 {
                    for (x, y, w) in [(0u16, 0u16, 80u16), (3, 2, 1), (0, 0, 0)] {
                        let area = Rect { x, y, width: w, height: h };
                        let (t, b, ws) = legacy_reference(area, menubar, fkey);
                        let got = ShellLayout::compute(area, &cfg);
                        assert_eq!(got.workspace, ws, "{area:?} menubar={menubar} fkey={fkey}");
                        assert_eq!(got.top_bar.is_some(), t == 1);
                        assert_eq!(got.bottom_bar.is_some(), b == 1);
                        assert_eq!(got.top_offset(), t);
                        if let Some(bar) = got.bottom_bar {
                            assert_eq!(bar.y, area.y + area.height - 1);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn display_mode_never_changes_legacy_layout() {
        use crate::ui::design::DisplayMode;
        let area = Rect { x: 0, y: 0, width: 100, height: 30 };
        let mut cfg = BiltermConfig {
            show_top_menubar: true,
            show_dos_fkey_bar: true,
            ..BiltermConfig::default()
        };
        let base = ShellLayout::compute(area, &cfg);
        for m in [DisplayMode::Standard, DisplayMode::Focus, DisplayMode::Zen] {
            cfg.appearance.display_mode = m;
            assert_eq!(ShellLayout::compute(area, &cfg), base);
        }
    }
}
