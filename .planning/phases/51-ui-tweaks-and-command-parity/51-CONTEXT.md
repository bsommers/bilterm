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
