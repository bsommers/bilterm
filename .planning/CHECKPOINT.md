# Session Checkpoint — `bilterm`

**Updated:** 2026-11-09T20:30:00-05:00  
**Status:** COMPLETE — all media/browser/video/voice TUI & URL bar enhancements, menu fixes, Phase 65 GUARD-01 (`SignalablePid`), `v3.2.0`, the main test-suite repair (#50), live OIA surface badge (#49) and VTE F10 hint (#51) merged to `main` (`96307c9`), and release binary installed.

## 1. Installed Release Binaries (`bilterm 3.2.0` @ `96307c9`, sha256 `154583c4…`)

- **Workspace Release Binary**: `/home/bill/src/ai/bilterm/target/release/bilterm` (`16M`)
- **Workspace Debug Binary**: `/home/bill/src/ai/bilterm/target/debug/bilterm` (`156M`)
- **User `$PATH` Binaries**:
  - `/home/bill/.local/bin/bilterm` (`16M`, primary on `$PATH`)
  - `/home/bill/.cargo/bin/bilterm` (`16M`)

## 2. Completed Work in This Session

1. **Phases 61–64 Merged (`7aaa11b`, PR #40)**:
   - Phase 61: Classic drawer 2-line rows (`:drawer rows`), focus-derived zoom (`z_stack.top()`), inactive-window focus click state preservation (`was_active` gate).
   - Phase 62: `Shift`/`Alt` mouse drag local selection override over mouse-tracking TUIs (`Herdr`/`vim`/`tmux`), split-read `OSC 52` reassembly, Vi copy-mode motions (`w b e`, `0 ^ $`, `gg G`, `H M L`, `/ ? n N`).
   - Phase 63: Cross-OS `Primary` modifier & key profiles, sticky Window mode (`w`/`Primary+O`), which-key overlay, 9-category keyboard menu bar, key conflict engine (`:keys check|list|explain|profile`).
   - Phase 64: `src/traffic/` lock-free ring buffer across all 33 byte boundaries (`H01..H33`), `TRAFFIC MONITOR` TUI/GUI window (`:traffic`).
2. **Daemon Supervisor & `:ssh-tmux` TDD Overhaul (`7aaa11b`, PR #40)**:
   - Real child PID tracking (eliminating `PID 0`), `-d`/`--headless` lock/registry registration, and floating `DAEMON SUPERVISOR` modal in TUI & GUI (`:daemon`).
   - PTY-less remote `:ssh-tmux` panes (no local double-echo/execution), `link_raw_input` password masking, immediate `primary_pane <-> %0` fallback binding with `list-panes` reconciliation on idle attach, and `:ssh-tmux prefill` fix.
3. **Release `v3.2.0` (`ec942fb`, PR #41, Tag `v3.2.0`) & Bug Audit (`5c2ea56`, PR #42)**:
   - Published GitHub Release `v3.2.0` and `docs/BUG_AUDIT_REPORT.md`.
4. **Phase 65 `GUARD-01` (`SignalablePid`) & Agent Guardrails (`d424638`, PR #43)**:
   - Registered Phase 65 in `.planning/ROADMAP.md` & `.planning/phases/65-architectural-guardrails-and-type-hardening/65-PLAN.md`.
   - Implemented `SignalablePid` in `src/daemon/types.rs` (`tests/signalable_pid_invariants.rs`, 12/12).
   - Updated `AGENTS.md` (`TEST-R01`, `PLAT-R01`, `SPEC-R05`, `TEST-R02`) and `swe-skills-2` (`93accf7`).
5. **Menu Bar `Leader m` & Dropdown Mouse Priority Fix (`458785d`, PR #44)**:
   - `Leader m` (`Ctrl+B m` / `Ctrl+X m`) opens the top menu bar across all profiles (`src/input/keymap_tables.rs`).
   - Reordered `handle_modal_mouse_event` before `handle_drawer_mouse_event` in `src/input/mouse.rs` (`tests/menubar_mouse_and_leader_m.rs`, 2/2).
6. **Browser, Video, Voice & OIA Surface Enhancements (`dce5340`, PR #48)**:
   - **Browser (`:browser`)**: Clickable/editable URL bar (`BrowserNavHit::UrlBar` or `Ctrl+L`, `Enter` to navigate with auto-`https://`, `Esc` to cancel), `+1` row prompt offset when `PromptAnchor::Top` so `occlude_rect` never hides the URL bar, and TrueColor `▀` half-block TUI rendering (`tests/browser_canvas_tui_and_url_edit.rs`, 4/4).
   - **Video (`:video`)**: Non-blocking `FfmpegPipeBackend` stdout reader thread decoding RGBA frames, TrueColor `▀` half-block TUI rendering, and clear backend/surface status messages (`tests/video_canvas_tui_and_ffmpeg.rs`, 5/5).
   - **OIA & Voice (`:voice`)**: `[TUI]` / `[GUI]` surface mode indicator in OIA status bar (`GOLDENS OK 11` preserved) and pre-flight microphone recorder / STT endpoint diagnostics (`tests/voice_and_oia_surface_badge.rs`, 4/4).

7. **Main Test-Suite Repair (PR #50)**: fixed all 29 failing tests on `main` — real regressions (modal drag routing before modal handler, floating-prompt border click, copy-mode trim/anchor/paging/word motions, Shift/Shift+Alt selection) plus stale expectations (MCP tool count 26, `[COPY]` badge offsets) and isolation (`BILTERM_RUNTIME_DIR` for phase19 ctl tests). Five copy-mode keys restored (Ctrl+Down, `i`/Tab/F8 exit, plain `c` yank, plain-nav drops transient selection, `R`). Daemon flakes: spawned children inherit listener fds until `exec` (`FD_INHERIT_LOCK` in `tests/daemon_lifecycle_and_pid_fix.rs`; retrying probe in `src/daemon/mod.rs` unit test for macOS).
8. **Live OIA Surface Badge (PR #49)**: `CompositePaneWidget::with_surface_badge` shows `[TUI]`/`[GUI]` for non-Classic profiles only (Classic goldens unchanged).
9. **VTE F10 Hint (PR #51)**: `keyconflict::vte_f10_hint` — startup hint when `VTE_VERSION` is set and F10 is still Quake (GNOME Terminal's menu accelerator eats F10). Not a keymap bug.
- **Deliberately not done**: tek OIA message truncation at 120 cols in Classic — fitting it would hide the clickable `[COPY]` badge while `oia_copy_badge_rect` keeps reporting it.

## 3. Working-Tree & Verification State

- **Branch**: `main` at `96307c9` (`origin/main`), only `.planning/CHECKPOINT.md` and `docs/CHECKPOINT.md` updated locally. Extra worktree `/tmp/bilterm-wt-pages` (`pages`) predates this session.
- **Verification**: full `cargo test` green on #49 rebase; CI (ubuntu + macOS + cargo-deny) green on `96307c9`; `GOLDENS OK 11`. `scripts/verify.sh` not re-run since `dce5340` (`PASS 135/135` there).
- **Next Step on Resume**: Start a fresh session from `docs/CHECKPOINT.md` (or execute Phase 65 `GUARD-02` / `GUARD-03` in `.planning/phases/65-architectural-guardrails-and-type-hardening/65-PLAN.md`).
