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
| 50-05-T1 | 05 | 3 | FLT-01, FLT-02 (D-01) | T-50-12 | bounded ring | integration | `cargo test -q --test phase50_traffic bus_tracer` | ❌ (created by task) | ⬜ pending |
| 50-05-T2 | 05 | 3 | FLT-01, FLT-02 (D-01) | T-50-11, T-50-13, T-50-14 | password-prompt redaction, sanitised previews, origin recorded | integration (incl. RPC dispatch) | `cargo test -q --test phase50_traffic bus_` | ❌ | ⬜ pending |
| 50-06-T1 | 06 | 4 | FLT-01 (SC3) | T-50-16 | herdr keys are keymap.rs rows run via `wm.apply`; classic keys unchanged | integration | `cargo test -q --test phase50_herdr_drawer keys_tracer` | ❌ | ⬜ pending |
| 50-06-T2 | 06 | 4 | FLT-01 (SC3) | T-50-16, T-50-17 | rename via existing validated path; `:drawer close` keeps its minimize meaning, pane close is `:drawer close-pane`; no direct `wm` mutation in the input module | integration + drift + source gate | `cargo test -q --test phase50_herdr_drawer keys_` ; `cargo test -q --test phase50_herdr_drawer keymap_herdr` ; `cargo test -q --test phase50_herdr_drawer drawer_nav_ops_` ; drawer_herdr.rs `wm.` allow-list gate ; `cargo test -q --lib` | ❌ | ⬜ pending |
| 50-06-T3 | 06 | 4 | FLT-01 (SC1) | — | animation only while herdr is visible | integration (GUI surface for scale) | `cargo test -q --test phase50_herdr_drawer` | ❌ | ⬜ pending |
| 50-07-T1 | 07 | 4 | FLT-01, FLT-02 (D-07, D-10) | T-50-18 | labels sanitised | integration | `cargo test -q --test phase50_agents_window window_tracer` | ❌ (created by task) | ⬜ pending |
| 50-07-T2 | 07 | 4 | FLT-01, FLT-02 (D-08, D-10) | T-50-19 | node cap 64, fixed iterations | unit + snapshot | `cargo test -q --test phase50_agents_window` | ❌ | ⬜ pending |
| 50-08-T1 | 08 | 5 | FLT-01, FLT-02 (D-07) | T-50-20, T-50-34 | read-only view over RPC; Esc lowers to `AgentsOp::Dismiss`; leader never trapped | integration | `cargo test -q --test phase50_agents_window window_tracer` | ❌ | ⬜ pending |
| 50-08-T2 | 08 | 5 | FLT-01, FLT-02 (D-07, D-08) | T-50-34, T-50-37 | every claimed key is a keymap.rs row run via `wm.apply`; no row rebinds a global chord; every op has a `:agents` twin | integration + drift + source gate | `cargo test -q --test phase50_agents_window` ; agents_window.rs `wm.` allow-list gate | ❌ | ⬜ pending |
| 50-15-T1 | 15 | 6 | FLT-01, FLT-02 (SC4) | T-50-15 | herdr hit-test runs before classic column zones; click lowers to `Row` + `Activate` | integration | `cargo test -q --test phase50_herdr_drawer mouse_tracer` | ❌ | ⬜ pending |
| 50-15-T2 | 15 | 6 | FLT-01, FLT-02 (SC3, SC4, D-07) | T-50-15, T-50-35 | only rendered buttons act; non-button header/footer cells create nothing | integration | `cargo test -q --test phase50_herdr_drawer mouse_` | ❌ | ⬜ pending |
| 50-15-T3 | 15 | 6 | FLT-01, FLT-02 (SC4) | T-50-15 | edge drag lowers to `DrawerSize`; one bookkeeping `drawer_mut()`; key/mouse parity | integration + parity + source gate | `cargo test -q --test phase50_herdr_drawer` ; drawer_herdr.rs `wm.` allow-list gate | ❌ | ⬜ pending |
| 50-09-T1 | 09 | 6 | FLT-01, FLT-02 (D-07) | T-50-22 | in-window events swallowed; title drag lowers to `SetRect` | integration | `cargo test -q --test phase50_agents_window window_tracer` | ❌ | ⬜ pending |
| 50-09-T2 | 09 | 6 | FLT-01, FLT-02 (D-07) | T-50-21 | rect/size/splitter clamps | integration + drift | `cargo test -q --test phase50_agents_window window_` ; `cargo test -q --test phase50_agents_window keymap_agents_window` ; `cargo test -q --test phase50_agents_window agents_ops_` | ❌ | ⬜ pending |
| 50-09-T3 | 09 | 6 | FLT-01, FLT-02 (D-08) | T-50-21 | permille clamp; key/mouse/colon parity; no direct `wm` mutation | integration + parity + source gate | `cargo test -q --test phase50_agents_window` ; agents_window.rs `wm.` allow-list gate | ❌ | ⬜ pending |
| 50-16-T1 | 16 | 7 | FLT-01, FLT-02 (D-07) | T-50-36 | pin badge lowers to `AgentsOp::Pin` | integration + drift | `cargo test -q --test phase50_agents_window window_tracer` ; `cargo test -q --test phase50_agents_window agents_ops_` | ❌ | ⬜ pending |
| 50-16-T2 | 16 | 7 | FLT-01, FLT-02 (SC4, D-07) | T-50-36 | scale clamp 75..=200; Ctrl+wheel lowers to `ScaleStep` | integration (GUI surface) + parity + source gate | `cargo test -q --test phase50_agents_window` ; agents_window.rs `wm.` allow-list gate | ❌ | ⬜ pending |
| 50-10-T1 | 10 | 8 | FLT-01, FLT-02 (D-09) | T-50-24 | read-only; redacted stays redacted on copy; focus cycle is Ctrl+O (F6 stays global) | integration + drift | `cargo test -q --test phase50_agents_window window_tracer_snoop` ; `cargo test -q --test phase50_agents_window keymap_agents_window` | ❌ | ⬜ pending |
| 50-10-T2 | 10 | 8 | FLT-01, FLT-02 (D-07, D-09) | T-50-25 | keyword is substring, capped | integration | `cargo test -q --test phase50_agents_window snoop_` | ❌ | ⬜ pending |
| 50-10-T3 | 10 | 8 | FLT-01, FLT-02 (D-09) | T-50-23 | no outward write path (no pane bytes, no bus event, `ComposeSubmit` has no colon form) | integration + parity + source gate | `cargo test -q --test phase50_agents_window compose_` ; `cargo test -q --test phase50_agents_window parity_snoop` ; agents_window.rs `wm.` allow-list gate | ❌ | ⬜ pending |
| 50-11-T1 | 11 | 8 | FLT-01, FLT-03 (D-10) | T-50-27 | disjoint texture ids | integration (GuiFrameSurface) | `cargo test -q --test phase50_gui_render render_gui_tracer` | ❌ (created by task) | ⬜ pending |
| 50-11-T2 | 11 | 8 | FLT-01, FLT-03 (D-10) | T-50-26 | texture dims capped | integration + digest snapshot | `cargo test -q --test phase50_gui_render render_gui_` | ❌ | ⬜ pending |
| 50-11-T3 | 11 | 8 | FLT-01, FLT-03 (D-05) | T-50-26 | — | integration + human-check | `cargo test -q --test phase50_gui_render` | ❌ | ⬜ pending |
| 50-13-T1 | 13 | 8 | FLT-01 (SC4) | — | human scope decision; the SC4 consequence of each option is stated in 50-14 and ROADMAP | checkpoint:decision | n/a (checkpoint, `gate="blocking-human"`) | — | ⬜ pending |
| 50-13-T2 | 13 | 8 | FLT-01 (SC4, D-07) | T-50-30 | Ctrl+wheel re-routed only inside panel rects | unit | `cargo test -q --test phase50_gui_runtime gui_runtime_` | ❌ (created by task) | ⬜ pending |
| 50-13-T3 | 13 | 8 | FLT-01 (SC4) | T-50-31 | — | unit + cross-target compile + human-check | `cargo check -q --lib --target x86_64-pc-windows-gnu && cargo check -q --lib --target aarch64-apple-darwin` | ❌ | ⬜ pending |
| 50-12-T1 | 12 | 9 | FLT-01, FLT-02 (D-06, D-07) | T-50-28 | partner label sanitised; glyph click lowers to `Row` + `OpenComms` | integration | `cargo test -q --test phase50_herdr_drawer comms_tracer` | ❌ | ⬜ pending |
| 50-12-T2 | 12 | 9 | FLT-01, FLT-02 (D-06) | T-50-28, T-50-29 | throttled animation redraw; comms keys are keymap.rs rows | integration + drift + source gate | `cargo test -q --test phase50_herdr_drawer comms_` ; `cargo test -q --test phase50_herdr_drawer keymap_herdr` ; drawer_herdr.rs `wm.` allow-list gate | ❌ | ⬜ pending |
| 50-14-T1 | 14 | 10 | FLT-01, FLT-02, FLT-03 | T-50-32 | — | doc coverage script (verb lists derived from src/pane/completion.rs: every `:agents` row verb, every `:drawer` verb added since `0915792`) | `bash scripts/check_phase50_doc_coverage.sh` | ❌ (created by task) | ⬜ pending |
| 50-14-T2 | 14 | 10 | FLT-01 (SC4 per 50-13, SC5), FLT-02, FLT-03 | T-50-SC | no dependency change since `0915792` | full suite + doc coverage | `scripts/verify.sh` ; `scripts/check_classic_goldens.sh` ; `bash scripts/check_phase50_doc_coverage.sh` | ✅ (script) | ⬜ pending |

Every runnable command above has a `<fails_when>` in its plan; test-name filters additionally fail on `0 passed`. Parity gate after every src-editing plan: `cargo test -q --test drawer_classic_parity && scripts/check_classic_goldens.sh`. The `wm.` allow-list gate is `test -f <module> && ! tr -d ' \t\n' < <module> | grep -oE 'wm\.[a-z_]+\(' | sort -u | grep -vxE 'wm\.(<allowed>)\('` with the per-plan allow-list written in that plan's `<verify>`; it fails when an input module changes WindowManager state without an Action.

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Classic-drawer golden snapshots committed on the untouched tree before any `src/` edit (none exist today — research finding 1) — plan 50-01 (wave 1); every other plan depends on it
- [ ] `scripts/check_classic_goldens.sh` immutability gate — plan 50-01
- [ ] Test files are created by the first task that needs them: `tests/phase50_herdr_drawer.rs` (50-02), `tests/phase50_agent_state.rs` (50-03), `tests/phase50_traffic.rs` (50-05), `tests/phase50_agents_window.rs` (50-07), `tests/phase50_gui_render.rs` (50-11), `tests/phase50_gui_runtime.rs` (50-13); `scripts/verify.sh` picks each up as a `t:` stage automatically
- [ ] Key tables live in `src/input/keymap.rs` (AGENTS.md Conventions item 1) and are tested as integration tests over `bilterm::input::keymap`: leader uniqueness (50-04); `HERDR_DRAWER_BINDINGS` uniqueness + no-shadow-of-globals (50-06, extended 50-12); `AGENTS_WINDOW_BINDINGS` scoped uniqueness + no-shadow-of-globals (50-08, extended 50-09, 50-16, 50-10)
- [ ] Action-lowering drift tests: `drawer_nav_ops_have_colon_completion_and_help` (50-06) and `agents_ops_have_colon_completion_and_help` (50-08), each driven by an exhaustive no-wildcard `match` helper so every new `DrawerNavOp` / `AgentsOp` variant must get a colon form, a completion verb and a help row (documented exemptions: `KeywordEdit`, `ComposeSubmit`, 50-10)
- [ ] Key/mouse parity tests: `keys_colon_equivalents_match_keys` (50-06), `parity_herdr_key_and_mouse_paths_match` (50-15), `window_keys_match_colon_forms` (50-08), `parity_window_geometry_key_mouse_colon_match` (50-09), `parity_pin_scale_key_mouse_colon_match` (50-16), `parity_snoop_key_mouse_colon_match` (50-10)

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
