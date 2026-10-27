# Session Checkpoint — Phase 29 Complete (Smooth & Mainframe Block/Page-Mode Scroller), `docs/LESSONS_LEARNED.md` & Expanded Regression Suite — 2026-10-27

- **Status:** COMPLETE — Completed **Phase 29 (`SCRL-01`, `SCRL-02`)**, authored **[`docs/LESSONS_LEARNED.md`](../docs/LESSONS_LEARNED.md)**, and expanded the integration test suite with **[`tests/smooth_and_mainframe_page_scrolling.rs`](../tests/smooth_and_mainframe_page_scrolling.rs)** (`5/5` passing) and **[`tests/session_lessons_and_regression_suite.rs`](../tests/session_lessons_and_regression_suite.rs)** (`7/7` passing).
- **Updated:** `2026-10-27`
- **Branch:** `main`
- **Installed Artifacts (macOS `aarch64-apple-darwin`):**
  - Binary: `/Users/bill/.local/bin/bilterm`
  - macOS App Bundle (with `Bilterm.icns` Dock/Finder icon, defaults to `bilterm --gui`): `/Users/bill/Applications/Bilterm.app`
  - Cross-Platform Icons: `docs/assets/icons/bilterm.{icns,ico,svg,png}` + `bilterm-{16..1024}.png`
- **Baseline Verification:**
  - `cargo +stable clippy -q --all-targets -- -D warnings` → **0 warnings, 0 errors**
  - `cargo +stable test -q` → **All unit & 26 integration test suites passing**

---

## 1. Done in This Session (With Evidence)

1. **Mouse & Keyboard Cut/Copy/Paste + Visual Selection Highlights + Focus Switcher + Toggleable Version Badge (`d766d7e`)**:
   - Full mouse click/drag selection and keyboard selection (`Shift+Arrows`, `Ctrl+A`, `Ctrl+X`/`Cmd+X`, `Ctrl+C`/`Cmd+C`, `Ctrl+V`/`Cmd+V`, `Alt+W`, `Ctrl+W`, `Ctrl+K`, `Ctrl+U`, `Ctrl+Y`) with bright cyan selection highlight in `FloatingCommandPrompt` (`src/pane/prompt.rs`) and `OutputCanvas` `CopyMode` (`src/pane/selection.rs`).
   - Direct focus toggling between `FloatingCommandPrompt` and `OutputCanvas` via `F8`, `Ctrl+Up` / `Ctrl+Down`, `Tab`, and mouse click.
   - Toggleable version badge (`:version`) in the top menu bar, 3270 OIA status bar, and `<F1>` Help modal.
2. **Powerline (`U+E0A0..=U+E0D7`) & Nerd Font PUA Rendering (`3910635`)**:
   - Added edge-to-edge procedural scanline rasterization for all Powerline & Powerline Extra symbols (``, ``, ``, ``, ``, ``, ``, `..`, `..`) in `src/ui/gui_font/geometric.rs` and `src/ui/bitmap_font.rs`.
   - Suppressed macOS CoreText `LastResort` (`[?]`) fallback boxes in `src/ui/gui_font/macos_ct.rs`, prioritized `"SF Mono"` / `"Menlo"` over `"Courier New"` in `src/pane/typography.rs`, and added TUI host PUA sanitization in `src/app.rs`.
3. **Cross-Platform `"bil;*` / `==> term█"` Icons (`docs/assets/icons/`, `src/ui/gui_font/mod.rs`, `src/ui/gui_macos.rs`, `scripts/install.sh`)**:
   - Packaged alpha-transparent squircle icons for macOS (`bilterm.icns`), Windows (`bilterm.ico`), and Linux (`bilterm-{16..1024}.png`, `bilterm.svg`), wired runtime `NSApplication setApplicationIconImage:` for `bilterm --gui`, and bundled `Bilterm.icns` into `Bilterm.app`.
4. **Persistent In-Place Scrollable `Herdr` Drawer (`src/wm/drawer.rs`, `src/ui/drawer.rs`, `src/input/modal.rs`, `src/input/mouse_wm.rs`)**:
   - Selecting a pane or saved session via `Enter` or mouse click switches to it while keeping the drawer open and focused in place (`Esc`, `F3`, `Alt+D`, or `[×]` closes it).
   - Added `Up`/`Down`/`j`/`k`/`PageUp`/`PageDown`/`Home`/`End` and Mouse Wheel scrolling with a visual scrollbar (`▲ █ ░ ▼`), saved-session listing, and `[+New]` session creation (`n`/`+`/`v`).
5. **Milestone `v2.0` Multi-Phase Plan (`Phases 29–34` in `.planning/REQUIREMENTS.md` & `.planning/ROADMAP.md`)**:
   - **Phase 29 (`SCRL-01`, `SCRL-02`)**: Smooth Scrolling by Default & Mainframe/Timeshare Block/Page-Mode Scroller (`MORE...` / `HOLDING`).
   - **Phase 30 (`SAFE-01`, `SAFE-02`)**: Explicit Exit Guard & Root/Final Session Protection.
   - **Phase 31 (`THM-01`, `THM-02`)**: Universal Open Color Scheme & Font Engine (`mbadolato/iterm2-color-schemes` `.itermcolors` + Alacritty/WezTerm/Ghostty/Base16 TOML/YAML without lock-in).
   - **Phase 32 (`DAEM-01`, `DAEM-02`)**: Persistent Background Multiplexer Daemon (`Herdr`-style multi-client attach/detach/reconnect).
   - **Phase 33 (`TREE-01`, `TREE-02`, `PERS-01`, `PERS-02`)**: Independent, Rooted & Tree/Subtree Sessions + Crash-Safe Layout & State Persistence.
   - **Phase 34 (`MCP-01`, `MCP-02`)**: TRON-Inspired Master Control Program (`MCP`) Root Terminal.

---

## 2. Bugs Fixed & Gotchas Discovered (Do Not Reintroduce)

- **`write_private_file` must use atomic sibling temp-file + `fs::rename`:** Never `remove_file(path)` + `create_private_file(path)` in-place on shared runtime paths (`shell-integration-<pid>`), or concurrent threads/panes will observe `ENOENT`.
- **`run_headless_server` Unix socket probe:** Only treat `ErrorKind::ConnectionRefused` or `ErrorKind::NotFound` as a stale socket; never unlink an existing socket on `PermissionDenied`.
- **`rust-toolchain.toml` vs local `stable` toolchain on macOS:** `rust-toolchain.toml` pins `1.90`, while local `rustup` has `stable-aarch64-apple-darwin` installed. Pass `+stable` (`cargo +stable test`, `cargo +stable clippy`, `cargo +stable build --release`) when running offline or without downloading `1.90`.
- **Test side-effect on SVG screenshots:** Running `cargo test` rewrites `docs/assets/screenshot-3278-inspector.svg`, `docs/assets/screenshot-3279-palette.svg`, and `docs/assets/screenshot-workspace.svg`. Revert them if unchanged before committing:
  ```bash
  git checkout -- docs/assets/screenshot-3278-inspector.svg docs/assets/screenshot-3279-palette.svg docs/assets/screenshot-workspace.svg
  ```

---

## 3. Next Steps (Exact Commands to Resume)

1. **Try Bilterm in Terminal TUI and GUI modes:**
   ```bash
   bilterm
   bilterm --gui
   open ~/Applications/Bilterm.app
   ```
2. **Verify working tree & baseline test gates:**
   ```bash
   git status -s
   cargo +stable clippy --all-targets -- -D warnings
   cargo +stable test --quiet
   ```
   *(Expect `0` warnings and `248` passing tests: `157` unit + `91` integration.)*
