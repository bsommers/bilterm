//! IBM 3270 Operator Information Area (OIA) bottom 1-row status bar renderer (`UI-02`, `SCRL-01..02`).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::pane::{CompositePane, PaneScrollMode, PromptAnchor};
use crate::pty::line_discipline::InputRoutingMode;
use crate::ui::glyphs::GlyphSet;
use crate::ui::theme::ThemePalette;
use crate::vt::Osc9Progress;

pub fn render_oia_status_bar(pane: &CompositePane, status_rect: Rect, buf: &mut Buffer) {
    render_oia_status_bar_themed(
        pane,
        status_rect,
        &ThemePalette::default(),
        &GlyphSet::default(),
        buf,
    );
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
    let system_state = if pane.is_password_prompt() {
        (format!(" {} !ECHO LOCK ", glyphs.status_lock_icon), Color::Red)
    } else if pane.vt().is_alternate_screen() {
        (format!(" {} ALT SCR ", glyphs.status_alt_icon), Color::Yellow)
    } else if pane.is_holding_inspected() {
        (
            format!(" {} HOLDING ", glyphs.status_scroll_icon),
            Color::Rgb(255, 200, 0),
        )
    } else if pane.is_page_holding() {
        (
            format!(" {} MORE... ", glyphs.status_scroll_icon),
            Color::Rgb(255, 255, 85),
        )
    } else if pane.vt().scrollback_offset() > 0 {
        (format!(" {} SCROLL ", glyphs.status_scroll_icon), Color::Cyan)
    } else if pane.has_foreground_subcommand() {
        (
            format!(" {} RUNNING ", glyphs.status_run_icon),
            Color::Rgb(85, 255, 255),
        )
    } else {
        (
            format!(" {} READY ", glyphs.status_ready_icon),
            Color::Rgb(0, 255, 102),
        )
    };

    let routing_badge = if pane.copy_mode().active {
        (" INPUT: CANVAS [F8/Tab/Esc] ", Color::Rgb(85, 255, 255))
    } else {
        match pane.effective_routing_mode() {
            InputRoutingMode::CommandPrompt => (" INPUT: CMD BOX ", Color::Rgb(0, 255, 102)),
            InputRoutingMode::RawPtyPassthrough => (" INPUT: RAW PTY ", Color::Rgb(255, 200, 0)),
        }
    };

    let anchor_label = match pane.prompt().anchor {
        PromptAnchor::Top => "PIN:TOP",
        PromptAnchor::Bottom => "PIN:BOT",
        PromptAnchor::SplitTop => "PIN:SPL-T",
        PromptAnchor::SplitBottom => "PIN:SPL-B",
        PromptAnchor::Floating { y_offset } => {
            if y_offset == 0 {
                "PIN:TOP"
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
    let coord_str = format!(" {} R:{:02} C:{:03} ", glyphs.cursor_pos_icon, disp_r, disp_c);
    let out_lines = pane.vt().output_line_count();
    let out_bytes = pane.vt().total_bytes_processed();
    let out_bytes_label = if out_bytes >= 1_048_576 {
        format!("{:.1}M", out_bytes as f64 / 1_048_576.0)
    } else if out_bytes >= 1024 {
        format!("{:.1}K", out_bytes as f64 / 1024.0)
    } else {
        format!("{out_bytes}B")
    };
    let out_str = format!(" {} OUT:{out_lines}L/{out_bytes_label}", glyphs.output_len_icon);
    let oia_title = if pane.show_version_badge() {
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
                .bg(Color::Red)
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
                .bg(Color::Rgb(85, 255, 255))
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
                .bg(Color::Rgb(200, 0, 0))
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
                .bg(Color::Rgb(0, 255, 102))
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
                .bg(Color::Rgb(85, 255, 255))
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
                .bg(Color::Rgb(255, 255, 85))
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(progress) = pane.vt().osc9_progress() {
        let (gauge_str, gauge_fg) = format_osc9_gauge(progress);
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
                .bg(Color::Rgb(255, 255, 85))
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
            (Color::Black, Color::Rgb(255, 255, 85))
        } else if pane.scroll_mode() == PaneScrollMode::BlockPage {
            (Color::Rgb(255, 255, 85), oia_bg)
        } else {
            (Color::Rgb(0, 255, 102), oia_bg)
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
        Style::default().fg(Color::Rgb(255, 255, 85)).bg(oia_bg),
    ));
    spans.push(Span::styled(sep, Style::default().fg(Color::White).bg(oia_bg)));
    spans.push(Span::styled(
        out_str,
        Style::default().fg(Color::Rgb(85, 255, 255)).bg(oia_bg),
    ));

    if let Some(msg) = pane.status_message() {
        spans.push(Span::styled(
            format!(" {sep} "),
            Style::default().fg(Color::White).bg(oia_bg),
        ));
        spans.push(Span::styled(
            msg.to_string(),
            Style::default()
                .fg(Color::Rgb(255, 255, 85))
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

fn format_osc9_gauge(progress: Osc9Progress) -> (String, Color) {
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
        Osc9Progress::Normal(pct) => (build_bar(pct), Color::Rgb(0, 255, 102)),
        Osc9Progress::Error(pct) => (format!("!{}", build_bar(pct)), Color::Rgb(255, 85, 85)),
        Osc9Progress::Paused(pct) => (format!("⏸{}", build_bar(pct)), Color::Rgb(255, 255, 85)),
        Osc9Progress::Indeterminate => ("[▓▓░░░░▓▓] BUSY".to_string(), Color::Rgb(85, 255, 255)),
    }
}
