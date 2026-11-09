//! IBM 3270 Operator Information Area (OIA) bottom 1-row status bar renderer (`UI-02`, `SCRL-01..02`).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::pane::{CompositePane, PaneScrollMode, PromptAnchor};
use crate::pty::line_discipline::InputRoutingMode;
use crate::ui::design::{AppearanceProfile, ColorRole, DesignTokens};
use crate::ui::glyphs::GlyphSet;
use crate::ui::theme::ThemePalette;
use crate::vt::Osc9Progress;

#[inline]
fn role_color(tokens: &DesignTokens, is_legacy: bool, role: ColorRole, legacy: Color) -> Color {
    if is_legacy {
        legacy
    } else {
        tokens.get(role)
    }
}

pub fn render_oia_status_bar(pane: &CompositePane, status_rect: Rect, buf: &mut Buffer) {
    render_oia_status_bar_themed(
        pane,
        status_rect,
        &ThemePalette::default(),
        &GlyphSet::default(),
        buf,
    );
}

/// Compact byte-rate label (`12B`, `1.5K`, `2.0M`) for the opt-in OIA traffic indicator.
fn rate_label(bps: u64) -> String {
    if bps >= 1_048_576 {
        format!("{:.1}M", bps as f64 / 1_048_576.0)
    } else if bps >= 1024 {
        format!("{:.1}K", bps as f64 / 1024.0)
    } else {
        format!("{bps}B")
    }
}

pub fn render_oia_status_bar_themed(
    pane: &CompositePane,
    status_rect: Rect,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    if status_rect.width == 0 || status_rect.height == 0 {
        return;
    }

    let oia_bg = palette.oia_bg;
    let sep = glyphs.oia_separator;
    // Tokens apply only to palettes that name a non-Legacy appearance profile.
    // Existing looks (incl. MODERN-SLATE / LIQUID-GLASS) keep their literals byte-identical.
    let uses_tokens = !matches!(palette.name.as_str(), "MODERN-SLATE" | "LIQUID-GLASS")
        && AppearanceProfile::from_name(&palette.name)
            .is_some_and(|p| p != AppearanceProfile::Legacy);
    let is_modern = sep == "·"
        || matches!(palette.name.as_str(), "MODERN-SLATE" | "LIQUID-GLASS")
        || uses_tokens;
    let is_legacy = !uses_tokens;
    let tokens = DesignTokens::from_palette(palette);
    let system_state = if pane.is_password_prompt() {
        (
            format!(" {} !ECHO LOCK ", glyphs.status_lock_icon),
            role_color(&tokens, is_legacy, ColorRole::Danger, Color::Red),
        )
    } else if pane.vt().is_alternate_screen() {
        (
            format!(
                " {} {} ",
                glyphs.status_alt_icon,
                if is_modern { "Alt" } else { "ALT SCR" }
            ),
            role_color(&tokens, is_legacy, ColorRole::Warning, Color::Yellow),
        )
    } else if pane.is_holding_inspected() {
        (
            format!(" {} HOLDING ", glyphs.status_scroll_icon),
            role_color(&tokens, is_legacy, ColorRole::Warning, Color::Rgb(255, 200, 0)),
        )
    } else if pane.is_page_holding() {
        (
            format!(" {} MORE... ", glyphs.status_scroll_icon),
            role_color(&tokens, is_legacy, ColorRole::Warning, Color::Rgb(255, 255, 85)),
        )
    } else if pane.vt().scrollback_offset() > 0 {
        (
            format!(" {} SCROLL ", glyphs.status_scroll_icon),
            role_color(&tokens, is_legacy, ColorRole::Accent, Color::Cyan),
        )
    } else if pane.has_foreground_subcommand() {
        (
            format!(
                " {} {} ",
                glyphs.status_run_icon,
                if is_modern { "Running" } else { "RUNNING" }
            ),
            role_color(&tokens, is_legacy, ColorRole::Accent, Color::Rgb(85, 255, 255)),
        )
    } else {
        (
            format!(
                " {} {} ",
                glyphs.status_ready_icon,
                if is_modern { "Ready" } else { "READY" }
            ),
            role_color(&tokens, is_legacy, ColorRole::Success, Color::Rgb(0, 255, 102)),
        )
    };

    let routing_badge = if pane.copy_mode().active {
        if pane.copy_mode().origin == crate::pane::CopyOrigin::Mouse {
            (
                " INPUT: SELECT [y/Enter copy | Esc] ",
                role_color(&tokens, is_legacy, ColorRole::Warning, Color::Rgb(255, 215, 0)),
            )
        } else if pane.copy_mode().selection_kind == crate::pane::CopySelectionKind::Block
            || pane.copy_mode().rect_mode
        {
            (
                if pane.vt().is_alternate_screen() {
                    " INPUT: RECT [Enter/Esc] "
                } else {
                    " INPUT: RECT [F8/Enter/Esc] "
                },
                role_color(&tokens, is_legacy, ColorRole::Warning, Color::Rgb(255, 215, 0)),
            )
        } else {
            (
                if pane.vt().is_alternate_screen() {
                    " INPUT: CANVAS [Esc] "
                } else {
                    " INPUT: CANVAS [F8/Tab/Esc] "
                },
                role_color(&tokens, is_legacy, ColorRole::Accent, Color::Rgb(85, 255, 255)),
            )
        }
    } else if pane.copy_mode().rect_mode {
        (
            " INPUT: RECT BOX ",
            role_color(&tokens, is_legacy, ColorRole::Warning, Color::Rgb(255, 215, 0)),
        )
    } else if is_modern {
        match pane.effective_routing_mode() {
            InputRoutingMode::CommandPrompt => (
                " ❯ Command ",
                role_color(&tokens, is_legacy, ColorRole::Accent, Color::Rgb(110, 231, 255)),
            ),
            InputRoutingMode::RawPtyPassthrough => (
                " ⇄ Raw PTY ",
                role_color(&tokens, is_legacy, ColorRole::Warning, Color::Rgb(255, 200, 0)),
            ),
        }
    } else {
        match pane.effective_routing_mode() {
            InputRoutingMode::CommandPrompt => (" INPUT: CMD BOX ", Color::Rgb(0, 255, 102)),
            InputRoutingMode::RawPtyPassthrough => (" INPUT: RAW PTY ", Color::Rgb(255, 200, 0)),
        }
    };

    let anchor_label = match pane.prompt().anchor {
        PromptAnchor::Top => if is_modern { "Top" } else { "PIN:TOP" },
        PromptAnchor::Bottom => if is_modern { "Bottom" } else { "PIN:BOT" },
        PromptAnchor::SplitTop => if is_modern { "Split-T" } else { "PIN:SPL-T" },
        PromptAnchor::SplitBottom => if is_modern { "Split-B" } else { "PIN:SPL-B" },
        PromptAnchor::Floating { y_offset } => {
            if y_offset == 0 {
                if is_modern { "Top" } else { "PIN:TOP" }
            } else if is_modern {
                "Float"
            } else {
                "PIN:FLT"
            }
        }
    };

    // Row/Col counter reflects active input focus: canvas cursor in CopyMode, editor cursor in CommandPrompt, VT100 in RawPtyPassthrough
    let (disp_r, disp_c) = if pane.copy_mode().active {
        (
            usize::from(pane.copy_mode().cursor_row) + 1,
            usize::from(pane.copy_mode().cursor_col) + 1,
        )
    } else {
        match pane.effective_routing_mode() {
            InputRoutingMode::CommandPrompt => {
                (pane.prompt().cursor_row + 1, pane.prompt().cursor_col + 1)
            }
            InputRoutingMode::RawPtyPassthrough => {
                let (vr, vc) = pane.vt().cursor_position();
                (usize::from(vr) + 1, usize::from(vc) + 1)
            }
        }
    };
    let coord_str = if is_modern {
        format!(" Ln {disp_r}, Col {disp_c} ")
    } else {
        format!(" {} R:{:02} C:{:03} ", glyphs.cursor_pos_icon, disp_r, disp_c)
    };
    let out_lines = pane.vt().output_line_count();
    let out_bytes = pane.vt().total_bytes_processed();
    let out_bytes_label = if out_bytes >= 1_048_576 {
        format!("{:.1}M", out_bytes as f64 / 1_048_576.0)
    } else if out_bytes >= 1024 {
        format!("{:.1}K", out_bytes as f64 / 1024.0)
    } else {
        format!("{out_bytes}B")
    };
    let out_str = if is_modern {
        format!(" {out_lines}L · {out_bytes_label}")
    } else {
        format!(" {} OUT:{out_lines}L/{out_bytes_label}", glyphs.output_len_icon)
    };
    let out_str = if pane.oia_traffic_enabled() {
        let (rx, tx) = pane.traffic_rates();
        if is_modern {
            format!("{out_str} ↓{}/s ↑{}/s", rate_label(rx), rate_label(tx))
        } else {
            format!("{out_str} RX:{} TX:{}", rate_label(rx), rate_label(tx))
        }
    } else {
        out_str
    };
    let oia_title = if is_modern {
        if palette.name == "LIQUID-GLASS" {
            if pane.show_version_badge() {
                concat!(" 💧 Bilterm v", env!("CARGO_PKG_VERSION"), " ")
            } else {
                " 💧 Bilterm "
            }
        } else if pane.show_version_badge() {
            concat!(" ◢ Bilterm v", env!("CARGO_PKG_VERSION"), " ")
        } else {
            " ◢ Bilterm "
        }
    } else if pane.show_version_badge() {
        concat!("3270-OIA v", env!("CARGO_PKG_VERSION"))
    } else {
        "3270-OIA"
    };

    let mut spans = vec![
        Span::styled(
            oia_title,
            Style::default()
                .fg(palette.oia_badge_fg)
                .bg(palette.oia_badge_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(sep, Style::default().fg(Color::White).bg(oia_bg)),
        Span::styled(
            system_state.0,
            Style::default().fg(system_state.1).bg(oia_bg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(sep, Style::default().fg(Color::White).bg(oia_bg)),
        Span::styled(
            routing_badge.0,
            Style::default().fg(routing_badge.1).bg(oia_bg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{sep} "), Style::default().fg(Color::White).bg(oia_bg)),
        Span::styled(anchor_label, Style::default().fg(Color::White).bg(oia_bg)),
    ];

    if pane.is_read_only() {
        spans.push(Span::styled(
            format!(" {sep}"),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            " 🔒 PROT ",
            Style::default()
                .fg(Color::White)
                .bg(role_color(&tokens, is_legacy, ColorRole::Danger, Color::Red))
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(remote) = pane.remote_target() {
        spans.push(Span::styled(
            format!(" {sep}"),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            format!(" 🌐 SSH:{remote} "),
            Style::default()
                .fg(Color::Black)
                .bg(role_color(&tokens, is_legacy, ColorRole::Accent, Color::Rgb(85, 255, 255)))
                .add_modifier(Modifier::BOLD),
        ));
    }

    if pane.vt().is_recording() {
        spans.push(Span::styled(
            format!(" {sep}"),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            " ⏺ REC ",
            Style::default()
                .fg(Color::White)
                .bg(role_color(&tokens, is_legacy, ColorRole::Danger, Color::Rgb(200, 0, 0)))
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(badge) = pane.stt_badge() {
        spans.push(Span::styled(
            format!(" {sep}"),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        let (badge_fg, badge_bg) = if badge.contains("REC") {
            (
                Color::White,
                role_color(&tokens, is_legacy, ColorRole::Danger, Color::Rgb(220, 0, 0)),
            )
        } else if badge.contains("STT") {
            (
                Color::Black,
                role_color(&tokens, is_legacy, ColorRole::Warning, Color::Rgb(255, 200, 0)),
            )
        } else {
            (
                Color::Black,
                role_color(&tokens, is_legacy, ColorRole::Success, Color::Rgb(0, 255, 102)),
            )
        };
        spans.push(Span::styled(
            format!(" {badge} "),
            Style::default()
                .fg(badge_fg)
                .bg(badge_bg)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if pane.vt().tektronix().active || pane.vt().tektronix().has_visible_content() {
        spans.push(Span::styled(
            format!(" {sep}"),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            " 📈 TEK-4014 ",
            Style::default()
                .fg(Color::Black)
                .bg(role_color(&tokens, is_legacy, ColorRole::Success, Color::Rgb(0, 255, 102)))
                .add_modifier(Modifier::BOLD),
        ));
    } else if pane.canvas_scene().has_visible_content() {
        spans.push(Span::styled(
            format!(" {sep}"),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            " 🎨 CANVAS ",
            Style::default()
                .fg(Color::Black)
                .bg(role_color(&tokens, is_legacy, ColorRole::Accent, Color::Rgb(85, 255, 255)))
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(notif) = pane.vt().notifications().last() {
        spans.push(Span::styled(
            format!(" {sep}"),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            format!(" 🔔 {}: {} ", notif.title, notif.body),
            Style::default()
                .fg(Color::Black)
                .bg(role_color(&tokens, is_legacy, ColorRole::Warning, Color::Rgb(255, 255, 85)))
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(progress) = pane.vt().osc9_progress() {
        let (gauge_str, gauge_fg) = format_osc9_gauge(progress, &tokens, is_legacy);
        spans.push(Span::styled(
            format!(" {sep}"),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            format!(" {gauge_str} "),
            Style::default()
                .fg(gauge_fg)
                .bg(oia_bg)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(block) = pane.vt().selected_command_block() {
        let total = pane.vt().command_blocks().len();
        spans.push(Span::styled(
            format!(" {sep}"),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            format!("BLK {}/{total}", block.index + 1),
            Style::default()
                .fg(Color::Black)
                .bg(role_color(&tokens, is_legacy, ColorRole::Warning, Color::Rgb(255, 255, 85)))
                .add_modifier(Modifier::BOLD),
        ));
    }

    if pane.scroll_mode() != PaneScrollMode::Smooth
        || pane.show_version_badge()
        || pane.scroll_state().explicit_badge
        || pane.scroll_state().is_kinetic_active()
    {
        let scrl_badge = pane.oia_scroll_badge();
        let (scrl_fg, scrl_bg) = if pane.is_page_holding() {
            (
                Color::Black,
                role_color(&tokens, is_legacy, ColorRole::Warning, Color::Rgb(255, 255, 85)),
            )
        } else if pane.scroll_mode() == PaneScrollMode::BlockPage {
            (
                role_color(&tokens, is_legacy, ColorRole::Warning, Color::Rgb(255, 255, 85)),
                oia_bg,
            )
        } else {
            (
                role_color(&tokens, is_legacy, ColorRole::Success, Color::Rgb(0, 255, 102)),
                oia_bg,
            )
        };
        spans.push(Span::styled(
            format!(" {sep} "),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            scrl_badge,
            Style::default()
                .fg(scrl_fg)
                .bg(scrl_bg)
                .add_modifier(Modifier::BOLD),
        ));
    }

    spans.push(Span::styled(
        format!(" {sep}"),
        Style::default().fg(Color::White).bg(oia_bg),
    ));
    spans.push(Span::styled(
        coord_str,
        Style::default()
            .fg(role_color(&tokens, is_legacy, ColorRole::TextMuted, Color::Rgb(255, 255, 85)))
            .bg(oia_bg),
    ));
    spans.push(Span::styled(sep, Style::default().fg(Color::White).bg(oia_bg)));
    spans.push(Span::styled(
        out_str,
        Style::default()
            .fg(role_color(&tokens, is_legacy, ColorRole::TextMuted, Color::Rgb(85, 255, 255)))
            .bg(oia_bg),
    ));

    // Mouse tracking indicator and Copy mode badge
    let is_child_tracking = pane.vt().is_mouse_sgr_or_tracking()
        && pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough;
    if is_child_tracking && !pane.mouse_select_sticky() {
        let used: usize = spans.iter().map(Span::width).sum();
        let avail = usize::from(status_rect.width).saturating_sub(used);
        if avail >= 34 {
            spans.push(Span::styled(format!(" {sep}"), Style::default().fg(Color::White).bg(oia_bg)));
            spans.push(Span::styled(
                " MOUSE->APP (Shift/Alt+drag select) ",
                Style::default()
                    .fg(Color::Black)
                    .bg(role_color(&tokens, is_legacy, ColorRole::Warning, Color::Rgb(255, 200, 0)))
                    .add_modifier(Modifier::BOLD),
            ));
        } else if avail >= 16 {
            spans.push(Span::styled(format!(" {sep}"), Style::default().fg(Color::White).bg(oia_bg)));
            spans.push(Span::styled(
                " MOUSE->APP ",
                Style::default()
                    .fg(Color::Black)
                    .bg(role_color(&tokens, is_legacy, ColorRole::Warning, Color::Rgb(255, 200, 0)))
                    .add_modifier(Modifier::BOLD),
            ));
        }
    } else if pane.mouse_select_sticky() {
        let used: usize = spans.iter().map(Span::width).sum();
        let avail = usize::from(status_rect.width).saturating_sub(used);
        if avail >= 18 {
            spans.push(Span::styled(format!(" {sep}"), Style::default().fg(Color::White).bg(oia_bg)));
            spans.push(Span::styled(
                " MOUSE: SELECT ",
                Style::default()
                    .fg(Color::Black)
                    .bg(role_color(&tokens, is_legacy, ColorRole::Accent, Color::Cyan))
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    // Clickable [COPY] badge
    {
        let used: usize = spans.iter().map(Span::width).sum();
        let avail = usize::from(status_rect.width).saturating_sub(used);
        if avail >= 12 {
            spans.push(Span::styled(format!(" {sep}"), Style::default().fg(Color::White).bg(oia_bg)));
            spans.push(Span::styled(
                if pane.copy_mode().active { " [COPY:ON] " } else { " [COPY] " },
                Style::default()
                    .fg(Color::Black)
                    .bg(role_color(&tokens, is_legacy, ColorRole::Success, Color::Rgb(0, 255, 102)))
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    let leader_hint = if std::env::var("TMUX").is_ok() { "Ctrl+X" } else { "Ctrl+B" };
    let alt_hint = if pane.vt().is_alternate_screen() && !pane.copy_mode().active {
        Some(format!("Copy: {leader_hint} ["))
    } else {
        None
    };
    let display_msg = if pane.copy_mode().active {
        let scope = if pane.vt().is_alternate_screen() {
            "screen only"
        } else {
            "history"
        };
        Some(crate::ui::copy_hints::copy_hint_line(
            pane.copy_mode(),
            scope,
            status_rect.width,
        ))
    } else {
        pane.status_message().map(str::to_string).or(alt_hint)
    };
    if let Some(msg) = display_msg {
        // The message is feedback for the command just run (`:agents list`, ...). Appended last it is
        // pushed off the row on a normal-width window, so when the detail segments leave no room for
        // it, keep only the brand + state head and let the message have the rest of the row.
        let used: usize = spans.iter().map(Span::width).sum();
        let needed = Span::raw(format!(" {sep} {msg}")).width();
        // Legacy keeps its historical layout byte-for-byte (Classic parity goldens)
        // unless Copy Mode is active and needs the row for the hint line.
        if (!is_legacy || pane.copy_mode().active) && used + needed > usize::from(status_rect.width) {
            spans.truncate(3);
        }
        spans.push(Span::styled(
            format!(" {sep} "),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            msg,
            Style::default()
                .fg(role_color(&tokens, is_legacy, ColorRole::Warning, Color::Rgb(255, 255, 85)))
                .bg(oia_bg)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let line = Line::from(spans);

    Block::default()
        .style(Style::default().fg(Color::White).bg(oia_bg))
        .render(status_rect, buf);
    Paragraph::new(line).render(status_rect, buf);
}

fn format_osc9_gauge(
    progress: Osc9Progress,
    tokens: &DesignTokens,
    is_legacy: bool,
) -> (String, Color) {
    let build_bar = |pct: u8| -> String {
        let filled = ((u16::from(pct.min(100)) * 8) / 100) as usize;
        let empty = 8usize.saturating_sub(filled);
        format!(
            "[{}{}] {:>3}%",
            "█".repeat(filled),
            "░".repeat(empty),
            pct.min(100)
        )
    };
    match progress {
        Osc9Progress::Normal(pct) => (
            build_bar(pct),
            role_color(tokens, is_legacy, ColorRole::Success, Color::Rgb(0, 255, 102)),
        ),
        Osc9Progress::Error(pct) => (
            format!("!{}", build_bar(pct)),
            role_color(tokens, is_legacy, ColorRole::Danger, Color::Rgb(255, 85, 85)),
        ),
        Osc9Progress::Paused(pct) => (
            format!("⏸{}", build_bar(pct)),
            role_color(tokens, is_legacy, ColorRole::Warning, Color::Rgb(255, 255, 85)),
        ),
        Osc9Progress::Indeterminate => (
            "[▓▓░░░░▓▓] BUSY".to_string(),
            role_color(tokens, is_legacy, ColorRole::Accent, Color::Rgb(85, 255, 255)),
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OiaLayout {
    pub title_end: usize,
    pub state_end: usize,
    pub routing_end: usize,
    pub pin_end: usize,
    pub prot_end: usize,
}

pub fn oia_layout(pane: &CompositePane) -> OiaLayout {
    let title_w = if pane.show_version_badge() {
        concat!("3270-OIA v", env!("CARGO_PKG_VERSION")).chars().count()
    } else {
        "3270-OIA".chars().count()
    };
    let state_w = if pane.is_password_prompt() {
        15
    } else if pane.vt().is_alternate_screen()
        || pane.is_holding_inspected()
        || pane.is_page_holding()
        || pane.has_foreground_subcommand()
    {
        11
    } else if pane.vt().scrollback_offset() > 0 {
        10
    } else {
        9
    };
    let routing_w = if pane.copy_mode().active { 28 } else { 16 };
    let anchor_w = match pane.prompt().anchor {
        PromptAnchor::SplitTop | PromptAnchor::SplitBottom => 9,
        _ => 7,
    };
    let title_end = title_w;
    let state_end = title_end + 1 + state_w;
    let routing_end = state_end + 1 + routing_w;
    let pin_end = routing_end + 2 + anchor_w;
    let prot_end = if pane.is_read_only() { pin_end + 11 } else { pin_end };
    OiaLayout {
        title_end,
        state_end,
        routing_end,
        pin_end,
        prot_end,
    }
}

/// Computes the exact `Rect` of the OIA microphone/STT badge inside `status_rect`, if present.
pub fn oia_mic_badge_rect(pane: &CompositePane, status_rect: Rect) -> Option<Rect> {
    let badge = pane.stt_badge()?;
    if status_rect.width == 0 || status_rect.height == 0 {
        return None;
    }
    let layout = oia_layout(pane);
    let mut offset = layout.pin_end;
    if pane.is_read_only() {
        offset = layout.prot_end;
    }
    if let Some(remote) = pane.remote_target() {
        offset += 2 + format!(" 🌐 SSH:{remote} ").chars().count();
    }
    if pane.vt().is_recording() {
        offset += 2 + 7;
    }
    offset += 2;
    let badge_text_w = format!(" {badge} ").chars().count();
    let badge_x = status_rect.x.saturating_add(offset as u16);
    Some(Rect::new(badge_x, status_rect.y, badge_text_w as u16, 1))
}

/// Computes the exact `Rect` of the OIA `MOUSE->APP` / `MOUSE: SELECT` badge.
pub fn oia_mouse_badge_rect(pane: &CompositePane, status_rect: Rect) -> Option<Rect> {
    if status_rect.width == 0 || status_rect.height == 0 {
        return None;
    }
    let is_child_tracking = pane.vt().is_mouse_sgr_or_tracking()
        && pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough;
    if !is_child_tracking && !pane.mouse_select_sticky() {
        return None;
    }
    let layout = oia_layout(pane);
    let mut offset = layout.pin_end;
    if pane.is_read_only() {
        offset = layout.prot_end;
    }
    if let Some(remote) = pane.remote_target() {
        offset += 2 + format!(" 🌐 SSH:{remote} ").chars().count();
    }
    if pane.vt().is_recording() {
        offset += 2 + 7;
    }
    offset += 2;
    if let Some(badge) = pane.stt_badge() {
        offset += format!(" {badge} ").chars().count() + 2;
    }
    offset += 28;
    let badge_text_w = if pane.mouse_select_sticky() {
        " MOUSE: SELECT ".chars().count()
    } else {
        " MOUSE->APP ".chars().count()
    };
    let badge_x = status_rect.x.saturating_add(offset as u16);
    Some(Rect::new(badge_x, status_rect.y, badge_text_w as u16, 1))
}

/// Computes the exact `Rect` of the OIA `[COPY]` badge.
pub fn oia_copy_badge_rect(pane: &CompositePane, status_rect: Rect) -> Option<Rect> {
    if status_rect.width == 0 || status_rect.height == 0 {
        return None;
    }
    let layout = oia_layout(pane);
    let mut offset = layout.pin_end;
    if pane.is_read_only() {
        offset = layout.prot_end;
    }
    if let Some(remote) = pane.remote_target() {
        offset += 2 + format!(" 🌐 SSH:{remote} ").chars().count();
    }
    if pane.vt().is_recording() {
        offset += 2 + 7;
    }
    offset += 2;
    if let Some(badge) = pane.stt_badge() {
        offset += format!(" {badge} ").chars().count() + 2;
    }
    offset += 28;
    if pane.vt().is_mouse_sgr_or_tracking() || pane.mouse_select_sticky() {
        offset += 16;
    }
    let badge_text_w = 8;
    let badge_x = status_rect.x.saturating_add(offset as u16);
    Some(Rect::new(badge_x, status_rect.y, badge_text_w as u16, 1))
}
