# Session Checkpoint — `v1.10.1` Released & Phase 28 Planned — 2026-10-25

**Status:** COMPLETE — `v1.10.1` released and pushed; ready for Phase 28 (`/gsd-execute-phase 28`) or next milestone work.

- **Updated:** `2026-10-25`
- **Branch:** `main` (synced with `origin/main`, tag `v1.10.1` @ `677b668`)
- **Latest Releases:**
  - `v1.10.1`: <https://github.com/bsommers/bilterm/releases/tag/v1.10.1> (14 cross-platform release archives + `SHA256SUMS`)
  - `v1.10.0`: <https://github.com/bsommers/bilterm/releases/tag/v1.10.0>
- **Baseline Verification:**
  - `cargo clippy --all-features --all-targets -- -D warnings` → **0 warnings, 0 errors**
  - `cargo test --quiet` → **235 passed, 0 failed** (`151` unit + `84` integration across 23 test binaries)
  - `cargo audit` → **0 CVEs, 0 unsound crates** (`247` lockfile packages, `100%` permissive OSI licenses)
  - `ARCH-R04` file size ceiling (`<= 800` LOC across all `72` files in `src/**/*.rs`) → **100% compliant** (max file is `src/input/keymap.rs` at `787` LOC)

---

## 1. User Decisions & Requests (Verbatim, This Session)

1. `"update from remote - windows bug fixes v1.9.1."`
2. `"what's next on the roadmap? i'm thinking we need also a high-resolution canvas for drawing graphs, images, plots, graphics, etc. Textronix is cool for retro vibe, we want also a modern equivalent of the goal utilizing all of the modern graphics, GPU, etc. need to be sure we can work in pure ssh sessions as well as native GUI (Linux, MacOS, Windows) modes. e.g. graphics probably won't work over an SSH session.right? or will they???"` → `"sweet! yes. LET'S DO IT!!!!"`
3. `"how to see the hires graphics?"` → `"awesome. save this as examples in the documentation and readme. do we have full /docs architecture, using, purpose, motivation, how its built, developer guides, etc?"`
4. `"- we need this : do we have full use of the highest resolutions available for graphics? - we need this : what about selectable font options per pane or window"`
5. `"should we bump the version and tag?"` → `"YES!"` (shipped `v1.10.0`)
6. `"/swe-skills:code-architecture-review"` → `"1, then, 2, then 3"` (1: Execute P0/P1 refactorings, 2: Dependency & supply-chain audit, 3: Phase 28 milestone plan)
7. `"ok. The other Claude session just finished a code review and remediation check out what was done and update everything and do a release."` (shipped `v1.10.1`)
8. `"checkpoint and compact so we can handoff"`

---

## 2. Done in This Session (With Evidence)

- **Phase 27 (`v1.10.0` — High-Resolution Pixel Canvas, Adaptive SSH/GUI Graphics & Per-Pane Typography):**
  - `src/ui/pixel_canvas.rs`: Anti-aliased RGBA framebuffer (`PixelCanvasBuffer`) with Xiaolin Wu lines/Beziers, SDF circles, high-DPI plots, radial focal lens, and `.ppm`/`.svg` export.
  - `src/ui/hires_subcell.rs`: `CanvasResolution` (`Auto` via `ioctl(TIOCGWINSZ)` `ws_xpixel × ws_ypixel`, `1x`, `2x`, `4x`, `720p`, `1080p`, `4k`, `<W>x<H>`) and `CanvasSubcellMode::Octant` (`2×4` 2-color `fg`+`bg` 24-bit TrueColor supersampling), plus `Kitty`/`Sixel`/`iTerm2` in-band encoders with `tmux` DCS passthrough (`\x1bPtmux;...\x1b\\`).
  - `src/ui/typography.rs`: Selectable per-pane/per-window font presets (`FontFamilyPreset`, `PaneFontOptions`, `:font ...`).
  - `src/main.rs`: `bilterm plot <line|bar|scatter|candlestick|heatmap|hires>` CLI subcommand.
- **Post-Review P0/P1 Hardening, Claude Cross-Review & `v1.10.1` Release (`276604c`, `677b668`):**
  - **P0.1 (`SEC-R01` Automation Confinement & Atomic Private Writes):**
    - Added `secure_fs::validate_plain_file_name`, `WindowManager::apply_automation(action, confine)`, `AutomationControlPlane::apply_with_policy(action, policy)`, and `CompositePane::execute_canvas_command_in(cmd, confine)`.
    - Confined both `:canvas export <name>` and `:record start <name>` / `:record workspace <name>` under `SnapshotPolicy::Confined` (JSON-RPC `wm.command`, `canvas.command`, MCP, and confined Playbooks) to `snapshot_dir`, rejecting `..`, `/`, `\`, NUL, and symlinks, and writing exports atomically (`0600`) via `secure_fs::write_private_file`.
    - Added `u16::try_from` bounds validation for `size` and `scale_pct` in `workspace.fontsize` (`src/api/rpc_handlers.rs`).
  - **P0.2 (Live `CanvasResolution` Wiring, Exact Export Dimensions & Cached `TIOCGWINSZ`):**
    - Wired `scene.resolution.resolve_px_per_cell(canvas.width, canvas.height)` into `render_graphical_canvas` (capped at `4..=16 × 8..=32` px/cell on-screen for 60 FPS `HalfBlock`/`Octant` rendering while preserving full uncapped resolution on `:canvas export`).
    - Cached `probe_terminal_pixel_size()` in `CACHED_TERMINAL_PIXELS: AtomicU64`, refreshed on `Event::Resize` (`refresh_terminal_pixel_size` in `src/app.rs`).
    - Updated `:canvas export` and `:canvas encode` to use exact `self.vt.size()` dimensions instead of hardcoded `(80, 24)`.
  - **P1.1 (`ui <-> api` Cycle Elimination):**
    - Extracted `handle_rpc_canvas` and `handle_rpc_fontsize` from `src/ui/` into `src/api/rpc_handlers.rs`. `src/ui/` now has **zero** imports from `crate::api`.
  - **P1.2 & P2.3 (`ARCH-R04` Module Decompression & Split Error Surfacing):**
    - Split `src/colon_command/execute.rs` (`789` → `298` LOC) + `src/colon_command/execute_wm.rs` (`505` LOC) and surfaced `wm.split_focused(...)` errors to the active pane status bar.
    - Extracted unit tests to `src/pane/tests.rs` (`src/pane/mod.rs` `788` → `677` LOC) and `src/wm/tests.rs` (`src/wm/z_stack.rs` `788` → `611` LOC).
  - **Dependency & Supply-Chain Audit (`docs/DEPENDENCY_AUDIT.md`):**
    - Set `vt100-ctt = { version = "0.17.1", default-features = false }` in `Cargo.toml`, pruning 13 unused transitive crates (duplicate `ratatui 0.29.0`, `tui-term 0.3.4`, unmaintained `paste 1.0.15` `RUSTSEC-2024-0436`, and `lru 0.12.5` `RUSTSEC-2026-0002`).

---

## 3. Bugs Fixed & Gotchas Discovered (Do Not Reintroduce)

- **Bypassing `WindowManager::apply` in RPC/Playbook:** Confinement for file-writing `:colon` commands (`:canvas export`, `:record start`) must go through `WindowManager::apply_automation(action, confine)` (`AutomationControlPlane::apply_with_policy`) rather than ad-hoc string matching in `rpc.rs` so `wm.command`, `canvas.command`, and `Playbook` share identical policy and error surfacing.
- **Off-by-one VT canvas height in `:canvas export`:** `self.vt.size()` returns `(canvas_rows, canvas_cols)` excluding the prompt row; do **not** add `.saturating_add(1)` to `canvas_rows` during export.
- **Test side-effect on SVG screenshots:** Running `cargo test` rewrites `docs/assets/screenshot-3278-inspector.svg`, `docs/assets/screenshot-3279-palette.svg`, and `docs/assets/screenshot-workspace.svg`. Always revert them before committing if unchanged:
  ```bash
  git checkout -- docs/assets/screenshot-3278-inspector.svg docs/assets/screenshot-3279-palette.svg docs/assets/screenshot-workspace.svg
  ```
- **Untracked local scratch directories:** `.flowscope/`, `extra_prompt_session_panels_etc.txt`, and `tmp/` exist in the working directory; never stage them (`git add` explicit paths only).

---

## 4. Working-Tree State

- **Branch:** `main` (clean tracked working tree; synced with `origin/main`)
- **Latest Tag:** `v1.10.1`
- **Background Processes / Stashes:** None (`0` background tasks, `0` git stashes)
- **Untracked Local Files (Ignore):** `.flowscope/`, `extra_prompt_session_panels_etc.txt`, `tmp/`

---

## 5. Next Steps (Exact Commands to Resume)

1. **Verify working tree & baseline test gates:**
   ```bash
   git status --short
   cargo clippy --all-features --all-targets -- -D warnings
   cargo test --quiet
   git checkout -- docs/assets/screenshot-3278-inspector.svg docs/assets/screenshot-3279-palette.svg docs/assets/screenshot-workspace.svg
   ```
   *(Expect `0` warnings and `235` passing tests: `151` unit + `84` integration.)*
2. **Execute Phase 28 (remaining P2 structural DAG decoupling & dirty-driven rendering) when ready:**
   - Review plan: [`.planning/phases/28-architecture-hardening-and-dag-decoupling/28-01-PLAN.md`](phases/28-architecture-hardening-and-dag-decoupling/28-01-PLAN.md) and [`docs/ARCHITECTURE_REVIEW.md`](../docs/ARCHITECTURE_REVIEW.md) (`P2.1` `pane <-> wm` / `action <-> colon_command` DAG cycle breaking, `P2.2` typed `Action` variants for `:canvas` / `:fontsize`, and `P1.3` dirty-driven rendering).
