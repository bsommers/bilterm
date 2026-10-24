---
phase: 21
slug: encapsulated-workspace-invariants
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-10-23
---

# Phase 21 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `#[test]` + `#[tokio::test]` + `insta` snapshot assertions |
| **Config file** | `Cargo.toml` |
| **Quick run command** | `cargo test --lib <module>` (`pane`, `wm`, `input`, `api`, `colon_command`, `ui`) |
| **Full suite command** | `cargo test && cargo clippy --all-targets -- -D warnings` |
| **Estimated runtime** | ~15 seconds incremental |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib` and `cargo clippy --all-targets -- -D warnings`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd-verify-work`:** Full suite + private-field gate + line-count gate must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Behavior | Test Type | Automated Command | Status |
|---------|------|------|-------------|----------|-----------|-------------------|--------|
| 21-01-01 | 01 | 1 | ARCH-R02 | Make `CompositePane` fields private, remove `pane.leader_pending`, add `new_base`, `submit_command`, `submit_prompt`, and method accessors | unit | `cargo test --lib pane::` | ⬜ pending |
| 21-01-02 | 01 | 1 | ARCH-R02 | Update `app`, `api`, `input`, `colon_command`, `ui`, `wm`, `main`, and `tests/` to use `CompositePane` methods only | unit + integration | `cargo test && cargo clippy --all-targets -- -D warnings` | ⬜ pending |
| 21-02-01 | 02 | 2 | ARCH-R02 | Introduce `enum Modal`, `enum Focus { Quick, Pane(PaneId) }`, `WindowManager::remove_pane`, and make `WindowManager` fields private | unit | `cargo test --lib wm::` | ⬜ pending |
| 21-02-02 | 02 | 2 | ARCH-R02 | Update `app`, `api`, `input`, `colon_command`, `ui`, `main`, and `tests/` to use `WindowManager` methods, `Modal`, and `Focus` | unit + integration | `cargo test && cargo clippy --all-targets -- -D warnings` | ⬜ pending |
| 21-03-01 | 03 | 3 | ARCH-R02 | Add invariant unit tests (reaping zoomed pane, fleet disconnect pane removal, `submit_command` parity, `Modal` exclusivity, `Focus` hit-testing) and update Phase 21 state | unit + full suite | `cargo test && cargo clippy --all-targets -- -D warnings && awk 'ENDFILE{if(FNR>800){print FILENAME,FNR; e=1}} END{exit e}' $(find src -name '*.rs')` | ⬜ pending |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify commands
- [x] Sampling continuity: every task has automated test + clippy verification
- [x] Wave 0 complete
- [x] No watch-mode flags
- [x] Feedback latency < 60s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-10-23
