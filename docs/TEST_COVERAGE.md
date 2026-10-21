# Test Suite & Coverage Report — `bilterm`

**Date:** `2026-10-20`
**Tooling used:** `cargo llvm-cov 0.8.5` (`llvm-tools-x86_64-unknown-linux-gnu`), `insta 1.42` snapshot testing, `ratatui::backend::TestBackend`

---

## 1. Test Suite Inventory

| Dimension | Value |
|---|---|
| Test framework(s) | `cargo test` (built-in Rust test harness) + `insta 1.42` snapshot assertions + `ratatui::backend::TestBackend` |
| Total test files | `22` (`14` inline unit-test modules in `src/**` + `8` integration/snapshot/torture test suites in `tests/*.rs`) |
| Total test cases | `53` (`31` unit tests + `22` integration, snapshot, streaming, and terminal torture/stress tests) |
| CI-enforced? | Local verification gate enforced (`cargo test && cargo clippy --all-targets -- -D warnings`); CI threshold gate recommended below |
| Runtime | `~0.35s` wall-clock for all 53 tests |

---

## 2. Coverage Summary

| Metric | Baseline (Before Audit) | Current (After Torture & Stress Suite) | Delta | Source |
|---|---|---|---|---|
| **Line coverage** | `73.22%` (`3251 / 4440`) | **`81.83%`** (`3913 / 4782`) | **`+8.61%`** | `cargo llvm-cov --summary-only` |
| **Function coverage** | `75.16%` (`242 / 322`) | **`83.00%`** (`288 / 347`) | **`+7.84%`** | `cargo llvm-cov --summary-only` |
| **Region coverage** | `73.11%` (`5127 / 7013`) | **`82.06%`** (`6463 / 7876`) | **`+8.95%`** | `cargo llvm-cov --summary-only` |

### Per-Module Coverage Breakdown (`cargo llvm-cov`)

| Module | Line Coverage | Function Coverage | Region Coverage |
|---|---|---|---|
| `src/wm/quick_terminal.rs` | `100.00%` (`50/50`) | `100.00%` (`5/5`) | `98.61%` |
| `src/ui/theme.rs` | `98.46%` (`128/130`) | `100.00%` (`6/6`) | `95.83%` |
| `src/ui/occlusion.rs` | `97.83%` (`45/46`) | `100.00%` (`4/4`) | `97.17%` |
| `src/ui/status_bar.rs` | `97.62%` (`41/42`) | `100.00%` (`1/1`) | `98.46%` |
| `src/vt/mod.rs` | `95.85%` (`416/434`) | `95.83%` (`46/48`) | `95.35%` |
| `src/pane/fuzzy_history.rs` | `95.83%` (`69/72`) | `100.00%` (`7/7`) | `96.36%` |
| `src/wm/split_tree.rs` | `93.23%` (`124/133`) | `91.67%` (`11/12`) | `89.66%` |
| `src/ui/glyphs.rs` | `92.75%` (`64/69`) | `80.00%` (`4/5`) | `87.76%` |
| `src/wm/z_stack.rs` | `92.31%` (`84/91`) | `88.24%` (`15/17`) | `93.02%` |
| `src/pty/mod.rs` | `91.48%` (`161/176`) | `65.22%` (`15/23`) | `83.40%` |
| `src/pty/line_discipline.rs` | `91.20%` (`114/125`) | `100.00%` (`8/8`) | `94.08%` |
| `src/ui/mod.rs` | `90.04%` (`497/552`) | `93.33%` (`14/15`) | `92.22%` |
| `src/ui/chrome.rs` | `88.17%` (`149/169`) | `87.50%` (`7/8`) | `91.85%` |
| `src/pane/completion.rs` | `86.10%` (`192/223`) | `64.29%` (`9/14`) | `83.49%` |
| `src/tmux/mod.rs` | `85.03%` (`142/167`) | `92.86%` (`13/14`) | `87.00%` |
| `src/ui/cursor.rs` | `83.78%` (`62/74`) | `100.00%` (`4/4`) | `84.14%` |
| `src/pane/colon_command.rs` | `83.72%` (`216/258`) | `82.35%` (`14/17`) | `88.33%` |
| `src/pane/highlight.rs` | `81.94%` (`127/155`) | `71.43%` (`5/7`) | `80.66%` |
| `src/wm/mod.rs` | `77.14%` (`189/245`) | `84.00%` (`21/25`) | `79.27%` |
| `src/pane/mod.rs` | `71.33%` (`520/729`) | `77.78%` (`49/63`) | `72.25%` |
| `src/config/mod.rs` | `69.29%` (`88/127`) | `80.00%` (`8/10`) | `65.09%` |
| `src/input/mod.rs` | `68.68%` (`353/514`) | `91.67%` (`11/12`) | `66.36%` |
| `src/app.rs` | `49.56%` (`56/113`) | `55.56%` (`5/9`) | `46.28%` |
| `src/error.rs` | `38.81%` (`26/67`) | `54.55%` (`6/11`) | `41.67%` |
| `src/main.rs` | `0.00%` (`0/21`) | `0.00%` (`0/2`) | `0.00%` |

> **Measurement Caveat:** `src/main.rs` (`21` lines) and `TerminalGuard::enter` / `App::run` require an interactive host TTY (`crossterm::terminal::enable_raw_mode()`), which fails with `ENOTTY` in headless CI test runners. All event-routing, PTY polling, and rendering methods inside `App` (`handle_crossterm_event`, `poll_all_ptys`, `flush_osc52_to_host`, `all_panes_exited`) are extracted and directly exercised in `tests/terminal_torture_and_stress.rs`.

---

## 3. Coverage Gaps (Prioritized)

### P0 — Critical / Security & Terminal Sanitization Paths (CLOSED)

| Function / Module | File | Risk & Verification Status |
|---|---|---|
| `inspect_pty_fd` & `!ECHO` password lock | `src/pty/line_discipline.rs` | **91.20% covered.** Verified with real Unix PTY `termios` (`ECHO` disabled) and snapshot tests ensuring `sudo`/`ssh` passwords never echo into `FloatingPromptState`. |
| `sanitize_binary_noise_chunk` | `src/vt/mod.rs` | **95.85% covered.** Verified against `tail -f /dev/random` binary streams containing embedded `DECSET 1049`, `DECTCEM ?25l`, and `DECSTBM` scroll-region traps. |
| `is_safe_terminal_cluster` & wide-char clipping | `src/ui/mod.rs` / `src/ui/occlusion.rs` | **90.04% / 97.83% covered.** Prevents C0/C1/bidi/variation-selector corruption and 2-column CJK/Emoji overflow at pane and floating-prompt boundaries. |

### P1 — Remaining Core Execution Paths

| Function / Module | File | Why it matters |
|---|---|---|
| `TmuxControlBridge::spawn_attach` & `send_keys` | `src/tmux/mod.rs` | Spawns external `tmux -CC` subprocesses; parser (`parse_tmux_control_line`, `unescape_tmux_output`) is 85.03% covered, but live subprocess spawning is only exercised when `tmux` is running. |
| `App::run` (`tokio::select!` loop) | `src/app.rs` | Top-level async loop awaiting `crossterm::event::EventStream` and `tokio::signal::ctrl_c`; individual arms (`handle_crossterm_event`, `poll_all_ptys`, `flush_osc52_to_host`) are covered directly. |
| `BiltermConfig::load` filesystem discovery | `src/config/mod.rs` | Reads `~/.config/bilterm/config.toml` and `theme.ini` from disk; TOML/INI parsers (`from_toml_str`, `parse_ini_theme`) are covered in unit & torture tests. |

### P2 — Remaining Edge Cases & Error Handling

| Function / Module | File | Missing scenario |
|---|---|---|
| `TerminalGuard::enter` & panic hook | `src/error.rs` | Requires a live TTY file descriptor (`stdout` raw mode + alternate screen); `Drop` and idempotent `restore()` are unit-tested. |
| Live PTY `Ctrl+C` / `Ctrl+D` writeback branches | `src/input/mod.rs` | `handle_command_prompt_key` branches that write `0x03` (`SIGINT`) or `0x04` (`EOF`) when `pane.pty` is `Some(...)` (most unit tests use `new_headless`). |

---

## 4. Recommended Additions & CI Gate Plan

1. **Enforce Coverage Gate in CI (`>= 80%` line coverage):**
   ```bash
   cargo llvm-cov --fail-under-lines 80 --summary-only
   ```
2. **Add Live-PTY Input Signal Test (`tests/pty_lifecycle.rs`):**
   Spawn a live `CompositePane::spawn_default` pane and dispatch `Ctrl+C` (`KeyCode::Char('c')` with `CONTROL`) and `Ctrl+D` (`KeyCode::Char('d')` with `CONTROL`) through `handle_key_event` to cover the live-PTY signal writeback branches in `src/input/mod.rs`.
3. **Add Temp-Dir Config Discovery Test (`src/config/mod.rs`):**
   Test `BiltermConfig` file loading from a temporary directory containing both `config.toml` and `theme.ini`.

---

## 5. What's Already Solid

- **VT100/VT220/Xterm State Machine (`src/vt/mod.rs` — `95.85%` line coverage):** Full coverage of `SGR` (Bold, Italic, Underline, Inverse, 16/256/TrueColor), `DECSET 47/1047/1049` alternate screen, `DECCKM ?1` application cursor keys, `?1000/1002/1003/1006` mouse tracking, `?1004` focus reporting, `?2026` synchronized output, `DECSET 2004` bracketed paste, `OSC 0/2` dynamic window titles, `OSC 7` working directory reporting, `OSC 52` clipboard, `OSC 133` (`A`/`B`/`C`/`D`) command blocks, `DSR`/`CPR`/`DA1` query responses, and binary-noise sanitization.
- **Immediate-Mode TUI Compositor (`src/ui/*` — `88%–98%` line coverage):** Deterministic `insta` snapshots and `terminal_torture_and_stress` tests verify floating and non-overlapping split command prompts (`Top`, `Bottom`, `Floating`, `SplitTop`, `SplitBottom`), all 8 border outline styles (`Solid`, `Double`, `Rounded`, `Thick`, `Dotted`, `Dashed`, `Quadruple`, `Ascii`), all 4 DOS drop-shadow styles (`None`, `Translucent`, `ThemeTinted`, `SolidHalfBlock`), all 5 retro themes, all 4 glyph packs, IBM 3270 cross-hair cursors, and wide CJK/Emoji occlusion.
- **Multi-Pane Window Manager (`src/wm/*` — `77%–100%` line coverage):** Binary split tree (`SplitTree`), z-order window stack (`ZStack`), Guake quick terminal (`QuickTerminal`), and local SGR 1006 mouse coordinate translation are thoroughly tested.
