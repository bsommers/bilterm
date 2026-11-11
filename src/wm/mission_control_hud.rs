//! Agent Mission Control HUD model + renderer (`HUD-01`, Phase 70).
//!
//! Pure state and pure buffer writes: no I/O, no wall clock (callers pass millisecond
//! timestamps), no PTY handles and no resizing. Threats: `T-HUD-01` (Approve is only enabled
//! while the card is `Blocked` and carries a pane generation token), `T-HUD-02` (cards of
//! no-echo panes render a masked mini viewport) and `T-HUD-03` (tile cap with paging, dirty
//! flag and a refresh-rate cap).

use std::collections::{BTreeMap, VecDeque};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::pane::PaneId;
use crate::wm::agent_state::AgentState;

/// Maximum tracked cards.
pub const MAX_CARDS: usize = 256;
/// Maximum tiles shown on one page.
pub const MAX_TILES_PER_PAGE: usize = 12;
/// Sparkline ring capacity (bytes/s samples).
pub const SPARK_CAP: usize = 32;
/// Minimum spacing between HUD redraws, milliseconds.
pub const MIN_REFRESH_MS: u64 = 100;
/// Preferred card width in cells.
pub const CARD_W: u16 = 34;
/// Narrowest card that is still drawn.
pub const MIN_CARD_W: u16 = 12;
/// Fixed card height in cells.
pub const CARD_H: u16 = 7;
/// Maximum traffic arcs tracked.
pub const MAX_ARCS: usize = 16;
/// Maximum arc cells produced per layout.
pub const MAX_ARC_CELLS: usize = 512;
/// Maximum characters kept for free-text badges.
pub const MAX_TEXT: usize = 64;
/// Sparkline glyph ramp.
pub const SPARK_GLYPHS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Fixed-capacity ring buffer of bytes/s samples.
#[derive(Clone, Debug, Default)]
pub struct SparkRing {
    samples: VecDeque<u32>,
}

impl SparkRing {
    pub fn push(&mut self, v: u32) {
        if self.samples.len() >= SPARK_CAP {
            self.samples.pop_front();
        }
        self.samples.push_back(v);
    }
    pub fn len(&self) -> usize {
        self.samples.len()
    }
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
    /// Samples oldest first.
    pub fn samples(&self) -> impl Iterator<Item = u32> + '_ {
        self.samples.iter().copied()
    }
    /// Renders the newest `width` samples as block glyphs, scaled to the window maximum.
    pub fn glyphs(&self, width: usize) -> String {
        let skip = self.samples.len().saturating_sub(width);
        let win: Vec<u32> = self.samples.iter().skip(skip).copied().collect();
        let max = win.iter().copied().max().unwrap_or(0);
        win.iter()
            .map(|&v| {
                if max == 0 {
                    SPARK_GLYPHS[0]
                } else {
                    let idx = (u64::from(v) * 7 / u64::from(max)) as usize;
                    SPARK_GLYPHS[idx.min(7)]
                }
            })
            .collect()
    }
}

/// Telemetry shown on one agent card.
#[derive(Clone, Debug)]
pub struct AgentCardTelemetry {
    pub pane: PaneId,
    pub title: String,
    pub state: AgentState,
    pub spark: SparkRing,
    pub tool: Option<String>,
    pub file: Option<String>,
    pub queue_depth: usize,
    /// Optional swarm node / worktree badge.
    pub badge: Option<String>,
    /// Swarm node owning this pane (Phase 69 wiring), refreshed with each HUD refresh.
    pub swarm: Option<crate::wm::swarm_cmd::SwarmNodeInfo>,
    /// No-echo pane: the mini viewport must not show content (`T-HUD-02`).
    pub masked: bool,
    /// Pane generation token carried by Approve (`T-HUD-01`).
    pub generation: u64,
    /// Last visible lines of the pane (read-only snapshot).
    pub preview: Vec<String>,
}

fn clip(s: &str) -> String {
    s.chars().take(MAX_TEXT).collect()
}

impl AgentCardTelemetry {
    pub fn new(pane: PaneId, title: &str, state: AgentState) -> Self {
        Self {
            pane,
            title: clip(title),
            state,
            spark: SparkRing::default(),
            tool: None,
            file: None,
            queue_depth: 0,
            badge: None,
            swarm: None,
            masked: false,
            generation: 0,
            preview: Vec::new(),
        }
    }
    pub fn push_rate(&mut self, bytes_per_sec: u32) {
        self.spark.push(bytes_per_sec);
    }
    pub fn set_badge(&mut self, b: Option<&str>) {
        self.badge = b.map(clip);
    }
    pub fn set_tool(&mut self, tool: Option<&str>, file: Option<&str>) {
        self.tool = tool.map(clip);
        self.file = file.map(clip);
    }
    /// State glyph for the card header.
    pub fn glyph(&self) -> char {
        match self.state {
            AgentState::Working => '●',
            AgentState::Blocked => '■',
            AgentState::Idle => '○',
            AgentState::Done => '✓',
            AgentState::Errored => '✗',
        }
    }
    /// Approve is enabled only while Blocked (`T-HUD-01`).
    pub fn approve_enabled(&self) -> bool {
        self.state == AgentState::Blocked
    }
}

/// Per-card action button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudButtonKind {
    Approve,
    Prompt,
    Diff,
    Zoom,
}

impl HudButtonKind {
    pub const ALL: [HudButtonKind; 4] = [Self::Approve, Self::Prompt, Self::Diff, Self::Zoom];
    pub fn label(self) -> &'static str {
        match self {
            Self::Approve => "[Approve]",
            Self::Prompt => "[Prompt]",
            Self::Diff => "[Diff]",
            Self::Zoom => "[Zoom]",
        }
    }
}

/// An activated button. `generation` is `Some` only for Approve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HudButton {
    pub pane: PaneId,
    pub kind: HudButtonKind,
    pub generation: Option<u64>,
}

/// Result of hit-testing a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudHit {
    Button(HudButton),
    /// A button that exists but is disabled (Approve while not Blocked).
    Disabled {
        pane: PaneId,
        kind: HudButtonKind,
    },
    Card(PaneId),
}

/// One cell of a traffic arc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArcCell {
    pub x: u16,
    pub y: u16,
    pub ch: char,
}

fn contains(r: Rect, x: u16, y: u16) -> bool {
    x >= r.x && y >= r.y && x < r.x.saturating_add(r.width) && y < r.y.saturating_add(r.height)
}

fn inner(r: Rect) -> Rect {
    Rect::new(
        r.x + 1,
        r.y + 1,
        r.width.saturating_sub(2),
        r.height.saturating_sub(2),
    )
}

/// Button rectangles for a card; buttons that do not fully fit are omitted.
pub fn button_rects(card: Rect) -> Vec<(HudButtonKind, Rect)> {
    let mut out = Vec::new();
    if card.width < 2 || card.height < CARD_H {
        return out;
    }
    let inn = inner(card);
    let y = card.y + 5;
    let mut x = inn.x;
    let end = inn.x + inn.width;
    for kind in HudButtonKind::ALL {
        let w = kind.label().chars().count() as u16;
        if x + w > end {
            break;
        }
        out.push((kind, Rect::new(x, y, w, 1)));
        x += w + 1;
    }
    out
}

/// HUD state: cards, selection, paging and refresh gating.
#[derive(Debug, Default)]
pub struct MissionControlHud {
    cards: BTreeMap<PaneId, AgentCardTelemetry>,
    selected: Option<PaneId>,
    page: usize,
    arcs: Vec<(PaneId, PaneId)>,
    dirty: bool,
    last_refresh_ms: Option<u64>,
}

impl MissionControlHud {
    pub fn new() -> Self {
        Self {
            dirty: true,
            ..Self::default()
        }
    }
    pub fn len(&self) -> usize {
        self.cards.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }
    /// Pane ids that have a card, ascending.
    pub fn card_ids(&self) -> Vec<PaneId> {
        self.cards.keys().copied().collect()
    }
    /// All cards in pane-id order.
    pub fn cards(&self) -> impl Iterator<Item = &AgentCardTelemetry> {
        self.cards.values()
    }
    pub fn card(&self, pane: PaneId) -> Option<&AgentCardTelemetry> {
        self.cards.get(&pane)
    }
    /// Inserts or replaces a card; rejects new panes beyond [`MAX_CARDS`].
    pub fn upsert_card(&mut self, card: AgentCardTelemetry) -> bool {
        if !self.cards.contains_key(&card.pane) && self.cards.len() >= MAX_CARDS {
            return false;
        }
        if self.selected.is_none() {
            self.selected = Some(card.pane);
        }
        self.cards.insert(card.pane, card);
        self.dirty = true;
        true
    }
    pub fn remove_card(&mut self, pane: PaneId) {
        if self.cards.remove(&pane).is_some() {
            if self.selected == Some(pane) {
                self.selected = self.cards.keys().next().copied();
            }
            self.arcs.retain(|(a, b)| *a != pane && *b != pane);
            self.dirty = true;
        }
    }
    /// Records a bytes/s sample for a pane.
    pub fn record_rate(&mut self, pane: PaneId, bytes_per_sec: u32) {
        if let Some(c) = self.cards.get_mut(&pane) {
            c.push_rate(bytes_per_sec);
            self.dirty = true;
        }
    }
    pub fn set_state(&mut self, pane: PaneId, state: AgentState) {
        if let Some(c) = self.cards.get_mut(&pane)
            && c.state != state
        {
            c.state = state;
            self.dirty = true;
        }
    }
    pub fn card_mut(&mut self, pane: PaneId) -> Option<&mut AgentCardTelemetry> {
        self.dirty = true;
        self.cards.get_mut(&pane)
    }
    /// Replaces traffic arcs (sender, recipient), capped at [`MAX_ARCS`].
    pub fn set_arcs(&mut self, arcs: &[(PaneId, PaneId)]) {
        self.arcs = arcs.iter().copied().take(MAX_ARCS).collect();
        self.dirty = true;
    }

    // --- selection & paging ---
    pub fn selected(&self) -> Option<PaneId> {
        self.selected
    }
    pub fn select(&mut self, pane: PaneId) -> bool {
        if self.cards.contains_key(&pane) {
            self.selected = Some(pane);
            self.dirty = true;
            true
        } else {
            false
        }
    }
    fn step_selection(&mut self, fwd: bool, page_size: usize) {
        let ids: Vec<PaneId> = self.cards.keys().copied().collect();
        if ids.is_empty() {
            return;
        }
        let cur = self
            .selected
            .and_then(|s| ids.iter().position(|&i| i == s))
            .unwrap_or(0);
        let n = ids.len();
        let next = if fwd {
            (cur + 1) % n
        } else {
            (cur + n - 1) % n
        };
        self.selected = Some(ids[next]);
        self.page = next / page_size.clamp(1, MAX_TILES_PER_PAGE);
        self.dirty = true;
    }
    pub fn select_next(&mut self, page_size: usize) {
        self.step_selection(true, page_size);
    }
    pub fn select_prev(&mut self, page_size: usize) {
        self.step_selection(false, page_size);
    }
    /// Tiles per page that fit `area`, capped at [`MAX_TILES_PER_PAGE`].
    pub fn page_size_for(area: Rect) -> usize {
        let (cols, rows) = Self::grid_dims(area);
        (cols * rows).min(MAX_TILES_PER_PAGE)
    }
    fn grid_dims(area: Rect) -> (usize, usize) {
        if area.width < MIN_CARD_W || area.height < CARD_H {
            return (0, 0);
        }
        let cols = usize::from((area.width / CARD_W).max(1));
        let rows = usize::from(area.height / CARD_H);
        (cols, rows)
    }
    pub fn page_count(&self, page_size: usize) -> usize {
        if page_size == 0 {
            return 0;
        }
        self.cards.len().div_ceil(page_size).max(1)
    }
    pub fn page(&self) -> usize {
        self.page
    }
    pub fn next_page(&mut self, page_size: usize) {
        let last = self.page_count(page_size).saturating_sub(1);
        self.page = (self.page + 1).min(last);
        self.dirty = true;
    }
    pub fn prev_page(&mut self) {
        self.page = self.page.saturating_sub(1);
        self.dirty = true;
    }

    /// Cards visible on the current page with their rectangles.
    pub fn layout(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        let (cols, rows) = Self::grid_dims(area);
        let size = (cols * rows).min(MAX_TILES_PER_PAGE);
        if size == 0 {
            return Vec::new();
        }
        let page = self.page.min(self.page_count(size) - 1);
        let cw = area.width / cols as u16;
        let mut out = Vec::new();
        for (i, id) in self.cards.keys().skip(page * size).take(size).enumerate() {
            let (c, r) = ((i % cols) as u16, (i / cols) as u16);
            out.push((
                *id,
                Rect::new(area.x + c * cw, area.y + r * CARD_H, cw, CARD_H),
            ));
        }
        out
    }

    // --- hit testing ---
    pub fn hit_test(&self, area: Rect, x: u16, y: u16) -> Option<HudHit> {
        for (id, rect) in self.layout(area) {
            if !contains(rect, x, y) {
                continue;
            }
            let card = self.cards.get(&id)?;
            for (kind, br) in button_rects(rect) {
                if contains(br, x, y) {
                    if kind == HudButtonKind::Approve {
                        return Some(if card.approve_enabled() {
                            HudHit::Button(HudButton {
                                pane: id,
                                kind,
                                generation: Some(card.generation),
                            })
                        } else {
                            HudHit::Disabled { pane: id, kind }
                        });
                    }
                    return Some(HudHit::Button(HudButton {
                        pane: id,
                        kind,
                        generation: None,
                    }));
                }
            }
            return Some(HudHit::Card(id));
        }
        None
    }
    /// True when an Approve token still matches the card (rejects stale tiles).
    pub fn approve_token_valid(&self, b: &HudButton) -> bool {
        b.kind == HudButtonKind::Approve
            && self
                .cards
                .get(&b.pane)
                .is_some_and(|c| c.approve_enabled() && Some(c.generation) == b.generation)
    }

    // --- traffic arcs ---
    /// Arc cells sender -> recipient along card top borders; cells inside card interiors are skipped.
    pub fn arc_cells(&self, area: Rect) -> Vec<ArcCell> {
        let lay = self.layout(area);
        let find = |p: PaneId| lay.iter().find(|(id, _)| *id == p).map(|(_, r)| *r);
        let mut out = Vec::new();
        for (from, to) in &self.arcs {
            let (Some(a), Some(b)) = (find(*from), find(*to)) else {
                continue;
            };
            if from == to {
                continue;
            }
            let (sx, ex) = (a.x + a.width / 2, b.x + b.width / 2);
            let (sy, ey) = (a.y, b.y);
            let mut path: Vec<(u16, u16, char)> = Vec::new();
            let hch = if ex >= sx { '»' } else { '«' };
            let (lo, hi) = (sx.min(ex), sx.max(ex));
            for x in lo..=hi {
                path.push((x, sy, hch));
            }
            let vch = if ey >= sy { '▼' } else { '▲' };
            let (lo, hi) = (sy.min(ey), sy.max(ey));
            for y in lo..=hi {
                path.push((ex, y, vch));
            }
            if let Some(last) = path.last_mut() {
                last.2 = '◉';
            }
            for (x, y, ch) in path {
                if lay.iter().any(|(_, r)| contains(inner(*r), x, y)) {
                    continue;
                }
                if out.len() >= MAX_ARC_CELLS {
                    return out;
                }
                out.push(ArcCell { x, y, ch });
            }
        }
        out
    }

    // --- refresh gating ---
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
    /// Clears the dirty flag after a refresh pass (refresh gating, not drawing, uses it).
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    /// True when a redraw is due: dirty and at least [`MIN_REFRESH_MS`] since the last one.
    pub fn should_refresh(&self, now_ms: u64) -> bool {
        self.dirty
            && self
                .last_refresh_ms
                .is_none_or(|t| now_ms.saturating_sub(t) >= MIN_REFRESH_MS)
    }
    /// Consumes the dirty flag when a refresh is due.
    pub fn take_refresh(&mut self, now_ms: u64) -> bool {
        if self.should_refresh(now_ms) {
            self.dirty = false;
            self.last_refresh_ms = Some(now_ms);
            true
        } else {
            false
        }
    }

    // --- rendering (pure buffer write) ---
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        for (id, rect) in self.layout(area) {
            if let Some(card) = self.cards.get(&id) {
                self.render_card(card, rect, buf, self.selected == Some(id));
            }
        }
        for c in self.arc_cells(area) {
            if let Some(cell) = buf.cell_mut((c.x, c.y)) {
                cell.set_char(c.ch)
                    .set_style(Style::default().fg(Color::Cyan));
            }
        }
    }

    fn render_card(&self, card: &AgentCardTelemetry, r: Rect, buf: &mut Buffer, sel: bool) {
        let bs = if sel {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        draw_border(buf, r, bs);
        let inn = inner(r);
        let w = usize::from(inn.width);
        let plain = Style::default();
        let mut head = format!("{} {}", card.glyph(), card.title);
        if let Some(b) = &card.badge {
            head.push_str(&format!(" <{b}>"));
        }
        if let Some(n) = &card.swarm {
            let wave = n.wave.map_or(String::new(), |w| format!(" w{w}"));
            head.push_str(&format!(" [{} {}{wave} r{}]", n.node, n.state, n.retries));
        }
        put(buf, inn.x, r.y + 1, &head, w, plain);
        let q = format!(" q:{}", card.queue_depth);
        let sw = w.saturating_sub(q.chars().count());
        put(
            buf,
            inn.x,
            r.y + 2,
            &card.spark.glyphs(sw),
            sw,
            Style::default().fg(Color::Green),
        );
        put(buf, inn.x + sw as u16, r.y + 2, &q, w - sw, plain);
        let tool = match (&card.tool, &card.file) {
            (Some(t), Some(f)) => format!("{t} {f}"),
            (Some(t), None) => t.clone(),
            (None, Some(f)) => f.clone(),
            (None, None) => String::new(),
        };
        put(buf, inn.x, r.y + 3, &tool, w, plain);
        let view = if card.masked {
            "▒▒▒▒ masked ▒▒▒▒".to_string()
        } else {
            card.preview.last().cloned().unwrap_or_default()
        };
        put(
            buf,
            inn.x,
            r.y + 4,
            &view,
            w,
            Style::default().fg(Color::Gray),
        );
        for (kind, br) in button_rects(r) {
            let off = kind == HudButtonKind::Approve && !card.approve_enabled();
            let st = if off {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM)
            } else {
                Style::default().fg(Color::Black).bg(Color::Gray)
            };
            put(buf, br.x, br.y, kind.label(), usize::from(br.width), st);
        }
    }
}

fn put(buf: &mut Buffer, x: u16, y: u16, s: &str, max: usize, st: Style) {
    if max == 0 || !contains(buf.area, x, y) {
        return;
    }
    let room = usize::from(buf.area.x + buf.area.width - x);
    buf.set_stringn(x, y, s, max.min(room), st);
}

fn draw_border(buf: &mut Buffer, r: Rect, st: Style) {
    if r.width < 2 || r.height < 2 {
        return;
    }
    let (x1, y1) = (r.x + r.width - 1, r.y + r.height - 1);
    let mut set = |x: u16, y: u16, ch: char| {
        if let Some(c) = buf.cell_mut((x, y)) {
            c.set_char(ch).set_style(st);
        }
    };
    for x in r.x..=x1 {
        set(x, r.y, '─');
        set(x, y1, '─');
    }
    for y in r.y..=y1 {
        set(r.x, y, '│');
        set(x1, y, '│');
    }
    set(r.x, r.y, '┌');
    set(x1, r.y, '┐');
    set(r.x, y1, '└');
    set(x1, y1, '┘');
}
