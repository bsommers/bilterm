# Session Checkpoint (2026-10-31)

**Status:** COMPLETE — `v2.5.1` implemented, verified (`202` unit + `310` integration tests), documented, and installed (`target/release/bilterm` & `~/.local/bin/bilterm`).

## Latest Milestone (`v2.4.0`–`v2.5.1`)
- **`v2.5.1` (Patch Release)**:
  - Fixed per-window theme & individual element color overrides (`:window cursor-color`, `bg`, `font-color`, `prompt-bg`, `prompt-color`, `status-color`, `badge-color`, `crosshair-color`, `shadow-color`), `:theme`, and `:iterm` so canvas backgrounds and interior widgets honor each window's resolved palette.
  - Renamed drawer header from `"HERDR"` to `"DRAWER"`, fixed `Ctrl+X` leader chords while drawer is focused, fixed `F7` when `BlockPage` is not holding a page, and wired `[_]`, `[□]`, `[X]`, and `[M:Push]` drawer controls.
  - Added top-level `:save [name]`, `:rename [old] <new>`, `:session save [name]`, and drawer `s`/`r`/`:` hotkeys.
  - Added `RawFkeyMode` (`:fkeys [auto|bilterm|app|send F1..F12]`, `--raw-fkeys`) and `:snazzy` / `:glass` / `:style` (`F11`) visual styles.
- **`v2.4.0` (Phase 39)**: Per-window themes (`:window theme`), outlines (`:window outline`), bold borders (`:window bold`), clearer `[_][□][X]` window controls, and `MIN_GRID_DIM = 2×2` VT grid clamp preventing 1-row soft-wrap underflow.
- **`v2.4.1` (Phase 40)**:
  - Interactive **Per-Window Theme Browser** (`:window theme`, `:window theme list`, `:window theme browse [dir]`, `:window theme import [path|dir|glob]`) scoped to the active window via `McpConsoleState::window_theme_target`.
  - Interactive **Window Style & Element Color Customizer** (`:window` / `:window menu`).
  - Per-window **Individual Theme Element Color Overrides** (`:window <element> <color|reset>` and `:window <elem> color <color>`) for `font-color`, `cursor-color`, `frame-color`, `inactive-border`, `controls-color`, `status-color`, `badge-color`, `title-color`, `bg`, `prompt-bg`, `prompt-color`, `crosshair-color`, and `shadow-color`, persisted in saved session TOML files (`SavedPaneState.colors`).
- **`v2.5.0` (Phase 41)**:
  - **2D Rectangular Block Select, Highlight & Copy** (`:rect`, `:select`, `:copy rect`, `Alt+R`, `Alt+LeftDrag`, `:rect mode [on|off|toggle]`, `:rect <r1> <c1> <r2> <c2> [copy|highlight]`, `:rect all [copy]`, `:rect copy`, `:rect cut`, `:rect clear`).
  - **Full Per-Pane Session Persistence (Layout + Shell + Running Software/Commands + CWD)**: `SavedPaneState` and `PaneJournalRecord` now persist and restore `shell`, `shell_args`, `running_command`, `cwd`, and `remote_target` for every window pane across every session, automatically relaunching running commands when `session_restore_processes = true` (`:session processes|cmd|shell|cwd|relaunch|info`).

## User decisions (verbatim)
- "make MacOS releases for this. also make a skill / command / scripts to do binary release per platform."
- "v2.1.2 - do release. make release page; add MacOS binaries. note that I have to build for Linux and Windows, too." / "v2.1.1 - build and attach v2.1.1. binaries for MacOS to it"
- Merge local vs remote without clobbering either → user: **"yes"** (to the stash / ff-pull / pop plan).
- Workflow should run the script from `main` + attach macOS to v2.2.0–v2.4.0 → user: **"yes"**.
- SSH into `bigmac.local` and use `/Users/bill/src/tools/win-devvm` (Parallels Windows 11 ARM VM) to complete the `v2.3.0` Windows builds.

## Done (committed + pushed)
- `066f4e5` fix `scripts/build-release-mac-win.sh --upload` on macOS: `ls bilterm-*.zip` failed under `pipefail` (no zips on mac) → nullglob + printf.
- `d0552c2` `.github/workflows/release-binaries.yml` checks out the default branch (`fetch-depth: 0`) instead of the tag; the script builds the tag in its own worktree, so script fixes now apply to old tags. **Not yet run on GitHub.**
- macOS archives (aarch64 + x86_64, verified arch / `--version` / system-only dylibs, re-downloaded and `SHA256SUMS`-checked) attached to v2.1.1, v2.1.2, v2.2.0, v2.3.0, v2.4.0. Existing Linux/Windows sums were merged, not overwritten.
- v2.1.2 release page created from CHANGELOG (`--latest=false`); v2.1.1 notes footer updated to list macOS.
- Skill `building-release-binaries` committed in `~/src/swe-skills-2` (`28b79df`) and pushed; its `build-binaries.sh` (bash 3.2-safe `REPO_ARGS`, plus `python3`/`python` fallback on Windows) is also in this repo as `scripts/build-binaries.sh`.
- v2.3.0 Windows `aarch64-pc-windows-msvc` and `x86_64-pc-windows-msvc` built natively inside the Parallels Windows 11 ARM VM on `bigmac.local` (`/Users/bill/src/tools/win-devvm/vm.sh`), verified (`bilterm.exe --version` → `bilterm 2.3.0`), uploaded to the `v2.3.0` GitHub release, and `SHA256SUMS` merged & re-verified after download.

## Release asset matrix (2026-10-31 11:48, verified via `gh release view`)
| Release | macOS | Windows | Linux |
|---|---|---|---|
| v2.4.0 (Latest) | ✅ | x86_64 + aarch64 msvc ✅ | gnu, musl x86_64 + aarch64 musl ✅ |
| v2.3.0 | ✅ | x86_64 + aarch64 msvc ✅ (+ x86_64 gnu) | gnu, musl x86_64 + aarch64 musl ✅ |
| v2.2.0 | ✅ | msvc | complete |
| v2.1.2 | ✅ | msvc | complete (release notes updated to list all platforms) |
| v2.1.1 | ✅ | gnu (+ `windows-msvc.zip` alias is a copy of the gnu build — same sha) | complete |

- aarch64-unknown-linux-musl for v2.3.0/v2.4.0 built on Linux via `scripts/build-binaries.sh <tag> --targets aarch64-unknown-linux-musl --upload` (static ELF aarch64, embedded `BILTERM v<ver>` checked; not executed — no qemu-aarch64); `SHA256SUMS` merged and re-verified after download.

## Known issues (resolved)
- ~~`cargo test` rewrites 5 `docs/assets/*.svg`~~ — fixed: screenshots now go to `target/tmp/screenshots/`; refresh committed SVGs with `BILTERM_UPDATE_SCREENSHOTS=1 cargo test -q --test full_suite_battle_and_perf`.
- ~~`scripts/build-release-assets.sh` hardcodes a stale `/tmp/claude-1000/...` zig path~~ — fixed: resolves `zig` from PATH, `$ZIG_DIR`, or `~/.local/bin`, else fails fast.
- ~~`~/bilterm-local-backup.patch`~~ — already deleted.
- `.planning/CHECKPOINT.md` is now a pointer to this file (was stale v2.4.0-in-progress state).
- v2.1.1–v2.2.0 lack Windows aarch64 (never built for those; not tracked as a gap).

## Lessons
- `scripts/build-binaries.sh` now uses `cargo zigbuild` for `*-windows-gnu` on Linux (plain cargo has no MinGW linker) and falls back to `python` when `python3` is a Microsoft Store alias stub on Windows.
- On `bigmac.local` (`win-devvm`), invoke `./vm.sh` via `zsh -l -c` so `/usr/local/bin/prlctl` is on `PATH`, and inside the Windows guest invoke `"C:\Program Files\Git\bin\bash.exe" -l -c` with `PATH=/clangarm64/bin:/c/Users/dev/AppData/Local/Programs/Python/Python312-arm64:/usr/bin:/c/Users/dev/.cargo/bin:$PATH`.
- Never run `cargo fmt` repo-wide (repo is not rustfmt-clean → 100+ file churn; v2.4.0 needed a revert commit `0df2ed5`).
- Run the FULL integration suite (`timeout 900 cargo test -q --tests --no-fail-fast`) before releases, not just lib tests (v2.3.0 shipped a 1-row-grid crash).

## Exact next command
None — release backfill is complete. Ready for next feature or milestone.
