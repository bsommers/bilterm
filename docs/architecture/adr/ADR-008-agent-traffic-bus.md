# ADR-008: Agent Traffic Bus (`FLT-02`, Phase 50 D-01)

- **Status**: Accepted for Phase 50
- **Date**: 2026-11-03
- **Requirements Covered**: `FLT-01`, `FLT-02`
- **Consumers**: Phase 50 Navigator zoom window graph and snoop view; Phase 51 agent messaging (`51-01` spec)

---

## Context

The Phase 50 Navigator zoom view shows which panes talk to which, and the snoop view shows what was sent. Both need a single, cheap record of "who sent what to whom". Phase 51 adds named agents and `agent.send`, which must publish into the same record rather than invent a second one. Today the only traffic sources are:

1. Input broadcast (`WindowManager::broadcast_command`).
2. `:mcp broadcast <subtree> <cmd>` (Master Control Program console, via `DaemonRequest::BroadcastSubtree`).
3. Automation-API calls `pane.exec` and `pane.send_keys` (JSON-RPC, HTTP and MCP stdio share `AutomationControlPlane::dispatch_method`).

Because `api` sits above `wm` in the layer DAG, the bus lives in `wm` ([`src/wm/traffic.rs`](../../../src/wm/traffic.rs)), not next to `AutomationEvent` in `src/api/playbook.rs`. The event schema is costly to change once Phase 51 builds on it, so it is recorded here.

---

## Decisions

### 1. Endpoint vocabulary

`TrafficEndpoint` is one of `User`, `Api`, `Mcp`, `Pane(PaneId)`, `Session(String)`, `Agent(String)`. `label()` returns `you`, `api`, `mcp`, `#<id>`, `@<session>`, or the agent name.

- `Mcp` means the **Master Control Program console** (`:mcp ...`). The Model Context Protocol transport (`bilterm --mcp`) is `Api`.
- `Agent(String)` is **reserved for Phase 51** named agents. Phase 50 never produces it.

### 2. Kind vocabulary

`TrafficKind` is one of `Input`, `Exec`, `Keys`, `Broadcast`, `Message`; `as_str()` returns `input | exec | keys | broadcast | message`. `Message` is **reserved for Phase 51**.

### 3. Event shape

`TrafficEvent { seq: u64, at: Instant, from, to, kind, preview: String, bytes: usize, redacted: bool }`.

- `seq` is monotonic and starts at 1. Readers keep a cursor and call `TrafficBus::events_since(seq)`; `latest_seq()` is `0` before the first event.
- `at` is a monotonic `Instant` (not wall-clock). Any externally published form is chosen in Phase 51.

### 4. Bounds

- The ring holds at most `TRAFFIC_CAP = 512` events; the oldest is evicted first and counted in `dropped()`.
- Previews are passed through `crate::vt::sanitize_display` (control characters, bidi controls and BOM removed) and capped at `PREVIEW_CAP = 160` chars, with a trailing ellipsis when cut.
- `bytes` is the byte length of the original content and is kept even when the preview is redacted.

### 5. Redaction

A preview aimed at a pane that is in a password / `!ECHO` prompt (including the canvas-password prompt) is replaced by `[redacted]` and `redacted = true`. `record_traffic` decides this from the target pane; `record_traffic_with` takes an explicit decision for callers that must evaluate it before dispatch (`pane.send_keys` captures the target and its prompt state before the chords run, because the chords may change that state).

### 6. In-memory only, single producer API

Nothing is persisted (no `secure_fs` use) and content is never written to tracing logs. All producers call `WindowManager::record_traffic` / `record_traffic_with`; `:mcp` deliveries are recorded by `record_new_mcp_broadcasts` from the growth of `daemon_state.broadcast_log`. Error paths record nothing. Readers use `WindowManager::traffic()` and `partners_of(pane, window, now)`; `:agents list` exposes `partners` per pane and a top-level `traffic` counter object.

---

## Consequences

- Phase 50 does **not** publish bus events on `/events` or `events.poll`; the published API is unchanged. Phase `51-01` chooses the external event names (the dotted `agent.message` style versus existing snake_case `type` values) with user sign-off.
- The internal enums stay independent of JSON, so the external names can differ from the Rust identifiers.
- Until Phase 51, the graph is star-shaped: external senders (`you`, `api`, `mcp`) fan out to panes and sessions. Tests inject synthetic events directly.
- A bounded ring means slow readers can miss events; they detect this by a gap between their cursor and the first retained `seq`.
- Adding a variant to `TrafficEndpoint` or `TrafficKind` is a breaking change for exhaustive matches in the Navigator renderers and must be done together with an ADR amendment.
