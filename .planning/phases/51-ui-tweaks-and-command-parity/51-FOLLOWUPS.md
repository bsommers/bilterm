# Phase 51 Follow-ups (Recorded 2026-11-05, All Resolved 2026-11-07)

All 11 follow-up items identified during Phase 51 have been fully resolved and verified.

## Behaviour

1. **[x] RESOLVED (2026-11-07): Docked traffic window has the same effective-area problem the AGENTS list had.**
   - *Fix location:* `src/wm/agents_geometry.rs:231` accounts for the dock strip when computing the traffic window workspace; `move_agents_window` releases dock attachment before clamping against target workspace bounds. Tested in `tests/panel_dock.rs`.
2. **[x] RESOLVED (2026-11-07): `:arrange` overwrites the no-room dock notice.**
   - *Fix location:* `src/wm/layout_profile.rs` joins the dock notice message (`Not enough room to dock panels`) with the layout arrangement summary instead of discarding it, so the user receives both messages in the status notification.
3. **[x] RESOLVED (2026-11-07): `:ssh-tmux` pane is labelled with the remote host though it is a local shell.**
   - *Fix location:* `src/pane/capture.rs` keys off the active runtime transport rather than unverified remote configuration, correctly attributing local vs. remote SSH/tmux sessions.
4. **[x] RESOLVED (2026-11-07): Palette Enter prefill for fill-in entries.**
   - *Fix location:* `src/input/modal.rs` and `src/input/modal_palette.rs` handle fill-in entries by populating the prompt with the command prefix upon Enter, per decision D-02.
5. **[x] RESOLVED (2026-11-07): `:arrange <pattern>` has no key chord.**
   - *Fix location:* `src/action.rs` and `src/input/keymap_tables.rs` added `Action::Arrange(Option<LayoutPattern>)` with dedicated key chords (`Alt+M` cycle, `Alt+Shift+M` menu) and exact action lowerings.
6. **[x] RESOLVED (2026-11-07): Data-only `:agents status` split.**
   - *Fix location:* `src/colon_command/execute_wm.rs` splits data-only status reporting (`:agents status` / `bilterm ctl agent.list`) from GUI/TUI window presentation (`:agents`, `:agents list`).

## Size (soft goal 800-1000 lines, ARCH-R04; hard backstop 1200)

7. **[x] RESOLVED (2026-11-07): `src/wm/drawer.rs` and `src/wm/agents_window.rs` line ceilings.**
   - *Fix location:* Split `src/wm/drawer.rs` into `src/wm/drawer.rs` + `src/wm/drawer/ops.rs` and `src/wm/agents_window.rs` into `src/wm/agents_window.rs` + `src/wm/agents_window_ops.rs`. Both modules are now comfortably under the 1000-line ceiling.
8. **[x] RESOLVED (2026-11-07): `tests/phase51_uniformity.rs` line ceiling.**
   - *Fix location:* Split matrix definitions and verification assertions into modular helper submodules under `tests/phase51_uniformity_support/`, bringing `tests/phase51_uniformity.rs` well under the limit.

## Test Hermeticity

9. **[x] RESOLVED (2026-11-07): Several tests read developer's real `XDG_STATE_HOME`.**
   - *Fix location:* Updated `tests/drawer_behaviour_coverage.rs`, `tests/slide_out_drawer_and_telemetry.rs`, `tests/phase50_1_capture.rs`, `tests/phase50_1_find_ops.rs`, and `tests/phase50_1_registry.rs` to configure isolated, per-test private temporary `XDG_STATE_HOME` and `HOME` directories via RAII guards, ensuring 100% hermetic execution under bare `cargo test`.

## Docs

10. **[x] RESOLVED (2026-11-07): `AGENTS.md` and `CLAUDE.md` source map file count.**
    - *Fix location:* Updated file counts and modular component hierarchies in `AGENTS.md` and `CLAUDE.md` to accurately reflect the 170+ modular Rust sources in the repository.
11. **[x] RESOLVED (2026-11-07): `docs/architecture.md:87` tmux bridge description.**
    - *Fix location:* Updated `docs/architecture.md:87` to describe the persistent SSH+tmux dual-mode bridge and transport layers implemented in Phase 51.1.
