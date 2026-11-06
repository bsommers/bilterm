# Phase 51 follow-ups (recorded by 51-13, 2026-11-05)

None of these block Phase 51 or 50.1 completion. Each is a candidate quick task or belongs to the named phase.

## Behaviour

1. **Docked traffic window has the same effective-area problem the AGENTS list had.** `agents_workspace` (`src/wm/agents_geometry.rs:231`) does not account for the dock strip for the traffic window the way the list's geometry does, and `move_agents_window` (`:258`) computes the target rect before it releases the dock, so a move of a docked traffic window is clamped against the wrong area. Fix: release the dock first (or compute the rect from the pre-dock workspace), then add a test next to `panel_dock` drift tests.
2. **`:arrange` overwrites the no-room dock notice.** The `:arrange` / `:layout <pattern>` executor in `src/wm/layout_profile.rs` (`Auto-arranged {count} window(s) ...`, lines ~875 and ~896) replaces the pane status that `dock_panels_on_arrange` sets (`Not enough room to dock panels`). Flags stay off and panels float, but the user does not see why. `:agents list dock` reports it correctly. Fix: join the two messages or let the dock status win (see 51-09 deviation 2, 51-11 deviation 5).
3. **`:ssh-tmux` pane is labelled with the remote host though it is a local shell** (G2 in user-guide chapter 12; belongs to Phase 51.1). `capture_context` in `src/pane/capture.rs` keys on `remote_target`, so every command typed there is recorded with the remote host and transport `ssh-tmux`. Phase 51.1 starts a real `ssh -tt` + `tmux -CC` transport; label from the transport actually in use.
4. **Palette Enter prefill for fill-in entries** (`src/input/modal.rs`) needs separate sign-off (decision D-02); `palette_enter_fill_in_entries_unchanged` pins today's behaviour.
5. **`:arrange <pattern>` has no key chord** (contract exception 4). A const `Arrange` `Action` would be needed first.
6. **Optional:** a data-only `:agents status` split (bare `:agents`, `ls`, `list` and `status` all open the window since 51-01/51-08; the stale contract exception 6 text says otherwise).

## Size (soft goal 800-1000 lines, ARCH-R04; hard backstop 1200)

7. `src/wm/drawer.rs` is 1071 lines and `src/wm/agents_window.rs` is 1177 lines. Both are over the soft goal and agents_window.rs is within 23 lines of the backstop. Split before the next feature touches either (candidates: drawer geometry/drag, agents_window mouse and tick helpers).
8. `tests/phase51_uniformity.rs` is 1317 lines (test file; the backstop applies to `src/**` and `session_lessons_and_regression_suite.rs`). Split the matrix data from the assertions if it grows.

## Test hermeticity

9. Several tests read the developer's real `XDG_STATE_HOME` (saved sessions) and fail when run outside `scripts/verify.sh`, which isolates it: `drawer_behaviour_coverage` (6 tests), `slide_out_drawer_and_telemetry` (1), `phase50_1_capture`, `phase50_1_find_ops`, `phase50_1_registry`, and a `rect_mode` test. They fail identically at the Phase 50 base and at HEAD. Make each set a private temp `XDG_STATE_HOME` (and `HOME`) itself so a bare `cargo test --test <name>` is hermetic.

## Docs

10. `AGENTS.md` and `CLAUDE.md` still say "48-file" source map; the tree has 173 `src/**/*.rs` files. The Phase 50.1 and Phase 51 files are now listed in `AGENTS.md`; rewording the count is left to the next docs pass.
11. `docs/architecture.md:87` still says the tmux bridge spawns `ssh -tt ... tmux -CC` (stale until 51.1; 50.1-14 deviation 4).
