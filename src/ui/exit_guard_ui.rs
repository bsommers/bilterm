//! Borland Turbo Vision / IBM 3270 Exit Guard modal & held-shell banner rendering (`SAFE-01`).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::pane::exit_guard::{ExitGuardChoice, ExitGuardState, HeldExitedShellState};
use crate::ui::chrome::{ShadowStyle, render_drop_shadow_styled};
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;

/// Renders the Borland Turbo Vision / IBM 3270 Exit Guard modal (`SAFE-01`).
pub fn render_exit_guard_modal(
    buf: &mut Buffer,
    area: Rect,
    palette: &ThemePalette,
    state: &ExitGuardState,
) {
    let clamped_area = area.intersection(buf.area);
    if clamped_area.width < 10 || clamped_area.height < 5 {
        return;
    }

    let max_w = clamped_area.width.saturating_sub(2).max(10);
    let modal_w = 78u16.clamp(54u16.min(max_w), max_w);
    let max_h = clamped_area.height.saturating_sub(1).max(5);
    let modal_h = 11u16.clamp(9u16.min(max_h), max_h);

    let modal_x = clamped_area.x + (clamped_area.width.saturating_sub(modal_w)) / 2;
    let modal_y = clamped_area.y + (clamped_area.height.saturating_sub(modal_h)) / 2;
    let modal_rect = Rect::new(modal_x, modal_y, modal_w, modal_h);

    occlude_rect(buf, modal_rect);
    render_drop_shadow_styled(buf, modal_rect, clamped_area, ShadowStyle::ThemeTinted, palette);

    let frame_bg = palette.window_frame_bg;
    let title_style = Style::default()
        .fg(palette.title_fg)
        .bg(frame_bg)
        .add_modifier(Modifier::BOLD);
    let border_style = Style::default()
        .fg(palette.active_border_fg)
        .bg(frame_bg)
        .add_modifier(Modifier::BOLD);
    let block = Block::default()
        .title(Span::styled(
            " ⚠ IBM 3270 / BORLAND EXIT GUARD [SAFE-01] ",
            title_style,
        ))
        .borders(Borders::ALL)
        .border_set(border::DOUBLE)
        .border_style(border_style)
        .style(Style::default().fg(Color::White).bg(frame_bg));

    let inner = block.inner(modal_rect);
    block.render(modal_rect, buf);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let session_kind = if state.is_root_session {
        "ROOT SESSION"
    } else {
        "FINAL PANE"
    };
    let selected_idx = state.selected_index.min(ExitGuardChoice::ALL.len() - 1);
    let badge_style = Style::default()
        .fg(palette.oia_badge_fg)
        .bg(palette.oia_badge_bg)
        .add_modifier(Modifier::BOLD);
    let unsel_style = Style::default()
        .fg(palette.title_fg)
        .bg(palette.prompt_surface_bg);

    let mut button_spans = Vec::with_capacity(ExitGuardChoice::ALL.len() * 2);
    for (idx, choice) in ExitGuardChoice::ALL.iter().copied().enumerate() {
        if idx > 0 {
            button_spans.push(Span::styled(" ", Style::default().bg(frame_bg)));
        }
        let style = if idx == selected_idx {
            badge_style
        } else {
            unsel_style
        };
        button_spans.push(Span::styled(choice.button_label(), style));
    }

    let lines = vec![
        Line::from(vec![
            Span::styled(format!(" [{session_kind}] "), badge_style),
            Span::styled(
                format!(" Session '{}' (Pane #{})", state.session_name, state.pane_id),
                title_style,
            ),
        ]),
        Line::from(Span::styled(
            format!(" Trigger: {}", state.trigger_reason),
            Style::default().fg(Color::White).bg(frame_bg),
        )),
        Line::from(Span::styled(
            " Closing this shell would terminate your active workspace.",
            Style::default().fg(palette.inactive_border_fg).bg(frame_bg),
        )),
        Line::from(""),
        Line::from(button_spans),
        Line::from(Span::styled(
            " Keys: ←/→/Tab select | Enter/r respawn | d detach | q quit | K kill | Esc cancel",
            Style::default().fg(palette.prompt_prefix_fg).bg(frame_bg),
        )),
    ];

    Paragraph::new(lines)
        .style(Style::default().fg(Color::White).bg(frame_bg))
        .render(inner, buf);
}

/// Renders a high-contrast `[SHELL EXITED]` hold-on-exit status banner inside `inner` (`SAFE-01`).
pub fn render_held_shell_exited_banner(
    buf: &mut Buffer,
    inner: Rect,
    palette: &ThemePalette,
    held: &HeldExitedShellState,
) {
    let clamped = inner.intersection(buf.area);
    if clamped.width == 0 || clamped.height == 0 {
        return;
    }

    let banner_y = clamped.y + clamped.height - 1;
    let banner_rect = Rect::new(clamped.x, banner_y, clamped.width, 1);
    let banner_style = Style::default()
        .fg(palette.oia_badge_fg)
        .bg(palette.title_fg)
        .add_modifier(Modifier::BOLD);

    for x in banner_rect.x..banner_rect.x + banner_rect.width {
        if let Some(cell) = buf.cell_mut((x, banner_y)) {
            cell.set_symbol(" ");
            cell.set_style(banner_style);
        }
    }

    Paragraph::new(Line::from(Span::styled(
        format!(" {} ", held.banner_message),
        banner_style,
    )))
    .render(banner_rect, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::ThemePreset;

    #[test]
    fn render_exit_guard_modal_and_banner_are_panic_free_from_0x0_to_80x24() {
        let palette = ThemePreset::TurboBlue.palette();
        let state = ExitGuardState::new(1, "root", true, "Ctrl+D EOF on root session");
        let held = HeldExitedShellState::new(Some(0));

        for (w, h) in [(0, 0), (1, 1), (9, 4), (10, 5), (40, 10), (80, 24)] {
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            render_exit_guard_modal(&mut buf, area, &palette, &state);
            render_held_shell_exited_banner(&mut buf, area, &palette, &held);
        }

        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        render_exit_guard_modal(&mut buf, area, &palette, &state);
        let rendered: String = (0..24)
            .map(|y| (0..80).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("EXIT GUARD"));
        assert!(rendered.contains("(r)espawn shell"));
    }
}
