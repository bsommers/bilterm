//! Contextual pane action strip (46-09): small `split / zoom / close` buttons on the focused pane's top border.
//!
//! Non-Legacy looks in Standard display mode only. Each button lowers to the same `Action` as its key or
//! `:` command (`WindowManager::apply`), so nothing here is a new code path. Layout and hit-testing share
//! `buttons` so they cannot disagree. Legacy, Minimal and Zen render exactly as before.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::action::Action;
use crate::config::BiltermConfig;
use crate::ui::design::{AppearanceProfile, ColorRole, DesignTokens, DisplayMode};
use crate::ui::shell::effective_display_mode;
use crate::ui::theme::ThemePalette;

/// Narrowest pane outer width that still shows the strip.
const MIN_WIDTH: u16 = 28;

/// `(absolute column, glyph, action)` for each button on the top border row of `outer`.
pub fn buttons(outer: Rect) -> Vec<(u16, &'static str, Action)> {
    if outer.width < MIN_WIDTH || outer.height < 3 {
        return Vec::new();
    }
    let right = outer.x + outer.width - 1;
    vec![
        (right - 6, "⊞", Action::SplitVertical),
        (right - 4, "⤢", Action::ZoomToggle),
        (right - 2, "✕", Action::ClosePane),
    ]
}

/// Whether the strip applies for this configuration.
pub fn enabled(cfg: &BiltermConfig) -> bool {
    AppearanceProfile::resolve(cfg) != AppearanceProfile::Legacy && effective_display_mode(cfg) == DisplayMode::Standard
}

/// Action under absolute (`col`, `row`) for the pane whose frame is `outer`, if a button is there.
pub fn action_at(cfg: &BiltermConfig, outer: Rect, col: u16, row: u16) -> Option<Action> {
    if !enabled(cfg) || row != outer.y {
        return None;
    }
    buttons(outer).into_iter().find(|(x, ..)| *x == col).map(|(.., a)| a)
}

/// Draws the strip over the top border of the focused pane.
pub fn render(cfg: &BiltermConfig, outer: Rect, palette: &ThemePalette, buf: &mut Buffer) {
    if !enabled(cfg) {
        return;
    }
    let t = DesignTokens::from_palette(palette);
    let style = Style::default()
        .fg(t.get(ColorRole::TextMuted))
        .bg(palette.window_frame_bg)
        .add_modifier(Modifier::BOLD);
    for (x, glyph, action) in buttons(outer) {
        let s = if matches!(action, Action::ClosePane) { style.fg(t.get(ColorRole::Danger)) } else { style };
        buf.set_stringn(x, outer.y, glyph, 1, s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_panes_have_no_buttons_and_wide_ones_have_three() {
        assert!(buttons(Rect::new(0, 0, 20, 10)).is_empty());
        let b = buttons(Rect::new(10, 2, 40, 10));
        assert_eq!(b.len(), 3);
        assert!(b.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(b.iter().all(|(x, ..)| *x > 10 && *x < 49));
    }
}
