---
phase: 50-agent-aware-workspace-navigator
plan: 05
subsystem: wm
tags: [traffic-bus, agents, redaction, rpc, mcp-console, adr]
status: complete

requires:
  - phase: 50-02
    provides: Navigator drawer and shared sanitisation conventions
  - phase: 50-03
    provides: ":agents list" and per-pane agent state
provides:
  - "TrafficEndpoint / TrafficKind / TrafficEvent / TrafficBus (512 ring, 160-char sanitised preview, seq cursor) in src/wm/traffic.rs"
  - "WindowManager::{traffic, record_traffic, record_traffic_with, partners_of} and crate-private record_new_mcp_broadcasts"
  - "Producers: broadcast_command, :mcp broadcast (via mcp_execute wrapper), RPC/HTTP/MCP pane.exec and pane.send_keys"
  - ":agents list per-agent partners plus top-level traffic counters"
  - "ADR-008 as the Phase 52 bus contract"
affects: [50-07, 50-08, 50-10, 50-11, 50-12, phase-51]

actuals:
  tokens: 15000
  tasks: 2
  commits: 2
plan_head_before: a21471bb8ce3d408cab3736feb4f0ff590e58f7d
plan_head_after: f42935a32dfd8de7f03b7061dbc8d16cf1537748

tech-stack:
  added: []
  patterns:
    - "Single producer API on WindowManager; ring buffer with eviction counter and seq cursor"
    - "Redaction decided before dispatch for send_keys/exec (record_traffic_with)"
    - "Wrapper (mcp_execute) around every mcp_console.execute call so no call site can forget to record"

key-files:
  created:
    - src/wm/traffic.rs
    - docs/architecture/adr/ADR-008-agent-traffic-bus.md
    - tests/phase50_traffic.rs
  modified:
    - src/wm/mod.rs
    - src/wm/v2_runtime.rs
    - src/api/rpc.rs
    - src/colon_command/execute_agents.rs
    - docs/architecture/OVERVIEW.md
    - docs/README.md

key-decisions:
  - "Bus lives in wm (api sits above wm in the layer DAG); in-memory only, no secure_fs, no tracing of content"
  - "Phase 50 publishes nothing on /events or events.poll; Phase 53-01 picks external names with user sign-off"
  - "Agent endpoint and Message kind are reserved for Phase 52; Mcp means the Master Control Program console, the MCP transport is Api"
  - "exec/send_keys evaluate the password-prompt redaction before dispatch and pass it explicitly"

requirements-completed: [FLT-01, FLT-02]

coverage:
  - id: D1
    description: "Broadcast, :mcp broadcast, RPC pane.exec and pane.send_keys land on a bounded, sanitised in-memory bus"
    requirement: FLT-02
    verification:
      - kind: unit
        ref: "tests/phase50_traffic.rs bus_tracer_broadcast_records_and_lists_partner, bus_api_origin_broadcast_labels_api, bus_mcp_broadcast_records_session_targets, bus_rpc_pane_exec_records_api_event, bus_rpc_send_keys_records_keys_event"
        status: pass
    human_judgment: false
  - id: D2
    description: "Ring cap, monotonic seq, cursor reads, preview sanitising and password redaction"
    requirement: FLT-02
    verification:
      - kind: unit
        ref: "tests/phase50_traffic.rs bus_ring_cap_evicts_oldest, bus_events_since_cursor, bus_preview_sanitized_and_capped, bus_password_target_redacted"
        status: pass
    human_judgment: false
  - id: D3
    description: ":agents list reports each pane's recent partners"
    requirement: FLT-01
    verification:
      - kind: unit
        ref: "tests/phase50_traffic.rs bus_tracer_broadcast_records_and_lists_partner, bus_partners_window_excludes_old_events"
        status: pass
    human_judgment: false
  - id: D4
    description: "ADR-008 documents the Phase 52 contract including reserved Agent / Message"
    requirement: FLT-02
    verification:
      - kind: command
        ref: "grep -c '^## Decisions' docs/architecture/adr/ADR-008-agent-traffic-bus.md"
        status: pass
    human_judgment: true
    rationale: "Adequacy of the contract for Phase 52 is a design judgement"
---

# Phase 50 Plan 05: Agent Traffic Bus Summary

**Bounded in-memory traffic bus (512 events, sanitised 160-char previews, password redaction, seq cursor) fed by input broadcast, `:mcp broadcast` and RPC/MCP `pane.exec` / `pane.send_keys`, surfaced as `partners` in `:agents list`, with the schema fixed in ADR-008 for Phase 52.**

## Performance

- Tasks: 2 (1 tracer, 1 tdd auto); files: 3 created, 6 modified
- Commits: `9f80646` (tracer), `f42935a` (sources, semantics, ADR)

## Accomplishments

- `src/wm/traffic.rs`: endpoint/kind vocabulary, `TrafficBus`, and the `WindowManager` producer/reader API. `partners_of` groups by the other endpoint within a window, most recent first.
- `broadcast_command` records one event per pane that received the command (`User`, or `Api` when `is_api_origin()`); events for panes already served are recorded even if a later pane errors.
- Every `mcp_console.execute` call in `src/wm/v2_runtime.rs` now goes through `mcp_execute`, which records `Mcp -> Session(name)` events for new `broadcast_log` entries.
- `pane.exec` and `pane.send_keys` record `Api -> Pane(id)` after success; send_keys captures target and password state before the chords run. One hook covers RPC, HTTP and MCP stdio.
- `:agents list` adds per-agent `partners: [{label, count}]` (60 s window) and `traffic: {latest_seq, len, dropped}`; existing fields untouched.
- ADR-008 plus links in `docs/architecture/OVERVIEW.md` and `docs/README.md`.

## Deviations from Plan

None to the plan's behaviour. Notes:

- The ring/cursor/sanitise/redaction semantics were complete in Task 1's `traffic.rs`, so Task 2's `bus_` tests passed on first run rather than being seen failing first (the new `record_new_mcp_broadcasts` helper was added in Task 2 to keep the Task 1 commit warning-free).
- Added the `mcp_execute` wrapper (instead of repeating before/after capture at six call sites) to satisfy "every call site".
- Added ADR-008 index links to `docs/architecture/OVERVIEW.md` and `docs/README.md` (docs-in-sync, not in the plan's file list).

## Known Stubs

None. `TrafficEndpoint::Agent` and `TrafficKind::Message` are intentionally unused until Phase 52 (documented in ADR-008, covered by `bus_agent_message_kind_reserved_for_phase51`).

## Threat Flags

None. T-50-11 (redaction before dispatch, tested), T-50-12 (512 ring + 160 cap), T-50-13 (`sanitize_display`), T-50-14 (origin `User` / `Api` / `Mcp` per producer) mitigated; T-50-SC: `git diff Cargo.toml Cargo.lock` clean.

## Verification

- `cargo test -q --test phase50_traffic`: 11 passed
- `scripts/verify.sh` (full): `PASS 55/55`
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`
- Acceptance greps: `pub struct TrafficBus` 1, `std::fs` in traffic.rs 0, `record_traffic_with` in rpc.rs 2, `record_new_mcp_broadcasts` in v2_runtime.rs 1, `## Decisions` in ADR-008 1

## Next Phase Readiness

Ready for 50-07/08/10/11/12 (graph and snoop readers use `events_since` and `partners_of`).

## Self-Check: PASSED

Created files exist (`src/wm/traffic.rs`, `docs/architecture/adr/ADR-008-agent-traffic-bus.md`, `tests/phase50_traffic.rs`); commits 9f80646 and f42935a are ancestors of HEAD.
