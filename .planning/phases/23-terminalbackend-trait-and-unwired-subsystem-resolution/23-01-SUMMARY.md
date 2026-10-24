# Plan 23-01 Summary: `TerminalBackend` Trait, `MockTerminalBackend`, Conformance Suite & Zero `vt100_ctt` Leak (`ARCH-R03`)

## Accomplishments
- Created `src/vt/backend.rs` defining `pub trait TerminalBackend`, `impl TerminalBackend for VtParser`, `pub struct MockTerminalBackend`, and the shared conformance test suite `assert_terminal_backend_conformance`.
- Added zero-allocation `CellView<'a>` and `GridSnapshot::contents()` in `src/vt/snapshot.rs` so `render_output_canvas` in `src/ui/mod.rs` renders at >60 FPS without heap-allocating per cell or touching `vt100_ctt`.
- Restricted `VtParser::screen()` to `#[cfg(test)] pub(in crate::vt)` and updated all callers in `src/ui/`, `src/pane/`, `src/api/`, `src/colon_command/`, `src/tmux/`, `src/wm/`, and `tests/` to use `TerminalBackend` / `GridSnapshot` / `CellSnapshot` / `CellView`.

## Verification
- Zero `vt100_ctt` occurrences outside `src/vt/`.
- All 148 unit tests and 16 integration test suites pass.
- `cargo clippy --all-targets -- -D warnings` passes with 0 warnings; all `src/**/*.rs` files are `<= 800` lines.
