---
phase: 1
slug: core-pty-vt100-engine-single-compositepane-prototype
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-10-19
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` + `insta` (`ratatui::backend::TestBackend`) |
| **Config file** | `Cargo.toml` |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test --all-targets` |
| **Estimated runtime** | ~3 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test --all-targets && cargo clippy --all-targets -- -D warnings`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 1-01-01 | 01 | 1 | PTY-01 | T-1-01 | Safe PTY process spawn and terminal state restoration on panic/drop | integration | `cargo test --test pty_lifecycle` | ✅ | ⬜ pending |
| 1-01-02 | 01 | 1 | PTY-02 | — | Deterministic VT100/ANSI/TrueColor & DECSET 47/1047/1049 tracking | unit | `cargo test vt::` | ✅ | ⬜ pending |
| 1-02-01 | 02 | 2 | PTY-03 | T-1-02 | `!ECHO` password prompt & sub-REPL detection never echoes secrets into floating buffer | unit | `cargo test line_discipline` | ✅ | ⬜ pending |
| 1-02-02 | 02 | 2 | PANE-01, PTY-02 | — | `CompositePane` auto-hides prompt on alt-screen/raw mode and emits zero `SIGWINCH` on prompt move/hide | unit | `cargo test pane::` | ✅ | ⬜ pending |
| 1-03-01 | 03 | 3 | PANE-01, PTY-01 | — | `TestBackend` + `insta` snapshot verification of full-height `OutputCanvas` + overlaid `FloatingCommandPrompt` + `OiaStatusBar` | snapshot | `cargo test --test composite_pane_snapshots` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `Cargo.toml` — Workspace dependencies (`ratatui`, `crossterm`, `portable-pty`, `vt100-ctt`, `tokio`, `libc`, `color-eyre`, `tracing`, `insta`)

---

## Manual-Only Verifications

All phase behaviors have automated verification via headless `portable-pty` shells and `ratatui::backend::TestBackend` snapshots.

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 10s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-10-19
