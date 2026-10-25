# Architecture Refactoring Log (`refactor-execute`)

Tracks incremental, behavior-preserving architectural refactorings and security hardening items executed from [`docs/ARCHITECTURE_REVIEW.md`](ARCHITECTURE_REVIEW.md).

---

### [2026-10-25] Refactoring Batch: Phase P0 & Phase P1 (`v1.10.0` Post-Review Hardening)

- **Tasks Completed**:
  1. **P0.1 — Confine `:canvas export <path>` under `SnapshotPolicy::Confined` & `secure_fs::write_private_file`**:
     - **Target Files**: [`src/ui/hires_subcell.rs`](../src/ui/hires_subcell.rs), [`src/api/rpc_handlers.rs`](../src/api/rpc_handlers.rs), [`src/api/rpc.rs`](../src/api/rpc.rs), [`src/api/playbook.rs`](../src/api/playbook.rs)
     - **Changes**: Added `CompositePane::export_canvas_confined` and `AutomationControlPlane::execute_canvas_command_confined`. All RPC (`canvas.command`, `wm.command`) and confined Playbook `:canvas export` invocations now validate filenames via `validate_snapshot_name`, confine writes to `snapshot_dir`, reject symlinks, and write atomically via `secure_fs::write_private_file` (`0600` on Unix).
  2. **P0.2 — Wire `CanvasResolution` into Live TUI Rendering, Dynamic Export Dimensions & Cached `TIOCGWINSZ` Probing**:
     - **Target Files**: [`src/ui/hires_subcell.rs`](../src/ui/hires_subcell.rs), [`src/ui/canvas_raster.rs`](../src/ui/canvas_raster.rs), [`src/app.rs`](../src/app.rs)
     - **Changes**: Cached `probe_terminal_pixel_size()` in `CACHED_TERMINAL_PIXELS: AtomicU64` with `refresh_terminal_pixel_size()` triggered on `Event::Resize` in `App::handle_crossterm_event`. Updated `render_graphical_canvas` to query `scene.resolution.resolve_px_per_cell(canvas.width, canvas.height)` instead of hardcoded `(4, 8)`, and updated `:canvas export` / `:canvas encode` to use `self.vt.size()` instead of hardcoded `(80, 24)`.
  3. **P1.1 — Eliminate `ui <-> api` Dependency Cycle via `src/api/rpc_handlers.rs`**:
     - **Target Files**: [`src/api/rpc_handlers.rs`](../src/api/rpc_handlers.rs), [`src/ui/canvas_raster.rs`](../src/ui/canvas_raster.rs), [`src/ui/typography.rs`](../src/ui/typography.rs)
     - **Changes**: Extracted `handle_rpc_canvas` and `handle_rpc_fontsize` out of `src/ui/` into `src/api/rpc_handlers.rs`. `src/ui/` now has **zero** imports from `crate::api`.
  4. **P1.2 & P2.3 — Decompress Near-Ceiling Modules & Surface Split Errors**:
     - **Target Files**: [`src/colon_command/execute.rs`](../src/colon_command/execute.rs) (`789` $\rightarrow$ `299` LOC), [`src/colon_command/execute_wm.rs`](../src/colon_command/execute_wm.rs) (`495` LOC), [`src/pane/mod.rs`](../src/pane/mod.rs) (`788` $\rightarrow$ `677` LOC + [`src/pane/tests.rs`](../src/pane/tests.rs)), [`src/wm/z_stack.rs`](../src/wm/z_stack.rs) (`788` $\rightarrow$ `611` LOC + [`src/wm/tests.rs`](../src/wm/tests.rs)), [`src/ui/canvas_raster.rs`](../src/ui/canvas_raster.rs) (`789` $\rightarrow$ `745` LOC), [`src/ui/typography.rs`](../src/ui/typography.rs) (`661` $\rightarrow$ `580` LOC).
     - **Changes**: Split workspace colon execution into `execute_wm.rs`, moved inline test suites to dedicated `tests.rs` modules, and surfaced `wm.split_focused(...)` errors into the active pane status bar.
  5. **Supply-Chain Hygiene — Prune Redundant `vt100-ctt` Default Features**:
     - **Target Files**: [`Cargo.toml`](../Cargo.toml), [`Cargo.lock`](../Cargo.lock)
     - **Changes**: Set `vt100-ctt = { version = "0.17.1", default-features = false }`, removing 13 unused transitive crates (`tui-term`, duplicate `ratatui 0.29.0`, unmaintained `paste 1.0.15` `RUSTSEC-2024-0436`, and `lru 0.12.5` `RUSTSEC-2026-0002`).
- **Verification**:
  - `cargo clippy --all-features --all-targets -- -D warnings`: ✅ `0` warnings, `0` errors
  - `cargo test`: ✅ `234` passed, `0` failed (including `rpc_canvas_export_enforces_snapshot_dir_confinement_and_dynamic_resolution`)
- **Status**: ✅ Complete
