//! Naming, validated queries and the single JSON builder behind `:traffic show`, `traffic.snapshot`
//! and `bilterm_traffic` (so colon, RPC, HTTP and MCP return byte-identical data).

use std::fmt::Write;

use crate::traffic::{
    FILTER_CAP, Filter, GroupKey, GroupResult, GroupRow, Locality, SNAPSHOT_LIMIT_MAX, SortKey, SortSpec,
};

pub const GROUP_NAMES: &[(&str, GroupKey)] = &[
    ("connection", GroupKey::Connection),
    ("session", GroupKey::Session),
    ("window", GroupKey::Window),
    ("pane", GroupKey::Pane),
    ("origin", GroupKey::Origin),
    ("locality", GroupKey::Locality),
    ("direction", GroupKey::Direction),
];
pub const SORT_NAMES: &[(&str, SortKey)] = &[
    ("rate", SortKey::Rate1s),
    ("rate10", SortKey::Rate10s),
    ("rate1m", SortKey::Rate1m),
    ("total", SortKey::Total),
    ("peak", SortKey::Peak),
    ("name", SortKey::Name),
];
pub const LOCALITY_NAMES: &[(&str, Option<Locality>)] = &[
    ("all", None),
    ("local", Some(Locality::Local)),
    ("loopback", Some(Locality::Loopback)),
    ("network", Some(Locality::Network)),
];

pub fn group_name(g: GroupKey) -> &'static str {
    GROUP_NAMES.iter().find(|(_, k)| *k == g).map_or("connection", |(n, _)| n)
}
pub fn sort_name(s: SortKey) -> &'static str {
    SORT_NAMES.iter().find(|(_, k)| *k == s).map_or("rate", |(n, _)| n)
}
pub fn locality_name(l: Option<Locality>) -> &'static str {
    LOCALITY_NAMES.iter().find(|(_, k)| *k == l).map_or("all", |(n, _)| n)
}

/// Accepted values list for error messages, e.g. `connection|session|...`.
pub fn names_of<T>(table: &[(&'static str, T)]) -> String {
    table.iter().map(|(n, _)| *n).collect::<Vec<_>>().join("|")
}

pub fn parse_group(s: &str) -> Result<GroupKey, String> {
    let l = s.to_ascii_lowercase();
    GROUP_NAMES
        .iter()
        .find(|(n, _)| *n == l || (l == "conn" && *n == "connection") || (l == "win" && *n == "window"))
        .map(|(_, k)| *k)
        .ok_or_else(|| format!("unknown group '{s}' (use {})", names_of(GROUP_NAMES)))
}
pub fn parse_sort(s: &str) -> Result<SortKey, String> {
    let l = s.to_ascii_lowercase();
    SORT_NAMES
        .iter()
        .find(|(n, _)| *n == l || (l == "rate1s" && *n == "rate"))
        .map(|(_, k)| *k)
        .ok_or_else(|| format!("unknown sort '{s}' (use {})", names_of(SORT_NAMES)))
}
pub fn parse_locality(s: &str) -> Result<Option<Locality>, String> {
    let l = s.to_ascii_lowercase();
    LOCALITY_NAMES
        .iter()
        .find(|(n, _)| *n == l)
        .map(|(_, k)| *k)
        .ok_or_else(|| format!("unknown locality '{s}' (use {})", names_of(LOCALITY_NAMES)))
}

/// Strips control characters and truncates to [`FILTER_CAP`] characters.
pub fn clean_filter(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(FILTER_CAP).collect()
}

/// Validated parameters shared by `:traffic show` and the RPC `traffic.snapshot` handler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrafficQuery {
    pub group: GroupKey,
    pub sort: SortSpec,
    pub filter: Filter,
    /// Series length in seconds (`1..=60`).
    pub window: usize,
    /// Row limit (`1..=500`).
    pub limit: usize,
}

impl Default for TrafficQuery {
    fn default() -> Self {
        Self {
            group: GroupKey::Connection,
            sort: SortSpec::default(),
            filter: Filter::default(),
            window: 60,
            limit: 100,
        }
    }
}

impl TrafficQuery {
    pub fn with_limit(mut self, n: usize) -> Self {
        self.limit = n.clamp(1, SNAPSHOT_LIMIT_MAX);
        self
    }
    pub fn with_window(mut self, n: usize) -> Self {
        self.window = n.clamp(1, 60);
        self
    }
}

fn esc(out: &mut String, s: &str) {
    out.push('"');
    for ch in crate::vt::sanitize_display(s, crate::traffic::LABEL_CAP * 2).chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn series(out: &mut String, v: &[u64], window: usize) {
    out.push('[');
    let from = v.len().saturating_sub(window);
    for (i, n) in v[from..].iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{n}");
    }
    out.push(']');
}

fn row_json(out: &mut String, r: &GroupRow, window: usize) {
    out.push_str("{\"key\":");
    esc(out, &r.key);
    out.push_str(",\"label\":");
    esc(out, &r.label);
    out.push_str(",\"locality\":");
    match r.locality {
        Some(l) => esc(out, l.as_str()),
        None => out.push_str("null"),
    }
    let x = &r.rates;
    let _ = write!(
        out,
        ",\"rx_total\":{},\"tx_total\":{},\"rate_1s_rx\":{},\"rate_1s_tx\":{},\"rate_10s_rx\":{},\"rate_10s_tx\":{},\
         \"rate_1m_rx\":{},\"rate_1m_tx\":{},\"peak_rx\":{},\"peak_tx\":{},\"esc_pct\":{},\"events_per_s\":{},\
         \"msgs_per_s\":{},\"channels\":{},\"children\":{},\"series_rx\":",
        r.rx_total, r.tx_total, x.rx_1, x.tx_1, x.rx_10, x.tx_10, x.rx_60, x.tx_60, r.peak_rx, r.peak_tx,
        r.esc_pct, r.events_per_s, r.msgs_per_s, r.channel_count, r.children
    );
    series(out, &r.series_rx, window);
    out.push_str(",\"series_tx\":");
    series(out, &r.series_tx, window);
    out.push('}');
}

/// Builds the canonical snapshot JSON text for one grouped result.
pub fn group_json(res: &GroupResult, q: &TrafficQuery, at_ms: u64, enabled: bool) -> String {
    let t = &res.totals;
    let esc_pct = if t.rx == 0 { 0 } else { t.esc * 100 / t.rx };
    let mut out = String::with_capacity(256 + res.rows.len() * 320);
    let _ = write!(
        out,
        "{{\"at_ms\":{at_ms},\"enabled\":{enabled},\"group_by\":\"{}\",\"sort\":\"{}\",\"descending\":{},\
         \"locality\":\"{}\",\"window\":{},\"limit\":{},\"totals\":{{\"rx\":{},\"tx\":{},\"msgs\":{},\"esc_pct\":{esc_pct}}},\"rows\":[",
        group_name(q.group),
        sort_name(q.sort.key),
        q.sort.descending,
        locality_name(q.filter.locality),
        q.window,
        q.limit,
        t.rx,
        t.tx,
        t.msgs
    );
    for (i, r) in res.rows.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        row_json(&mut out, r, q.window);
    }
    let _ = write!(out, "],\"truncated\":{}}}", res.truncated);
    out
}

/// One-group history JSON: last `n` per-second buckets (oldest first).
pub fn history_json(row: Option<&GroupRow>, key: &str, group: GroupKey, n: usize) -> String {
    let mut out = String::new();
    let _ = write!(out, "{{\"group_by\":\"{}\",\"key\":", group_name(group));
    esc(&mut out, key);
    out.push_str(",\"found\":");
    out.push_str(if row.is_some() { "true" } else { "false" });
    out.push_str(",\"series_rx\":");
    series(&mut out, row.map_or(&[][..], |r| &r.series_rx), n.clamp(1, 60));
    out.push_str(",\"series_tx\":");
    series(&mut out, row.map_or(&[][..], |r| &r.series_tx), n.clamp(1, 60));
    out.push('}');
    out
}
