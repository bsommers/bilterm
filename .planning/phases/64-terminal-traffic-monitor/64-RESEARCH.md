# Phase 64: First-Class Terminal Traffic Monitor - Research

**Researched:** 2026-11-08
**Domain:** Rust TUI multiplexer instrumentation: lock-free byte metering at every I/O boundary, a leaf `traffic` metrics layer, a floating utility window (TUI + GUI), and UNI-R01 surface parity
**Confidence:** HIGH on hook points and layer placement (every site below was opened this session); MEDIUM on perf budgets (targets, not yet measured; Plan 64-01 Task 5 calibrates them)

## User Constraints (from the invoking request; no CONTEXT.md exists for Phase 64)

### Locked Decisions
Verbatim from the request:
> "a first-class terminal traffic monitor and associated architectural layer fixtures to enable. Like a network monitor, but shows aggregate terminal traffic, local vs network, by connection (local vs each remote), by session, by window, etc."

1. A low-overhead instrumentation seam at every byte boundary (PTY read/write, SSH channels, RPC/MCP/WebSocket clients, clipboard/OSC) counting bytes in/out, escape-sequence vs printable, events/sec, with lock-free/atomic counters and zero cost when disabled.
2. A `traffic` metrics layer placed correctly in the layer DAG with a typed `ConnectionId`/origin taxonomy (local PTY, ssh host:port, rpc client, web client), session/window/pane attribution, rolling 1s/10s/1m rates, totals, peaks, bounded memory.
3. A TRAFFIC monitor utility window (TUI + GUI) with group-by (connection/session/window/pane/direction), sparklines, sort, filter.
4. UNI-R01 surfaces: `:traffic` colon verbs, palette, menu (View -> Traffic Monitor), RPC `traffic.*`, MCP `bilterm_traffic`, optional OIA status-bar rate indicator.
5. Perf budget and tests (headless fixtures that inject synthetic traffic).

### Claude's Discretion
Module name and path of the metrics layer, wire/payload accounting rules, window geometry and key chords, sparkline implementation, perf-budget numbers, number and split of plans.

### Deferred Ideas (OUT OF SCOPE)
- Kernel/OS-level network capture (eBPF, `/proc/net`, pcap). Bilterm meters its own boundaries only (rejected in ADR, alternative C).
- Persisting traffic history to disk or exporting to Prometheus/OpenTelemetry (future phase; bounded in-memory only here).
- Content inspection / payload capture. The meter never stores bytes of traffic.
- Migrating the Phase 50 AGENT TRAFFIC bus (`src/wm/traffic.rs`) onto the new meter. They are different concepts (see Pitfall 1).

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TMON-01 | Atomic, zero-cost-when-disabled byte/chunk/escape/event instrumentation at every I/O boundary | Hook inventory (33 sites), `ChannelCounters` design, de-dup rule |
| TMON-02 | Leaf `traffic` layer: typed `Origin`/`ConnectionId`, rolling 1s/10s/1m windows, peaks, bounded memory, pure group-by | Layer DAG analysis, ring-bucket sampler, conservation invariants |
| TMON-03 | TRAFFIC MONITOR floating window (TUI + GUI): group-by connection/session/window/pane/direction/origin, sparklines, sort, filter, pause | Phase 57 CLIPBOARD window as template (Section 6c of the Developer Guide) |
| TMON-04 | UNI-R01 parity: mouse, keys, `:traffic`, palette, View menu, `traffic.*` RPC, `bilterm_traffic` MCP, OIA indicator | Command-parity checklist (AGENTS.md "Automation API and command parity") |
| TMON-05 | Perf budget gates and headless synthetic-traffic fixtures, layer-DAG and byte-boundary audit tests | `tests/vt_worker_and_snapshot_seam.rs` DAG test pattern, `scripts/verify.sh` stages |
</phase_requirements>

## Project Constraints (from CLAUDE.md / AGENTS.md)

- `ARCH-R04`: every `src/**/*.rs` targets ~800-1000 lines (hard test backstop 1200); split into cohesive submodules.
- `ARCH-R01`: `src/input/mod.rs::handle_wm_key_event` stays `<= 80` lines; every user-triggerable operation lowers to `crate::action::Action` and runs through `WindowManager::apply` (`src/colon_command/execute_wm.rs`); every new `Action` variant is covered in `src/action.rs::tests::every_action_is_reachable_from_keymap_colon_or_rpc`.
- `ARCH-R02`: `WindowManager` and `CompositePane` fields stay private (new WM state is a `pub(crate)` field on a state struct in its own `src/wm/<panel>.rs`, per Developer Guide 6c).
- `SEC-R01..04`: runtime files only via `src/secure_fs.rs` (`0700` dirs, `0600` atomic files, `validate_safe_name`); never secrets on argv.
- `UNI-R01`: mouse (TUI and GUI), keys (declared in a `KeyBinding` table), colon + palette + help + RPC/HTTP/MCP/`bilterm ctl`, with a parity test; exceptions need a reason in `PALETTE_EXCLUSIONS` or `uniformity_exceptions`.
- Layer DAG: `vt / config / theme / tmux / util -> pane -> wm -> action / colon_command / input -> api / app -> lib / main`; `pane` must not import `wm`/`ui`/`api`/`config`, `pty` must not import `vt`/`pane`/`wm`/`ui`/`api`, `vt` must not import `pane`/`wm`/`ui`/`api`, `ui` must not import `api` (enforced by the test quoted below).
- Verification: `scripts/verify.sh` (one line; on FAIL read `target/verify/<stage>/fail.txt`). Cargo with `-q`.
- Changes land by branch + PR only (never push to `main`).

## Summary

Bilterm has no unified byte meter today. The only byte counter is `VtParser::total_bytes_processed` (`src/vt/mod.rs:213`, incremented once per `process` call), used by the OIA `OUT:` label (`src/ui/oia.rs:183`) and the drawer sparkline (`src/wm/drawer.rs:283`). It counts only post-PTY output, only per pane, with no direction, no rates, no connection identity, and nothing for RPC/MCP/WebSocket/clipboard/ssh. [VERIFIED: grep of `total_bytes_processed` this session]

Every PTY byte funnels through two functions: the reader thread in `PtySession::spawn` (`src/pty/mod.rs:160`) and `PtySession::write_all` (`src/pty/mod.rs:202`). Local shells, `ssh` panes and the Phase 51.1 `ssh -tt ... tmux -CC` link (`src/tmux/transport.rs:69` calls `PtySession::spawn`) all use them. Control-plane bytes funnel through one generic JSON-RPC connection function (`src/api/server.rs:509`), the HTTP/SSE handlers (`src/api/http.rs`), the WebSocket loop (`src/api/ws.rs:490,513`) and the MCP stdio loop (`src/api/mcp.rs:560,566`). That makes a seam cheap: about five chokepoints carry nearly all bytes and 33 hook sites cover every boundary.

The design: a new **leaf** module `src/traffic/` (std only, imports nothing from the crate) holding `Origin`, `ConnectionId`, per-channel atomic `ChannelCounters`, a cold-path `TrafficMeter` registry with a clock-injected 1-second bucket sampler (1s/10s/1m rates, peaks, totals), and pure group-by/sort/filter over immutable snapshots. Producers (pty, vt, pane, tmux, api) own an `Arc<ChannelCounters>` and bump relaxed atomics behind one relaxed flag load; the `WindowManager` adopts cells lazily during its existing per-frame tick, so none of the ten `CompositePane::spawn` call sites change. Session/window attribution is resolved at snapshot time through an `AttributionMap` built by `wm`, because windows move between sessions while their PTY stays live (SES-01).

**Primary recommendation:** Build `src/traffic/` as a std-only leaf layer with per-channel relaxed-atomic cells and a cold sampler; instrument the 33 hook sites behind a single `if cell.flags() & ENABLED` branch; resolve session/window at snapshot time in `wm`; ship the window and all UNI-R01 surfaces by cloning the Phase 57 CLIPBOARD window structure; enforce with a layer-DAG test and a byte-boundary audit test.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Count bytes at PTY / socket / stdio boundary | Boundary owner (`pty`, `vt::worker`, `tmux`, `api`, `app`, `clipboard`, `stt`) | `traffic` (cell type) | Only the owner of the `read`/`write` call sees the true byte count; counting later (e.g. VT ingest) misses tx and coalescing |
| Atomic cells, taxonomy, rolling windows, group-by | `traffic` leaf layer | none | Pure data; must be importable from `pty` upward without cycles |
| Escape vs printable classification | `pane` (consumer thread at VT ingest) | `traffic::classify` (pure fn) | Keeps the reader thread to a bare `fetch_add`; classification runs only when the detail flag is on |
| Session / window / pane attribution | `wm` (`traffic_view.rs`) | `traffic::group` (pure) | Topology (sessions, background sessions, moves) lives in `wm`; resolved at snapshot, never stamped at record time |
| Monitor window state, geometry, ops | `wm` (`traffic_monitor.rs`) | `ui::typography::PanelTarget` | Same ownership as `ClipboardWindowState` |
| Render (TUI cells, GUI vector card) | `ui` | none | Draws only from the geometry layout; Sparkline from `ratatui` |
| Keys / mouse planning | `input` | `wm` geometry (single hit table) | Developer Guide 6c steps 5-6 |
| Colon verbs, `Action`, palette, menu | `colon_command` / `action` / `ui::modals` / `command_registry` | none | ARCH-R01 |
| RPC `traffic.*`, MCP `bilterm_traffic` | `api` | `wm` (data) | Same dispatcher for all transports |
| Async tasks (RPC/HTTP/WS/MCP) meter their own sockets | `api` | `traffic::TrafficMeter::open_channel` | Tasks run off the WM thread; they hold `Arc` cells, never `&mut wm` |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `std::sync::atomic` (`AtomicU64`, `AtomicU8`, `AtomicBool`, `Ordering::Relaxed`) | Rust 1.90 (`rust-toolchain.toml` channel `1.90`) | Lock-free per-channel counters | No dependency, wait-free `fetch_add`, sufficient for monotonic counters read by a sampler |
| `std::sync::{Arc, Mutex, OnceLock}` | std | Shared cells, cold-path registry | Registry touched only at adopt / sample / snapshot |
| `ratatui::widgets::Sparkline` | `ratatui 0.30.2` (`ratatui-widgets 0.3.0`) | TUI sparklines | Already a dependency; `pub struct Sparkline<'a>` [VERIFIED: ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ratatui-widgets-0.3.0/src/sparkline.rs:66] |
| `crate::api::json::JsonValue` | in repo | RPC/MCP payloads | Zero-dependency JSON already used by every RPC handler |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `crate::secure_fs` | in repo | Optional `:traffic export` file (stretch) | Only if export is added; `0600` atomic write |
| `crate::pane::redact` | in repo | Not needed for counts | Do not feed traffic labels through it; labels are sanitised by `traffic::label::sanitize` |
| `tests/phase51_uniformity_support` | in repo | UNI-R01 parity harness | Reuse for the surfaces parity test |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Per-cell atomics | `metrics` / `prometheus` crate | New dependency, global registry (breaks per-WM test isolation), label cardinality hazards; see ADR alt. A |
| Per-cell atomics | `tracing` events as counters | Allocation and subscriber dispatch per event; cannot be zero-cost; see ADR alt. B |
| Cold sampler with 1s buckets | Per-event timestamped ring | Per-event cost on the hot path; see ADR alt. E |
| `ratatui::Sparkline` | Hand-rolled block glyph mapper | `drawer.rs` already hand-rolls one; the GUI path needs a number series either way, so the shared `Vec<u64>` series is the contract and TUI uses the widget |

**Installation:** none. No new crates. `Cargo.toml` is untouched. [VERIFIED: `Cargo.toml` `[dependencies]` already has `ratatui = "0.30.0"`, `tokio = { version = "1.44.0", features = ["full"] }`; read this session]

**Version verification:** ratatui 0.30.2 and ratatui-widgets 0.3.0 are present in the local cargo registry (`ls ~/.cargo/registry/src/*/` shows `ratatui-0.30.2`; path above for widgets). No network package lookup needed.

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| (none added) | - | - | - | - | n/a | Phase installs no external packages |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
 BYTE BOUNDARIES (producers; lock-free, relaxed atomics)             COLD PATH (WM thread, <= 4 Hz work)
 ---------------------------------------------------------------     ---------------------------------------------
 child shell / ssh ──PTY──> pty reader thread (pty/mod.rs:160) ──┐
 WM keystrokes/replies ───> PtySession::write_all (pty/mod.rs:202)┤
 ssh -tt tmux -CC link ──> same PtySession (tmux/transport.rs:69) ┤
 tmux %output payload ───> pane.process_pty_output (tmux/mod.rs:577)┤   ChannelCounters (Arc, one per channel)
 VT worker seam ─────────> vt/worker.rs:303,472,610,615           ┼──> rx/tx bytes, rx/tx msgs, esc/ctrl/print,
 RPC unix/tcp ───────────> Metered<S> (api/server.rs:509)         ┤    events, flags(enabled|classify|oia),
 HTTP / SSE ─────────────> api/http.rs:277,253,552                ┤    published 1s rates (for OIA)
 WebSocket ──────────────> api/ws.rs:490,513                      ┤              │
 MCP stdio ──────────────> api/mcp.rs:560,566                     ┤              │  adopt (lazy, by ChannelId)
 host TTY out / events ──> main.rs:515, app.rs:55,124             ┤              ▼
 clipboard / OSC52 / STT ─> pane/mod.rs:852, clipboard.rs, stt   ┘     WindowManager::traffic_tick(now_ms)
                                                                       (wm/traffic_view.rs, called from poll_all_ptys)
 pane VT ingest (pane/mod.rs:720,750) ── classify(esc/ctrl/print) ─┐          │ walks panes + background sessions +
   only when classify flag set                                      │          │ quick terminal + tmux link driver
                                                                    ▼          ▼
                                                          TrafficMeter registry (Mutex, cold)
                                                          ├─ ChannelMeta {Origin, ConnectionId, pane, role}
                                                          ├─ 60 x 1s buckets / channel  -> 1s,10s,1m rates, peaks
                                                          └─ closed-channel roll-up (bounded)
                                                                       │ snapshot()  -> immutable TrafficSnapshot
                                                                       ▼
        AttributionMap (wm: pane -> session, window title) ──> traffic::group::group_by(snapshot, key, sort, filter)
                                                                       │
              ┌────────────────────────────┬───────────────────────────┴───────────┬──────────────────────┐
              ▼                            ▼                                       ▼                      ▼
   TrafficMonitorState (wm)        RPC traffic.snapshot/history          MCP bilterm_traffic       OIA ↑/↓ rate
   ui/traffic_monitor_ui.rs/_gui   (api/rpc_traffic.rs)                  (api/mcp.rs)              (reads pane cell)
```

### Recommended Project Structure
```
src/
├── traffic/                      # NEW leaf layer (std only; imports NOTHING from crate)
│   ├── mod.rs                    # re-exports, TRAFFIC constants
│   ├── origin.rs                 # Origin, Locality, Surface, Role, ConnectionKey/ConnectionId, label sanitiser
│   ├── cell.rs                   # ChannelCounters (atomics), flags, TrafficTap alias, record_* fns
│   ├── classify.rs               # pure incremental escape/ctrl/printable classifier
│   ├── window.rs                 # 1s-bucket ring, rates (1s/10s/1m), peaks, sparkline series
│   ├── meter.rs                  # TrafficMeter registry: adopt/open_channel/close/sample/snapshot, bounds
│   ├── group.rs                  # pure GroupBy/SortKey/Filter over TrafficSnapshot, AttributionMap
│   └── fixture.rs                # SyntheticTraffic + FixtureClock (pub: integration tests need it)
├── config/traffic.rs             # [traffic] section (enabled, classify, oia_indicator, label_hosts)
├── wm/traffic_view.rs            # WindowManager::traffic_tick / traffic_snapshot / attribution (adopts cells)
├── wm/traffic_monitor.rs         # TrafficMonitorState + TrafficMonitorOp (window state machine)
├── wm/traffic_monitor_geometry.rs# rect / layout / hit (single source for render + mouse)
├── pane/traffic_tap.rs           # CompositePane accessors: traffic_cell(), classify-on-ingest
├── ui/traffic_monitor_ui.rs      # TUI renderer
├── ui/traffic_monitor_gui.rs     # GUI vector card (TRAFFIC_MONITOR_TEXTURE_ID = 990_000)
├── input/traffic_monitor.rs      # pure plan_traffic_monitor_mouse + applier
├── input/traffic_monitor_keys.rs # TRAFFIC_MONITOR_BINDINGS table
├── colon_command/parse_traffic.rs
├── api/rpc_traffic.rs            # traffic.snapshot|history|window|reset|config
└── (hooks in pty, vt/worker, pane, tmux, api, app, main, clipboard, stt)
```
GUI texture id `990_000` follows the sequence `GRAPH_TEXTURE_ID 950_000`, `SESSION_MAP_TEXTURE_ID 970_000`, `CLIPBOARD_WINDOW_TEXTURE_ID 980_000` [VERIFIED: src/ui/agents_gui.rs:27, src/ui/session_map_gui.rs:9, src/ui/clipboard_window_gui.rs:9]; `990_000` is the next free slot [ASSUMED: confirm no other const uses it at execution].

### Pattern 1: Producer-owned atomic cell, WM-side lazy adoption
**What:** `PtySession` creates `Arc<ChannelCounters>` in `spawn` and bumps it on read and write. `WindowManager::traffic_tick` walks every pane (live, background-session, quick terminal) and the tmux link driver, and `meter.adopt(cell, meta)` any cell whose `ChannelId` is not yet registered. Closed state is detected by `Arc::strong_count == 1` (the registry is the last holder).
**When to use:** any boundary whose owner is constructed in a layer that cannot see `wm` (`pty`, `pane`, `tmux`).
**Why:** removes the need to change ten `CompositePane::spawn` call sites: `src/wm/mod.rs:684,758`, `src/wm/window_manage_bulk.rs:160,195`, `src/wm/session_manager.rs:215`, `src/wm/session.rs:239`, `src/wm/quick_terminal.rs:76`, `src/wm/window_manage.rs:415`, `src/wm/layout_profile.rs:537,550` [VERIFIED: grep `CompositePane::spawn` this session].
**Example:**
```rust
// src/traffic/cell.rs  (sketch; std only)
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering::Relaxed};
pub const F_ENABLED: u8 = 1; pub const F_CLASSIFY: u8 = 2; pub const F_OIA: u8 = 4;
pub struct ChannelCounters {
    id: u32, flags: AtomicU8,
    rx_bytes: AtomicU64, tx_bytes: AtomicU64, rx_msgs: AtomicU64, tx_msgs: AtomicU64,
    rx_esc_bytes: AtomicU64, rx_esc_seqs: AtomicU64, rx_ctrl_bytes: AtomicU64, rx_print_bytes: AtomicU64,
    events: AtomicU64,
    last_rx_bps: AtomicU64, last_tx_bps: AtomicU64, // published by the sampler for the OIA
}
impl ChannelCounters {
    #[inline] pub fn rx(&self, n: usize) {
        if self.flags.load(Relaxed) & F_ENABLED == 0 { return; }
        self.rx_bytes.fetch_add(n as u64, Relaxed); self.rx_msgs.fetch_add(1, Relaxed);
    }
    #[inline] pub fn tx(&self, n: usize) { /* mirror of rx */ }
    #[inline] pub fn classify_enabled(&self) -> bool { self.flags.load(Relaxed) & (F_ENABLED|F_CLASSIFY) == (F_ENABLED|F_CLASSIFY) }
}
```

### Pattern 2: Wire vs Payload roles (no double counting)
**What:** every channel has `role: Wire | Payload | Both`. A local PTY pane is `Both`. The raw `ssh -tt tmux -CC` stream is `Wire` (no pane). Decoded `%output` bytes fed to a mapped tmux pane are `Payload` (pane set, connection = the link). Connection/origin/total views sum `Wire|Both`; session/window/pane views sum `Payload|Both` with a pane.
**Invariants (tested):** sum by connection == sum by origin == grand total == sum of Wire/Both channels; sum by pane == sum of Payload/Both channels with a pane; closing or evicting a channel never changes totals (roll-up).
**De-dup rule:** count a byte at the lowest boundary only. Keystroke and query-reply writes that reach the PTY (`src/pane/mod.rs:731,785,804`, `src/pane/scroll.rs:748`, `src/tmux/transport.rs:110,157`) are counted once at `src/pty/mod.rs:202`; `write_to_pty` counts tx payload only when `self.pty.is_none()` (tmux-mapped pane).

### Pattern 3: Cold sampler with injected clock
**What:** `TrafficMeter::sample(now_ms)` returns immediately unless `now_ms / 1000` advanced; on a boundary it reads each cell's cumulative atomics, pushes the per-second delta into a 60-slot ring, updates peaks, and stores `last_rx_bps/last_tx_bps` back into the cell for the OIA. No test sleeps; the clock is a parameter (same discipline as `LinkMachine::tick(now_ms)`, Developer Guide 6d).
**Rates:** 1s = last bucket; 10s = sum(last 10)/10; 1m = sum(last 60)/60 (young channels divide by elapsed buckets, not 10/60).

### Pattern 4: Snapshot + pure group-by
`TrafficMeter::snapshot()` copies the registry into `TrafficSnapshot { at_ms, channels: Vec<ChannelRow>, evicted: Totals }` (immutable, `Send`). `group_by(&snapshot, GroupKey, &AttributionMap, &Filter, SortSpec) -> Vec<GroupRow>` is a pure function: no WM, no clock, testable with `SyntheticTraffic`.

### Pattern 5: Metered async stream for the RPC listener
One generic wrapper `Metered<S: AsyncRead + AsyncWrite>` implementing `poll_read`/`poll_write` counts every byte on the socket with a single hook at `src/api/server.rs:509` (covers the `read_until` at `src/api/server.rs:562` and the five `wr.write_all` sites at `:595,635,644,658,670`). HTTP and WS hold concrete `tokio::net::TcpStream` (`serve_ws(mut stream: tokio::net::TcpStream, ...)` at `src/api/ws.rs:478`), so they are instrumented at explicit sites.

### Anti-Patterns to Avoid
- **Counting in `VtParser::process`:** misses tx, non-PTY channels and tmux-mapped panes, and runs after coalescing (`src/pane/mod.rs:750-778`).
- **A global `static` registry of channels:** tests run in one process in parallel (`WindowManager::new_headless` per test); a global registry cross-contaminates assertions. Only the default-flags knob may be process-global (set before panes spawn).
- **Stamping session/window at record time:** `window.move` (SES-01) changes a live PTY's session; attribute at snapshot time.
- **Putting `PaneId`-typed or `wm`-typed fields in `traffic`:** the leaf layer uses `u64`/`usize` keys; `pane::PaneId` is `pub type PaneId = usize` [VERIFIED: src/pane/mod.rs:48] so `usize` is interchangeable without an import.
- **Per-peer-port labels:** ephemeral ports explode cardinality. Web/RPC clients are labelled by class + allocated id (`web-3`, `rpc-unix#5`).

## Byte-Boundary Hook Inventory (all opened this session)

Legend: **C** = chokepoint covering many call paths; **E** = event/metadata only (no wire bytes). "role" per Pattern 2.

| # | File:line | Verbatim anchor | Boundary | Dir | Role | Notes |
|---|-----------|-----------------|----------|-----|------|-------|
| H01 | `src/pty/mod.rs:160` | `match reader.read(&mut buf) {` | PTY master read (local shell, ssh pane, ssh-tmux link) | rx | Both | **C**. `Ok(n)` arm: `cell.rx(n)`; `Ok(0)`/`Err` arm: `cell.close_hint()` |
| H02 | `src/pty/mod.rs:202` | `writer.write_all(bytes)?;` | `PtySession::write_all` | tx | Both | **C**. Covers `src/pane/mod.rs:731,785,804`, `src/pane/scroll.rs:748`, `src/tmux/transport.rs:110,157` |
| H03 | `src/vt/worker.rs:303` | `match pty_reader.read(&mut buf) {` | VT worker pipe thread | rx | Both | worker is test/seam today (no `VtWorkerHandle` use under `src/` outside `worker.rs`); instrument for parity |
| H04 | `src/vt/worker.rs:472` | `let _ = writer.write_all(&data);` | `VtWorkerCommand::WritePty` | tx | Both | |
| H05 | `src/vt/worker.rs:610` | `let _ = writer.write_all(&resp);` | ghostty query replies | tx | Both | cfg `ghostty-vt` |
| H06 | `src/vt/worker.rs:615` | `let _ = writer.write_all(&resp);` | VT query replies | tx | Both | |
| H07 | `src/pane/mod.rs:720` | `pub fn process_pty_output(&mut self, bytes: &[u8]) {` | VT ingest (direct feed) | rx | classify only | calls `classify` when flag set; bytes already counted at H01/H13 |
| H08 | `src/pane/mod.rs:750` | `pub fn ingest_pty_events(&mut self, events: Vec<PtyEvent>) -> bool {` | VT ingest (coalesced) | rx | classify only | same |
| H09 | `src/pane/mod.rs:796` | `pub fn write_to_pty(&mut self, bytes: &[u8]) -> Result<()> {` | pane input | tx | Payload | count only when `self.pty.is_none()` (tmux-mapped pane) |
| H10 | `src/pane/mod.rs:852` | `fn drain_osc52_decoded(&mut self) {` | inbound OSC 52 | E | - | `cell.event(decoded.len())` |
| H11 | `src/pane/mod.rs:827` | `pub fn record_clipboard_event(` | copy/yank/cut | E | - | |
| H12 | `src/pane/mod.rs:838` | `pub fn record_media_passthrough(&mut self, mime: impl Into<String>, bytes_len: usize) {` | media paste | E | - | `bytes_len` is the metered size |
| H13 | `src/tmux/mod.rs:577` | `pane.process_pty_output(data);` | tmux `%output` payload | rx | Payload | pane = mapped pane; connection = link |
| H14 | `src/wm/tmux_io.rs:454` | `pane.process_pty_output(&rendered);` | capture-pane hydrate | rx | Payload | |
| H15 | `src/wm/tmux_link.rs:524` | `pane.process_pty_output(&poll.raw_output);` | pre-control ssh banner text | rx | Payload | |
| H16 | `src/tmux/mod.rs:721` | `if stdin.write_all(cmd.as_bytes()).await.is_err() {` | legacy local `tmux -CC` stdin | tx | Wire | origin `TmuxLocal` |
| H17 | `src/tmux/mod.rs:729` | `while let Ok(Some(line)) = lines.next_line().await {` | legacy local `tmux -CC` stdout | rx | Wire | count `line.len()+1` |
| H18 | `src/tmux/transport.rs:69` | `let pty = PtySession::spawn(cfg)?;` | link transport spawn | - | - | cell exposed via `LinkDriver::traffic_cell()`; adopted at `src/wm/tmux_link.rs:273,276` (`conn.connect(&target, size)`) |
| H19 | `src/api/server.rs:509` | `serve_jsonrpc_connection(stream, req_tx, token)` | RPC unix/tcp socket | rx+tx | Wire | **C**. `Metered<S>` wrapper; covers `:562` read and `:595,635,644,658,670` writes. Plane meter via `bind_plane`-style seam in `serve_loop` (`:480`) |
| H20 | `src/api/http.rs:277` | `.read(&mut chunk)` (and `:349`) | HTTP request head/body | rx | Wire | `read_request` |
| H21 | `src/api/http.rs:253` | `w.write_all(head.as_bytes()).await?;` | HTTP response (`write_response`/`write_typed`, `:224-255`) | tx | Wire | |
| H22 | `src/api/http.rs:552` | `stream.write_all(out.as_bytes()).await?;` | SSE events | tx | Wire | |
| H23 | `src/api/http.rs:558` | `r = stream.read(&mut probe) => {` | SSE EOF probe | rx | Wire | count `n` if `Ok(n>0)` |
| H24 | `src/api/ws.rs:490` | `r = stream.read(&mut chunk) => {` | WebSocket frames in | rx | Wire | label `web-N` from `ClientViewRegistry::allocate_id` (`src/api/client_view.rs:51`) |
| H25 | `src/api/ws.rs:513` | `stream.write_all(&wire).await?;` (and close frame `:516`) | WebSocket frames out | tx | Wire | |
| H26 | `src/api/mcp.rs:560` | `while let Some(line) = lines.next_line().await? {` | MCP stdio in | rx | Wire | origin `Mcp` |
| H27 | `src/api/mcp.rs:566` | `stdout.write_all(format!("{resp}\n").as_bytes()).await?;` | MCP stdio out | tx | Wire | |
| H28 | `src/app.rs:124` | `out.write_all(wire.as_bytes())?;` | outbound OSC 52 to host terminal | tx | Wire | origin `HostTty` |
| H29 | `src/main.rs:515` | `let backend = CrosstermBackend::new(io::stdout());` | all host-terminal render output | tx | Wire | wrap `io::stdout()` in `CountingWriter<Stdout>`; type `Terminal<CrosstermBackend<io::Stdout>>` appears in `src/app.rs:51`, so the wrapper must preserve that type or the signature changes in `app.rs` (see Pitfall 6) |
| H30 | `src/app.rs:55` | `let mut reader = EventStream::new();` | host terminal input events | E | - | count events by kind, not bytes (bytes are not observable) |
| H31 | `src/clipboard.rs:445` (and `:463`) | `let _ = stdin.write_all(text.as_bytes());` | OS clipboard write via `wl-copy`/`xclip`/`xsel` | tx | Wire | origin `ClipboardOs`; `clipboard.rs` is below `pane`, uses a free-function tap (`clipboard::set_traffic_tap`) |
| H32 | `src/clipboard.rs:493` | `let _ = out.read_to_end(&mut buf);` | OS clipboard read | rx | Wire | |
| H33 | `src/stt/provider.rs:244,252` and `:339,346` | `.write_all(&request)` / `.read_to_end(&mut response_bytes)` | STT HTTP provider over TCP | tx/rx | Wire | origin `Stt{host}`; locality by `is_loopback(host)` |

**Not read this session (candidates, audit in Plan 64-02 Task 0):** `src/daemon/process.rs:56` and `src/daemon/mod.rs:239` (`UnixStream::connect` for daemon attach IPC). `src/api/server.rs:286,300,305` are the `bilterm ctl` client, a separate short-lived process: exempt with reason.

**Total: 33 hook sites in 15 files.** Five chokepoints (H01, H02, H19, H20/H21, H24/H25) carry essentially all steady-state traffic.

Only `http.rs:115-120` guarantees the HTTP listener is loopback-only (`refusing to serve HTTP on a non-loopback listener`) [VERIFIED: src/api/http.rs:117-120], so HTTP/WS channels are `Locality::Loopback`. JSON-RPC TCP may be non-loopback with `--allow-remote` [CITED: docs/DEVELOPER_AND_AGENT_GUIDE.md section 3.4], so TCP RPC locality is decided from the peer address at accept (`serve_loop`, `src/api/server.rs:503-509`).

## Layer DAG Analysis

The DAG test, quoted verbatim [VERIFIED: tests/vt_worker_and_snapshot_seam.rs:219-262]:
- `pty` forbids `"crate::vt"`, `"crate::pane"`, `"crate::wm"`, `"crate::ui"`, `"crate::api"`
- `vt` forbids `"crate::pane"`, `"crate::wm"`, `"crate::ui"`, `"crate::api"`
- `pane` forbids `"crate::wm"`, `"crate::ui"`, `"crate::api"`, `"crate::config"`
- `ui` forbids `"crate::api"`

None forbids `crate::traffic`, so a new leaf module is importable from `pty`, `vt`, `pane`, `tmux`, `wm`, `api`, `ui`, `app` with no DAG-test change. Consequences:
1. `traffic` must import nothing from the crate (otherwise `pty -> traffic -> X` could create a cycle). New test enforces this.
2. The pure snapshot/group types live in `traffic`, so `ui` and `api` render/serialise them without importing `wm`.
3. Anything that needs topology (`AttributionMap`) is built in `wm` and passed in.
4. `pane` cannot read `config`, so the `[traffic]` knobs reach cells through `meter.apply_config()` (flag flips on every registered cell) called from `wm`, not by the pane reading config.
5. The OIA indicator reads `pane.traffic_cell().rates()` (published atomics) and the `F_OIA` flag on the cell: `render_oia_status_bar_themed(pane, ...)` receives only `&CompositePane` (`src/ui/oia.rs:34-40`), so no signature change is needed.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TUI sparkline | Custom glyph mapper | `ratatui::widgets::Sparkline` (TUI) + shared `Vec<u64>` series (GUI) | Widget exists; the series is the contract |
| Floating window drag/resize/dock/pin | New geometry code | Clone `ClipboardWindowState` + `panel_dock` + `PanelTarget` patterns (Developer Guide 6c) | Eight prior panels already share the contract; deviations break `:arrange` |
| Colon -> RPC -> MCP plumbing | Bespoke dispatch | `Action::TrafficMonitor(op)` + `colon_form()` + `dispatch_method` + `RPC_METHODS` | Conformance test `tests/api_conformance.rs` fails otherwise |
| Palette/help/completion sync | Manual lists | `command_registry::catalog()` rows + `PALETTE_EXCLUSIONS` | Drift tests fail on mismatch |
| Atomic registration of closed channels | Weak refs / finalizers | `Arc::strong_count == 1` check in the sampler | Deterministic, no `Drop` hooks in hot types |
| RPC byte counting at 6 write sites | Edit each `write_all` | `Metered<S>` wrapper at the accept site | One hook, cannot be forgotten by new writes |
| Time in tests | `thread::sleep` | Injected `now_ms` (`FixtureClock`) | Matches `LinkMachine`; no flakiness |
| Randomised conservation tests | New dev-dependency (`proptest`) | Deterministic LCG in the test | `Cargo.toml` dev-deps are only `insta`, `tempfile` [VERIFIED: Cargo.toml:38-40] |

**Key insight:** the cheapest correct meter is "a monotonic counter per owner plus a cold sampler that diffs it"; every richer design (events, locks, label maps) puts cost on the path whose cost we are trying to measure.

## Common Pitfalls

### Pitfall 1: Name collision with the Phase 50 AGENT TRAFFIC bus
**What goes wrong:** `src/wm/traffic.rs` (`TrafficBus`, `TrafficEndpoint`, `TRAFFIC_CAP`), the `PanelTarget::Agents` "AGENT TRAFFIC window", the `:agents traffic` sub-verb (`src/colon_command/execute_agents.rs:291`: `("traffic", traffic),`), the `View -> Agent Traffic (Ctrl+Alt+A)` menu row and `tests/phase50_traffic.rs` all already use "traffic" for *who-messaged-whom*. [VERIFIED: grep this session]
**Why:** reusing the word invites a bad merge or a wrong `use`.
**How to avoid:** the new things are always `traffic_meter` / `TrafficMeter` / `TRAFFIC MONITOR` / `TrafficMonitorOp` / `Action::TrafficMonitor` / `traffic.*` RPC; the crate module is `crate::traffic` (path `src/traffic/`), distinct from `crate::wm::traffic`. Docs call the old one "Agent Traffic". The colon head `:traffic` is free (only the `:agents` sub-verb uses the word) [VERIFIED: grep]. Help text for both cross-references the other.
**Warning signs:** `use crate::wm::traffic::*` in new files; a palette entry reading just "Traffic".

### Pitfall 2: Hot-path cost creep
**What goes wrong:** classification or `Instant::now()` slips into the reader thread (`src/pty/mod.rs:160`), which sits in front of a bounded channel of 32 (`mpsc::channel(32)`, `src/pty/mod.rs:154`) and can throttle a flooding child.
**How to avoid:** reader thread does exactly `if flags&ENABLED { fetch_add x2 }`. Classification runs on the consumer thread in `ingest_pty_events` only when `F_CLASSIFY` is set. Bench gate in Plan 64-01.

### Pitfall 3: Double counting wire vs payload
**What goes wrong:** ssh-tmux link bytes counted at the PTY (H01) and again as pane payload (H13) inflate connection totals.
**How to avoid:** `Role` and the conservation tests; connection totals sum `Wire|Both` only.

### Pitfall 4: Session attribution drift
**What goes wrong:** window moved to another session keeps old stamps.
**How to avoid:** attribution map built each snapshot via `resolve_pane_session_and_title` logic (`src/wm/rename_dialog.rs:157-206`) over live + background sessions; no stored session on channels.

### Pitfall 5: Unbounded growth from closed panes / clients
**How to avoid:** `MAX_CHANNELS = 256` live+closed, `MAX_CLOSED = 64` retained; evicted rolled into `Totals` so lifetime sums stay conserved; labels capped at 48 chars; snapshot rows capped by `limit` (default 100, clamp 1..=500). Follows the project's `bounding-untrusted-resource-use` rule (peer-controlled client count: `MAX_WEB_CLIENTS = 64` precedent, `src/api/client_view.rs:14`).

### Pitfall 6: `Terminal<CrosstermBackend<io::Stdout>>` type coupling
**What goes wrong:** wrapping stdout changes the concrete backend type used in `src/app.rs:51` and `src/main.rs:515` and in GUI paths.
**How to avoid:** prefer counting render output by diffing the ratatui backend's flush if a wrapper is invasive: a `CountingStdout` newtype that implements `Write` and is aliased as `HostOut` used by both files. If that spreads beyond `app.rs`/`main.rs`, fall back to counting only H28 and reporting host-TTY tx as "OSC/host writes" with a documented limitation. Decide in Plan 64-02 Task 3 after reading `app.rs:40-60` and `main.rs:500-530`.

### Pitfall 7: Test isolation through process globals
**How to avoid:** all counters live in per-cell atomics; the only global is `DEFAULT_FLAGS: AtomicU8` read at cell creation, set from config at startup. Tests that need "disabled" call `cell.set_flags(0)` / `meter.set_enabled(false)`.

### Pitfall 8: `file > 1200 lines`
`src/wm/mod.rs` is already large; add only fields, `mod` lines and one tick call there (Plan 64-01 owns the single edit). All new logic goes in new files.

## Code Examples

### Metered stream (RPC listener)
```rust
// src/api/server.rs near :509 (sketch)
let tap = meter.open_channel(Origin::RpcUnix /* or RpcTcp{loopback} */, ConnectionLabel::auto());
tokio::spawn(async move {
    let stream = Metered::new(stream, tap);           // counts poll_read / poll_write bytes
    if let Err(e) = serve_jsonrpc_connection(stream, req_tx, token).await { /* unchanged */ }
});
```

### Pane-side classify hook
```rust
// src/pane/mod.rs  ingest_pty_events, before self.vt.process(&coalesced)   (sketch)
if let Some(cell) = self.pty_traffic_cell() {
    if cell.classify_enabled() { crate::traffic::classify::observe(cell, &coalesced); }
}
```

### Synthetic fixture
```rust
// tests/phase64_traffic_core.rs (sketch)
let mut fx = SyntheticTraffic::new();
let local = fx.channel(Origin::LocalPty, Some(1), Role::Both);
let ssh   = fx.channel(Origin::Ssh { host: "prod-db".into(), port: 22 }, Some(2), Role::Both);
fx.at(0).rx(local, 4096).tx(local, 12).rx(ssh, 1_000_000);
fx.advance_ms(1000);                           // drives meter.sample(1000)
let snap = fx.meter().snapshot();
assert_eq!(group_by(&snap, GroupKey::Connection, ..).len(), 2);
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Per-pane `total_bytes_processed` (rx only, no rates) | Per-channel directional atomics + sampler | This phase | OIA `OUT:` and drawer sparkline can later read the meter; left untouched here |
| Tracing/log based diagnostics | Typed metrics layer | This phase | `bilterm.log` stays for debugging; meter is for operators |

**Deprecated/outdated:** none. `VtParser::total_bytes_processed` stays (used by `agent_state.rs:344` classification rules).

## Runtime State Inventory

Not a rename/refactor/migration phase: omitted.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 (MEASURED, Plan 64-01 Task 5; see ADR section 7) | Perf budgets (disabled <= 1 ns, enabled <= 5 ns/op, classify >= 300 MB/s worst case, sampler <= 50 us @ 256 ch, snapshot+group <= 200 us) are achievable | Perf table (Validation Architecture) | Budgets recalibrated by Plan 64-01 Task 5 before gating; no user impact |
| A2 | `990_000` is a free GUI texture id | Project Structure | Collision caught at compile/const-uniqueness test; trivial renumber |
| A3 | Ctrl+Alt+T and Leader `#` are unused global/leader chords | Surfaces | Verified against `keymap_tables.rs` lists of Ctrl+Alt chars (a A b B f F h k l m M n N p P r R s S v V w W y Y; Left/Right/Up/Down) and leader chars shown by grep this session, but `keymap.rs` reserved-chord handling (`keymap.rs:61`) and raw-passthrough reservations were not re-read; confirm in Plan 64-03 Task 1 |
| A4 | Counting render output via a `Write` newtype is feasible without changing GUI paths | Hook H29 | Falls back to counting H28 only (documented limitation) |
| A5 | Default `enabled = true` with bytes-only counting is acceptable to the user; `classify` default off until the monitor is open or `[traffic] classify = true` | Config | If user wants fully off by default, flip one default |
| A6 | `traffic.config set` (toggling `enabled`) should sit behind the `allow_config_write` gate because it alters observability | Security | If wrong, relax to ungated; low risk |
| A7 | Daemon attach IPC (`src/daemon/process.rs:56`, `src/daemon/mod.rs:239`) moves bytes worth metering | Hook inventory | If it carries only tiny control frames, exempt with a reason in the audit fixture |
| A8 | `VtWorkerHandle` is not wired into production panes (no non-test use under `src/` outside `worker.rs`) | H03-H06 | If wired later, hooks already exist; if wired now elsewhere, a double count risk appears (conservation test catches it) |

## Open Questions

1. **Should HTTP/WS be separate connections per request or aggregated per class?**
   - Known: HTTP handlers spawn a task per TCP connection (`src/api/http.rs:151`), each with a `permit` (`MAX_CONNECTIONS`).
   - Unclear: showing every short-lived HTTP request as a row is noise.
   - Recommendation: one channel per *class* for HTTP request/response (`Origin::Http`), one per WebSocket client (`web-N`), one per RPC socket connection; closed channels roll up. Decide row policy in Plan 64-02 Task 4; the group-by `origin` view already aggregates.
2. **Is `ssh` run as a raw pane program (not tmux link) identifiable as a remote connection?**
   - Known: `RemoteHostProfile` has `destination` and `port` (`src/remote/mod.rs:22-28`); `build_ssh_args` places `--` before the destination (Developer Guide 3.4).
   - Unclear: how a pane records its ssh destination; only `remote_target` (`src/pane/mod.rs:80`) is set for tmux-link panes (`src/pane/capture.rs:134,145` show the `user@host/session` form).
   - Recommendation: add `pane::shell_args()`-based parse helper `remote::ssh_destination_from_args(&[String]) -> Option<(String, Option<u16>)>` (reads args after `--`, `-p`), used by `wm/traffic_view.rs` to classify a pane's PTY channel as `Origin::Ssh` instead of `LocalPty`. Plan 64-02 Task 2.
3. **Host-TTY byte counting (Pitfall 6).** Resolve by reading `app.rs:40-60`, `main.rs:500-530` at execution.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| rustc / cargo | build, tests | yes | 1.90.0 (rust-toolchain `1.90`) | - |
| `scripts/verify.sh` | gates | yes | repo script | - |
| Linux PTY (`/dev/ptmx`) | end-to-end PTY counting test | yes (Linux host) | - | test skips on non-Linux via `cfg(target_os = "linux")` |
| ssh server | real ssh test | not required | - | all ssh/network paths tested with `SyntheticTraffic` and `PtyLinkConnector` mocks (`wm.set_link_connector`) |
| `xclip`/`wl-copy` | clipboard boundary | not required | - | `os_set_clipboard` is `#[cfg(not(test))]`; test via the registry-level event hook |

**Missing dependencies with no fallback:** none.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness + `insta 1.42.2` (snapshots) via `scripts/verify.sh` stages (one stage per `tests/*.rs`, plus `clippy`, `unit`) |
| Config file | `Cargo.toml`, `scripts/verify.sh` (perf tier lists `t:full_suite_battle_and_perf`) |
| Quick run command | `scripts/verify.sh --quick` |
| Full suite command | `scripts/verify.sh` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TMON-01 | cell counts rx/tx/msgs; disabled records nothing; classify splits esc/ctrl/print across chunk boundaries | unit | `scripts/verify.sh --only unit` | Wave 0 (64-01) |
| TMON-01 | real PTY `echo` is counted rx>0, tx>0 | integration | `scripts/verify.sh --only t:phase64_traffic_hooks` | Wave 0 (64-02) |
| TMON-02 | rolling 1s/10s/1m rates, peaks, young-channel divisor, bounded rings, closed roll-up conservation | unit | `scripts/verify.sh --only t:phase64_traffic_core` | Wave 0 (64-01) |
| TMON-02 | layer DAG: `traffic` imports nothing; byte-boundary audit manifest | integration | `scripts/verify.sh --only t:phase64_layer_dag` | Wave 0 (64-01) |
| TMON-03 | window state machine, geometry single-source hit table, TUI render snapshot, GUI card renders | integration | `scripts/verify.sh --only t:phase64_traffic_window` | Wave 0 (64-03) |
| TMON-04 | one `Action` from key, mouse, colon, palette, menu, RPC, MCP | integration | `scripts/verify.sh --only t:phase64_traffic_surfaces` | Wave 0 (64-03) |
| TMON-05 | perf: disabled vs enabled vs classify overhead ratio | perf | `scripts/verify.sh --tier perf` | Wave 0 (64-01) |

### Perf budget (targets; calibrated in Plan 64-01 Task 5, then asserted)
| Path | Budget | Test form |
|------|--------|-----------|
| Disabled hot path (`rx`/`tx`) | <= 1 ns/op, 0 allocations | ratio vs empty-loop baseline <= 4x in debug; absolute in release |
| Enabled bytes-only | <= 5 ns/op uncontended | same |
| Classify, ground fast path | >= 1 GB/s | `#[cfg(not(debug_assertions))]` absolute; debug uses ratio vs `Vec::iter().filter` |
| Classify, escape-dense worst case | >= 300 MB/s | same |
| `sample()` non-boundary | <= 50 ns | loop of 1e6 calls |
| `sample()` boundary, 256 channels | <= 50 us | |
| `snapshot()` + `group_by` 256 ch | <= 200 us | |
| Memory ceiling | <= 0.6 MB (256 x (counters ~128 B + 60 buckets x 32 B)) | `size_of`-based computed bound test |
| End-to-end regression | PTY flood throughput with monitor enabled within 2% (release) / 25% (debug) of disabled | relative, median of 5, same process |

### Sampling Rate
- **Per task commit:** `scripts/verify.sh --quick`
- **Per wave merge:** `scripts/verify.sh`
- **Phase gate:** full suite green plus `scripts/verify.sh --tier perf` before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `src/traffic/*` unit tests (64-01)
- [ ] `tests/phase64_traffic_core.rs`, `tests/phase64_layer_dag.rs` (64-01)
- [ ] `tests/phase64_traffic_hooks.rs` (64-02)
- [ ] `tests/phase64_traffic_window.rs`, `tests/phase64_traffic_surfaces.rs` (64-03)
- [ ] no framework install needed

## Security Domain

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no (no new auth; RPC auth unchanged) | existing `auth.login` / bearer token |
| V3 Session Management | no | - |
| V4 Access Control | yes | `traffic.reset`, `traffic.config set` behind `config_write_denied` (`src/wm/mod.rs:398`); read methods always allowed |
| V5 Input Validation | yes | filter strings capped (64 chars), `limit` clamped 1..=500, group/sort enums closed, labels sanitised (ASCII printable, 48 chars) |
| V6 Cryptography | no | - |
| V8 Data Protection | yes | meter stores counts and labels only, never bytes; host labels may reveal infrastructure: `[traffic] label_hosts = true` default, `false` hashes to `host-<n>` |

### Known Threat Patterns for this stack
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Unbounded channel/label growth from hostile clients (many WS/RPC connects) | DoS | `MAX_CHANNELS`, closed-channel roll-up, class+id labels, existing `MAX_WEB_CLIENTS`/`MAX_CONNECTIONS` |
| Label injection (escape sequences in `host`/`client_id` rendered in TUI) | Tampering | `traffic::label::sanitize` strips controls and caps length; renderer additionally uses `sanitize_display` (`crate::vt::sanitize_display`) per Developer Guide 6c step 4 |
| Information disclosure via RPC (`traffic.snapshot` exposes hosts) | Info disclosure | same auth as other RPC reads; `label_hosts=false` option |
| Observer effect / side channel of keystroke timing via per-second tx rate | Info disclosure | resolution capped at 1 s buckets; no per-event timestamps stored |
| Disabling the meter over RPC to hide activity | Repudiation | `traffic.config set` is write-gated and logged via `tracing` (`bilterm::traffic`) |

## Sources

### Primary (HIGH confidence)
- Repository source opened this session: `src/pty/mod.rs:90-205`, `src/vt/worker.rs:200-340,455-480,595-620`, `src/pane/mod.rs:190-240,700-870`, `src/tmux/transport.rs:1-175`, `src/tmux/mod.rs:565-580,680-740`, `src/api/server.rs:480-680`, `src/api/http.rs:100-160,255-300,462-570`, `src/api/ws.rs:28-60,465-520`, `src/api/mcp.rs:548-570`, `src/api/client_view.rs:1-100`, `src/app.rs:50-135`, `src/main.rs:505-525`, `src/clipboard.rs:430-500`, `src/stt/provider.rs:225-260`, `src/wm/traffic.rs:1-40`, `src/wm/rename_dialog.rs:150-215`, `src/wm/panel_dock.rs:1-60`, `src/ui/oia.rs:1-70,170-200`, `src/ui/modals.rs:385-445`, `src/ui/typography.rs:79-90`
- `tests/vt_worker_and_snapshot_seam.rs:195-270` (layer DAG test)
- `AGENTS.md`, `docs/DEVELOPER_AND_AGENT_GUIDE.md` (sections 3, 6b, 6c, 6d)
- `.planning/phases/57-clipboard-fix-and-viewer-window/57-0{1,2,3}-PLAN.md`, `57-CONTRACT.md`, `.planning/phases/60-web-frontend-surface-and-protocol-bridge/60-ARCHITECTURE-ADR.md`
- `~/.cargo/registry/src/index.crates.io-*/ratatui-widgets-0.3.0/src/sparkline.rs`

### Secondary (MEDIUM confidence)
- none

### Tertiary (LOW confidence)
- Perf numbers (training-knowledge estimates of relaxed `fetch_add` and branch cost), tagged A1.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - std atomics plus an existing dependency; no new packages
- Architecture: HIGH - hook sites and DAG constraints read directly from source
- Pitfalls: HIGH for naming/DAG/attribution; MEDIUM for host-TTY wrapper (not fully read)
- Perf budgets: MEDIUM-LOW until measured

**Research date:** 2026-11-08
**Valid until:** 2026-12-08 (code-structure research; re-verify line numbers at execution since branch `plan/phases-61-64` may move them)
