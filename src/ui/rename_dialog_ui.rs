//! Interactive TUI Rename / Move Popup Dialog renderer and hit-testing (`SES-01..05`).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::occlusion::occlude_rect;
use crate::ui::visual_style::{
    apply_frosted_glass_surface, polish_glass_panel_rect, render_liquid_ambient_halo,
};
use crate::wm::{inset_window_frame, rect_contains, WindowManager};

/// Hit-test target inside the interactive Rename / Move dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenameDialogHit {
    /// Top-right `[X]` close button.
    Close,
    /// Click inside the editable input box at character offset `0..`.
    InputField(usize),
    /// `[ Enter: Save ]` button.
    SaveButton,
    /// `[ Esc: Cancel ]` button.
    CancelButton,
    /// `[ Ctrl+U: Clear ]` button.
    ClearButton,
}

/// Computes the centered bounding [`Rect`] of the Rename / Move popup dialog within `area`.
pub fn rename_dialog_rect(area: Rect) -> Rect {
    let w = 56.min(area.width.saturating_sub(2)).max(1);
    let h = 8.min(area.height.saturating_sub(2)).max(1);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect::new(x, y, w, h)
}

/// Returns the `(save_rect, cancel_rect, clear_rect)` button rectangles inside `dialog_rect`.
pub fn rename_dialog_button_rects(dialog_rect: Rect) -> (Rect, Rect, Rect) {
    let btn_y = (dialog_rect.y + 6).min(dialog_rect.bottom().saturating_sub(2));
    let save_rect = Rect::new(dialog_rect.x + 2, btn_y, 15, 1);
    let cancel_rect = Rect::new(dialog_rect.x + 19, btn_y, 15, 1);
    let clear_rect = Rect::new(dialog_rect.x + 36, btn_y, 17, 1);
    (save_rect, cancel_rect, clear_rect)
}

/// Hit-tests `(col, row)` inside `dialog_rect`.
pub fn hit_test_rename_dialog(dialog_rect: Rect, col: u16, row: u16) -> Option<RenameDialogHit> {
    if !rect_contains(dialog_rect, col, row) {
        return None;
    }
    if row == dialog_rect.y && col >= dialog_rect.right().saturating_sub(6) {
        return Some(RenameDialogHit::Close);
    }
    let (save_r, cancel_r, clear_r) = rename_dialog_button_rects(dialog_rect);
    if rect_contains(save_r, col, row) {
        return Some(RenameDialogHit::SaveButton);
    }
    if rect_contains(cancel_r, col, row) {
        return Some(RenameDialogHit::CancelButton);
    }
    if rect_contains(clear_r, col, row) {
        return Some(RenameDialogHit::ClearButton);
    }
    if row >= dialog_rect.y + 2 && row <= dialog_rect.y + 4 {
        let text_start = dialog_rect.x + 4;
        let offset = usize::from(col.saturating_sub(text_start));
        return Some(RenameDialogHit::InputField(offset));
    }
    None
}

fn put_str_clipped(buf: &mut Buffer, x: u16, y: u16, max_w: u16, text: &str, style: Style) {
    if max_w == 0 || y >= buf.area.bottom() {
        return;
    }
    let mut col = x;
    let limit = (x + max_w).min(buf.area.right());
    for ch in text.chars() {
        if col >= limit {
            break;
        }
        if let Some(cell) = buf.cell_mut((col, y)) {
            let mut tmp = [0u8; 4];
            cell.set_symbol(ch.encode_utf8(&mut tmp));
            cell.set_style(style);
        }
        col += 1;
    }
}

/// Renders the interactive Rename / Move popup dialog on top of `buf` when open.
pub fn render_rename_dialog(wm: &WindowManager, area: Rect, buf: &mut Buffer) {
    let Some(dialog) = wm.rename_dialog() else {
        return;
    };
    if area.width < 20 || area.height < 6 {
        return;
    }

    let cfg = wm.config();
    let palette = cfg.palette();
    let glyphs = cfg.glyph_set();
    let rect = rename_dialog_rect(area);

    if cfg.liquid_glass {
        render_liquid_ambient_halo(buf, rect, area, &palette);
    } else if cfg.drop_shadows {
        render_drop_shadow_styled(buf, rect, area, cfg.shadow_style, &palette);
    }
    occlude_rect(buf, rect);
    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, rect, palette.window_frame_bg, &palette);
    }

    render_borland_frame_styled(
        rect,
        &dialog.title,
        true,
        cfg.window_outline,
        &palette,
        &glyphs,
        buf,
    );

    let inner = inset_window_frame(rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let base_style = Style::default()
        .fg(palette.text_fg.unwrap_or(palette.title_fg))
        .bg(palette.window_frame_bg);
    if !cfg.is_glass() {
        for y in inner.y..inner.bottom() {
            for x in inner.x..inner.right() {
                if let Some(c) = buf.cell_mut((x, y)) {
                    c.set_symbol(" ");
                    c.set_style(base_style);
                }
            }
        }
    }

    // Top-right [X] close badge on border
    if rect.width >= 12 {
        let close_x = rect.right().saturating_sub(5);
        let close_style = Style::default()
            .fg(palette.active_border_fg)
            .bg(palette.window_frame_bg)
            .add_modifier(Modifier::BOLD);
        put_str_clipped(buf, close_x, rect.y, 3, "[X]", close_style);
    }

    // Row 1: Subtitle
    let sub_style = Style::default()
        .fg(palette.title_fg)
        .bg(palette.window_frame_bg);
    put_str_clipped(
        buf,
        inner.x + 1,
        rect.y + 1,
        inner.width.saturating_sub(2),
        &dialog.subtitle,
        sub_style,
    );

    // Rows 2..=4: 3-row boxed editable input field
    if rect.height >= 6 && inner.width >= 6 {
        let box_x = rect.x + 2;
        let box_w = rect.width.saturating_sub(4);
        let border_style = Style::default()
            .fg(palette.active_border_fg)
            .bg(palette.window_frame_bg);
        let field_bg = Color::Rgb(16, 24, 40);
        let field_style = Style::default()
            .fg(Color::White)
            .bg(field_bg)
            .add_modifier(Modifier::BOLD);

        let top_y = rect.y + 2;
        let mid_y = rect.y + 3;
        let bot_y = rect.y + 4;

        // Top box border: ┌───┐
        put_str_clipped(buf, box_x, top_y, 1, "┌", border_style);
        for x in (box_x + 1)..(box_x + box_w.saturating_sub(1)) {
            put_str_clipped(buf, x, top_y, 1, "─", border_style);
        }
        put_str_clipped(
            buf,
            box_x + box_w.saturating_sub(1),
            top_y,
            1,
            "┐",
            border_style,
        );

        // Middle input row: │ text█ │
        put_str_clipped(buf, box_x, mid_y, 1, "│", border_style);
        for x in (box_x + 1)..(box_x + box_w.saturating_sub(1)) {
            if let Some(cell) = buf.cell_mut((x, mid_y)) {
                cell.set_symbol(" ");
                cell.set_style(field_style);
            }
        }
        put_str_clipped(
            buf,
            box_x + box_w.saturating_sub(1),
            mid_y,
            1,
            "│",
            border_style,
        );

        let text_x = box_x + 2;
        let text_max_w = box_w.saturating_sub(4);
        let chars: Vec<char> = dialog.current_input.chars().collect();
        let cursor = dialog.cursor.min(chars.len());
        let visible_cap = usize::from(text_max_w.saturating_sub(1)).max(1);
        let scroll = cursor.saturating_sub(visible_cap.saturating_sub(1));

        for (i, &ch) in chars.iter().skip(scroll).take(usize::from(text_max_w)).enumerate() {
            let cx = text_x + i as u16;
            if let Some(cell) = buf.cell_mut((cx, mid_y)) {
                let mut tmp = [0u8; 4];
                cell.set_symbol(ch.encode_utf8(&mut tmp));
                cell.set_style(field_style);
            }
        }

        // Render visible block/reversed caret at cursor position
        let caret_col = text_x + (cursor.saturating_sub(scroll) as u16).min(text_max_w.saturating_sub(1));
        if let Some(cell) = buf.cell_mut((caret_col, mid_y)) {
            let caret_ch = chars.get(cursor).copied().unwrap_or('█');
            let mut tmp = [0u8; 4];
            cell.set_symbol(caret_ch.encode_utf8(&mut tmp));
            if cursor < chars.len() {
                cell.set_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(palette.active_border_fg)
                        .add_modifier(Modifier::BOLD),
                );
            } else {
                cell.set_style(
                    Style::default()
                        .fg(palette.active_border_fg)
                        .bg(field_bg)
                        .add_modifier(Modifier::BOLD),
                );
            }
        }

        // Bottom box border: └───┘
        put_str_clipped(buf, box_x, bot_y, 1, "└", border_style);
        for x in (box_x + 1)..(box_x + box_w.saturating_sub(1)) {
            put_str_clipped(buf, x, bot_y, 1, "─", border_style);
        }
        put_str_clipped(
            buf,
            box_x + box_w.saturating_sub(1),
            bot_y,
            1,
            "┘",
            border_style,
        );
    }

    // Row 6: Action buttons
    let (save_r, cancel_r, clear_r) = rename_dialog_button_rects(rect);
    let save_style = Style::default()
        .fg(Color::Black)
        .bg(palette.active_border_fg)
        .add_modifier(Modifier::BOLD);
    let btn_style = Style::default()
        .fg(palette.title_fg)
        .bg(palette.oia_bg)
        .add_modifier(Modifier::BOLD);

    put_str_clipped(buf, save_r.x, save_r.y, save_r.width, "[ Enter: Save ]", save_style);
    if cancel_r.right() <= rect.right().saturating_sub(1) {
        put_str_clipped(
            buf,
            cancel_r.x,
            cancel_r.y,
            cancel_r.width,
            "[ Esc: Cancel ]",
            btn_style,
        );
    }
    if clear_r.right() <= rect.right().saturating_sub(1) {
        put_str_clipped(
            buf,
            clear_r.x,
            clear_r.y,
            clear_r.width,
            "[ Ctrl+U: Clear ]",
            btn_style,
        );
    }

    if cfg.is_glass() {
        polish_glass_panel_rect(buf, rect, palette.window_frame_bg, &palette, true);
    }
}
