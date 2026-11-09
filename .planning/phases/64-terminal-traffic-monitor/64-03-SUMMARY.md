---
phase: 64-terminal-traffic-monitor
plan: 03
status: complete
requirements: [TMON-03, TMON-04, TMON-05]
---

# Plan 64-03 Summary

TRAFFIC MONITOR utility window plus full UNI-R01 surface uniformity, cloned from the CLIPBOARD window.

## Delivered
- `src/wm/traffic_monitor.rs` (+ `traffic_monitor/data.rs`): state, `TrafficMonitorOp` (39 variants, `colon_form` round-trip), frozen-snapshot pause, drill stack (depth 4), filter cap 64, classify on while open, API write gate, `TrafficQuery` and the single snapshot JSON builder.
- `src/wm/traffic_monitor_geometry.rs`: `TrafficLayout::compute` + `hit` single source for renderers and mouse; `src/wm/panel_dock.rs` (`DockPanel::Traffic`) and `src/ui/typography.rs` (`PanelTarget::Traffic`, 75..200).
- `src/ui/traffic_monitor_ui.rs` (TUI, `ratatui::widgets::Sparkline`), `src/ui/traffic_monitor_gui.rs` (`TRAFFIC_MONITOR_TEXTURE_ID = 990_000`), wired in `ui/mod.rs`, `renderer.rs` (scaled path + mask), `view_model.rs` (`trafficMonitor`).
- `Action::TrafficMonitor`, `:traffic` / `:netmon` (`parse_traffic.rs`), keys (`Ctrl+Alt+T` reserved global, Leader `$`), in-window table, mouse planner, palette rows, View menu row, completion, registry rows/exclusions, help lines.
- `src/api/rpc_traffic.rs`: `traffic.snapshot|history|window|reset|config`; MCP tool `bilterm_traffic` (25 tools); MCP stdio loop counts its own bytes on an `Origin::Mcp` wire channel (H26/H27).
- Docs: user guide chapter 13 + index, Developer Guide section 6f, AGENTS.md bullet, `scripts/check_phase64_doc_coverage.sh` (run by `tests/phase64_traffic_surfaces.rs`).
- Tests: `tests/phase64_traffic_window.rs` (13, two insta snapshots at 120x40 and 80x24), `tests/phase64_traffic_surfaces.rs` (9), parse round-trip unit tests.

## Deviations
1. Leader chord is `$`, not `#` (`#` is `ArrangeGrid`). `Ctrl+Alt+T` was free.
2. State field stays `visible` (not `active`) because the 64-01 seam in `traffic_view.rs` / `wm/mod.rs` reads it; `active()` accessor added.
3. Extra files: `src/wm/traffic_monitor/data.rs` (child module, keeps files small).
4. "Double-click drills" is implemented as clicking the already selected row (the planner is stateless, as for CLIPBOARD).
5. `Scale(u16)` is an absolute percent (`:traffic scale 75..200`); Ctrl+wheel steps it by 10.
6. `traffic.snapshot` JSON key order is alphabetical (the API `JsonValue` is a BTreeMap); colon `show` stores builder order. Parity is asserted on parsed value / canonical JSON text, not raw builder text.
7. Tool-count constants bumped 24 to 25 in `tests/{phase50_1_api,phase51_1_api,v110_v120_v130_*}.rs` and `api_mcp_resources.rs` list.
8. The doc gate is exercised from the surfaces test rather than wired into `verify.sh` (phase 50 gates are not wired there either).
9. Palette rows for `reachable-via-picker` exclusions cover pin/unpin/dock/float/move/resize/rect/size.
