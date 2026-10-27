# Session Checkpoint — macOS Native CoreText/Cocoa GUI (`--gui` / `Bilterm.app`), 250-Test Suite & Hardening — 2026-10-27

- **Status:** COMPLETE — Implemented native macOS Cocoa / CoreText / CoreGraphics Retina GUI (`bilterm --gui` and `Bilterm.app`), installed on macOS (`~/.local/bin/bilterm` + `~/Applications/Bilterm.app`), verified all `250/250` tests (`159` unit + `91` integration), and zero-warning `clippy`.
- **Updated:** `2026-10-27`
- **Branch:** `main`
- **Installed Artifacts (macOS `aarch64-apple-darwin`):**
  - Binary: `/Users/bill/.local/bin/bilterm` (`v1.11.1`)
  - macOS App Bundle (Spotlight / Launchpad / Finder, defaults to `bilterm --gui`): `/Users/bill/Applications/Bilterm.app`
  - Manpage: `/Users/bill/.local/share/man/man1/bilterm.1`
  - Shell Completions: `/Users/bill/.local/share/bilterm/completions`
- **Baseline Verification:**
  - `cargo +stable clippy --all-targets -- -D warnings` → **0 warnings, 0 errors**
  - `cargo +stable test --quiet` → **250 passed, 0 failed** (`159` unit + `91` integration across 20 integration test binaries)

---

## 1. Done in This Session (With Evidence)

1. **Native macOS Cocoa / CoreText / CoreGraphics Retina GUI (`src/ui/gui_font/macos_ct.rs`, `src/ui/gui_macos.rs`, `src/ui/gui_window.rs`) & `Bilterm.app` Default `--gui` Mode (`scripts/install.sh`, `src/launcher.rs`)**:
   - Implemented `MacosGuiCanvas` (`src/ui/gui_font/macos_ct.rs`) using CoreText (`CTFontRef` + `CTFontDrawGlyphs` + `CTFontCreateForString` Unicode glyph fallback + `CTFontManagerRegisterFontsForURL` custom `.ttf`/`.otf` loading) paired with `draw_geometric_cell_bgra` and `CGBitmapContext` -> `CGImageRef` at 1:1 Retina scale.
   - Implemented `run_macos_gui_window` (`src/ui/gui_macos.rs`) driving a native AppKit `NSApplication` + `NSWindow` + `CALayer` (`CATransaction` zero-latency presentation) with full keyboard translation, `Option`-as-`Alt`, `Cmd+V` clipboard paste (`NSPasteboard`), `Cmd++`/`Cmd+-`/`Cmd+0` and `Ctrl/Cmd+Wheel` font zoom, mouse click/drag/scroll, and live Retina window resizing.
   - Updated `Bilterm.app` (`scripts/install.sh` & `src/launcher.rs::macos_launcher_script_contents`) to launch `exec "${TARGET_BIN}" --gui "$@"` by default with standard user/Homebrew `PATH` entries.
2. **Pulled `origin/main` up to `v1.11.1` (`2483433`)**:
   - Fast-forwarded `main` to include Phase 28 architecture hardening, Native OS GUI (`bilterm --gui` / `-G`), 1:1 HiDPI ClearType TTF/OTF font engine, desktop launcher integration (`scripts/install.sh`), and ConPTY 3270 prompt fixes.
2. **Fixed Parallel-Test Race in `secure_fs::write_private_file` (`src/secure_fs.rs`)**:
   - **Root Cause:** `write_private_file` previously called `fs::remove_file(path)` before `create_private_file(path)`, creating a window where concurrent `prepare_shell_integration` callers in the same PID (`shell-integration-<pid>`) saw `NotFound` (`pty::shell_integration::tests::rc_files_are_private`).
   - **Fix:** `write_private_file` now creates a `0600` sibling temporary file (`.<name>.tmp.<unique_suffix>`, `O_CREAT | O_EXCL | O_NOFOLLOW`) and atomically renames (`fs::rename`) it into place.
3. **Fixed Unix Socket Stale-Check Hang in `run_headless_server` (`src/api/server.rs`)**:
   - **Root Cause:** `run_headless_server` matched any `Err(_)` from `UnixStream::connect(&path).await` as a stale socket, unlinked `path`, and entered `serve_jsonrpc_unix`. Under sandboxed execution where `connect()` returns `PermissionDenied`, `unix_socket_live_daemon_is_not_stolen` stole the socket and blocked forever in the accept loop.
   - **Fix:** Restricted stale-socket unlinking strictly to `ErrorKind::ConnectionRefused | ErrorKind::NotFound`; any other probe error returns `Err(BiltermError::Config(...))` immediately.
4. **Rust 1.99 Clippy Cleanliness (`chunks_exact_to_as_chunks`)**:
   - Updated constant-sized `.chunks_exact(2)` / `.chunks_exact(4)` calls in `src/pane/typography.rs`, `src/ui/gui_window.rs`, `src/ui/pixel_canvas.rs`, and `src/ui/renderer.rs` to `.as_chunks::<N>().0`.
5. **macOS Local + App Bundle Installation**:
   - Built release binary (`cargo +stable build --release --quiet`) and ran `./scripts/install.sh --local --force`, installing `/Users/bill/.local/bin/bilterm` and `/Users/bill/Applications/Bilterm.app`.
6. **Agent Skill Surface Optimization (`~/.gemini/config` & `~/.gemini/antigravity`)**:
   - Disabled 5 non-Rust plugins (`science`, `flutter`, `firebase`, `android-cli-plugin`, `chrome-devtools-plugin`).
   - Switched `gsd-surface` from `full` (71 skills) to `standard` (15 core workflow skills, ~274 tokens), with full 71-skill source cached in `~/.gemini/antigravity/gsd-source/`.
   - Moved 7 duplicate standalone skills (`api-contract-audit`, `code-architecture-review`, `dependency-audit`, `github-issues-script`, `pr-review`, `refactor-execute`, `release`) to `~/.gemini/config/skills-dedup-backup/`.

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
