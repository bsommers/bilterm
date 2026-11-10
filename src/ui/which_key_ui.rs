//! Which-key rows, footer line and popup panel (`KEY-01`, Phase 63-02).
//!
//! One generator ([`rows_for`]) reads the live tables (the leader rows of the effective keymap and
//! `WINDOW_MODE_BINDINGS`); the leader status hint, the Window-mode footer and the popup panel all
//! render from it, so no key text is hand-written. The popup is a ratatui widget drawn into the
//! workspace cell buffer, which the TUI shows directly and the GUI rasterises like every other
//! overlay, so there is a single code path for both surfaces.

use std::sync::OnceLock;

use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::action::{Action, WinSub, WindowModeOp};
use crate::config::BiltermConfig;
use crate::config::keys::KeysConfig;
use crate::input::keyenv::KeyEnv;
use crate::input::keyprofile::{EffectiveKeymap, KeyProfile, WhichKeyRow, leader_key_label};
use crate::input::window_mode_tables::{
    WHICH_KEY_PIN, WinScope, footer_actions, rows_for_scope,
};
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::design::{AppearanceProfile, ColorRole, DesignTokens};
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{apply_frosted_glass_surface, render_liquid_ambient_halo};
use crate::wm::{KeyMode, WindowManager, WorkspaceLayoutMode, inset_window_frame};

/// Leader actions shown in the one-line leader hint, in display order.
const LEADER_FOOTER_ACTIONS: &[Action] = &[
    Action::SplitHorizontal,
    Action::SplitVertical,
    Action::ZoomToggle,
    Action::DeckNext,
    Action::DrawerToggle,
    Action::WindowMode(WindowModeOp::Enter),
    Action::MenuOpen { category: None },
    Action::CopyModeToggle,
    Action::PaletteToggle,
    Action::HelpToggle,
];

/// Short label of `action` for footers and popup rows (local map; `Action` has no `label()`).
pub fn short_label(action: &Action) -> String {
    let dir = |dx: i16, dy: i16| match (dx.signum(), dy.signum()) {
        (-1, _) => "left",
        (1, _) => "right",
        (_, -1) => "up",
        _ => "down",
    };
    match action {
        Action::SplitHorizontal => "SplitH".into(),
        Action::SplitVertical => "SplitV".into(),
        Action::ZoomToggle => "Zoom".into(),
        Action::DeckNext => "Deck".into(),
        Action::DeckPrev => "Deck back".into(),
        Action::DrawerToggle => "Drawer".into(),
        Action::DrawerFocus => "Drawer focus".into(),
        Action::WindowMode(WindowModeOp::Enter | WindowModeOp::Toggle) => "Window".into(),
        Action::WindowMode(WindowModeOp::Exit) => "Exit".into(),
        Action::WindowMode(WindowModeOp::Sub(sub)) => format!("{} mode", sub.name()),
        Action::MenuOpen { .. } => "Menu".into(),
        Action::CopyModeToggle => "Copy".into(),
        Action::PaletteToggle => "Palette".into(),
        Action::HelpToggle => "Help".into(),
        Action::ClosePane => "Close".into(),
        Action::LayoutMode(WorkspaceLayoutMode::Stacked) => "Stack".into(),
        Action::LayoutMode(WorkspaceLayoutMode::Tiled) => "Tile".into(),
        Action::LayoutCycle => "Layout".into(),
        Action::FocusPane(n) => format!("Pane {n}"),
        Action::PaneFocusDirection { dx, dy } => format!("Focus {}", dir(*dx, *dy)),
        Action::PaneResize { dw, dh } => format!("Resize {}", dir(*dw, *dh)),
        Action::PaneMove { dx, dy } => format!("Move {}", dir(*dx, *dy)),
        Action::AgentsList(_) => "Agents".into(),
        Action::AgentsHud(_) => "HUD".into(),
        Action::Find(_) => "Find".into(),
        other => catalog_label(other),
    }
}

/// Label for actions without a hand-picked short name: the palette-catalog command that lowers to
/// the same action (`sessions map toggle`), else the humanised variant name (`Drawer nav`).
fn catalog_label(action: &Action) -> String {
    static CATALOG: OnceLock<Vec<(Action, String)>> = OnceLock::new();
    let catalog = CATALOG.get_or_init(|| {
        let mut out: Vec<(Action, String)> = Vec::new();
        for (command, _, _) in crate::ui::modals::PALETTE_CATALOG {
            if let Some(Ok(parsed)) = crate::colon_command::parse_colon_command(command)
                && !out.iter().any(|(a, _)| *a == parsed)
            {
                out.push((parsed, command.trim_start_matches(':').to_string()));
            }
        }
        out
    });
    if let Some((_, label)) = catalog.iter().find(|(a, _)| a == action) {
        return label.clone();
    }
    let debug = format!("{action:?}");
    let name: String = debug.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push(' ');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn group_of(action: &Action) -> &'static str {
    match action {
        Action::FocusPane(_) | Action::PaneFocusDirection { .. } | Action::DeckNext | Action::DeckPrev => "focus",
        Action::SplitHorizontal
        | Action::SplitVertical
        | Action::ZoomToggle
        | Action::ClosePane
        | Action::LayoutMode(_)
        | Action::LayoutCycle
        | Action::PaneResize { .. }
        | Action::PaneMove { .. } => "layout",
        Action::WindowMode(_) | Action::MenuOpen { .. } | Action::PaletteToggle | Action::HelpToggle => "mode",
        Action::DrawerToggle | Action::DrawerFocus | Action::DrawerLayout(_) => "drawer",
        _ => "other",
    }
}

/// Aliases listed per popup row (the rest stay reachable, they are just not advertised).
const MAX_ALIASES: usize = 4;

/// Groups `(key, action)` pairs by distinct action (first-seen order), listing aliases with the
/// shortest first (`z / Z / F5`).
fn collapse(pairs: impl Iterator<Item = (KeyCode, Action)>, group: Option<&'static str>) -> Vec<WhichKeyRow> {
    let mut grouped: Vec<(Action, Vec<String>)> = Vec::new();
    for (code, action) in pairs {
        let label = leader_key_label(code);
        match grouped.iter_mut().find(|(a, _)| *a == action) {
            Some((_, keys)) => keys.push(label),
            None => grouped.push((action, vec![label])),
        }
    }
    grouped
        .into_iter()
        .map(|(action, mut keys)| {
            keys.sort_by_key(String::len);
            keys.truncate(MAX_ALIASES);
            WhichKeyRow { chord: keys.join("/"), label: short_label(&action), group: group.unwrap_or_else(|| group_of(&action)) }
        })
        .collect()
}

/// Rows of the popup for `mode` under `km`: one row per distinct action of the live table.
pub fn rows_for(mode: &KeyMode, km: &EffectiveKeymap) -> Vec<WhichKeyRow> {
    match mode {
        KeyMode::Normal => Vec::new(),
        KeyMode::LeaderPending => collapse(km.leader_rows().into_iter(), None),
        KeyMode::Window(sub) => collapse(
            rows_for_scope(WinScope::of(*sub)).map(|(_, _, code, action)| (*code, action.clone())),
            None,
        ),
    }
}

/// Rows for the default `[keys]` config under the auto profile of `env`.
pub fn which_key_rows(mode: &KeyMode, env: &KeyEnv) -> Vec<WhichKeyRow> {
    rows_for(mode, &KeyProfile::resolve(&KeysConfig::default(), env))
}

/// Rows for a live workspace: the table rows plus the user's `[keys.window_mode]` additions.
pub fn rows_for_wm(wm: &WindowManager) -> Vec<WhichKeyRow> {
    let mode = wm.key_mode();
    let mut rows = rows_for(&mode, &wm.keymap());
    if mode.is_window() {
        for (chord, command) in &wm.config().keys.window_mode {
            rows.push(WhichKeyRow {
                chord: chord.clone(),
                label: command.trim_start_matches(':').chars().take(20).collect(),
                group: "custom",
            });
        }
    }
    rows
}

fn canonical_key(scope: WinScope, action: &Action) -> Option<String> {
    rows_for_scope(scope).find(|(_, _, _, a)| a == action).map(|(_, _, code, _)| leader_key_label(*code))
}

/// Single-character canonical keys of `acts` concatenated (`hjkl`).
fn key_run(scope: WinScope, acts: &[Action]) -> String {
    acts.iter().filter_map(|a| canonical_key(scope, a)).filter(|k| k.chars().count() == 1).collect()
}

fn family_label(scope: WinScope, action: &Action) -> &'static str {
    match (scope, action) {
        (_, Action::PaneResize { .. }) => "resize",
        (WinScope::Move, _) => "shift",
        _ => "move",
    }
}

/// The Window-mode footer chips for `scope`, e.g. `["hjkl move", "1-9 jump", "z zoom", ...]`.
fn window_chips(scope: WinScope) -> Vec<String> {
    let acts = footer_actions(scope);
    let mut chips = Vec::new();
    let mut i = 0;
    while i < acts.len() {
        let a = &acts[i];
        match a {
            Action::PaneFocusDirection { .. } | Action::PaneResize { .. } | Action::PaneMove { .. } => {
                let end = acts[i..]
                    .iter()
                    .position(|n| std::mem::discriminant(n) != std::mem::discriminant(a))
                    .map_or(acts.len(), |p| i + p);
                chips.push(format!("{} {}", key_run(scope, &acts[i..end]), family_label(scope, a)));
                i = end;
            }
            Action::FocusPane(_) if matches!(acts.get(i + 1), Some(Action::FocusPane(_))) => {
                let first = canonical_key(scope, a).unwrap_or_default();
                let last = canonical_key(scope, &acts[i + 1]).unwrap_or_default();
                chips.push(format!("{first}-{last} jump"));
                i += 2;
            }
            _ => {
                if let Some(key) = canonical_key(scope, a) {
                    chips.push(format!("{key} {}", short_label(a).to_lowercase()));
                }
                i += 1;
            }
        }
    }
    if scope == WinScope::Focus {
        chips.push(format!("{WHICH_KEY_PIN} help"));
    }
    let exit = canonical_key(scope, &Action::WindowMode(WindowModeOp::Exit)).unwrap_or_else(|| "Esc".into());
    chips.push(format!("{exit} exit"));
    chips
}

/// One-line hint for the active prefix mode, generated from the live tables.
/// `leader_label` names the leader chord shown in the leader hint.
pub fn footer_line(mode: &KeyMode, km: &EffectiveKeymap, leader_label: &str) -> String {
    match mode {
        KeyMode::Normal => String::new(),
        KeyMode::LeaderPending => {
            let body: Vec<String> = LEADER_FOOTER_ACTIONS
                .iter()
                .filter_map(|a| km.leader_keys_for_action(a).first().map(|c| format!("{}={}", leader_key_label(*c), short_label(a))))
                .collect();
            format!("⌨ LEADER [{leader_label}]: {}", body.join(" "))
        }
        KeyMode::Window(sub) => format!("WINDOW [{}]  {}", sub.name(), window_chips(WinScope::of(*sub)).join("  ")),
    }
}

/// Leader footer rows (chord and label per footer action) for `km`.
pub fn leader_footer_rows(km: &EffectiveKeymap) -> Vec<WhichKeyRow> {
    LEADER_FOOTER_ACTIONS
        .iter()
        .filter_map(|a| {
            km.leader_keys_for_action(a).first().map(|c| WhichKeyRow {
                chord: leader_key_label(*c),
                label: short_label(a),
                group: "leader",
            })
        })
        .collect()
}

fn role(cfg: &BiltermConfig, palette: &ThemePalette, role: ColorRole, legacy: Color) -> Color {
    if AppearanceProfile::resolve(cfg) == AppearanceProfile::Legacy {
        legacy
    } else {
        DesignTokens::from_palette(palette).get(role)
    }
}

const CHORD_COL_MAX: usize = 12;
const LABEL_COL_MAX: usize = 16;

/// Geometry of the popup: `(outer rect, columns, rows per column)`; `None` when `area` is too small.
pub fn panel_layout(area: Rect, rows: &[WhichKeyRow]) -> Option<(Rect, usize, usize)> {
    if rows.is_empty() || area.width < 20 || area.height < 5 {
        return None;
    }
    let width_of = |f: &dyn Fn(&WhichKeyRow) -> usize| rows.iter().map(f).max().unwrap_or(1);
    let chord_w = width_of(&|r| r.chord.chars().count()).min(CHORD_COL_MAX);
    let label_w = width_of(&|r| r.label.chars().count()).min(LABEL_COL_MAX);
    let col_w = chord_w + 1 + label_w + 2;
    let usable = usize::from(area.width.saturating_sub(4)).min(96);
    let cols = (usable / col_w).max(1);
    let max_rows = usize::from(area.height.saturating_sub(4)).max(1);
    let per_col = rows.len().div_ceil(cols).min(max_rows);
    let w = u16::try_from(cols * col_w + 2).unwrap_or(area.width).min(area.width);
    let h = u16::try_from(per_col + 2).unwrap_or(area.height).min(area.height);
    let x = area.x + (area.width - w) / 2;
    let y = area.y + area.height - h;
    Some((Rect::new(x, y, w, h), cols, per_col))
}

/// Draws the popup panel for `rows` into the bottom of `area`.
#[allow(clippy::too_many_arguments)]
pub fn render_which_key_panel(
    area: Rect,
    title: &str,
    rows: &[WhichKeyRow],
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some((rect, cols, per_col)) = panel_layout(area, rows) else { return };
    if cfg.liquid_glass {
        render_liquid_ambient_halo(buf, rect, area, palette);
    } else if cfg.drop_shadows {
        render_drop_shadow_styled(buf, rect, area, cfg.shadow_style, palette);
    }
    occlude_rect(buf, rect);
    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, rect, palette.window_frame_bg, palette);
    }
    render_borland_frame_styled(rect, title, true, cfg.window_outline, palette, glyphs, buf);
    let inner = inset_window_frame(rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let chord_w = rows.iter().map(|r| r.chord.chars().count()).max().unwrap_or(1).min(CHORD_COL_MAX);
    let label_w = rows.iter().map(|r| r.label.chars().count()).max().unwrap_or(1).min(LABEL_COL_MAX);
    let accent = role(cfg, palette, ColorRole::Accent, Color::Rgb(255, 176, 0));
    let text = if cfg.is_modern() { palette.text_fg.unwrap_or(Color::White) } else { Color::White };
    let bg = palette.window_frame_bg;
    let capacity = cols * per_col;
    let truncated = rows.len() > capacity;
    let shown = if truncated { capacity.saturating_sub(1) } else { rows.len() };
    let mut lines: Vec<Line> = Vec::with_capacity(per_col);
    for r in 0..per_col {
        let mut spans: Vec<Span> = Vec::new();
        for c in 0..cols {
            let idx = c * per_col + r;
            if idx < shown {
                let row = &rows[idx];
                let chord: String = row.chord.chars().take(chord_w).collect();
                let label: String = row.label.chars().take(label_w).collect();
                spans.push(Span::styled(format!("{chord:<chord_w$} "), Style::default().fg(accent).bg(bg).add_modifier(Modifier::BOLD)));
                spans.push(Span::styled(format!("{label:<label_w$}  "), Style::default().fg(text).bg(bg)));
            } else if truncated && idx == shown {
                let more = format!("+{} more", rows.len() - shown);
                spans.push(Span::styled(format!("{more:<w$}", w = chord_w + 1 + label_w + 2), Style::default().fg(text).bg(bg)));
            }
        }
        lines.push(Line::from(spans));
    }
    Block::default().style(Style::default().fg(text).bg(bg)).render(inner, buf);
    Paragraph::new(lines).render(inner, buf);
}

/// Title of the popup for the workspace's current key mode.
pub fn panel_title(wm: &WindowManager) -> String {
    match wm.key_mode() {
        KeyMode::LeaderPending => format!(" WHICH KEY · {} · Esc cancel ", wm.config().leader_key),
        KeyMode::Window(sub) => {
            let sub_name = match sub {
                WinSub::Focus => "focus",
                other => other.name(),
            };
            let exit = canonical_key(WinScope::of(sub), &Action::WindowMode(WindowModeOp::Exit)).unwrap_or_default();
            format!(" WINDOW [{sub_name}] · {WHICH_KEY_PIN} pin · {exit} exit ")
        }
        KeyMode::Normal => String::new(),
    }
}

/// Draws the popup when the workspace key mode wants it (delay elapsed or pinned).
pub(super) fn render_which_key(
    wm: &WindowManager,
    area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    if !wm.which_key_visible() {
        return;
    }
    let rows = rows_for_wm(wm);
    render_which_key_panel(area, &panel_title(wm), &rows, cfg, palette, glyphs, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::WinSub;
    use crate::input::keyenv::{HostOs, Surface};

    #[test]
    fn window_footer_is_generated_from_the_table() {
        let km = KeyProfile::resolve(&KeysConfig::default(), &KeyEnv::new(HostOs::Linux, Surface::Tui));
        let line = footer_line(&KeyMode::Window(WinSub::Focus), &km, "Ctrl+X");
        assert_eq!(line, "WINDOW [focus]  hjkl move  1-9 jump  z zoom  s stack  t tile  x close  ? help  Esc exit");
        let resize = footer_line(&KeyMode::Window(WinSub::Resize), &km, "Ctrl+X");
        assert_eq!(resize, "WINDOW [resize]  hjkl resize  Esc exit");
    }

    #[test]
    fn leader_rows_collapse_aliases_shortest_first() {
        let km = KeyProfile::resolve(&KeysConfig::default(), &KeyEnv::new(HostOs::Linux, Surface::Tui));
        let rows = rows_for(&KeyMode::LeaderPending, &km);
        let zoom = rows.iter().find(|r| r.label == "Zoom").expect("zoom row");
        assert!(zoom.chord.starts_with("z"), "{}", zoom.chord);
        assert!(zoom.chord.contains("F5"));
    }
}
