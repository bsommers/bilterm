# Plan 23-02 Summary: Unwired-Subsystem Resolution, `ADR-007` & Feature Matrix (`ARCH-R05`)

## Accomplishments
- Wired `TmuxControlSubprocess` into `WindowManager` (`attach_tmux_control`, `is_tmux_control_attached`, `poll_tmux_control`) and called `poll_tmux_control` inside `WindowManager::poll_all_ptys` so `App` pumps live `tmux -CC` control lines on every tick.
- Implemented `TerminalBackend` for `GhosttyVtEngine` under `--features ghostty-vt` and documented `VtWorkerHandle` and `ghostty-vt` as experimental opt-in subsystems.
- Added `rust-toolchain.toml` (`channel = "1.90"`), updated `flake.nix` (`cargo-hack` + `zig_0_15`), and wrote `docs/architecture/adr/ADR-007-terminal-backend-trait-and-subsystem-resolution.md`.

## Verification
- Feature matrix (`--no-default-features`, `--features gui`, `--features ghostty-vt`, `--all-features`) compiles cleanly.
- `ghostty_vt_tests::ghostty_engine_snapshot_reflects_text_and_sgr_color` passes under `--features ghostty-vt`.
- `cargo clippy --all-features --all-targets -- -D warnings` passes with 0 warnings.
