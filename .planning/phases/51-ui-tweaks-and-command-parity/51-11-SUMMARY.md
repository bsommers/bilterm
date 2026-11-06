---
phase: 51-ui-tweaks-and-command-parity
plan: 11
subsystem: docs
tags: [docs, user-guide, api, uniformity, UIT-01, UIT-02, UIT-03, UIT-04, UNI-R01]
requires: ["51-03", "51-04", "51-05", "51-06", "51-07", "51-08", "51-09", "51-10"]
provides:
  - "Chapter 11: Agents list window, Tiling floating panels, Command palette coverage, Surface uniformity (UNI-R01 table and exceptions), every new :agents and :drawer verb"
  - "Chapter 06: Ctrl+Alt+N, leader A / y / Y, in-window list keys, list mouse gestures, menu entries"
  - "docs/API.md: agents.list, agents.demo, bilterm_agents_list, bilterm_agents_demo, 13-tool count, palette_exclusions"
  - "Developer guide: Phase 51 source files, MCP tool count 13, palette exclusion rule, section 6c Adding a floating panel"
affects: [51-12, 51-13]
key-files:
  modified:
    - docs/user-guide/11-navigator-sidebar-and-agent-traffic.md
    - docs/user-guide/06-keyboard-mouse-and-command-reference.md
    - docs/API.md
    - docs/DEVELOPER_AND_AGENT_GUIDE.md
decisions:
  - "docs describe the shipped code: each key, gesture, verb, menu label, RPC field and tool name was checked by grep or by reading the code the SUMMARYs of waves 1-3 name; no claim rests on plan text alone"
  - "MCP tool count is 13 (tools/list), not the 12 written in the plan task text"
  - "docs/user-guide/index.md is not edited: it already links chapter 11 and the gate passes"
status: complete
plan_head_before: f1f548c7a29e248bafcdf9965118e62f00b76d37
plan_head_after: 6996165
commits: 2
actuals:
  tokens: 12482
  tasks: 2
  commits: 2
---

# Phase 51 Plan 11: Docs Summary

Phase 51 is documented from the shipped code: chapter 11 covers the AGENTS list window, demo stop, tiling, palette parity and the surface table, chapter 06 and the API reference cover the new keys, verbs, RPC and MCP surface, and the developer guide lists the new files. `scripts/check_phase50_doc_coverage.sh` prints `DOCS OK 86`.

Commits (task commits; this SUMMARY is a third commit):
- `ebdd226` docs(51-11): chapter 11 (task 1)
- `6996165` docs(51-11): chapter 06, API and developer guide (task 2)

## Gates

- `bash scripts/check_phase50_doc_coverage.sh`: `DOCS OK 86` (exit 0). Baseline at the reset point: `DOCS FAIL: 28 missing`.
- `bash scripts/check_phase50_1_doc_coverage.sh`: `DOCS OK 33` (exit 0).
- Plan greps: `:agents demo stop` in chapter 11 = 3; `agents.list` in API.md = 8; `Ctrl+Alt+N` in chapter 06 = 4; `agents_list_geometry` in developer guide = 1; `^## Agents list window` = 1; `^## Tiling floating panels` = 1; `:drawer style classic` = 1. `bilterm_agents_demo` and `palette_exclusions` are present in API.md.
- `git diff --exit-code HEAD -- src scripts Cargo.toml Cargo.lock`: exit 0 (docs only).
- Behaviour cross-checks (run for the claims about the API and palette): `cargo test -q --test phase51_api_parity` 12 passed; `--test phase51_palette_parity` 6 passed; `--test phase51_agents_list_cmd` 10 passed; `--test phase51_demo_stop` 16 passed.

## Verified against code

- Keys: `Ctrl+Alt+N` (`src/input/keymap.rs` lines 190-191, reserved through raw passthrough by `reserved_global_action`); leader `A`, `y`, `Y` (lines 33-35); list keys (`AGENTS_LIST_BINDINGS`, `src/input/agents_list_keys.rs`).
- Mouse: `plan_agents_list_mouse` in `src/input/agents_list.rs`. The list handler runs before the traffic window, and an open menu-bar dropdown outranks both (`src/input/mouse.rs` lines 604-614).
- Menu labels: `src/ui/modals.rs` lines 396 (Window > Tile Windows & Panels) and 406-411 (View entries).
- Tiling: `dock_layout` strip rules (`src/wm/panel_dock.rs`, right strip `clamp(w/3, 28, 48)` at width >= 100, bottom `max(6, h/3)`, floor 20x6); arrange patterns and the `dock_panels_on_arrange` hook (`src/wm/layout_profile.rs`). `:layout tile|stack|cycle` and `Alt+M` take `LayoutMode` / `LayoutCycle` and never reach the dock hook.
- Demo: start splits to three panes and re-runs replace prior events; stop closes only `spawned` panes and purges `[demo]` events and demo topology pairs (`src/wm/agents_demo.rs`). No write gate in `src/api/rpc_agents.rs`.
- API: `agents.list` op set, token rules (`[A-Za-z0-9+\-_.#%:]`, 32 chars, 8 tokens), `invalid agents.list parameter` (-32000), result fields (`agents_list`, scalars and ids only); `agents.demo` `stop` boolean; `bilterm_agents_list` (`op`, `args` string) and `bilterm_agents_demo` (`stop`) in `tools/list`; 13 tool definitions in `src/api/mcp.rs`.
- Palette: `Ctrl+P`, `Alt+Space`, leader `Space` (`src/input/keymap.rs` lines 69, 155-156); `PALETTE_EXCLUSIONS` with the closed reason vocabulary and `palette_exclusions` in `bilterm://commands` (`src/command_registry.rs`, `src/api/mcp_resources.rs`); Enter on a fill-in entry runs the bare verb (`palette_enter_fill_in_entries_unchanged`).
- Drawer verbs: `minimize` / `maximize` (size 0 / max), `pin` / `unpin` (fixed / on-demand), `mode float|push`, `edge`, `style classic|navigator`, `rows 1|2` (`src/colon_command/parse_visual.rs`).

## Deviations from Plan

1. **MCP tool count.** Task 2 text says "the new tool count (12)". The repo has 13 tools (`tools/list`), and the docs state 13 (the count set by plan 51-10 and the user brief).
2. **Theme-preset exclusions.** Contract section 7 and the 51-04 SUMMARY say 13 `:window theme <preset>` exclusions. `src/command_registry.rs` has 14 `reachable-via-picker` rows (all 14 presets). The docs say 14.
3. **Contract exception 6 is stale.** The contract says `:agents list status` is data-only. Since 51-01 and 51-08, `AgentsOp::List` also opens the window, so bare `:agents`, `:agents ls`, `:agents status` and `:agents list` all open it. Chapter 11 describes the shipped behaviour.
4. **`palette_exclusions` on `:commands`.** The plan says the field is visible in `:commands` data and `bilterm://commands`. Only `bilterm://commands` was verified in code, so the docs name only that resource.
5. **Arrange no-room notice.** 51-09 deviation 2 still holds at HEAD: `:arrange` overwrites the no-room status with its own message. The docs state that after `:arrange` the status shows the arrange result. `:agents list dock` does report `No room to dock the agents list`.
6. **Absolute list scale.** `:agents list scale <75..200>` returns no `data` (it runs the generic panel-scale path), but `agents.list` still carries the top-level `agents_list` object. Documented in API.md as observed.
7. **Developer guide MCP count.** The source map ("11 agent tools") and section 6b ("11 tools") also said 11. Both now say 13. These lines are not in the plan's must-have list but are in the same file.
8. **`docs/user-guide/index.md` is not modified.** It already links chapter 11; the gate passes without an edit.
9. **Ctrl+Alt+Shift+N.** The contract lists it as an alias; the docs document only `Ctrl+Alt+N`.

## AGENTS.md follow-ups (not edited, per plan)

- AGENTS.md MCP line (currently "11 tools", around line 202): update to 13, adding `bilterm_agents_list` and `bilterm_agents_demo`.
- AGENTS.md file map: add the 11 Phase 51 files listed in the developer guide, section 2.
- Follow-ups from the plans: palette Enter-prefill for fill-in entries (`src/input/modal.rs`, needs sign-off, D-02); a key for `:arrange <pattern>` (needs a const Action, exception 4).

## Known Stubs

None. Docs only.

## Threat Flags

None. Docs only. T-51-29 is mitigated: each security-relevant statement was checked against code. `agents.demo` `stop` is stated as not gated (no gate in `rpc_agents.rs`); the list payload is stated as scalars and ids only (`summary_json`, `agents_list` object); `palette_exclusions` is stated with its closed vocabulary; topology export stays refused over the API.

## Self-Check: PASSED

- `docs/user-guide/11-navigator-sidebar-and-agent-traffic.md`, `docs/user-guide/06-keyboard-mouse-and-command-reference.md`, `docs/API.md`, `docs/DEVELOPER_AND_AGENT_GUIDE.md`: modified and committed.
- Commits `ebdd226` and `6996165` are on the branch after the reset to `feat/navigator-rows-and-launch` (`f1f548c`).
