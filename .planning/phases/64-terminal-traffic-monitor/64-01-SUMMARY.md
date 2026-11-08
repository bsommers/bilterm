---
phase: 64-terminal-traffic-monitor
plan: 01
status: complete
requirements: [TMON-01, TMON-02, TMON-05]
---

# Plan 64-01 Summary

Leaf `src/traffic/` metrics layer, config, WindowManager seam, layer-DAG/byte-boundary fixtures and measured perf gates.

## Delivered
- `src/traffic/{mod,origin,cell,classify,window,meter,group,fixture}.rs`: std-only leaf (enforced by `tests/phase64_layer_dag.rs`).
- `src/config/traffic.rs` plus a `traffic` field on `BiltermConfig` (`[traffic]`, defaults enabled/!classify/!oia/label_hosts).
- `src/wm/traffic_view.rs`, placeholder `traffic_monitor.rs` / `traffic_monitor_geometry.rs`, single edit of `src/wm/mod.rs` (fields, three `blur()` lines, one `traffic_tick` call in `poll_all_ptys`).
- `tests/phase64_traffic_core.rs` (WM seam, isolation, perf, memory), `tests/phase64_layer_dag.rs` (leaf rule, 33-entry HOOK_MANIFEST all Pending, BOUNDARY_EXEMPT audit).
- `scripts/verify.sh` perf tier now includes `t:phase64_traffic_core`; ADR section 7 holds measured numbers.

## Measured (release, Ryzen 9 7950X)
disabled rx 0.37 ns, enabled rx 3.28 ns, classify 3519 / 1305 MB/s (ground / dense), sample 1.9 ns / 29.9 us @256, snapshot+group_by 185.9 us @256, memory 0.59 MB. All within budget (snapshot+group within 7%).

## Deviations
1. `WindowManager` already has a `traffic: TrafficBus` field (Phase 50), so the meter field is `traffic_meter` (private).
2. `traffic_tick` is `pub` (not `pub(crate)`) so integration tests can drive it with an injected clock; it returns true only when the monitor window is visible, so idle workspaces gain no redraws and classic goldens are unchanged.
3. Plan text says the OSC `\x1b]0;title\x07` is 11 escape bytes; it is 10 (ESC ] 0 ; t i t l e BEL). Tests assert 10.
4. `Bucket` uses u32 for msgs/esc/events (32 bytes) and one ring carries both directions, so the memory formula is `256 * (size_of cell + size_of Ring + 256)` rather than two rings.
5. DAG test asserts `config/mod.rs` (not `config/traffic.rs`) is free of `crate::traffic`, since `to_view` must import the view type.
6. Extra exemptions in BOUNDARY_EXEMPT for files that really contain I/O primitives: `api/tests.rs`, `stt/audio.rs`, `pane/video_canvas/ffmpeg.rs`, `daemon/mod.rs`, `daemon/process.rs` (daemon ones flagged for 64-02 Task 0).
7. `ChannelCounters::reset()`, `TrafficSnapshot::omitted` and `Totals` roll-up of snapshot-limit overflow were added; the overflow channel occupies the 256th slot.
