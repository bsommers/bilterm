# Test Suite & Coverage Report — `bilterm` (`v1.4.0`)

**Date:** `2026-10-21`
**Tooling used:** `cargo llvm-cov 0.8.5` (`llvm-tools-x86_64-unknown-linux-gnu`), `insta 1.42` snapshot testing, `ratatui::backend::TestBackend`, `tokio::test` async TCP/UDS RPC harnesses

---

## 1. Test Suite Inventory

| Dimension | Value |
|---|---|
| Test framework(s) | `cargo test` + `insta 1.42` snapshot assertions + `ratatui::backend::TestBackend` + `tokio::test` |
| Total test files | `30` (`15` inline unit-test modules in `src/**` + `15` integration, snapshot, API, MCP, playbook, remote fleet, stress & torture suites in `tests/*.rs`) |
| Total test cases | **`85`** (`39` unit tests + `46` integration, snapshot, CLI, JSON-RPC, MCP, playbook, remote SSH/fleet, performance & torture tests) |
| CI / Local Quality Gate | `cargo test --all-targets && cargo clippy --all-targets -- -D warnings && cargo llvm-cov --fail-under-lines 80 --summary-only` |
| Runtime | `~2.1s` wall-clock for all 85 tests across 16 test binaries |

---

## 2. Coverage Summary (`v1.4.0`)

| Metric | Baseline (Before `v1.4.0` Battle Suite) | Current (`v1.4.0` Full Battle Suite) | Delta | Source |
|---|---|---|---|---|
| **Line coverage** | `77.19%` (`9,014 / 11,678`) | **`83.45%`** (`9,745 / 11,678`) | **`+6.26%`** | `cargo llvm-cov --all-targets --summary-only` |
| **Region coverage** | `77.04%` (`14,791 / 19,198`) | **`82.74%`** (`15,885 / 19,198`) | **`+5.70%`** | `cargo llvm-cov --all-targets --summary-only` |
| **Function coverage** | `75.35%` (`599 / 795`) | **`81.76%`** (`650 / 795`) | **`+6.41%`** | `cargo llvm-cov --all-targets --summary-only` |

### Per-Module Coverage Breakdown (`cargo llvm-cov --all-targets`)

| Module | Line Coverage | Function Coverage | Region Coverage | Status |
|---|---|---|---|---|
| `src/remote/mod.rs` (`v1.4.0` SSH & Fleet) | **`100.00%`** (`168/168`) | **`100.00%`** (`16/16`) | **`98.84%`** | **A+** |
| `src/vt/snapshot.rs` (`GridSnapshot` Seam) | **`100.00%`** (`113/113`) | **`100.00%`** (`15/15`) | **`94.78%`** | **A+** |
| `src/ui/occlusion.rs` (Wide CJK/Emoji Clipping) | **`100.00%`** (`46/46`) | **`100.00%`** (`4/4`) | **`98.11%`** | **A+** |
| `src/wm/quick_terminal.rs` (Guake Overlay) | **`100.00%`** (`50/50`) | **`100.00%`** (`5/5`) | **`98.61%`** | **A+** |
| `src/config/mod.rs` (TOML/INI/Remotes/Keys) | **`99.27%`** (`136/137`) | **`90.00%`** (`9/10`) | **`93.10%`** | **A+** |
| `src/ui/theme.rs` (3278/3279/Borland Palettes) | **`99.23%`** (`129/130`) | **`100.00%`** (`6/6`) | **`97.02%`** | **A+** |
| `src/ui/status_bar.rs` (DOS F-Key Bar) | **`97.62%`** (`41/42`) | **`100.00%`** (`1/1`) | **`98.46%`** | **A+** |
| `src/wm/z_stack.rs` (Z-Order Deck & Raise) | **`96.81%`** (`91/94`) | **`100.00%`** (`18/18`) | **`96.07%`** | **A+** |
| `src/vt/ghostty_backend.rs` (`libghostty-vt` FFI) | **`96.34%`** (`79/82`) | **`100.00%`** (`12/12`) | **`96.84%`** | **A+** |
| `src/pty/shell_integration.rs` (`OSC 133`/`OSC 7`) | **`95.88%`** (`93/97`) | **`88.89%`** (`8/9`) | **`92.59%`** | **A+** |
| `src/ui/glyphs.rs` (Unicode/CP437/Nerd/ASCII) | **`95.88%`** (`93/97`) | **`80.00%`** (`4/5`) | **`89.80%`** | **A+** |
| `src/pane/fuzzy_history.rs` (`nucleo-matcher`) | **`95.83%`** (`69/72`) | **`100.00%`** (`7/7`) | **`96.36%`** | **A+** |
| `src/wm/split_tree.rs` (BSP H/V Split Tree) | **`93.98%`** (`125/133`) | **`91.67%`** (`11/12`) | **`90.52%`** | **A** |
| `src/pane/history_store.rs` (Persistent History) | **`93.10%`** (`189/203`) | **`94.74%`** (`18/19`) | **`89.11%`** | **A** |
| `src/pty/mod.rs` (`portable-pty` Lifecycle) | **`92.82%`** (`168/181`) | **`65.22%`** (`15/23`) | **`84.77%`** | **A** |
| `src/pane/colon_command.rs` (`:colon` Engine) | **`92.76%`** (`615/663`) | **`90.00%`** (`27/30`) | **`93.04%`** | **A** |
| `src/pty/line_discipline.rs` (`!ECHO` Password Lock)| **`91.20%`** (`114/125`) | **`100.00%`** (`8/8`) | **`94.08%`** | **A** |
| `src/ui/chrome.rs` (Borland Borders & Shadows) | **`88.76%`** (`150/169`) | **`87.50%`** (`7/8`) | **`92.16%`** | **A-** |
| `src/pane/completion.rs` (`<Tab>` Completion) | **`86.10%`** (`192/223`) | **`64.29%`** (`9/14`) | **`83.49%`** | **B+** |
| `src/wm/mod.rs` (WindowManager + Sessions + Fleet) | **`85.76%`** (`536/625`) | **`88.33%`** (`53/60`) | **`85.78%`** | **B+** |
| `src/ui/cursor.rs` (3270 Cross-Hair & Carets) | **`85.14%`** (`63/74`) | **`100.00%`** (`4/4`) | **`86.21%`** | **B+** |
| `src/ui/mod.rs` (3-Pass Compositor & Modals) | **`85.06%`** (`1,235/1,452`) | **`89.29%`** (`25/28`) | **`85.15%`** | **B+** |
| `src/api/mod.rs` (JSON-RPC 2.0 + MCP + Playbooks) | **`83.86%`** (`1,065/1,270`) | **`55.32%`** (`52/94`) | **`80.85%`** | **B+** |
| `src/vt/mod.rs` (VT100/TrueColor/Kitty/Sixel/OSC) | **`83.75%`** (`1,526/1,822`) | **`86.36%`** (`133/154`) | **`83.70%`** | **B+** |
| `src/vt/worker.rs` (Dedicated VT Worker Thread) | **`82.22%`** (`296/360`) | **`63.64%`** (`21/33`) | **`81.29%`** | **B+** |
| `src/pane/highlight.rs` (Live Shell Highlighter) | **`81.94%`** (`127/155`) | **`71.43%`** (`5/7`) | **`80.66%`** | **B+** |
| `src/ui/renderer.rs` (`GuiWorkspaceRenderer` PPM) | **`81.45%`** (`224/275`) | **`91.67%`** (`11/12`) | **`86.76%`** | **B+** |
| `src/tmux/mod.rs` (`tmux -CC` Daemon & Passthrough)| **`81.09%`** (`253/312`) | **`85.71%`** (`24/28`) | **`81.53%`** | **B+** |
| `src/pane/mod.rs` (`CompositePane` & Prompt State) | **`74.79%`** (`786/1,051`) | **`80.85%`** (`76/94`) | **`75.68%`** | **B** |
| `src/input/mod.rs` (Modal Input & Mouse Router) | **`69.33%`** (`807/1,164`) | **`92.00%`** (`23/25`) | **`68.16%`** | **B** |
| `src/main.rs` (CLI Entrypoint & Subcommands) | **`66.37%`** (`75/113`) | **`71.43%`** (`10/14`) | **`56.44%`** | **B** |
| `src/error.rs` (RAII `TerminalRestoreGuard`) | **`50.75%`** (`34/67`) | **`72.73%`** (`8/11`) | **`48.81%`** | **TTY-bound** |
| `src/app.rs` (`App` Event Loop Coordinator) | **`50.44%`** (`57/113`) | **`55.56%`** (`5/9`) | **`46.81%`** | **TTY-bound** |
| **TOTAL (`11,678` executable lines)** | **`83.45%`** (`9,745/11,678`) | **`81.76%`** (`650/795`) | **`82.74%`** (`15,885/19,198`) | **PASS (`>= 80%`)** |

---

## 3. Multi-Tier Verification Matrix (Unit, Script/API, Integration, Stress, Performance, Torture)

| Test Category | Test Binary / Module | Cases | Key Invariants Verified |
|---|---|---|---|
| **1. Unit Tests** | `src/**` inline `#[cfg(test)]` + `tests/full_suite_battle_and_perf.rs` | `41` | `VtParser`, `LineDiscipline` (`!ECHO`), `SplitTree`, `ZStack`, `QuickTerminal`, `GlyphSet`, `ThemePalette`, `occlude_rect`, `RemoteHostProfile`, `FleetRegistry`, `GridSnapshot` |
| **2. Script / JSON-RPC / MCP / CLI API** | `tests/v110_v120_v130_automation_api_mcp_playbooks.rs` + `tests/full_suite_battle_and_perf.rs` | `7` | JSON-RPC 2.0 over TCP & Unix sockets, all 8 MCP `2024-11-05` tools (`bilterm_*`), `Playbook` TOML engine, `--version`, `--help`, `--completions`, `--man`, `--gui-dump`, `--playbook` |
| **3. Integration & Snapshot Tests** | `tests/composite_pane_snapshots.rs`, `tests/phase2..5_*.rs`, `tests/pty_lifecycle.rs`, `tests/v020..v100_*.rs` | `28` | Live `portable-pty` shell spawning, zero-`SIGWINCH` floating prompt movement, `OSC 133` command blocks, `tmux -CC` bi-directional layout hydration, Kitty/Sixel graphics, `asciinema` `.cast` recording |
| **4. Performance & Stress Benchmarks** | `tests/full_suite_battle_and_perf.rs` (`performance_and_stress_benchmark_*`) | `1` | 32-pane mixed local/SSH/`tmux -CC` workspace, 3.84 MB ANSI/TrueColor stream throughput, 100-frame `160x40` compositor latency (`<16ms` 60 FPS budget), 1,000 JSON-RPC `fleet.exec` calls (`<5ms`/call) |
| **5. Terminal Torture & Fuzz Suite** | `tests/terminal_torture_and_stress.rs`, `tests/vttest_esctest_fuzz_torture.rs`, `tests/tab_completion_and_streaming.rs` | `8` | `/dev/random` binary flood with embedded `DECSET 1049`/`?25l`/`DECSTBM` traps, `vttest`/`esctest` split UTF-8/CSI boundary fuzzing, 99,999-param CSI bombs, 16-session simultaneous tile/stack/zoom |
