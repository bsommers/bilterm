---
phase: 64-terminal-traffic-monitor
plan: 02
status: complete
requirements: [TMON-01, TMON-02, TMON-05]
---

# Plan 64-02 Summary

Every terminal byte boundary (H01-H33) is instrumented with the `traffic` layer; cells are adopted lazily from `WindowManager::traffic_tick`.

## Delivered
- PTY (`pty/mod.rs`) and VT worker (`vt/worker.rs`) chokepoints; `FeedOutput` is not counted (INV-04).
- `pane/traffic_tap.rs`: PTY cell access, lazy `OnceLock` payload cell, classify-on-ingest, OIA rates; tmux `%output`/hydrate/banner count payload rx.
- `remote::ssh_destination_from_args`; `LinkDriver::traffic_cell` (default `None`, `PtyLinkDriver` overrides).
- `wm/traffic_view.rs`: `adopt_traffic_cells` (panes, background sessions, quick terminal, link wire cell, legacy tmux cell, clipboard) once per sampled second.
- `api/metered.rs` `Metered<S>`: RPC unix/tcp (`server.rs`), HTTP/SSE (`http.rs`, one wrapper per connection, SSE retargeted), WS (`ws.rs`, `web-N` channel opened at `hello`, earlier bytes flushed into it). `WebServeOptions.meter`/`WsContext.meter` set by `bind_plane`.
- Legacy `tmux -CC`, host TTY (`app::CountingStdout`/`HostOut`, plus host-input events and OSC 52), clipboard tap, STT provider tap, OIA rate segment (opt-in).
- Tests: `tests/phase64_traffic_hooks.rs` (19), `tests/phase64_layer_dag.rs` manifest flipped (no Pending; strict adjacency check; synthetic-fixture audit test).

## Task 0 decisions
- Daemon `UnixStream::connect` sites are connect-only liveness probes: exempted (no new `Origin`).
- Pitfall 6: `CountingStdout` newtype + `HostOut` alias (only `app.rs`/`main.rs` mention the stdout type).

## Deviations
1. MCP stdio H26/H27 are `Deferred` to 64-03 (per plan task title); manifest has a `Deferred` status.
2. HTTP/WS use `Metered` wrapping rather than per-site counts; manifest tap strings for those hooks are `Metered`/`CountingStdout`/`traffic_cell`/`input_tap`/`traffic_tap()` (file-wide), others must be within 40 lines of the anchor.
3. Adoption runs only when the one-second sample advances, so a new pane is attributed within 1 s (counts are cumulative, nothing is lost). Respawn/drop closure relies on the existing sampler (`strong_count == 1`); the PTY reader thread releases its clone when the child is reaped.
4. `payload_cell()` takes `&self` (OnceLock) so classification can run from `&self`.
5. Clipboard tap installed by `traffic_tick` only when not headless; `has_traffic_tap()` added.
6. `TmuxControlSubprocess.traffic: Option<TrafficTap>` field added (legacy spawn only).
7. No dedicated disabled-PTY-flood perf test beyond 64-01's perf tier (which stays green).
