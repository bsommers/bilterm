//! Keyboard-owner badge (`FOCUS-01`): while a side window holds the keyboard, the status row of the
//! top pane shows `KBD:<WINDOW>` at its right edge, so it is visible that typing does not reach
//! the shell. Nothing is drawn while the pane owns the keyboard.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use crate::wm::keyboard_focus::KeyboardOwner;

/// The badge text for `owner`; `None` for the pane.
pub fn keyboard_badge_text(owner: KeyboardOwner) -> Option<String> {
    (!owner.is_pane()).then(|| format!(" KBD:{} ", owner.badge()))
}

/// Draws the badge right-aligned on `status_rect` (one row), reversed and bold over whatever the
/// row shows. Skipped when the row is too narrow to keep its left-hand fields readable.
pub fn render_keyboard_badge(owner: KeyboardOwner, status_rect: Rect, buf: &mut Buffer) {
    let Some(text) = keyboard_badge_text(owner) else { return };
    let width = text.chars().count() as u16;
    if status_rect.height == 0 || status_rect.width < width.saturating_add(8) {
        return;
    }
    let x0 = status_rect.right() - width;
    for (i, ch) in text.chars().enumerate() {
        if let Some(cell) = buf.cell_mut((x0 + i as u16, status_rect.y)) {
            cell.set_char(ch);
            cell.modifier.insert(Modifier::REVERSED | Modifier::BOLD);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pane_has_no_badge_and_a_window_is_named() {
        assert_eq!(keyboard_badge_text(KeyboardOwner::Pane), None);
        assert_eq!(keyboard_badge_text(KeyboardOwner::AgentsList).as_deref(), Some(" KBD:AGENT-LIST "));
    }

    #[test]
    fn the_badge_is_right_aligned_and_skipped_on_a_narrow_row() {
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        render_keyboard_badge(KeyboardOwner::Drawer, area, &mut buf);
        let row: String = (0..40).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(row.ends_with(" KBD:DRAWER "), "{row:?}");
        let narrow = Rect::new(0, 0, 14, 1);
        let mut buf = Buffer::empty(narrow);
        render_keyboard_badge(KeyboardOwner::Drawer, narrow, &mut buf);
        assert!((0..14).all(|x| buf[(x, 0)].symbol() == " "));
    }
}
