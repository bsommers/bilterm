//! Phase 51-03 (UIT-01, D-01): `:agents demo stop` undoes exactly what `:agents demo` created.
//! `primitive_` tests cover the bus and topology removal primitives.

use std::time::{Duration, Instant};

use bilterm::wm::agents_window::NodeKey;
use bilterm::wm::traffic::{TrafficBus, TrafficEndpoint, TrafficKind};

fn bus_with_mixed_events() -> (TrafficBus, Instant) {
    let now = Instant::now();
    let mut bus = TrafficBus::new();
    let pane = TrafficEndpoint::Pane;
    bus.record(TrafficEndpoint::User, pane(1), TrafficKind::Input, "ls", false, now);
    bus.record(pane(1), pane(2), TrafficKind::Message, "[demo] a", false, now);
    bus.record(pane(2), pane(1), TrafficKind::Message, "[demo] b", false, now);
    bus.record(pane(1), pane(3), TrafficKind::Message, "[demo] c", false, now);
    bus.record(TrafficEndpoint::User, pane(2), TrafficKind::Input, "pwd", false, now + Duration::from_secs(1));
    (bus, now)
}

#[test]
fn primitive_retain_events_removes_only_matching() {
    let (mut bus, _) = bus_with_mixed_events();
    assert_eq!(bus.len(), 5);
    let removed = bus.retain_events(|e| e.kind != TrafficKind::Message);
    assert_eq!(removed, 3);
    let left: Vec<&str> = bus.iter().map(|e| e.preview.as_str()).collect();
    assert_eq!(left, vec!["ls", "pwd"], "user events survive in order");
}

#[test]
fn primitive_retain_events_keeps_seq_counters() {
    let (mut bus, now) = bus_with_mixed_events();
    assert_eq!(bus.latest_seq(), 5);
    let dropped = bus.dropped();
    bus.retain_events(|e| e.kind != TrafficKind::Message);
    assert_eq!(bus.latest_seq(), 5, "latest_seq never decrements");
    assert_eq!(bus.dropped(), dropped, "retain is not ring eviction");
    let next = bus.record(TrafficEndpoint::User, TrafficEndpoint::Pane(1), TrafficKind::Input, "x", false, now);
    assert_eq!(next, 6, "sequence numbers are never reused");
}

#[test]
fn primitive_remove_pair_removes_one_pair() {
    let now = Instant::now();
    let mut bus = TrafficBus::new();
    let pane = TrafficEndpoint::Pane;
    bus.record(pane(1), pane(2), TrafficKind::Message, "m", false, now);
    bus.record(pane(2), pane(3), TrafficKind::Message, "m", false, now);
    bus.record(TrafficEndpoint::User, pane(1), TrafficKind::Input, "m", false, now);
    assert_eq!(bus.topology().len(), 3);
    let (a, b) = (NodeKey::pane(2), NodeKey::pane(1));
    let mut topo = bus.topology().clone();
    assert!(topo.remove_pair(&a, &b), "argument order does not matter");
    assert!(!topo.remove_pair(&a, &b), "second removal is a no-op");
    assert_eq!(topo.len(), 2);
    assert!(topo.pair(&NodeKey::pane(2), &NodeKey::pane(3)).is_some());
    assert!(topo.pair(&NodeKey("user".to_string()), &NodeKey::pane(1)).is_some());
}
