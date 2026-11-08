//! Headless synthetic-traffic fixtures (std only).
//!
//! `pub` rather than `#[cfg(test)]` because integration tests in `tests/` compile the library
//! without `cfg(test)`.

use super::cell::TrafficTap;
use super::classify::observe;
use super::meter::TrafficMeter;
use super::origin::{Origin, Role};

/// Manually advanced millisecond clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct FixtureClock {
    pub now_ms: u64,
}

/// A meter plus injectable channels and a fake clock.
pub struct SyntheticTraffic {
    pub meter: TrafficMeter,
    pub clock: FixtureClock,
    pub cells: Vec<TrafficTap>,
}

impl Default for SyntheticTraffic {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntheticTraffic {
    pub fn new() -> Self {
        let meter = TrafficMeter::new();
        meter.set_classify(true);
        meter.sample(0);
        Self { meter, clock: FixtureClock::default(), cells: Vec::new() }
    }

    /// Opens a channel; returns its index for `rx`/`tx`/`event`/`esc`.
    pub fn channel(&mut self, origin: Origin, pane: Option<usize>, role: Role) -> usize {
        let label = match &origin {
            Origin::Ssh { host, .. } => host.clone(),
            Origin::Ws { client } => client.clone(),
            o => o.class_label().to_string(),
        };
        self.cells.push(self.meter.open_channel(origin, &label, pane, role));
        self.cells.len() - 1
    }

    pub fn rx(&self, i: usize, n: usize) {
        self.cells[i].rx(n);
    }
    pub fn tx(&self, i: usize, n: usize) {
        self.cells[i].tx(n);
    }
    pub fn event(&self, i: usize, n: usize) {
        self.cells[i].event(n);
    }
    /// Observes `n` synthetic `ESC [ 0 m` sequences (4 escape bytes each).
    pub fn esc(&self, i: usize, n: usize) {
        for _ in 0..n {
            observe(&self.cells[i], b"\x1b[0m");
        }
    }

    /// Advances the fake clock, sampling once per elapsed second boundary.
    pub fn advance_ms(&mut self, ms: u64) {
        let target = self.clock.now_ms + ms;
        loop {
            let next = (self.clock.now_ms / 1000 + 1) * 1000;
            if next > target {
                break;
            }
            self.clock.now_ms = next;
            self.meter.sample(next);
        }
        self.clock.now_ms = target;
    }

    /// local pane 1, `ssh prod-db:22` pane 2, `ssh web-1:2222` pane 3.
    pub fn scenario_local_plus_two_ssh() -> Self {
        let mut s = Self::new();
        let l = s.channel(Origin::LocalPty, Some(1), Role::Both);
        let a = s.channel(Origin::Ssh { host: "prod-db".into(), port: 22 }, Some(2), Role::Both);
        let b = s.channel(Origin::Ssh { host: "web-1".into(), port: 2222 }, Some(3), Role::Both);
        for sec in 0..5u64 {
            s.rx(l, 1_000 + 100 * sec as usize);
            s.tx(l, 50);
            s.rx(a, 20_000);
            s.tx(a, 400);
            s.rx(b, 5_000);
            s.esc(a, 10);
            s.advance_ms(1000);
        }
        s
    }

    /// Adds a tmux link (wire + payload pane), a web client, RPC and MCP.
    pub fn scenario_mixed() -> Self {
        let mut s = Self::scenario_local_plus_two_ssh();
        let w = s.channel(Origin::TmuxLink { dest: "bastion".into(), session: "work".into() }, None, Role::Wire);
        let p = s.channel(Origin::TmuxLink { dest: "bastion".into(), session: "work".into() }, Some(4), Role::Payload);
        let ws = s.channel(Origin::Ws { client: "web-1".into() }, None, Role::Wire);
        let rpc = s.channel(Origin::RpcUnix, None, Role::Wire);
        let mcp = s.channel(Origin::Mcp, None, Role::Wire);
        for _ in 0..3 {
            s.rx(w, 9_000);
            s.tx(w, 300);
            s.rx(p, 8_000);
            s.tx(p, 250);
            s.rx(ws, 2_000);
            s.tx(ws, 6_000);
            s.rx(rpc, 120);
            s.tx(rpc, 800);
            s.rx(mcp, 64);
            s.tx(mcp, 512);
            s.event(ws, 1);
            s.advance_ms(1000);
        }
        s
    }
}

/// Deterministic 64-bit LCG stream (no external crate).
pub fn lcg(seed: u64) -> impl Iterator<Item = u64> {
    let mut x = seed;
    std::iter::repeat_with(move || {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        x >> 33
    })
}

#[cfg(test)]
mod tests {
    use super::super::group::{AttributionMap, Filter, GroupKey, SortKey, SortSpec, group_by};
    use super::super::origin::Locality;
    use super::*;

    fn attr() -> AttributionMap {
        let mut a = AttributionMap::default();
        a.insert(1, "main".into(), "zsh".into());
        a.insert(2, "main".into(), "prod".into());
        a.insert(3, "work".into(), "tmux".into());
        a
    }

    fn grp(s: &SyntheticTraffic, k: GroupKey, f: &Filter, sort: SortSpec) -> Vec<super::super::group::GroupRow> {
        group_by(&s.meter.snapshot(500), k, &attr(), f, sort, 100).rows
    }

    fn by_name() -> SortSpec {
        SortSpec { key: SortKey::Name, descending: false }
    }

    fn scenario() -> SyntheticTraffic {
        let mut s = SyntheticTraffic::new();
        let l = s.channel(Origin::LocalPty, Some(1), Role::Both);
        let ssh = s.channel(Origin::Ssh { host: "prod-db".into(), port: 22 }, Some(2), Role::Both);
        let w = s.channel(Origin::TmuxLink { dest: "bastion".into(), session: "work".into() }, None, Role::Wire);
        let p = s.channel(Origin::TmuxLink { dest: "bastion".into(), session: "work".into() }, Some(3), Role::Payload);
        let ws = s.channel(Origin::Ws { client: "web-1".into() }, None, Role::Wire);
        s.rx(l, 100);
        s.tx(l, 10);
        s.rx(ssh, 1000);
        s.tx(ssh, 20);
        s.rx(w, 500);
        s.tx(w, 5);
        s.rx(p, 400);
        s.tx(p, 4);
        s.rx(ws, 7);
        s.tx(ws, 70);
        s.advance_ms(1000);
        s
    }

    #[test]
    fn connection_rows_and_payload_not_double_counted() {
        let s = scenario();
        let rows = grp(&s, GroupKey::Connection, &Filter::default(), by_name());
        let keys: Vec<_> = rows.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, ["local", "ssh:prod-db:22", "tmux-link:bastion/work", "ws:web-1"]);
        let t = rows.iter().find(|r| r.key.starts_with("tmux-link")).unwrap();
        assert_eq!((t.rx_total, t.tx_total), (500, 5), "payload channel excluded");
    }

    #[test]
    fn session_window_pane_locality_direction() {
        let s = scenario();
        let rows = grp(&s, GroupKey::Session, &Filter::default(), by_name());
        let m: Vec<_> = rows.iter().map(|r| (r.key.as_str(), r.rx_total)).collect();
        assert_eq!(m, [("main", 1100), ("work", 400)]);
        let rows = grp(&s, GroupKey::Locality, &Filter::default(), by_name());
        assert_eq!(rows.len(), 3);
        assert_eq!(rows.iter().find(|r| r.key == "local").unwrap().locality, Some(Locality::Local));
        let rows = grp(&s, GroupKey::Direction, &Filter::default(), by_name());
        let m: Vec<_> = rows.iter().map(|r| (r.key.as_str(), r.rx_total + r.tx_total)).collect();
        assert_eq!(m, [("rx", 100 + 1000 + 500 + 7), ("tx", 10 + 20 + 5 + 70)]);
        assert_eq!(grp(&s, GroupKey::Pane, &Filter::default(), by_name()).len(), 3);
        assert_eq!(grp(&s, GroupKey::Window, &Filter::default(), by_name())[0].key, "main/prod");
    }

    #[test]
    fn unattributed_pane_maps_to_dash() {
        let mut s = SyntheticTraffic::new();
        let c = s.channel(Origin::LocalPty, None, Role::Both);
        s.rx(c, 9);
        let rows = grp(&s, GroupKey::Session, &Filter::default(), by_name());
        assert_eq!(rows[0].key, "-");
    }

    #[test]
    fn sort_and_filter() {
        let s = scenario();
        let r = grp(&s, GroupKey::Connection, &Filter::default(), SortSpec { key: SortKey::Total, descending: true });
        assert_eq!(r[0].key, "ssh:prod-db:22");
        let r = grp(&s, GroupKey::Connection, &Filter::default(), SortSpec { key: SortKey::Total, descending: false });
        assert_eq!(r[0].key, "ws:web-1");
        for k in [SortKey::Rate1s, SortKey::Rate10s, SortKey::Rate1m, SortKey::Peak] {
            let r = grp(&s, GroupKey::Connection, &Filter::default(), SortSpec { key: k, descending: true });
            assert_eq!(r[0].key, "ssh:prod-db:22");
        }
        let f = Filter { text: "PROD".into(), locality: None };
        assert_eq!(grp(&s, GroupKey::Connection, &f, by_name()).len(), 1);
        let f = Filter { text: "tmux".into(), locality: None };
        assert_eq!(grp(&s, GroupKey::Session, &f, by_name()).len(), 1, "matches session 'work' via tmux window");
        let f = Filter { text: String::new(), locality: Some(Locality::Loopback) };
        assert_eq!(grp(&s, GroupKey::Connection, &f, by_name())[0].key, "ws:web-1");
        let f = Filter { text: "x".repeat(500), locality: None };
        assert!(grp(&s, GroupKey::Connection, &f, by_name()).is_empty());
        let res = group_by(&s.meter.snapshot(500), GroupKey::Connection, &attr(), &Filter::default(), by_name(), 2);
        assert!(res.truncated && res.rows.len() == 2);
    }

    #[test]
    fn conservation_over_lcg_injection() {
        let mut s = SyntheticTraffic::new();
        let mut idx = Vec::new();
        for i in 0..6usize {
            idx.push(s.channel(Origin::LocalPty, Some(i + 1), Role::Both));
        }
        idx.push(s.channel(Origin::Ssh { host: "h".into(), port: 22 }, Some(10), Role::Both));
        idx.push(s.channel(Origin::TmuxLink { dest: "d".into(), session: "s".into() }, None, Role::Wire));
        idx.push(s.channel(Origin::TmuxLink { dest: "d".into(), session: "s".into() }, Some(11), Role::Payload));
        idx.push(s.channel(Origin::Ws { client: "web-1".into() }, None, Role::Wire));
        idx.push(s.channel(Origin::Http, None, Role::Wire));
        let mut rng = lcg(42);
        let (mut wire, mut payload) = (0u64, 0u64);
        let roles: Vec<Role> = {
            let snap = s.meter.snapshot(500);
            idx.iter().map(|&i| snap.channels.iter().find(|c| c.meta.channel_id == s.cells[i].id()).unwrap().meta.role).collect()
        };
        for step in 0..200 {
            let ci = (rng.next().unwrap() % idx.len() as u64) as usize;
            let n = (rng.next().unwrap() % 5000) as usize;
            let tx = rng.next().unwrap().is_multiple_of(2);
            if tx { s.tx(idx[ci], n) } else { s.rx(idx[ci], n) }
            if roles[ci].is_wire() { wire += n as u64; }
            if roles[ci].is_payload() { payload += n as u64; }
            if step % 50 == 0 { s.advance_ms(1000); }
        }
        s.advance_ms(1000);
        let mut a = AttributionMap::default();
        for p in 1..=11 { a.insert(p, format!("s{}", p % 3), format!("w{p}")); }
        let snap = s.meter.snapshot(500);
        let sum = |k| {
            group_by(&snap, k, &a, &Filter::default(), by_name(), 1000)
                .rows.iter().map(|r| r.rx_total + r.tx_total).sum::<u64>()
        };
        let grand = group_by(&snap, GroupKey::Connection, &a, &Filter::default(), by_name(), 1000).totals;
        assert_eq!(grand.rx + grand.tx, wire);
        for k in [GroupKey::Connection, GroupKey::Origin, GroupKey::Locality, GroupKey::Direction] {
            assert_eq!(sum(k), wire, "{k:?}");
        }
        for k in [GroupKey::Session, GroupKey::Window] {
            let rows_payload: u64 = payload
                - snap.channels.iter().filter(|c| c.meta.role.is_payload() && c.meta.pane.is_none())
                    .map(|c| c.reading.rx_bytes + c.reading.tx_bytes).sum::<u64>();
            assert_eq!(sum(k), rows_payload, "{k:?}");
        }
        assert_eq!(sum(GroupKey::Pane), sum(GroupKey::Session));
    }
}
