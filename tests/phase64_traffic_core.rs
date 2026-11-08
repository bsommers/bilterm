//! Phase 64 core: WindowManager seam, headless injection, perf budgets (TMON-01/02/05).

use bilterm::traffic::fixture::SyntheticTraffic;
use bilterm::traffic::{Origin, Role};
use bilterm::wm::WindowManager;
use bilterm::wm::split_tree::SplitDirection;

#[test]
fn headless_wm_has_empty_enabled_snapshot() {
    let wm = WindowManager::new_headless(24, 80);
    let s = wm.traffic_snapshot(100);
    assert!(s.channels.is_empty());
    assert!(s.enabled);
}

#[test]
fn rpc_channel_shows_up_after_tick() {
    let mut wm = WindowManager::new_headless(24, 80);
    let cell = wm.traffic_meter().open_channel(Origin::RpcUnix, "rpc", None, Role::Wire);
    cell.rx(10);
    assert!(!wm.traffic_tick(1000), "no extra redraw while the monitor is hidden");
    let s = wm.traffic_snapshot(10);
    assert_eq!(s.channels.len(), 1);
    assert_eq!(s.channels[0].reading.rx_bytes, 10);
    assert_eq!(s.channels[0].rates.rx_1, 10);
}

#[test]
fn focus_and_deck_shuffle_still_work() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.focus_and_raise(1);
    assert_eq!(wm.active_pane_id(), 1);
    assert!(wm.shuffle_deck_next().is_some());
    assert!(wm.shuffle_deck_prev().is_some());
    assert!(wm.focus_and_raise(p2));
}

#[test]
fn tick_on_headless_panes_is_identity() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.split_focused(SplitDirection::Horizontal).unwrap();
    wm.split_focused(SplitDirection::Vertical).unwrap();
    for t in [0, 500, 1000, 2000, 90_000] {
        wm.traffic_tick(t);
    }
    assert!(wm.traffic_snapshot(500).channels.is_empty());
    assert!(!wm.poll_all_ptys() || wm.traffic_snapshot(500).channels.is_empty());
}

#[test]
fn two_window_managers_do_not_share_state() {
    let a = WindowManager::new_headless(10, 40);
    let b = WindowManager::new_headless(10, 40);
    a.traffic_meter().open_channel(Origin::Mcp, "m", None, Role::Wire).rx(1);
    assert_eq!(a.traffic_snapshot(5).channels.len(), 1);
    assert!(b.traffic_snapshot(5).channels.is_empty());
}

#[test]
fn fixture_scenarios_are_deterministic() {
    let a = SyntheticTraffic::scenario_mixed().meter.snapshot(500);
    let b = SyntheticTraffic::scenario_mixed().meter.snapshot(500);
    let sum = |s: &bilterm::traffic::TrafficSnapshot| {
        s.channels.iter().map(|c| (c.reading.rx_bytes, c.reading.tx_bytes, c.rates.rx_10)).collect::<Vec<_>>()
    };
    assert_eq!(sum(&a), sum(&b));
    assert!(a.channels.len() >= 8);
}

// ---------------------------------------------------------------------------------------------
// Perf calibration (ADR section 7). Medians of 5 runs; strict absolute budgets in release,
// ratio-only in debug.
// ---------------------------------------------------------------------------------------------

mod perf {
    use bilterm::traffic::cell::{F_CLASSIFY, F_ENABLED};
    use bilterm::traffic::classify::observe;
    use bilterm::traffic::{
        AttributionMap, BUCKETS, ChannelCounters, Filter, GroupKey, Origin, Ring, Role, SortSpec,
        TrafficMeter, group_by,
    };
    use std::hint::black_box;
    use std::time::Instant;

    fn median(mut v: Vec<f64>) -> f64 {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    }

    /// Median over 5 runs of `f(iters)` expressed in ns per iteration.
    fn ns_per_iter(iters: u64, mut f: impl FnMut(u64)) -> f64 {
        median(
            (0..5)
                .map(|_| {
                    let t = Instant::now();
                    f(iters);
                    t.elapsed().as_nanos() as f64 / iters as f64
                })
                .collect(),
        )
    }

    /// Median over 5 runs of `f(iters)` expressed in MB/s given `bytes_per_iter`.
    fn mb_per_s(iters: u64, bytes_per_iter: usize, f: impl FnMut(u64)) -> f64 {
        let ns = ns_per_iter(iters, f);
        bytes_per_iter as f64 / ns * 1000.0
    }

    #[test]
    fn perf_budgets_measured() {
        const N: u64 = 1_000_000;
        let release = !cfg!(debug_assertions);

        let empty = ns_per_iter(N, |n| {
            for i in 0..n {
                black_box(i);
            }
        });
        let off = ChannelCounters::with_flags(0);
        let disabled = ns_per_iter(N, |n| {
            for i in 0..n {
                off.rx(black_box(64));
                black_box(i);
            }
        });
        let on = ChannelCounters::with_flags(F_ENABLED);
        let enabled = ns_per_iter(N, |n| {
            for i in 0..n {
                on.rx(black_box(64));
                black_box(i);
            }
        });

        let cls = ChannelCounters::with_flags(F_ENABLED | F_CLASSIFY);
        let printable = vec![b'a'; 8192];
        let mut dense = Vec::with_capacity(8192);
        while dense.len() + 20 <= 8192 {
            dense.extend_from_slice(b"\x1b[38;2;123;45;67m");
        }
        let (pn, dn) = (20_000u64, 20_000u64);
        let ground = mb_per_s(pn, printable.len(), |n| {
            for _ in 0..n {
                observe(&cls, black_box(&printable));
            }
        });
        let escdense = mb_per_s(dn, dense.len(), |n| {
            for _ in 0..n {
                observe(&cls, black_box(&dense));
            }
        });

        let m = TrafficMeter::new();
        let keep: Vec<_> = (0..255)
            .map(|i| {
                let c = m.open_channel(Origin::Ssh { host: format!("h{i}"), port: 22 }, "h", Some(i + 1), Role::Both);
                c.rx(1000);
                c
            })
            .collect();
        m.sample(1000);
        let sample_nb = ns_per_iter(N, |n| {
            for _ in 0..n {
                black_box(m.sample(black_box(1500)));
            }
        });
        let mut sec = 1u64;
        let sample_b = ns_per_iter(300, |n| {
            for _ in 0..n {
                sec += 1;
                for c in keep.iter().take(8) {
                    c.rx(10);
                }
                black_box(m.sample(sec * 1000));
            }
        }) / 1000.0; // ns -> us
        let attr = AttributionMap::default();
        let snap_group = ns_per_iter(300, |n| {
            for _ in 0..n {
                let s = m.snapshot(500);
                black_box(group_by(&s, GroupKey::Connection, &attr, &Filter::default(), SortSpec::default(), 500));
            }
        }) / 1000.0;

        eprintln!(
            "PERF[{}] empty={empty:.2}ns disabled={disabled:.2}ns enabled={enabled:.2}ns \
             classify_ground={ground:.0}MB/s classify_dense={escdense:.0}MB/s \
             sample_nonboundary={sample_nb:.1}ns sample_boundary@256={sample_b:.1}us \
             snapshot+group@256={snap_group:.1}us",
            if release { "release" } else { "debug" }
        );

        // Ratio gates (both profiles).
        assert!(disabled <= 4.0 * empty.max(0.25) + 1.0, "disabled rx too slow: {disabled}ns vs empty {empty}ns");
        assert!(enabled <= 20.0 * disabled.max(0.25) + 5.0, "enabled rx too slow: {enabled}ns");
        if release {
            assert!(disabled <= 2.0, "disabled <= 1ns budget (+headroom): {disabled}");
            assert!(enabled <= 10.0, "enabled <= 5ns budget (+headroom): {enabled}");
            assert!(ground >= 1000.0, "classify ground >= 1 GB/s: {ground}");
            assert!(escdense >= 300.0, "classify dense >= 300 MB/s: {escdense}");
            assert!(sample_nb <= 100.0, "sample non-boundary <= 50ns (+headroom): {sample_nb}");
            assert!(sample_b <= 100.0, "sample boundary@256 <= 50us (+headroom): {sample_b}");
            assert!(snap_group <= 400.0, "snapshot+group@256 <= 200us (+headroom): {snap_group}");
        }
        drop(keep);
    }

    #[test]
    fn memory_ceiling_bound() {
        use std::mem::size_of;
        // One shared ring carries both directions; 256 B allows for per-channel heap in the meta.
        let per_channel = size_of::<ChannelCounters>() + size_of::<Ring>() + 256;
        let total = bilterm::traffic::MAX_CHANNELS * per_channel;
        eprintln!("MEM per_channel={per_channel}B total={total}B buckets={BUCKETS}");
        assert!(total <= 600_000, "memory ceiling exceeded: {total}");
    }
}
