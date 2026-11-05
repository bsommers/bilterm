# Phase 51 Context: UI Tweaks & Command Parity

Source: user request 2026-11-05 after GUI re-test of Phase 50.

## Requests (verbatim intent)
- Add `:agents demo stop`.
- Sweep all command verbs; every verb needs a command-palette option that triggers its functionality (missing example: `:drawer style navigator`).
- `:agents list` pops up a floating window, pinnable like the drawer and MCP console, listing only agents; key and mouse controls plus command verbs; it tiles when the window-tiling function is invoked. A window-tiling function already exists.
- Do GSD research and planning before executing.

## Standing process decision
Everything is to run as autonomously as possible, dispatching subagents with maximum parallelism (disjoint file ownership per plan; orchestrator-only `Action`/`apply` wiring; explicit `model` on every subagent; planning agents use sonnet/haiku).

## Known facts
- `AgentsOp::Close` and `Toggle` exist; `:agents list` currently only sets a status message.
- `:agents demo` splits to 3 panes and backdates 13 `[demo]` Message events; a stop needs to remember the pane ids and filter the ring.
- Palette lives in `src/pane/completion.rs` / command palette (Phase 12/47); help catalog in `src/ui/help_catalog.rs`.

## Planning defaults (chosen by orchestrator to keep autonomy; user may override at plan review)
- D-01 demo stop: re-running `:agents demo` REPLACES prior demo events; stop is allowed over RPC/MCP only if other `:agents` ops are (no new gate); demo events recorded unredacted (record_traffic_with redact=false) so prefix match works; purge topology pairs too.
- D-02 palette: add explicit entries for fixed sub-verb sets (incl. `drawer style navigator|classic`, `drawer rows 1|2`); exclusion table `PALETTE_EXCLUSIONS` with reasons; skip the 13 `:window theme <preset>` entries (picker covers them). Do NOT change modal.rs Enter-prefill behavior (needs separate sign-off; list as follow-up).
- D-03 agents list: separate `Action::AgentsList(AgentsListOp)` + `PanelTarget::AgentsList`; keep `AgentsOp::List` (also opens the window); default rows = all tracked panes; global toggle chord chosen by planner after verifying it is free (candidate Ctrl+Alt+N).
- D-04 tiling: only the `:arrange`/`:layout grid|columns|...` family docks open panels (agents list + agents traffic into a strip; MCP via existing Push); `:layout tile/cycle/Alt+M` unchanged; drawer persisted mode untouched; no panels open => output byte-identical.
- Research: 51-RESEARCH-A-palette.md, -B-agents-list-tiling.md, -C-demo-stop.md (read them; B proposes Wave 0 orchestrator seam + 5 parallel plans B1-B5; A proposes probe-based parity test).
- File limits: agents_window.rs, drawer.rs, layout_profile.rs near 1200 backstop; new logic in new files.
