# ADR-007: `TerminalBackend` Port Trait & Unwired-Subsystem Resolution (`ARCH-R03`, `ARCH-R05`)

- **Status**: Accepted (`Phase 23`)
- **Date**: 2026-10-24
- **Requirements Covered**: `ARCH-R03`, `ARCH-R05`

---

## Context

In the Phase 19–23 architecture review (`ARCHITECTURE_REVIEW_PLAN.md` P1.3 and P1.5), two structural issues were identified:

1. **VT Emulator Abstraction Leak (`ARCH-R03` / P1.3)**:
   `VtParser::screen() -> &vt100_ctt::Screen` was public and called across `src/ui/`, `src/pane/`, `src/api/`, `src/colon_command/`, `src/tmux/`, and `src/wm/`. In addition, `src/ui/mod.rs::cell_to_ratatui_style` accepted `&vt100_ctt::Cell`. This coupled UI rendering, Vi copy mode, and RPC text extraction directly to `vt100_ctt` internal types.
2. **Unwired / Experimental Subsystems (`ARCH-R05` / P1.5)**:
   Three subsystems (`TmuxControlSubprocess`, `VtWorkerHandle`, and `--features ghostty-vt`) needed a clear architectural resolution: either wire them into the live runtime or mark them explicitly as experimental opt-in seams with a passing feature matrix.

---

## Decisions

### 1. `TerminalBackend` Trait, `CellView` & Zero `vt100_ctt` Leakage (`ARCH-R03`)

- Defined [`pub trait TerminalBackend`](../../../src/vt/backend.rs) in `src/vt/backend.rs` as the canonical port for virtual terminal emulation (`process`, `set_size`, `size`, `cursor_position`, `is_cursor_visible`, `snapshot`, `cell_view`, `cell_snapshot`, `row_text`, `contents`, `DECSET` mode queries, Kitty keyboard flags, scrollback navigation, and query/OSC 52 draining).
- Restricted `VtParser::screen(&self)` to `#[cfg(test)] pub(in crate::vt)` so no module outside `src/vt/` can ever reference `vt100_ctt::Screen` or `vt100_ctt::Cell`.
- Added zero-allocation [`CellView<'a>`](../../../src/vt/snapshot.rs) so `render_output_canvas` in [`src/ui/mod.rs`](../../../src/ui/mod.rs) renders cells at >60 FPS without heap allocations or `vt100_ctt` coupling.
- Implemented a second self-contained backend ([`MockTerminalBackend`](../../../src/vt/backend.rs)) with zero `vt100_ctt` dependency, and verified both `VtParser` and `MockTerminalBackend` against the shared `assert_terminal_backend_conformance` suite.

### 2. Resolution of `TmuxControlSubprocess`, `VtWorkerHandle`, and `ghostty-vt` (`ARCH-R05`)

| Subsystem | Location | Resolution | Details |
|-----------|----------|------------|---------|
| **`TmuxControlSubprocess`** | [`src/tmux/mod.rs`](../../../src/tmux/mod.rs) & [`src/wm/mod.rs`](../../../src/wm/mod.rs) | **Wired into Live Runtime** | `WindowManager` now holds `tmux_control: Option<TmuxControlSubprocess>` and `tmux_bridge: TmuxControlBridge`, exposing `attach_tmux_control`, `is_tmux_control_attached`, and `poll_tmux_control`. `WindowManager::poll_all_ptys` pumps `poll_tmux_control` on every 16ms tick. |
| **`VtWorkerHandle`** | [`src/vt/worker.rs`](../../../src/vt/worker.rs) | **Marked Experimental Opt-In (`ARCH-02`)** | Explicitly documented in module rustdoc and architecture guides as an experimental background worker pipeline exercised by `tests/vt_worker_and_snapshot_seam.rs`, while production `CompositePane` instances use synchronous `PtySession` + `VtParser`. |
| **`ghostty-vt` Feature (`GhosttyVtEngine`)** | [`src/vt/ghostty_backend.rs`](../../../src/vt/ghostty_backend.rs), [`Cargo.toml`](../../../Cargo.toml), [`rust-toolchain.toml`](../../../rust-toolchain.toml), [`flake.nix`](../../../flake.nix) | **Marked Experimental Opt-In & Builds in Feature Matrix** | `GhosttyVtEngine` implements `TerminalBackend` under `#[cfg(feature = "ghostty-vt")]` against `libghostty-vt = "0.2.2"`. `rust-toolchain.toml` pins Rust `1.90` (matching `libghostty-vt`'s MSRV) and `flake.nix` provides `zig_0_15` and `cargo-hack`. |

---

## Consequences

- **Positive**:
  - `grep -rn "vt100_ctt" src/` matches **only** files inside `src/vt/` (enforced by automated test `no_vt100_ctt_types_appear_outside_src_vt`).
  - Every Cargo feature combination (`--no-default-features`, `--features gui`, `--features ghostty-vt`, `--all-features`) compiles and passes `clippy -D warnings`.
  - `TmuxControlSubprocess` streams hydrate `WindowManager` panes automatically during normal `poll_all_ptys` ticks.
