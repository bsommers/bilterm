---
phase: "50"
slug: herdr-style-left-sidebar-agent-aware-workspace-navigator
status: draft
nyquist_compliant: true
wave_0_complete: false
created: "2026-11-03"
---

# Phase 50 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Detailed test architecture: see `50-RESEARCH.md` § Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + insta snapshots |
| **Config file** | `Cargo.toml`, `tests/snapshots/` |
| **Quick run command** | `scripts/verify.sh` (read single output line) |
| **Full suite command** | `scripts/verify.sh` |
| **Estimated runtime** | see `scripts/verify.sh` (project-defined) |

---

## Sampling Rate

- **After every task commit:** Run `scripts/verify.sh`
- **After every plan wave:** Run `scripts/verify.sh` and `git diff --exit-code tests/snapshots/` (classic goldens must not change)
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** per `scripts/verify.sh`

---

## Per-Task Verification Map

Populated by the planner from `50-RESEARCH.md` § Validation Architecture; every task carries an `<automated>` command with `<fails_when>`.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 50-01-T1 | 01 | 1 | FLT-01 (SC5, D-02) | T-50-01 | goldens generated with `src/` untouched | snapshot | `cargo test -q --test drawer_classic_parity legacy_left` | ❌ W0 (created by task) | ⬜ pending |
| 50-01-T2 | 01 | 1 | FLT-01 (SC5, D-02) | T-50-01, T-50-02 | gate script; temp XDG isolation; no host paths | snapshot (cells + style digest + GUI digest) | `cargo test -q --test drawer_classic_parity` | ❌ W0 (created by task) | ⬜ pending |
| 50-02-T1 | 02 | 2 | FLT-01 (SC1, D-02) | T-50-05 | width clamps | integration | `cargo test -q --test phase50_herdr_drawer` | ❌ (created by task) | ⬜ pending |
| 50-02-T2 | 02 | 2 | FLT-01 (SC1) | T-50-03, T-50-04 | titles sanitised + capped | integration + unit | `cargo test -q --test phase50_herdr_drawer tree_` ; `cargo test -q --lib sanitize_display` | ❌ | ⬜ pending |
| 50-03-T1 | 03 | 2 | FLT-01 (SC2, D-03) | T-50-06 | `:agents list` carries no tail text | integration (local + RPC) | `cargo test -q --test phase50_agent_state state_tracer` | ❌ (created by task) | ⬜ pending |
| 50-03-T2 | 03 | 2 | FLT-01 (SC2, D-03) | T-50-08 | advisory state only | unit (synthetic `Instant`) | `cargo test -q --test phase50_agent_state state_` | ❌ | ⬜ pending |
| 50-03-T3 | 03 | 2 | FLT-01 (D-03) | T-50-07 | substring-only patterns, 16 x 80 caps | unit | `cargo test -q --test phase50_agent_state state_profile` | ❌ | ⬜ pending |
| 50-04-T1 | 04 | 3 | FLT-01 (SC2, SC3, D-04, D-05) | T-50-09 | — | integration | `cargo test -q --test phase50_herdr_drawer attention_tracer` | ❌ | ⬜ pending |
| 50-04-T2 | 04 | 3 | FLT-01 (SC2, SC3, D-04, D-05) | T-50-10 | jump is user-initiated only | integration | `cargo test -q --test phase50_herdr_drawer` | ❌ | ⬜ pending |
| 50-05-T1 | 05 | 3 | FLT-01 (D-01) | T-50-12 | bounded ring | integration | `cargo test -q --test phase50_traffic bus_tracer` | ❌ (created by task) | ⬜ pending |
| 50-05-T2 | 05 | 3 | FLT-01 (D-01) | T-50-11, T-50-13, T-50-14 | password-prompt redaction, sanitised previews, origin recorded | integration (incl. RPC dispatch) | `cargo test -q --test phase50_traffic bus_` | ❌ | ⬜ pending |
| 50-06-T1 | 06 | 4 | FLT-01 (SC3, SC4) | T-50-15 | herdr hit-test runs before classic column zones | integration | `cargo test -q --test phase50_herdr_drawer keys_tracer` | ❌ | ⬜ pending |
| 50-06-T2 | 06 | 4 | FLT-01 (SC3) | T-50-16, T-50-17 | rename via existing validated path; `:drawer close` keeps its minimize meaning, pane close is `:drawer close-pane` | integration + unit drift | `cargo test -q --test phase50_herdr_drawer keys_` ; `cargo test -q --lib keymap_herdr_drawer_keys_unique` ; `cargo test -q --lib` | ❌ | ⬜ pending |
| 50-06-T3 | 06 | 4 | FLT-01 (SC1, SC4) | T-50-15 | header/footer clicks create nothing | integration (GUI surface for scale) | `cargo test -q --test phase50_herdr_drawer` | ❌ | ⬜ pending |
| 50-07-T1 | 07 | 4 | FLT-01 (D-07, D-10) | T-50-18 | labels sanitised | integration | `cargo test -q --test phase50_agents_window window_tracer` | ❌ (created by task) | ⬜ pending |
| 50-07-T2 | 07 | 4 | FLT-01 (D-08, D-10) | T-50-19 | node cap 64, fixed iterations | unit + snapshot | `cargo test -q --test phase50_agents_window` | ❌ | ⬜ pending |
| 50-08-T1 | 08 | 5 | FLT-01 (D-07) | T-50-20, T-50-34 | read-only view over RPC; Esc / leader never trapped | integration | `cargo test -q --test phase50_agents_window window_tracer` | ❌ | ⬜ pending |
| 50-08-T2 | 08 | 5 | FLT-01 (D-07, D-08) | T-50-34 | leader passes through while the window has focus | integration | `cargo test -q --test phase50_agents_window` | ❌ | ⬜ pending |
| 50-09-T1 | 09 | 6 | FLT-01 (D-07) | T-50-22 | in-window events swallowed | integration | `cargo test -q --test phase50_agents_window window_tracer` | ❌ | ⬜ pending |
| 50-09-T2 | 09 | 6 | FLT-01 (D-07, D-08) | T-50-21 | rect/splitter clamps | integration + unit | `cargo test -q --test phase50_agents_window window_` ; `cargo test -q --lib keymap_agents_window_keys_unique` | ❌ | ⬜ pending |
| 50-09-T3 | 09 | 6 | FLT-01 (D-07) | T-50-21 | scale clamp 75..=200 | integration (GUI surface) | `cargo test -q --test phase50_agents_window` | ❌ | ⬜ pending |
| 50-10-T1 | 10 | 7 | FLT-01 (D-09) | T-50-24 | read-only; redacted stays redacted on copy | integration | `cargo test -q --test phase50_agents_window window_tracer_snoop` | ❌ | ⬜ pending |
| 50-10-T2 | 10 | 7 | FLT-01 (D-09) | T-50-25 | keyword is substring, capped | integration | `cargo test -q --test phase50_agents_window snoop_` | ❌ | ⬜ pending |
| 50-10-T3 | 10 | 7 | FLT-01 (D-09) | T-50-23 | no outward write path (no pane bytes, no bus event) | integration | `cargo test -q --test phase50_agents_window compose_` | ❌ | ⬜ pending |
| 50-11-T1 | 11 | 7 | FLT-01 (D-10) | T-50-27 | disjoint texture ids | integration (GuiFrameSurface) | `cargo test -q --test phase50_gui_render render_gui_tracer` | ❌ (created by task) | ⬜ pending |
| 50-11-T2 | 11 | 7 | FLT-01 (D-10) | T-50-26 | texture dims capped | integration + digest snapshot | `cargo test -q --test phase50_gui_render render_gui_` | ❌ | ⬜ pending |
| 50-11-T3 | 11 | 7 | FLT-01 (D-05) | T-50-26 | — | integration + human-check | `cargo test -q --test phase50_gui_render` | ❌ | ⬜ pending |
| 50-12-T1 | 12 | 8 | FLT-01 (D-06, D-07) | T-50-28 | partner label sanitised | integration | `cargo test -q --test phase50_herdr_drawer comms_tracer` | ❌ | ⬜ pending |
| 50-12-T2 | 12 | 8 | FLT-01 (D-06) | T-50-28, T-50-29 | throttled animation redraw | integration | `cargo test -q --test phase50_herdr_drawer comms_` | ❌ | ⬜ pending |
| 50-13-T1 | 13 | 7 | FLT-01 (SC4) | — | human scope decision | checkpoint:decision | n/a (checkpoint, `gate="blocking-human"`) | — | ⬜ pending |
| 50-13-T2 | 13 | 7 | FLT-01 (SC4, D-07) | T-50-30 | Ctrl+wheel re-routed only inside panel rects | unit | `cargo test -q --test phase50_gui_runtime gui_runtime_` | ❌ (created by task) | ⬜ pending |
| 50-13-T3 | 13 | 7 | FLT-01 (SC4) | T-50-31 | — | unit + cross-target compile + human-check | `cargo check -q --lib --target x86_64-pc-windows-gnu && cargo check -q --lib --target aarch64-apple-darwin` | ❌ | ⬜ pending |
| 50-14-T1 | 14 | 9 | FLT-01 | T-50-32 | — | doc coverage script | verb loop over chapter 11 (see 50-14-PLAN Task 1) | ❌ | ⬜ pending |
| 50-14-T2 | 14 | 9 | FLT-01 (SC5) | T-50-SC | no dependency change since `0915792` | full suite | `scripts/verify.sh` ; `scripts/check_classic_goldens.sh` | ✅ (script) | ⬜ pending |

Every runnable command above has a `<fails_when>` in its plan; test-name filters additionally fail on `0 passed`. Parity gate after every src-editing plan: `cargo test -q --test drawer_classic_parity && scripts/check_classic_goldens.sh`.

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Classic-drawer golden snapshots committed on the untouched tree before any `src/` edit (none exist today — research finding 1) — plan 50-01 (wave 1); every other plan depends on it
- [ ] `scripts/check_classic_goldens.sh` immutability gate — plan 50-01
- [ ] Test files are created by the first task that needs them: `tests/phase50_herdr_drawer.rs` (50-02), `tests/phase50_agent_state.rs` (50-03), `tests/phase50_traffic.rs` (50-05), `tests/phase50_agents_window.rs` (50-07), `tests/phase50_gui_render.rs` (50-11), `tests/phase50_gui_runtime.rs` (50-13); `scripts/verify.sh` picks each up as a `t:` stage automatically
- [ ] Key-uniqueness tests: leader table (50-04, integration test on `bilterm::input::keymap`), herdr drawer keys (50-06) and agents window keys (50-09, scoped per window pane) as lib unit tests inside their private input modules, which carry the documented key-table exception (ARCH-R01)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| GUI sidebar scale/pin in a real window | FLT-01 (SC4) | `set_gui_surface` has no runtime caller until 50-13 (scope chosen at its checkpoint) | Launch `bilterm --gui`, `:drawer style herdr`, pin drawer (`*`), Ctrl+wheel over the drawer |
| GUI vector icons and animated graph look smooth | FLT-01 (D-05, D-10) | visual quality and animation feel | 50-11 Task 3 `<human-check>` |
| Win32 / macOS Ctrl+wheel panel scale | FLT-01 (SC4) | cfg-gated code cannot run on the Linux build host | 50-13 Task 3 `<human-check>` (only if `fix-all`) |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies (only 50-13-T1 is a checkpoint)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
