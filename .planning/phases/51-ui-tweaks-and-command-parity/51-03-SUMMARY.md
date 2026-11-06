---
phase: 51-ui-tweaks-and-command-parity
plan: 03
subsystem: wm
tags: [agents-demo, demo-stop, traffic-bus, topology, UIT-01]
requires: ["51-02"]
provides:
  - "TrafficBus::retain_events, TrafficBus::remove_topology_pair, Topology::remove_pair"
  - "WindowManager::start_agents_demo / stop_agents_demo, DemoStartReport / DemoStopReport, DEMO_SCRIPT (agents_demo.rs)"
  - ":agents demo stop real body (executor only formats status)"
affects: [51-04, 51-07, 51-08, 51-10, 51-11, 51-12]
key-files:
  created:
    - tests/phase51_demo_stop.rs
  modified:
    - src/wm/agents_demo.rs
    - src/wm/traffic.rs
    - src/wm/agents_topology.rs
    - src/colon_command/execute_agents_topology.rs
decisions:
  - "demo events are identified by predicate (kind Message + `[demo] ` prefix + both endpoints in the demo endpoint set), never by a flag on TrafficEvent (ADR-008)"
  - "a topology pair is purged only when no remaining ring event links its two panes"
  - "seed_agents_demo_for_tests kept (phase51_seams.rs still uses it; not in this plan's files)"
status: complete
plan_head_before: 0d52caeb
plan_head_after: 0b07c59dbdd33d6f24b376c7515e43126a03890d
commits: 2
actuals:
  tokens: 14000
  tasks: 2
  commits: 2
---

# Phase 51 Plan 03: `:agents demo stop` Summary

`:agents demo stop` closes exactly the panes the demo spawned, removes the demo's `[demo]` Message events and topology pairs, resets stale graph/snoop references, and reports counts; re-running `:agents demo` replaces earlier demo events without leaking panes.

Commits: `3d1f1db` (Task 1 primitives), `0b07c59` (Task 2 start/stop, executor, tests).

## Gates

- `cargo test -q --test phase51_demo_stop`: 16 passed (3 primitive + 13 behaviour, incl. two extras: `stop_keeps_real_messages_that_look_similar`, `stop_keeps_ui_refs_to_surviving_nodes`).
- `cargo test -q --test agents_launch_affordances --test phase50_agents_window --test phase50_traffic --test phase50_agent_state --test phase51_seams --test phase51_tracer`: all pass (7, 19, 70, 11, 10, 10).
- `cargo clippy -q --all-targets -- -D warnings`: clean.
- `scripts/verify.sh`: `PASS 80/80 86s`. `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`.
- `git diff --exit-code HEAD -- Cargo.toml Cargo.lock`: clean.
- Acceptance greps: `pub fn retain_events` 1, `pub fn remove_pair` 1, `Reserved for Phase 51` in traffic.rs 0, `pub fn stop_agents_demo` 1, `record_traffic_with` in agents_demo.rs 1, `wm.record_traffic(` in executor 0, `remove_pane` in agents_demo.rs 4, executor 182 lines (<= 220).

## Assumption A1 finding

`impl Drop for PtySession` (src/pty/mod.rs:285) closes the output channel and, on a reaper thread, kills and waits the child and drops the master. `remove_pane` drops the `CompositePane` (and so its session), so closing via `remove_pane` terminates the pane's shell. This is the same path `:close-pane` uses.

## Deviations from Plan

None - plan executed as written, with these notes:

- Added `TrafficBus::remove_topology_pair` (thin wrapper) because `Topology` is a private field of the bus and `agents_demo.rs` is not a descendant of `traffic`. File is in the plan's list.
- Topology purge uses "no remaining linking event" rather than unconditional removal, so a real Phase 52 Pane-to-Pane pair between demo endpoints would survive.
- `seed_agents_demo_for_tests` was not deleted (51-02 tests depend on it and that test file is not mine).
- The `status` text of start now also mentions `:agents demo stop`; wording still starts `Demo:`.
- Task 1 RED run was not observed separately (tests were written against absent methods, so compile failure was implicit).

## Known Stubs

None.

## Threat Flags

None. T-51-06 (spawned-only close, missing ids skipped, clear on session reset) and T-51-07 (three-part predicate) mitigated by tests `stop_closes_spawned_panes`, `session_reset_clears_demo`, `stop_keeps_real_messages_that_look_similar`.

## Self-Check: PASSED

Files present: tests/phase51_demo_stop.rs, src/wm/agents_demo.rs. Commits 3d1f1db and 0b07c59 are ancestors of HEAD.
