//! Dynamic, memory-aware scrollback sizing (`[scrollback]` config).
//!
//! `vt100-ctt` stores scrollback in a lazily-growing `VecDeque<Row>` whose maximum length is
//! fixed at parser construction. Bilterm therefore starts every pane with a modest capacity and
//! periodically *rebalances*: panes that are nearly full double their capacity while the global
//! memory budget has headroom, and the largest panes contract toward a fair share when the
//! budget is exceeded or the host runs low on available memory. Changing a capacity rebuilds the
//! parser via [`crate::vt::VtParser::rebuild`], which keeps the newest history lines.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Size of one `vt100_ctt::Cell` in bytes.
pub const CELL_BYTES: u64 = 32;
/// Approximate per-row overhead (`Vec` header, wrap flag, `VecDeque` slot, allocator slack).
pub const ROW_OVERHEAD_BYTES: u64 = 64;
/// Floor for the automatic memory budget.
pub const MIN_AUTO_BUDGET_BYTES: u64 = 64 * MIB;
/// Budget used when total system memory cannot be probed.
pub const FALLBACK_BUDGET_BYTES: u64 = 256 * MIB;
/// Default initial per-pane capacity for live panes before any rebalance.
pub const DEFAULT_INITIAL_LINES: usize = 10_000;
/// Minimum interval between periodic rebalances from the app tick.
pub const REBALANCE_INTERVAL: Duration = Duration::from_secs(2);

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;
const TIB: u64 = 1024 * GIB;

/// Host memory figures in bytes; `None` when unknown on this platform.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemorySnapshot {
    pub total_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
}

/// Test seam for probing host memory.
pub trait MemoryProbe: Send {
    fn snapshot(&self) -> MemorySnapshot;
}

/// Probes the real host (`/proc/meminfo` on Linux, `sysctl hw.memsize` on macOS).
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemMemoryProbe;

impl MemoryProbe for SystemMemoryProbe {
    fn snapshot(&self) -> MemorySnapshot {
        system_memory_snapshot()
    }
}

/// Returns a fixed snapshot (tests, headless mode).
#[derive(Clone, Copy, Debug, Default)]
pub struct FixedMemoryProbe(pub MemorySnapshot);

impl MemoryProbe for FixedMemoryProbe {
    fn snapshot(&self) -> MemorySnapshot {
        self.0
    }
}

/// Parses `MemTotal` / `MemAvailable` (kB) from Linux `/proc/meminfo` text.
pub fn parse_meminfo(text: &str) -> MemorySnapshot {
    let field = |name: &str| {
        text.lines().find_map(|line| {
            let rest = line.strip_prefix(name)?.strip_prefix(':')?;
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            Some(kb.saturating_mul(KIB))
        })
    };
    MemorySnapshot {
        total_bytes: field("MemTotal"),
        available_bytes: field("MemAvailable"),
    }
}

#[cfg(target_os = "linux")]
fn system_memory_snapshot() -> MemorySnapshot {
    std::fs::read_to_string("/proc/meminfo")
        .map(|t| parse_meminfo(&t))
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn system_memory_snapshot() -> MemorySnapshot {
    let mut value: u64 = 0;
    let mut len = std::mem::size_of::<u64>();
    // SAFETY: `hw.memsize` is a NUL-terminated name; `value`/`len` describe a valid u64 buffer.
    let rc = unsafe {
        libc::sysctlbyname(
            c"hw.memsize".as_ptr(),
            (&mut value as *mut u64).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    MemorySnapshot {
        total_bytes: (rc == 0 && value > 0).then_some(value),
        available_bytes: None,
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn system_memory_snapshot() -> MemorySnapshot {
    MemorySnapshot::default()
}

/// `memory_budget` setting: `"auto"`, `"<n>%"` of total RAM, or an absolute size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "IntOrString", into = "IntOrString")]
pub enum MemoryBudget {
    /// 10% of total RAM, at least 64 MiB.
    #[default]
    Auto,
    /// Percentage (`1..=100`) of total RAM.
    Percent(u8),
    /// Absolute byte count.
    Bytes(u64),
}

/// `max_lines` setting: `"auto"` (derived from the budget) or a fixed hard cap per pane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "IntOrString", into = "IntOrString")]
pub enum LineLimit {
    #[default]
    Auto,
    Fixed(usize),
}

/// Lenient TOML scalar accepted for scrollback sizes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IntOrString {
    Int(u64),
    Str(String),
}

impl TryFrom<IntOrString> for MemoryBudget {
    type Error = String;
    fn try_from(v: IntOrString) -> Result<Self, String> {
        match v {
            IntOrString::Int(n) => Ok(Self::Bytes(n)),
            IntOrString::Str(s) => parse_memory_budget(&s),
        }
    }
}

impl From<MemoryBudget> for IntOrString {
    fn from(v: MemoryBudget) -> Self {
        match v {
            MemoryBudget::Auto => Self::Str("auto".into()),
            MemoryBudget::Percent(p) => Self::Str(format!("{p}%")),
            MemoryBudget::Bytes(b) => Self::Int(b),
        }
    }
}

impl TryFrom<IntOrString> for LineLimit {
    type Error = String;
    fn try_from(v: IntOrString) -> Result<Self, String> {
        match v {
            IntOrString::Int(n) => Ok(Self::Fixed(n as usize)),
            IntOrString::Str(s) if s.trim().eq_ignore_ascii_case("auto") => Ok(Self::Auto),
            IntOrString::Str(s) => parse_line_count(&s).map(Self::Fixed),
        }
    }
}

impl From<LineLimit> for IntOrString {
    fn from(v: LineLimit) -> Self {
        match v {
            LineLimit::Auto => Self::Str("auto".into()),
            LineLimit::Fixed(n) => Self::Int(n as u64),
        }
    }
}

/// Parses `"auto"`, `"25%"`, `"512MiB"`, `"8G"`, `"1048576"` into a [`MemoryBudget`].
pub fn parse_memory_budget(text: &str) -> Result<MemoryBudget, String> {
    let s = text.trim();
    if s.eq_ignore_ascii_case("auto") {
        return Ok(MemoryBudget::Auto);
    }
    if let Some(pct) = s.strip_suffix('%') {
        let p: u8 = pct
            .trim()
            .parse()
            .map_err(|_| format!("invalid scrollback memory percentage `{s}`"))?;
        if !(1..=100).contains(&p) {
            return Err(format!(
                "scrollback memory percentage `{s}` must be 1..=100"
            ));
        }
        return Ok(MemoryBudget::Percent(p));
    }
    parse_byte_size(s).map(MemoryBudget::Bytes)
}

/// Parses a binary size such as `"8GiB"`, `"512M"`, `"64KB"` or a bare byte count.
pub fn parse_byte_size(text: &str) -> Result<u64, String> {
    let s = text.trim();
    let split = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    let n: u64 = num
        .parse()
        .map_err(|_| format!("invalid scrollback memory size `{s}`"))?;
    let mult = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" | "kib" => KIB,
        "m" | "mb" | "mib" => MIB,
        "g" | "gb" | "gib" => GIB,
        "t" | "tb" | "tib" => TIB,
        _ => {
            return Err(format!(
                "unknown size unit in `{s}` (use KiB, MiB, GiB, TiB)"
            ));
        }
    };
    n.checked_mul(mult)
        .ok_or_else(|| format!("scrollback memory size `{s}` overflows"))
}

fn parse_line_count(text: &str) -> Result<usize, String> {
    text.trim()
        .replace('_', "")
        .parse()
        .map_err(|_| format!("invalid scrollback line count `{text}`"))
}

fn de_lenient_usize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<usize, D::Error> {
    match IntOrString::deserialize(d)? {
        IntOrString::Int(n) => Ok(n as usize),
        IntOrString::Str(s) => parse_line_count(&s).map_err(serde::de::Error::custom),
    }
}

/// `[scrollback]` configuration table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScrollbackConfig {
    /// Global budget for all panes' scrollback (`"auto"`, `"<n>%"`, `"<n>[KMGT]iB"`).
    pub memory_budget: MemoryBudget,
    /// Hard per-pane cap in lines (`"auto"` = limited only by the budget).
    pub max_lines: LineLimit,
    /// Floor that contraction never goes below.
    #[serde(deserialize_with = "de_lenient_usize")]
    pub min_lines: usize,
    /// Capacity each new pane starts with before growing by doubling.
    #[serde(deserialize_with = "de_lenient_usize")]
    pub initial_lines: usize,
    /// Contract when `MemAvailable` drops below this percentage of total RAM.
    pub low_memory_pct: u8,
}

impl Default for ScrollbackConfig {
    fn default() -> Self {
        Self {
            memory_budget: MemoryBudget::Auto,
            max_lines: LineLimit::Auto,
            min_lines: 2_000,
            initial_lines: DEFAULT_INITIAL_LINES,
            low_memory_pct: 10,
        }
    }
}

/// Estimated heap bytes for one scrollback row of `cols` cells.
pub fn row_bytes(cols: u16) -> u64 {
    u64::from(cols.max(1)) * CELL_BYTES + ROW_OVERHEAD_BYTES
}

/// Resolves the configured budget into bytes against the host's total memory.
pub fn resolve_budget_bytes(budget: MemoryBudget, total_bytes: Option<u64>) -> u64 {
    match budget {
        MemoryBudget::Bytes(b) => b.max(MIB),
        MemoryBudget::Percent(p) => total_bytes
            .map(|t| ((u128::from(t) * u128::from(p) / 100) as u64).max(MIB))
            .unwrap_or(FALLBACK_BUDGET_BYTES),
        MemoryBudget::Auto => total_bytes
            .map(|t| (t / 10).max(MIN_AUTO_BUDGET_BYTES))
            .unwrap_or(FALLBACK_BUDGET_BYTES),
    }
}

/// Configuration resolved against a memory snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedPolicy {
    pub budget_bytes: u64,
    pub max_lines: Option<usize>,
    pub min_lines: usize,
    pub initial_lines: usize,
    /// `true` when available memory is below the low-memory watermark.
    pub memory_pressure: bool,
}

impl ResolvedPolicy {
    pub fn resolve(cfg: &ScrollbackConfig, mem: MemorySnapshot) -> Self {
        let max_lines = match cfg.max_lines {
            LineLimit::Auto => None,
            LineLimit::Fixed(n) => Some(n.max(1)),
        };
        let min_lines = cfg.min_lines.min(max_lines.unwrap_or(usize::MAX));
        let memory_pressure = match (mem.total_bytes, mem.available_bytes) {
            (Some(total), Some(avail)) => {
                avail < (total / 100).saturating_mul(u64::from(cfg.low_memory_pct))
            }
            _ => false,
        };
        Self {
            budget_bytes: resolve_budget_bytes(cfg.memory_budget, mem.total_bytes),
            max_lines,
            min_lines,
            initial_lines: cfg.initial_lines,
            memory_pressure,
        }
    }

    /// Largest capacity a single pane of `cols` columns may ever reach.
    pub fn pane_cap_lines(&self, cols: u16) -> usize {
        let by_budget = usize::try_from(self.budget_bytes / row_bytes(cols)).unwrap_or(usize::MAX);
        by_budget
            .min(self.max_lines.unwrap_or(usize::MAX))
            .max(self.min_lines)
    }

    /// Capacity for a fresh pane.
    pub fn initial_capacity(&self, cols: u16) -> usize {
        self.initial_lines
            .max(self.min_lines)
            .min(self.pane_cap_lines(cols))
    }
}

/// Per-pane input to [`plan_capacities`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaneUsage {
    pub used_rows: usize,
    pub capacity: usize,
    pub cols: u16,
}

impl PaneUsage {
    pub fn used_bytes(&self) -> u64 {
        self.used_rows as u64 * row_bytes(self.cols)
    }
}

/// Computes the target capacity for every pane (same order as `usages`).
///
/// * Contraction: under memory pressure, every pane holding more than its fair share
///   (`budget / panes`) shrinks to it; when total usage exceeds the budget, the largest panes
///   shrink to their fair share until usage fits. Never below `min_lines`.
/// * Growth: a pane ≥ 90% full doubles, bounded by its cap and by its usage plus an equal split
///   of the free budget among growing panes (so an active pane can borrow budget idle panes are
///   not using).
pub fn plan_capacities(usages: &[PaneUsage], policy: &ResolvedPolicy) -> Vec<usize> {
    let mut targets: Vec<usize> = usages.iter().map(|u| u.capacity).collect();
    if usages.is_empty() {
        return targets;
    }
    let budget = policy.budget_bytes;
    let fair_bytes = budget / usages.len() as u64;
    let fair_rows = |u: &PaneUsage| {
        usize::try_from(fair_bytes / row_bytes(u.cols))
            .unwrap_or(usize::MAX)
            .max(policy.min_lines)
    };
    let mut used: Vec<u64> = usages.iter().map(PaneUsage::used_bytes).collect();
    let mut total: u64 = used.iter().sum();
    let mut shrunk = vec![false; usages.len()];

    let mut shrink = |i: usize, used: &mut Vec<u64>, total: &mut u64, targets: &mut Vec<usize>| {
        let u = &usages[i];
        let target = fair_rows(u);
        if u.used_rows > target || u.capacity > policy.pane_cap_lines(u.cols) {
            let target = target
                .min(policy.pane_cap_lines(u.cols))
                .max(policy.min_lines);
            let kept = u.used_rows.min(target) as u64 * row_bytes(u.cols);
            *total = total.saturating_sub(used[i]).saturating_add(kept);
            used[i] = kept;
            targets[i] = target;
            shrunk[i] = true;
        }
    };

    if policy.memory_pressure {
        for i in 0..usages.len() {
            shrink(i, &mut used, &mut total, &mut targets);
        }
    } else if total > budget {
        let mut order: Vec<usize> = (0..usages.len()).collect();
        order.sort_by(|&a, &b| used[b].cmp(&used[a]));
        for i in order {
            if total <= budget {
                break;
            }
            shrink(i, &mut used, &mut total, &mut targets);
        }
    }
    // Panes whose fixed cap dropped below their current capacity (e.g. config change).
    for (i, u) in usages.iter().enumerate() {
        let cap = policy.pane_cap_lines(u.cols);
        if !shrunk[i] && targets[i] > cap {
            targets[i] = cap;
            shrunk[i] = true;
        }
    }

    if policy.memory_pressure {
        return targets;
    }
    let growers: Vec<usize> = usages
        .iter()
        .enumerate()
        .filter(|(i, u)| {
            !shrunk[*i]
                && u.used_rows.saturating_mul(10) >= u.capacity.saturating_mul(9)
                && u.capacity < policy.pane_cap_lines(u.cols)
        })
        .map(|(i, _)| i)
        .collect();
    if growers.is_empty() {
        return targets;
    }
    let headroom = budget.saturating_sub(total) / growers.len() as u64;
    for i in growers {
        let u = &usages[i];
        let allowed_rows =
            usize::try_from((used[i] + headroom) / row_bytes(u.cols)).unwrap_or(usize::MAX);
        let next = u
            .capacity
            .saturating_mul(2)
            .min(allowed_rows)
            .min(policy.pane_cap_lines(u.cols));
        if next > u.capacity {
            targets[i] = next;
        }
    }
    targets
}

/// Owned by the window manager: throttles rebalances and remembers which panes it has sized.
pub struct ScrollbackGovernor {
    probe: Box<dyn MemoryProbe>,
    last_run: Option<Instant>,
    seen: BTreeSet<usize>,
    quick_seen: bool,
    last_policy: Option<ResolvedPolicy>,
}

impl std::fmt::Debug for ScrollbackGovernor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrollbackGovernor")
            .field("last_run", &self.last_run)
            .field("last_policy", &self.last_policy)
            .finish_non_exhaustive()
    }
}

impl ScrollbackGovernor {
    pub fn new(probe: Box<dyn MemoryProbe>) -> Self {
        Self {
            probe,
            last_run: None,
            seen: BTreeSet::new(),
            quick_seen: false,
            last_policy: None,
        }
    }

    pub fn set_probe(&mut self, probe: Box<dyn MemoryProbe>) {
        self.probe = probe;
    }

    /// `true` when the periodic interval has elapsed since the last rebalance.
    pub fn due(&self, now: Instant) -> bool {
        self.last_run
            .is_none_or(|t| now.duration_since(t) >= REBALANCE_INTERVAL)
    }

    /// Resolves the policy against a fresh memory probe and records the run time.
    pub fn begin(&mut self, cfg: &ScrollbackConfig, now: Instant) -> ResolvedPolicy {
        self.last_run = Some(now);
        let policy = ResolvedPolicy::resolve(cfg, self.probe.snapshot());
        self.last_policy = Some(policy);
        policy
    }

    /// Policy from the most recent rebalance, if any.
    pub fn last_policy(&self) -> Option<ResolvedPolicy> {
        self.last_policy
    }

    /// Marks `id` as sized; returns `true` the first time a pane is seen.
    pub fn mark_seen(&mut self, id: usize) -> bool {
        self.seen.insert(id)
    }

    /// Marks the persistent quick terminal as sized; returns `true` the first time it is seen.
    pub fn mark_quick_seen(&mut self) -> bool {
        !std::mem::replace(&mut self.quick_seen, true)
    }

    /// Forgets panes that no longer exist.
    pub fn retain_seen(&mut self, live: &BTreeSet<usize>) {
        self.seen.retain(|id| live.contains(id));
    }
}

impl Default for ScrollbackGovernor {
    fn default() -> Self {
        Self::new(Box::new(SystemMemoryProbe))
    }
}

/// Formats bytes as MiB with one decimal.
pub fn format_mib(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / MIB as f64)
}

/// Appends one screen row (as currently visible, honouring the scrollback offset) to `out` as
/// SGR-styled text, returning the number of cells written. Trailing default blanks are trimmed
/// unless `keep_full` (soft-wrapped rows must keep their full width).
pub(crate) fn serialize_row(
    screen: &vt100_ctt::Screen,
    row: u16,
    keep_full: bool,
    out: &mut Vec<u8>,
) -> usize {
    let (_, cols) = screen.size();
    let mut cells: Vec<(&vt100_ctt::Cell, usize)> = Vec::with_capacity(usize::from(cols));
    for c in 0..cols {
        if let Some(cell) = screen.cell(row, c) {
            if cell.is_wide_continuation() {
                continue;
            }
            cells.push((cell, if cell.is_wide() { 2 } else { 1 }));
        }
    }
    if !keep_full {
        while cells.last().is_some_and(|(cell, _)| {
            !cell.has_contents()
                && cell.bgcolor() == vt100_ctt::Color::Default
                && !cell.inverse()
                && !cell.underline()
        }) {
            cells.pop();
        }
    }
    let mut prev: Option<SgrKey> = None;
    let mut width = 0;
    for (cell, w) in cells {
        let key = SgrKey::of(cell);
        if prev != Some(key) {
            key.write(out);
            prev = Some(key);
        }
        if cell.has_contents() {
            out.extend_from_slice(cell.contents().as_bytes());
        } else {
            out.push(b' ');
        }
        width += w;
    }
    width
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct SgrKey {
    fg: vt100_ctt::Color,
    bg: vt100_ctt::Color,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    inverse: bool,
}

impl SgrKey {
    fn of(cell: &vt100_ctt::Cell) -> Self {
        Self {
            fg: cell.fgcolor(),
            bg: cell.bgcolor(),
            bold: cell.bold(),
            dim: cell.dim(),
            italic: cell.italic(),
            underline: cell.underline(),
            inverse: cell.inverse(),
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        let mut s = String::from("\x1b[0");
        for (on, code) in [
            (self.bold, "1"),
            (self.dim, "2"),
            (self.italic, "3"),
            (self.underline, "4"),
            (self.inverse, "7"),
        ] {
            if on {
                s.push(';');
                s.push_str(code);
            }
        }
        push_color(&mut s, self.fg, 30, 90, 38);
        push_color(&mut s, self.bg, 40, 100, 48);
        s.push('m');
        out.extend_from_slice(s.as_bytes());
    }
}

fn push_color(s: &mut String, color: vt100_ctt::Color, base: u8, bright: u8, ext: u8) {
    match color {
        vt100_ctt::Color::Default => {}
        vt100_ctt::Color::Idx(i) if i < 8 => s.push_str(&format!(";{}", base + i)),
        vt100_ctt::Color::Idx(i) if i < 16 => s.push_str(&format!(";{}", bright + i - 8)),
        vt100_ctt::Color::Idx(i) => s.push_str(&format!(";{ext};5;{i}")),
        vt100_ctt::Color::Rgb(r, g, b) => s.push_str(&format!(";{ext};2;{r};{g};{b}")),
    }
}

#[cfg(test)]
#[path = "scrollback_tests.rs"]
mod tests;
