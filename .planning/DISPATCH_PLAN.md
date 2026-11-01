# UI Modernization Dispatch Plan (Phases 45-48)

Ownership is strict. Claude does not run agy-owned work; agy tasks are not run as Claude subagents unless the user overrides.
Rules: full task text per dispatch, one writer per file, one git worktree per agy task, agy NEVER commits, Claude reviews (spec then quality), verifies, merges, commits.
Verify (every task): `cargo clippy -q --all-targets -- -D warnings && cargo test -q`

## Claude (this session), serial
| Step | Work | Files |
|---|---|---|
| 45-01 | Token/material API, Obsidian profile | `src/ui/design/**` (new) |
| 45-02 | `[appearance]` config + migration, back-compat with snazzy/glass/modern/liquid flags | `src/config/mod.rs` |
| G2 | API FROZEN: commit + tag `ui-api-frozen` | |
| 45-04 | Bundled mono font path in GUI renderer | `src/ui/renderer/**` font code |
| 46 | Shell, top bar, pills, display modes (incl. Zen), pane chrome | `wm/mod.rs`, `ui/mod.rs`, `ui/chrome.rs`, `wm/chrome_style.rs` |
| 47-wire | `Action` + `apply` wiring for palette/appearance | `action.rs`, `colon_command/**` |
| 47-03b | Appearance modal logic | with agy 47-03 |
| 48-rev | Review perf/a11y results, final polish | |
| all | Reviews, verify, commits | |

## agy (user runs in another window), starts only after G2
Template:
```
cd /home/bill/src/ai/bilterm && git worktree add ../bilterm-<t> -b ui/<t> ui-api-frozen
cd ../bilterm-<t> && agy -p "<task text>" --mode accept-edits
```
| Task | Branch | Sole-owned file | Task text essentials |
|---|---|---|---|
| 45-03a | ui/oia | `src/ui/oia.rs` | Replace `Color::Rgb` literals with design tokens; no behavior change; snapshots unchanged |
| 45-03b | ui/status | `src/ui/status_bar.rs` | same |
| 45-03c | ui/modals-tok | `src/ui/modals.rs` | same (colors only; layout is 47-02) |
| 45-03d | ui/cursor | `src/ui/cursor_cmd.rs` | same |
| 45-03e | ui/theme-import | `src/ui/theme_import.rs` | same |
| 47-01 | ui/drawers | `ui/drawer.rs`, `ui/mcp_ui.rs`, `ui/exit_guard_ui.rs` | Restyle to materials; fix drawer stair-step artifact |
| 47-02 | ui/palette | `ui/modals.rs` (after 45-03c merged), palette file | Floating-panel material, palette/menu restyle |
| 47-03 | ui/appearance | `src/ui/appearance/**` (new) | Appearance settings modal view (transparency, borders, density, spacing); wiring by Claude |
| 48-a..c | tests/* | `tests/**` only | Snapshot per style x scene; perf budget; accessibility/contrast |
Merge order: 45-03* (any order) -> 47-01 -> 47-02 -> 47-03 -> 48.

## Next action
Claude starts 45-01 now. Nothing for agy until G2.
