---
phase: 50-agent-aware-workspace-navigator
plan: 10
subsystem: input
tags: [agents-window, snoop, conversation, filters, compose-gate, colon-command]
status: complete

requires:
  - phase: 50-16
    provides: "agents window pin/scale, PanelTarget::Agents, panel_space mouse remap"
  - phase: 50-09
    provides: "handle_agents_window_mouse, agents_pane_rects / agents_hit, wm. allow-list gate"
  - phase: 50-08
    provides: "AGENTS_WINDOW_BINDINGS, AgentsKeyScope, agents_window_action"
provides:
  - "SnoopState / WindowPane / SnoopLine / snoop_lines / copy_selected_snoop / apply_*_filter / open_agents_window_on_edge on WindowManager (agents_window.rs)"
  - "ui::agents_ui::snoop_layout (SnoopLayout, SnoopButton): one layout for render and mouse hit-test"
  - "AgentsOp FocusPane, SnoopStep, SnoopSelect, SnoopScroll, Follow, Filter(SnoopFilterOp), KeywordEdit, Copy, ComposeSubmit"
  - "Key scopes Snoop / KeywordEntry / Compose; Ctrl+O focus cycle (F6 stays global DeckNext)"
  - ":agents focus|line|scroll|follow|filter|copy"
  - "ComposeGate::Unavailable seam for Phase 52 (52-02)"
affects: [50-11, 50-12, 50-13, 50-14, 52-02]

actuals:
  tokens: 26400
  tasks: 3
  commits: 3
plan_head_before: c5a82f8b30b63fdc7aa1d7dc04033b6aa2f37a5e
plan_head_after: bd5f824c4eebfa21ce8dcc4cf0fc31f71fc180c0

tech-stack:
  added: []
  patterns:
    - "snoop_layout is the single placement source: renderer draws from it, mouse handler hit-tests it"
    - "Scope resolution: own scope, then printable-char rule (keyword Push / compose swallow), then Window scope"
    - "Viewport anchored by bottom-line index while follow is off, so new traffic never moves the view"

key-files:
  created: []
  modified:
    - src/wm/agents_window.rs
    - src/wm/agents_cmd.rs
    - src/wm/agent_state.rs
    - src/ui/agents_ui.rs
    - src/input/agents_window.rs
    - src/input/keymap.rs
    - src/colon_command/parse_agents.rs
    - src/colon_command/execute_agents.rs
    - src/pane/completion.rs
    - src/ui/help_catalog.rs
    - tests/phase50_agents_window.rs
    - tests/snapshots/phase50_agents_window__render_tui_graph_snapshot_static.snap

key-decisions:
  - "Default selection is the newest line (selected_seq None); `k` therefore selects the previous line on first press"
  - "Scroll is the index of the bottom visible filtered line (not an offset from the newest), so appended traffic does not shift a scrolled view; Home = scroll 0, clamped by the viewport"
  - "Scrolling down while following is a no-op; PageDown back to the bottom does not re-enable follow (End / f / marker do)"
  - "Any filter change returns to follow mode and drops a selection that was filtered out"
  - "Mouse: a press inside the conversation pane (incl. buttons) focuses it first; a press on the compose line focuses Compose; [send] additionally applies ComposeSubmit"
  - "Age is measured against AgentTracker::last_update (new accessor), `--` before the tracker has run"

patterns-established:
  - "Drift-test exemptions: only KeywordEdit(_) and ComposeSubmit have no colon form, each with its reason in agents_op_colon_form"
  - "Esc overrides of the Window Esc row: Grab and KeywordEntry (uniqueness test allows exactly these two)"

requirements-completed: []

duration: ~1h
completed: 2026-11-04
---

# Phase 50 Plan 10: Snoop view and disabled compose line Summary

**Conversation (snoop) pane of the agents window with follow, agent/pair/keyword filters, history scroll and copy on key, mouse and `:agents` paths, plus a visibly disabled `send as me` line behind a `ComposeGate::Unavailable` seam.**

## What was built

- **Task 1 (e541f01, tracer):** `SnoopState`, `WindowPane`, `snoop_lines()` (bus events in order, newest last), `copy_selected_snoop()` (`<from> -> <to> [<kind>] <preview>`, redacted stays `[redacted]`), `snoop_layout` with the `[copy]` badge, Ctrl+O `FocusPane(None)` row, `Snoop` scope (`j`/`k`/Up/Down/`y`), `:agents focus|line|copy`.
- **Task 2 (185afe6):** filters (agent, unordered pair, case-insensitive substring capped at 64 chars, AND-combined, no regex), chips with `×`, header buttons, follow marker, wheel and PageUp/PageDown/Home/End, keyword entry scope (`KeywordEdit` Begin/Push/Pop/Commit/Cancel), Enter or second click on a graph edge filters to the pair and focuses the conversation, `open_agents_window_on_edge` for 50-12, `:agents scroll|follow|filter`.
- **Task 3 (bd5f824):** `ComposeGate::Unavailable` and `compose_gate()`, `AgentsOp::ComposeSubmit` (no payload, no colon form), `Compose` scope Enter row, printable keys swallowed with no buffer, `[send]` badge click, muted `send as me ▸ disabled: ...` line, and the parity test.

## Verification

- `scripts/verify.sh` (full): `PASS 56/56`; scoped run `clippy unit t:phase50_agents_window t:phase50_traffic t:api_conformance t:api_config_commands t:drawer_classic_parity`: PASS.
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11` after each task commit (classic drawer output unchanged).
- `wm.` allow-list gate on `src/input/agents_window.rs` prints nothing; `git diff --exit-code Cargo.toml Cargo.lock` clean; `allow_agent_messaging` count in `src/config/dotted.rs` is 0.
- 18 new tests: `window_tracer_snoop_line_select_and_copy`, 11 `snoop_*`, 5 `compose_*`, `parity_snoop_key_mouse_colon_match`; the existing drift tests (`window_keys_match_colon_forms`, key uniqueness, no global shadowing, completion/help coverage) cover every new row and op.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `AgentTracker::last_update()`**
- **Found during:** Task 1
- **Issue:** message age must use the tracker's last update instant (deterministic), but `last_now` is private to `agent_state`.
- **Fix:** added a read-only accessor in `src/wm/agent_state.rs` (not in the plan's file list).
- **Commit:** e541f01

**2. [Rule 1 - Bug] Snapshot `render_tui_graph_snapshot_static` regenerated twice**
- **Found during:** Tasks 1, 2 and 3
- **Issue:** the right pane now renders header, buttons and compose line instead of bare bus lines; the old snapshot is intentionally obsolete.
- **Fix:** reviewed the diff and accepted the new snapshot each task (the file is committed with the task that changed the output).
- **Commits:** e541f01, 185afe6, bd5f824

**3. [Rule 1 - Bug] Palette entry `:agents filter text` did not parse**
- **Found during:** Task 2 verify (`every_palette_entry_with_complete_args_parses_ok`)
- **Fix:** palette row is `:agents filter text echo`.
- **Commit:** 185afe6

**4. [Rule 1 - Bug] `PageDown` after `Home` did not move the view**
- **Found during:** Task 2 test `snoop_follow_and_scroll`
- **Issue:** scroll arithmetic used the stored index instead of the clamped bottom line actually drawn.
- **Fix:** `scroll_snoop` starts from `snoop_view_range`'s bottom.
- **Commit:** 185afe6

### Other notes

- The plan's `SnoopLayout` field list was extended (`entry`, `lines_area`) and `SnoopRows` thresholds added so tiny panes degrade (title row at height >= 3, buttons >= 5, chip/entry row >= 7, compose >= 4).
- The user-facing compose notice is truncated by pane width (about 24 visible characters at 120 columns); the full reason is in `ComposeGate`.

## Known Stubs

- `src/wm/agents_window.rs` `compose_gate()` and the `send as me` line: intentionally disabled by D-09 (no Phase 52 permission gate yet); resolved by Phase 52 plan 52-02, which adds an open `ComposeGate` variant. Not a placeholder for missing data, so it was not added to `.planning/WINDOWS.md`.

## Threat Flags

None. Mitigations T-50-23..25 hold: no outward write path (tests assert no pane bytes, prompt text or bus event; no `:agents send`/`compose` verb), substring-only keyword filter capped at 64 chars, copy is an explicit action and keeps `[redacted]`.

## Self-Check: PASSED

- Files exist: `src/wm/agents_window.rs` (copy_selected_snoop, ComposeGate), `src/ui/agents_ui.rs` (snoop_layout), `src/input/keymap.rs` (Ctrl+O row, Snoop/KeywordEntry/Compose scopes), `tests/phase50_agents_window.rs`.
- Commits e541f01, 185afe6, bd5f824 are ancestors of HEAD.
