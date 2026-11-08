# Phase 64 Architecture Decision Record (ADR): First-Class Terminal Traffic Monitor and the `traffic` Metrics Layer

**Date:** 2026-11-08
**Status:** Proposed (awaiting plan-check; no user-confirmed `[ASSUMED]` items blocked on this ADR beyond those in the Assumptions table)
**Context:** Phase 64 (`TMON-01..05`)
**Deciders:** Core Engineering Team; planner and plan-checker agents
**Supersedes:** nothing. Complements Phase 50 "Agent Traffic" (a different concept; see section 9).

---

## 1. Context and Problem Statement

Operators run many sessions, windows and panes, some local, some over `ssh` or the Phase 51.1 `ssh -tt ... tmux -CC` link, some driven by RPC/MCP/WebSocket clients. Bilterm cannot answer "where is my terminal traffic going?": the only counter is `VtParser::total_bytes_processed` (`src/vt/mod.rs:213`), per pane, rx only, no rates, no connection identity.

We need a *network-monitor-like* view of **terminal** traffic: aggregate and per-group bytes in/out, local vs network, per connection, per session, per window/pane, escape-sequence vs printable mix, events/sec, rolling 1s/10s/1m rates, peaks and sparklines, reachable from every surface (UNI-R01), with a documented performance budget and headless synthetic-traffic fixtures.

Constraints from the repo: cycle-free layer DAG with `pane` forbidden from importing `wm`/`ui`/`api`/`config` (`tests/vt_worker_and_snapshot_seam.rs:219-262`); `WindowManager`/`CompositePane` private fields (ARCH-R02); everything user-triggerable lowers to `Action` (ARCH-R01); files near 800-1000 lines (ARCH-R04); runtime files via `secure_fs` (SEC-R01..04); tests run many `WindowManager::new_headless` instances in one process.

## 2. Decision Drivers

1. **Zero cost when disabled; negligible when enabled.** The PTY reader thread feeds a bounded channel of 32 (`src/pty/mod.rs:154`); added work there throttles floods.
2. **Complete coverage with few hooks.** 33 boundary sites exist (RESEARCH hook inventory); five chokepoints dominate.
3. **Correct DAG placement.** The producer layers (`pty`, `vt`, `pane`, `tmux`) sit *below* `wm`; the metrics types must be importable from all of them.
4. **Topology changes under live PTYs.** `window.move` (SES-01) re-homes a window while its PTY keeps running, so attribution must be late-bound.
5. **Bounded memory and hostile-client resilience** (peer-controlled channel counts and labels).
6. **Test isolation.** Parallel tests each own a headless WM; no cross-test global state.
7. **Determinism.** Clock-injected, no sleeps, like `LinkMachine::tick(now_ms)`.
8. **UNI-R01 parity** and **module-size limits** with parallelisable, file-disjoint plans.

## 3. Decision

### 3.1 Layer placement: a new **leaf** layer `src/traffic/`

`traffic` sits at layer 0 beside `vt`/`config`/`theme`/`util`, and **imports nothing from the crate** (std only). Everything above may import it:

```
traffic (leaf, std only)
  ▲   ▲    ▲     ▲      ▲       ▲
 pty  vt  tmux  pane    wm     api  (+ app, ui, clipboard, stt)
```

Existing DAG tests forbid specific imports and none names `crate::traffic` [VERIFIED: tests/vt_worker_and_snapshot_seam.rs:219-262], so `pty`/`vt`/`pane` may depend on it with no test change. A new test (`tests/phase64_layer_dag.rs`) asserts `src/traffic/**` contains no `crate::` path other than `crate::traffic`, closing the cycle risk. Topology-dependent code (`AttributionMap`, adoption of cells) lives in `wm` (`src/wm/traffic_view.rs`); window state lives in `wm` (`src/wm/traffic_monitor.rs`), exactly like `ClipboardWindowState`.

### 3.2 Counting model: producer-owned relaxed atomics + cold sampler

- Each boundary owns an `Arc<ChannelCounters>` (alias `TrafficTap`): `rx_bytes`, `tx_bytes`, `rx_msgs`, `tx_msgs`, `rx_esc_bytes`, `rx_esc_seqs`, `rx_ctrl_bytes`, `rx_print_bytes`, `events`, plus a `flags: AtomicU8` (`ENABLED | CLASSIFY | OIA`) and two published-rate atomics for the OIA.
- Hot path: `if flags.load(Relaxed) & ENABLED != 0 { fetch_add(...) }`. Disabled = one relaxed load and a predictable branch.
- Classification (escape vs control vs printable) is separate, runs on the *consumer* thread at VT ingest (`src/pane/mod.rs:720,750`), and only when `CLASSIFY` is set. Its state machine persists across chunk boundaries in the cell.
- A cold `TrafficMeter` registry (Mutex, touched only at adopt/open/close/sample/snapshot) diffs cumulative counters once per second into a 60-slot ring per channel: 1s/10s/1m rates, peaks and sparkline series fall out of the ring. Time is an argument (`sample(now_ms)`).

### 3.3 Identity: typed `Origin` -> `ConnectionKey`, plus `Locality`

```rust
enum Origin {
    LocalPty,                                  // shell child on a local PTY
    Ssh { host: String, port: u16 },           // pane running ssh in a PTY
    TmuxLink { dest: String, session: String },// Phase 51.1 ssh -tt tmux -CC (wire) and its panes (payload)
    TmuxLocal,                                 // legacy local tmux -CC subprocess
    RpcUnix, RpcTcp { remote: bool },          // JSON-RPC sockets
    Http, Sse, Ws { client: String },          // loopback HTTP surface, web clients ("web-3")
    Mcp,                                       // MCP stdio
    HostTty,                                   // the terminal Bilterm itself runs in
    ClipboardOs, Stt { host: String },         // OS clipboard helper, STT provider
}
enum Locality { Local, Loopback, Network }     // derived from Origin
enum Role { Wire, Payload, Both }              // double-count guard
struct ConnectionId(u32);                      // opaque; allocated per distinct ConnectionKey
```
`ConnectionKey` is the normalised, bounded form of `Origin` (all local PTYs share the single key `local`; each distinct ssh `host:port` is one key; each web client one key). `ChannelMeta { channel_id, connection, origin, pane: Option<usize>, role, opened_ms, closed_ms }`.

### 3.4 Attribution: late-bound

Session and window are **not stored** on channels. `wm::traffic_view` builds an `AttributionMap { pane -> (session, window_title) }` per snapshot by walking live panes, background sessions and the quick terminal (the same resolution order as `resolve_pane_session_and_title`, `src/wm/rename_dialog.rs:157-206`) and passes it to the pure `traffic::group::group_by`.

### 3.5 Adoption: lazy, no spawn-site churn

`PtySession::spawn` creates the cell. `WindowManager::traffic_tick(now_ms)`, called from `poll_all_ptys` beside `update_drawer_telemetry` (`src/wm/mod.rs:857`), adopts any unseen cell from every pane plus the tmux link driver and then samples. The ten `CompositePane::spawn` call sites are untouched. Async API tasks, which have no `&mut WindowManager`, call `meter.open_channel(origin, label)` on a cloned `TrafficMeter` handle obtained through the existing plane-binding seam (`bind_plane`, `src/api/http_web.rs:30`; `serve_loop`, `src/api/server.rs:480`).

### 3.6 Window and surfaces

`TRAFFIC MONITOR` is a floating, pinnable, dockable utility window (`DockPanel::Traffic`, `PanelTarget::Traffic`, GUI texture `990_000`) following Developer Guide 6c verbatim. One `Action::TrafficMonitor(TrafficMonitorOp)` with `colon_form()`; `:traffic` (alias `:netmon`); `Ctrl+Alt+T` global + Leader `#`; View menu row; palette row; RPC `traffic.snapshot|history|window|reset|config`; MCP `bilterm_traffic`; OIA `↓rx ↑tx` indicator (opt-in, `[traffic] oia_indicator`).

## 4. Alternatives Considered

### A. The `metrics` / Prometheus-style crate with a global registry
**Pro:** ecosystem familiarity, exporters.
**Con:** new dependency (supply-chain gate for every package, `deny.toml`), global recorder breaks per-WM test isolation (driver 6), label-cardinality hazards with peer-controlled labels, not zero-cost when disabled (recorder dispatch), still need our own rolling windows. **Rejected.**

### B. Use `tracing` events as the counters
**Pro:** already a dependency; one mechanism for logs and metrics.
**Con:** subscriber dispatch and field formatting per event; cannot be zero cost; `bilterm.log` volume explosion at PTY rates; no cheap aggregation. **Rejected** (tracing remains for lifecycle events, `bilterm::traffic` target).

### C. OS-level capture (eBPF, `/proc/net/dev`, pcap, `ss`)
**Pro:** sees real network bytes including TLS overhead.
**Con:** privileged or platform-specific (Linux first, macOS second, Windows third is a hard project constraint), cannot attribute to pane/window/session, cannot see local PTY or clipboard bytes at all, and ssh `ControlMaster` multiplexing hides per-pane identity. **Rejected**; Bilterm meters its own boundaries. (Deferred idea: optionally cross-reference `/proc/net/dev` later.)

### D. Per-pane counters only (extend `total_bytes_processed`)
**Pro:** trivial; no new layer.
**Con:** rx only, post-coalescing, no tx, no RPC/MCP/WS/clipboard, no connection taxonomy, no rates. Fails the request. **Rejected.**

### E. Event stream: send a `TrafficEvent` over an mpsc channel per read/write
**Pro:** exact timestamps, trivially extensible.
**Con:** an allocation or channel send per chunk on the reader thread, unbounded or backpressure-prone queue, and a consumer that must keep up with floods (`yes`, `cat hugefile`). Violates driver 1. **Rejected.**

### F. Mutex-protected registry updated on every read/write
**Con:** contention between the reader thread, the WM thread and async tasks; priority inversion risk with the render loop. **Rejected**; mutex is confined to cold paths.

### G. Wrapper traits (`MeteredRead`/`MeteredWrite`) at every call site instead of cells
**Pro:** consistent shape.
**Con:** the readers/writers are heterogeneous (`portable-pty` boxed `Read`/`Write`, tokio streams, `std::process` pipes, `io::stdout`); a trait wrapper forces type changes (`Terminal<CrosstermBackend<io::Stdout>>` at `src/app.rs:51`). We use the wrapper idea **only** where the type is generic (`Metered<S>` for the RPC listener) and plain cell calls elsewhere.

### H. Count only inside `VtParser::process`
**Con:** single hook but misses tx, every non-PTY channel, tmux-mapped panes, and observes coalesced chunks (`src/pane/mod.rs:750-778`). **Rejected.**

### I. Stamp session/window on the channel at record time
**Con:** wrong after `window.move`/`session.rename`/bulk ops; would need invalidation hooks in every topology mutation (Phases 53/54). **Rejected** in favour of snapshot-time attribution.

### J. Put the metrics types in `wm` (next to `src/wm/traffic.rs`)
**Con:** `pty`/`pane`/`tmux` cannot import `wm` (DAG test); producers would need a trait object injected from above, adding indirection to the hot path. **Rejected.**

### K. Reuse/extend the Phase 50 `TrafficBus`
**Con:** different semantics (message provenance with previews, `TRAFFIC_CAP = 512` events, redaction) and a public contract in ADR-008; mixing byte metering into it would break `tests/phase50_traffic.rs`. **Rejected**; names kept strictly distinct.

### L. Always-on classification
**Con:** a state machine over every byte for a feature most sessions never open. **Rejected:** `CLASSIFY` is off until the monitor window is open (auto) or `[traffic] classify = true`.

## 5. Consequences

**Positive:** one tiny, DAG-safe module; five chokepoints give ~complete coverage; test-isolated; cheap; late-bound attribution stays correct across moves; window/surface work is a mechanical clone of Phase 57.

**Negative / accepted:** (1) 33 touch points across 15 files, mitigated by a *byte-boundary audit fixture* that fails when a known I/O primitive appears in an unlisted file; (2) host-TTY output counting may be limited by the `Terminal<CrosstermBackend<io::Stdout>>` type (research Pitfall 6); (3) 1-second resolution (no sub-second peaks); (4) lazy adoption means a channel is invisible for up to one tick (<= 16 ms in the TUI loop) and bytes before adoption are still counted because the cell is cumulative.

## 6. Invariants (enforced by tests)

| ID | Invariant |
|----|-----------|
| TRAFFIC-INV-01 | `src/traffic/**` imports nothing from the crate except itself. |
| TRAFFIC-INV-02 | Disabled cells record nothing and allocate nothing. |
| TRAFFIC-INV-03 | Conservation: sum by connection == sum by origin == grand total == sum of `Wire|Both` channels; sum by pane == sum of `Payload|Both` channels with a pane; eviction/closure never changes totals. |
| TRAFFIC-INV-04 | A byte is counted at the lowest boundary only (PTY writes via `PtySession::write_all`; `write_to_pty` counts only when `pty.is_none()`). |
| TRAFFIC-INV-05 | Memory is bounded: `MAX_CHANNELS = 256`, `MAX_CLOSED = 64`, 60 buckets/channel, label <= 48 chars, snapshot `limit` clamp 1..=500, filter <= 64 chars. |
| TRAFFIC-INV-06 | Every `Action::TrafficMonitor` variant is reachable from keymap, colon or RPC; every hook-manifest entry still contains its tap call. |
| TRAFFIC-INV-07 | The meter never stores payload bytes or previews. |
| TRAFFIC-INV-08 | `handle_wm_key_event` stays <= 80 lines; no file exceeds 1200 lines. |

## 7. Performance Budget (targets; calibrated by Plan 64-01 Task 5)

| Path | Budget |
|------|--------|
| Disabled `rx`/`tx` | <= 1 ns, 0 alloc |
| Enabled bytes-only `rx`/`tx` | <= 5 ns uncontended |
| Classify ground fast path / dense-escape worst case | >= 1 GB/s / >= 300 MB/s |
| `sample()` non-boundary / boundary @256 channels | <= 50 ns / <= 50 us |
| `snapshot()` + `group_by` @256 channels | <= 200 us |
| Memory ceiling | <= 0.6 MB |
| Monitor render (TUI, 100 rows) | <= 1 ms |
| End-to-end PTY flood regression, monitor on vs off | <= 2% release / <= 25% debug |

All numbers are `[ASSUMED]` until measured; the benchmark task writes the measured values back into this table and the tests assert the measured value plus headroom (skill: sizing-limits-from-measurement).

## 8. Plan decomposition and file ownership

| Wave | Plan | Owns (exclusive) | Depends on |
|------|------|------------------|------------|
| 1 | 64-01 Traffic layer, fixtures, WM wiring seam | `src/traffic/**`, `src/config/traffic.rs`, `src/config/mod.rs` (+1 field), `src/wm/mod.rs`, `src/wm/traffic_view.rs`, `src/wm/traffic_monitor.rs` + `traffic_monitor_geometry.rs` (placeholders, bodies owned by 64-03), `scripts/verify.sh` (perf stage list), `src/lib.rs`, `tests/phase64_traffic_core.rs`, `tests/phase64_layer_dag.rs` | none |
| 2 | 64-02 Byte-boundary hooks | `src/pty/mod.rs`, `src/vt/worker.rs`, `src/pane/{mod,traffic_tap}.rs`, `src/tmux/{mod,transport,link}.rs`, `src/wm/{tmux_io,tmux_link,traffic_view}.rs` (traffic_view.rs created by 64-01, extended by 64-02 only; wave order makes this sequential), `src/api/{server,http,http_web,ws,metered}.rs`, `src/app.rs`, `src/main.rs`, `src/clipboard.rs`, `src/stt/provider.rs`, `src/remote/mod.rs`, `src/ui/oia.rs`, `tests/phase64_traffic_hooks.rs` | 64-01 |
| 2 | 64-03 Monitor window + UNI-R01 surfaces | `src/wm/traffic_monitor{,_geometry}.rs`, `src/wm/panel_dock.rs`, `src/ui/traffic_monitor_{ui,gui}.rs`, `src/ui/{mod,renderer,typography,modals,help_catalog_data}.rs`, `src/input/**` (traffic files + `mod.rs`, `mouse.rs`, `keymap*.rs`), `src/action.rs`, `src/colon_command/**`, `src/command_registry.rs`, `src/pane/completion.rs`, `src/api/{rpc,rpc_dispatch,rpc_traffic,mcp,mod,view_model}.rs`, docs, `tests/phase64_traffic_{window,surfaces}.rs` | 64-01 |

64-02 and 64-03 are file-disjoint and run in parallel. `src/wm/mod.rs` is touched only by 64-01 (fields incl. `traffic_scale_pct`, the three `blur()` lines, `mod`/`pub use` lines for every later file, the single `traffic_tick` call), so no later plan edits it. `src/api/mcp.rs` is owned wholly by 64-03, including the stdio byte hooks H26/H27 (64-02 supplies the `Metered`/cell API; 64-03 wires the two stdio lines). `src/ui/oia.rs` is owned by 64-02 and reads only the pane's cell.

## 9. Relationship to Phase 50 "Agent Traffic"

Phase 50's `TrafficBus` (`src/wm/traffic.rs`, `AGENT TRAFFIC` window, `:agents traffic`, View -> "Agent Traffic (Ctrl+Alt+A)") records *who sent what to whom* (previews, redaction, 512-event ring). Phase 64 records *how many bytes moved through which boundary*. They share no code. User-visible names: "Agent Traffic" vs "Traffic Monitor". The Traffic Monitor help text links to Agent Traffic and vice versa.

## 10. Security and Privacy

Counts and sanitised labels only (TRAFFIC-INV-07). Host/destination labels can reveal infrastructure; `[traffic] label_hosts = false` maps them to `host-<n>`. `traffic.reset` and `traffic.config set` go through `config_write_denied` (`src/wm/mod.rs:398`); reads are always allowed. 1-second resolution limits keystroke-timing leakage. Optional export (stretch) must use `secure_fs::write_private_file_atomic` and `validate_safe_name`.

## 11. Open Items for Plan Checker

1. Confirm Ctrl+Alt+T and Leader `#` are free (A3).
2. Host-TTY wrapper feasibility (research Pitfall 6).
3. Daemon attach IPC hooks (`src/daemon/process.rs:56`, `src/daemon/mod.rs:239`) unread; Plan 64-02 Task 0 audits and either hooks or exempts with a reason.
4. HTTP row policy (per-connection vs per-class).
