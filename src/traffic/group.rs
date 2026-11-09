//! Pure group-by / sort / filter over a [`TrafficSnapshot`] (std only, no clock, no WM).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::meter::{ChannelRow, Totals, TrafficSnapshot};
use super::origin::Locality;
use super::window::Rates;
use super::{BUCKETS, FILTER_CAP};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GroupKey {
    Connection,
    Origin,
    Locality,
    Session,
    Window,
    Pane,
    Direction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SortKey {
    Rate1s,
    Rate10s,
    Rate1m,
    Total,
    Peak,
    Name,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortSpec {
    pub key: SortKey,
    pub descending: bool,
}

impl Default for SortSpec {
    fn default() -> Self {
        Self { key: SortKey::Rate1s, descending: true }
    }
}

/// Case-insensitive substring filter over label, class, session and window; optional locality.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub text: String,
    pub locality: Option<Locality>,
}

/// `pane -> (session, window title)`, built late-bound by the window manager per snapshot.
#[derive(Clone, Debug, Default)]
pub struct AttributionMap(pub HashMap<usize, (String, String)>);

impl AttributionMap {
    pub fn get(&self, pane: usize) -> Option<&(String, String)> {
        self.0.get(&pane)
    }
    pub fn insert(&mut self, pane: usize, session: String, window: String) {
        self.0.insert(pane, (session, window));
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GroupRow {
    pub key: String,
    pub label: String,
    pub locality: Option<Locality>,
    pub rx_total: u64,
    pub tx_total: u64,
    pub rates: Rates,
    /// Max single second of the summed per-second series (last minute).
    pub peak_rx: u64,
    pub peak_tx: u64,
    pub esc_pct: u64,
    pub events_per_s: u64,
    pub msgs_per_s: u64,
    pub series_rx: Vec<u64>,
    pub series_tx: Vec<u64>,
    pub channel_count: usize,
    /// Distinct panes contributing.
    pub children: usize,
}

#[derive(Clone, Debug, Default)]
pub struct GroupResult {
    pub rows: Vec<GroupRow>,
    /// Grand totals of the whole unfiltered snapshot (Wire|Both channels + evicted + omitted).
    pub totals: Totals,
    pub truncated: bool,
}

struct Acc {
    label: String,
    locality: Option<Locality>,
    rx: u64,
    tx: u64,
    esc: u64,
    rates: Rates,
    s_rx: [u64; BUCKETS],
    s_tx: [u64; BUCKETS],
    channels: usize,
    panes: BTreeSet<usize>,
}

impl Acc {
    fn new(label: String, locality: Option<Locality>) -> Self {
        Self {
            label,
            locality,
            rx: 0,
            tx: 0,
            esc: 0,
            rates: Rates::default(),
            s_rx: [0; BUCKETS],
            s_tx: [0; BUCKETS],
            channels: 0,
            panes: BTreeSet::new(),
        }
    }

    fn add(&mut self, r: &ChannelRow, dir: Option<bool>) {
        // dir: None = both, Some(true) = rx only, Some(false) = tx only.
        let (do_rx, do_tx) = match dir {
            None => (true, true),
            Some(rx) => (rx, !rx),
        };
        if do_rx {
            self.rx += r.reading.rx_bytes;
            self.esc += r.reading.esc_bytes;
            self.rates.rx_1 += r.rates.rx_1;
            self.rates.rx_10 += r.rates.rx_10;
            self.rates.rx_60 += r.rates.rx_60;
            for (a, b) in self.s_rx.iter_mut().zip(r.series_rx.iter()) {
                *a += *b;
            }
        }
        if do_tx {
            self.tx += r.reading.tx_bytes;
            self.rates.tx_1 += r.rates.tx_1;
            self.rates.tx_10 += r.rates.tx_10;
            self.rates.tx_60 += r.rates.tx_60;
            for (a, b) in self.s_tx.iter_mut().zip(r.series_tx.iter()) {
                *a += *b;
            }
        }
        self.rates.msgs_10 += r.rates.msgs_10;
        self.rates.events_10 += r.rates.events_10;
        self.rates.esc_10 += r.rates.esc_10;
        self.channels += 1;
        if let Some(p) = r.meta.pane {
            self.panes.insert(p);
        }
    }

    fn into_row(self, key: String) -> GroupRow {
        GroupRow {
            key,
            label: self.label,
            locality: self.locality,
            rx_total: self.rx,
            tx_total: self.tx,
            peak_rx: self.s_rx.iter().copied().max().unwrap_or(0),
            peak_tx: self.s_tx.iter().copied().max().unwrap_or(0),
            esc_pct: if self.rx == 0 { 0 } else { self.esc * 100 / self.rx },
            events_per_s: self.rates.events_10,
            msgs_per_s: self.rates.msgs_10,
            rates: self.rates,
            series_rx: self.s_rx.to_vec(),
            series_tx: self.s_tx.to_vec(),
            channel_count: self.channels,
            children: self.panes.len(),
        }
    }
}

fn matches(r: &ChannelRow, f: &Filter, needle: &str, attr: &AttributionMap) -> bool {
    if let Some(l) = f.locality
        && r.meta.origin.locality() != l
    {
        return false;
    }
    if needle.is_empty() {
        return true;
    }
    let (sess, win) = r
        .meta
        .pane
        .and_then(|p| attr.get(p))
        .map(|(s, w)| (s.as_str(), w.as_str()))
        .unwrap_or(("", ""));
    [r.meta.label.as_str(), r.meta.origin.class_label(), r.meta.connection.as_str(), sess, win]
        .iter()
        .any(|h| h.to_ascii_lowercase().contains(needle))
}

fn sort_value(r: &GroupRow, k: SortKey) -> u64 {
    match k {
        SortKey::Rate1s => r.rates.rx_1 + r.rates.tx_1,
        SortKey::Rate10s => r.rates.rx_10 + r.rates.tx_10,
        SortKey::Rate1m => r.rates.rx_60 + r.rates.tx_60,
        SortKey::Total => r.rx_total + r.tx_total,
        SortKey::Peak => r.peak_rx + r.peak_tx,
        SortKey::Name => 0,
    }
}

/// Groups, filters, sorts and limits `snap`. Pure.
pub fn group_by(
    snap: &TrafficSnapshot,
    key: GroupKey,
    attr: &AttributionMap,
    filter: &Filter,
    sort: SortSpec,
    limit: usize,
) -> GroupResult {
    let needle: String = filter.text.chars().take(FILTER_CAP).collect::<String>().to_ascii_lowercase();

    let mut totals = snap.evicted;
    totals.add(&snap.omitted);
    for r in snap.channels.iter().filter(|r| r.meta.role.is_wire()) {
        totals.add_reading(&r.reading);
    }

    let mut accs: BTreeMap<String, Acc> = BTreeMap::new();
    for r in snap.channels.iter().filter(|r| matches(r, filter, &needle, attr)) {
        let wire = r.meta.role.is_wire();
        let payload = r.meta.role.is_payload();
        let (sess, win) = r
            .meta
            .pane
            .and_then(|p| attr.get(p))
            .map(|(s, w)| (s.clone(), w.clone()))
            .unwrap_or_else(|| ("-".to_string(), "-".to_string()));
        let loc = r.meta.origin.locality();
        match key {
            GroupKey::Connection if wire => {
                let k = r.meta.connection.as_str().to_string();
                accs.entry(k.clone()).or_insert_with(|| Acc::new(k, Some(loc))).add(r, None);
            }
            GroupKey::Origin if wire => {
                let k = r.meta.origin.class_label().to_string();
                accs.entry(k.clone()).or_insert_with(|| Acc::new(k, Some(loc))).add(r, None);
            }
            GroupKey::Locality if wire => {
                let k = loc.as_str().to_string();
                accs.entry(k.clone()).or_insert_with(|| Acc::new(k, Some(loc))).add(r, None);
            }
            GroupKey::Direction if wire => {
                for (k, rx) in [("rx", true), ("tx", false)] {
                    accs.entry(k.to_string())
                        .or_insert_with(|| Acc::new(k.to_string(), None))
                        .add(r, Some(rx));
                }
            }
            GroupKey::Session if payload => {
                accs.entry(sess.clone()).or_insert_with(|| Acc::new(sess, None)).add(r, None);
            }
            GroupKey::Window if payload => {
                let k = format!("{sess}/{win}");
                accs.entry(k.clone()).or_insert_with(|| Acc::new(k, None)).add(r, None);
            }
            GroupKey::Pane if payload => {
                if let Some(p) = r.meta.pane {
                    let k = format!("pane {p}");
                    let label = if win == "-" { k.clone() } else { format!("pane {p}: {win}") };
                    accs.entry(k).or_insert_with(|| Acc::new(label, None)).add(r, None);
                }
            }
            _ => {}
        }
    }

    let mut rows: Vec<GroupRow> = accs.into_iter().map(|(k, a)| a.into_row(k)).collect();
    // `rows` is name-ascending (BTreeMap order); stable sort keeps that as the tiebreak.
    rows.sort_by(|a, b| {
        let ord = if sort.key == SortKey::Name {
            a.key.cmp(&b.key)
        } else {
            sort_value(a, sort.key).cmp(&sort_value(b, sort.key))
        };
        let ord = if sort.descending { ord.reverse() } else { ord };
        ord.then_with(|| a.key.cmp(&b.key))
    });
    let truncated = rows.len() > limit;
    rows.truncate(limit);
    GroupResult { rows, totals, truncated }
}
