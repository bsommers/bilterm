# Architecture Refactoring Log (`refactor-execute`)

Tracks incremental, behavior-preserving architectural refactorings and security hardening items executed from [`docs/ARCHITECTURE_REVIEW.md`](ARCHITECTURE_REVIEW.md).

---

### [2026-10-25] Refactoring Batch: Phase P0 & Phase P1 (`v1.10.0` Post-Review Hardening)

- **Tasks Completed**:
  1. **P0.1 — Confine `:canvas export <path>` and `:record start <path>` under `SnapshotPolicy::Confined` & `secure_fs::write_private_file`**:
     - **Target Files**: [`src/secure_fs.rs`](../src/secure_fs.rs), [`src/wm/mod.rs`](../src/wm/mod.rs), [`src/colon_command/execute_wm.rs`](../src/colon_command/execute_wm.rs), [`src/ui/hires_subcell.rs`](../src/ui/hires_subcell.rs), [`src/ui/canvas_raster.rs`](../src/ui/canvas_raster.rs), [`src/api/rpc_handlers.rs`](../src/api/rpc_handlers.rs), [`src/api/rpc.rs`](../src/api/rpc.rs), [`src/api/playbook.rs`](../src/api/playbook.rs)
     - **Changes**: Added `secure_fs::validate_plain_file_name`, `WindowManager::apply_automation(action, confine)`, `AutomationControlPlane::apply_with_policy(action, policy)`, and `CompositePane::execute_canvas_command_in(cmd, confine)`. All RPC (`canvas.command`, `wm.command`) and confined Playbook `:canvas export` and `:record start <path>` invocations now flow through `WindowManager::apply_automation`, validate plain filenames, confine writes strictly to `snapshot_dir`, surface command errors directly to the caller, reject symlinks, and write atomically via `secure_fs::write_private_file` (`0600` on Unix).
  2. **P0.2 — Wire `CanvasResolution` into Live TUI Rendering, Dynamic Export Dimensions & Cached `TIOCGWINSZ` Probing**:
     - **Target Files**: [`src/ui/hires_subcell.rs`](../src/ui/hires_subcell.rs), [`src/ui/canvas_raster.rs`](../src/ui/canvas_raster.rs), [`src/app.rs`](../src/app.rs)
     - **Changes**: Cached `probe_terminal_pixel_size()` in `CACHED_TERMINAL_PIXELS: AtomicU64` with `refresh_terminal_pixel_size()` triggered on `Event::Resize` in `App::handle_crossterm_event`. Updated `render_graphical_canvas` to query `scene.resolution.resolve_px_per_cell(canvas.width, canvas.height)` (capped at `4..=16 x 8..=32` on-screen while preserving full uncapped resolution on `:canvas export`), and updated `:canvas export` / `:canvas encode` to use exact `self.vt.size()` canvas dimensions instead of hardcoded `(80, 24)`.
  3. **P1.1 — Eliminate `ui <-> api` Dependency Cycle via `src/api/rpc_handlers.rs`**:
     - **Target Files**: [`src/api/rpc_handlers.rs`](../src/api/rpc_handlers.rs), [`src/ui/canvas_raster.rs`](../src/ui/canvas_raster.rs), [`src/ui/typography.rs`](../src/ui/typography.rs)
     - **Changes**: Extracted `handle_rpc_canvas` and `handle_rpc_fontsize` (with checked `u16::try_from` bounds on `size` and `scale_pct`) out of `src/ui/` into `src/api/rpc_handlers.rs`. `src/ui/` now has **zero** imports from `crate::api`.
  4. **P1.2 & P2.3 — Decompress Near-Ceiling Modules & Surface Split Errors**:
     - **Target Files**: [`src/colon_command/execute.rs`](../src/colon_command/execute.rs) (`789` $\rightarrow$ `298` LOC), [`src/colon_command/execute_wm.rs`](../src/colon_command/execute_wm.rs) (`505` LOC), [`src/pane/mod.rs`](../src/pane/mod.rs) (`788` $\rightarrow$ `677` LOC + [`src/pane/tests.rs`](../src/pane/tests.rs)), [`src/wm/z_stack.rs`](../src/wm/z_stack.rs) (`788` $\rightarrow$ `611` LOC + [`src/wm/tests.rs`](../src/wm/tests.rs)), [`src/ui/canvas_raster.rs`](../src/ui/canvas_raster.rs) (`789` $\rightarrow$ `755` LOC), [`src/ui/typography.rs`](../src/ui/typography.rs) (`661` $\rightarrow$ `580` LOC).
     - **Changes**: Split workspace colon execution into `execute_wm.rs`, moved inline test suites to dedicated `tests.rs` modules, and surfaced `wm.split_focused(...)` errors into the active pane status bar.
  5. **Supply-Chain Hygiene — Prune Redundant `vt100-ctt` Default Features**:
     - **Target Files**: [`Cargo.toml`](../Cargo.toml), [`Cargo.lock`](../Cargo.lock)
     - **Changes**: Set `vt100-ctt = { version = "0.17.1", default-features = false }`, removing 13 unused transitive crates (`tui-term`, duplicate `ratatui 0.29.0`, unmaintained `paste 1.0.15` `RUSTSEC-2024-0436`, and `lru 0.12.5` `RUSTSEC-2026-0002`).
- **Verification**:
  - `cargo clippy --all-features --all-targets -- -D warnings`: ✅ `0` warnings, `0` errors
  - `cargo test`: ✅ `235` passed, `0` failed (including `rpc_canvas_export_enforces_snapshot_dir_confinement_and_dynamic_resolution` and `rpc_confinement_covers_record_start_and_export_dimensions_match_canvas`)
- **Status**: ✅ Complete

---

### [2026-10-25] Refactoring Batch: Phase 28 Waves 2 & 3 (Strict `pty -> vt -> pane` DAG, Single-Pass VT Scanner & Dirty-Driven Redraw)

- **Tasks Completed**:
  1. **P1.1 (Part B) — Strict `error/secure_fs -> pty -> vt -> pane` Acyclic Layer DAG**:
     - **Target Files**: [`src/pty/line_discipline.rs`](../src/pty/line_discipline.rs), [`src/vt/snapshot.rs`](../src/vt/snapshot.rs), [`src/vt/mod.rs`](../src/vt/mod.rs), [`src/vt/backend.rs`](../src/vt/backend.rs), [`src/vt/ghostty_backend.rs`](../src/vt/ghostty_backend.rs), [`src/vt/tests.rs`](../src/vt/tests.rs), [`src/pane/typography.rs`](../src/pane/typography.rs), [`src/pane/mod.rs`](../src/pane/mod.rs), [`src/pane/canvas_scene.rs`](../src/pane/canvas_scene.rs), [`src/ui/typography.rs`](../src/ui/typography.rs), [`src/ui/hires_subcell.rs`](../src/ui/hires_subcell.rs)
     - **Changes**: Relocated `Osc133Phase` to `src/pty/line_discipline.rs`, `LineSizeAttr` + `scan_dec_line_size_escapes` to `src/vt/snapshot.rs`, and `FontFamilyPreset` + `PaneFontOptions` + `CanvasResolution` + `probe_terminal_pixel_size` to `src/pane/typography.rs` (with compatibility re-exports in `src/vt/` and `src/ui/`). Eliminated all upward `crate::ui`, `crate::wm`, `crate::config`, and `crate::api` imports across `src/pty/`, `src/vt/`, and `src/pane/`.
  2. **P1.2 (Part B) & P2.2 — Module Decompression & Single-Pass `ESC` Control Sequence Scanner**:
     - **Target Files**: [`src/colon_command/parse.rs`](../src/colon_command/parse.rs) (`732` $\rightarrow$ `412` LOC), [`src/colon_command/parse_visual.rs`](../src/colon_command/parse_visual.rs) (`338` LOC), [`src/vt/scan.rs`](../src/vt/scan.rs) (`776` $\rightarrow$ `352` LOC), [`src/vt/scan_osc_dcs.rs`](../src/vt/scan_osc_dcs.rs) (`378` LOC), [`src/wm/mod.rs`](../src/wm/mod.rs) (`758` LOC), [`src/wm/quick_terminal.rs`](../src/wm/quick_terminal.rs) (`127` LOC)
     - **Changes**: Extracted visual/shader/drawer/font/canvas `:colon` parsing into `src/colon_command/parse_visual.rs`, extracted OSC/DCS/APC sequence handlers into `src/vt/scan_osc_dcs.rs`, and replaced the 14-pass `text.match_indices(...)` loops in `VtParser::scan_control_sequences` with a single-pass `ESC` (`\x1b`) dispatcher (plus zero-allocation fast-return when `!bytes.contains(&0x1b)`).
  3. **P1.3 — `FloatingPromptState` Buffer & Cursor Encapsulation**:
     - **Target Files**: [`src/pane/prompt_editor.rs`](../src/pane/prompt_editor.rs), [`src/ui/status_bar.rs`](../src/ui/status_bar.rs), [`tests/composite_pane_snapshots.rs`](../tests/composite_pane_snapshots.rs)
     - **Changes**: Restricted `FloatingPromptState` internal buffer/cursor fields (`input`, `lines`, `cursor_row`, `cursor_col`, `cursor_char_idx`, `scroll_offset`, `h_scroll_offset`, `history_index`) to `pub(crate)` and exposed read-only accessors (`lines()`, `cursor_row()`, `cursor_col()`, `cursor_char_idx()`, `scroll_offset()`, `h_scroll_offset()`, `history_index()`).
  4. **P2.1 & P3.2 — Dirty-Driven Redraw, Cached `PixelCanvasBuffer` Rasterization & Automated Layer DAG Gate**:
     - **Target Files**: [`src/app.rs`](../src/app.rs), [`src/pane/mod.rs`](../src/pane/mod.rs), [`src/wm/mod.rs`](../src/wm/mod.rs), [`src/ui/canvas_raster.rs`](../src/ui/canvas_raster.rs), [`tests/vt_worker_and_snapshot_seam.rs`](../tests/vt_worker_and_snapshot_seam.rs)
     - **Changes**: Added `App::needs_redraw` gated on Crossterm input events and `WindowManager::poll_all_ptys(&mut self) -> bool`, added thread-local `RASTER_CACHE` (`RasterCacheKey -> Arc<PixelCanvasBuffer>`) in `render_graphical_canvas`, and enforced layer DAG acyclicity in `tests/vt_worker_and_snapshot_seam.rs`.
- **Verification**:
  - `cargo clippy --all-features --all-targets -- -D warnings`: ✅ `0` warnings, `0` errors
  - `cargo test`: ✅ `235` passed, `0` failed
- **Status**: ✅ Complete
