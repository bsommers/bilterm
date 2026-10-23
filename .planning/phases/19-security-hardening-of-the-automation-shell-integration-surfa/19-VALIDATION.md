---
phase: 19
slug: security-hardening-of-the-automation-shell-integration-surfaces
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-10-23
---

# Phase 19 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `#[test]` + `#[tokio::test]` (insta for UI snapshots) |
| **Config file** | none (Cargo.toml; tempfile dev-dep) |
| **Quick run command** | `cargo test --lib <module>` (api / remote / secure_fs / shell_integration / history_store) |
| **Full suite command** | `cargo test && cargo clippy --all-targets -- -D warnings` |
| **Estimated runtime** | ~90 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib <module>`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 19-01-01 | 01 | 1 | SEC-R03 | T-19-01 | private dir 0700, rejects loose mode / symlink / non-dir / bad names | unit | `cargo test --lib secure_fs::tests::private_dir` | ❌ W0 (new src/secure_fs.rs) | ⬜ pending |
| 19-01-02 | 01 | 1 | SEC-R03, SEC-R01 | T-19-02, T-19-03, T-19-04 | O_EXCL+O_NOFOLLOW 0600 files; 0600 secret reader; constant-time eq | unit | `cargo test --lib secure_fs::` | ❌ W0 | ⬜ pending |
| 19-02-01 | 02 | 2 | SEC-R04, SEC-R03 | T-19-10, T-19-11 | `-o...` destination rejected; `--` before destination; ControlPath `<private>/ssh-%C` | unit + integration | `cargo test --lib remote:: && cargo test --test full_suite_battle_and_perf unit_exhaustively_covers_remote_profiles` | partial (update existing) | ⬜ pending |
| 19-02-02 | 02 | 2 | SEC-R04, SEC-R03 | T-19-10, T-19-01 | invalid destination creates no pane; session fallback private | unit | `cargo test --lib wm::` | ✅ (add tests) | ⬜ pending |
| 19-03-01 | 03 | 2 | SEC-R03 | T-19-01 | rc files 0600 in 0700 per-pid dir; planted symlink not followed | unit | `cargo test --lib shell_integration` | ✅ (add tests) | ⬜ pending |
| 19-03-02 | 03 | 2 | SEC-R03 | T-19-02, T-19-15 | default export O_EXCL 0600 in private dir; fallback dir private | unit | `cargo test --lib history_store` | ✅ (update asserts) | ⬜ pending |
| 19-03-03 | 03 | 2 | SEC-R03 | T-19-02 | default `.cast` exclusive 0600 in private dir | unit | `cargo test --lib vt:: && cargo test --lib colon_command` | ✅ (add tests) | ⬜ pending |
| 19-04-01 | 04 | 2 | SEC-R01 | T-19-05, T-19-03 | listen policy (unix default, loopback, --allow-remote + token); redacted Debug | unit | `cargo test --lib api::tests::listen_policy` | ✅ (add tests) | ⬜ pending |
| 19-04-02 | 04 | 2 | SEC-R01 | T-19-06, T-19-17, T-19-07, T-19-18 | 0600 socket, peer uid, stale/live probe, auth.login gate, abusive clients | tokio | `cargo test --lib api::tests` | ✅ (add tests) | ⬜ pending |
| 19-04-03 | 04 | 2 | SEC-R01 | T-19-03 | client auth.login; `unix:` endpoints | tokio | `cargo test --lib api::tests && cargo test --test v110_v120_v130_automation_api_mcp_playbooks` | ✅ | ⬜ pending |
| 19-05-01 | 05 | 3 | SEC-R02 | T-19-08, T-19-09 | ppm_path traversal/absolute/symlink rejected; RPC playbooks confined; CLI playbook unchanged | unit + integration | `cargo test --lib api::tests::snapshot && cargo test --test v110_v120_v130_automation_api_mcp_playbooks` | ✅ (add tests) | ⬜ pending |
| 19-05-02 | 05 | 3 | SEC-R04 | T-19-10 | fleet.register / :remote add reject `-` destinations | unit | `cargo test --lib api::tests::fleet_register && cargo test --lib colon_command` | ✅ (add tests) | ⬜ pending |
| 19-06-01 | 06 | 4 | SEC-R01, SEC-R02 | T-19-05, T-19-03 | binary refuses non-loopback w/o flags; socket roundtrip; ctl token flow | e2e (binary) | `cargo test --test phase19_security_cli` | ❌ W0 (new tests/phase19_security_cli.rs) | ⬜ pending |
| 19-06-02 | 06 | 4 | SEC-R01, SEC-R02 | T-19-12 | man page documents flags + SECURITY | e2e | `cargo test --test phase19_security_cli man_page_documents_new_flags` | ❌ W0 | ⬜ pending |
| 19-07-01 | 07 | 5 | SEC-R01..R04 | T-19-12, T-19-19 | docs show safe defaults | grep | `grep -q token-file README.md && grep -q allow-remote docs/running.md` | ✅ | ⬜ pending |
| 19-07-02 | 07 | 5 | SEC-R01..R04 | — | changelog + REQUIREMENTS define SEC-R IDs | grep | `grep -c "SEC-R0[1-4]" .planning/REQUIREMENTS.md` (>= 8) | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/secure_fs.rs` `mod tests` — created RED-first by 19-01 (SEC-R03, SEC-R01 token primitive); base paths injected via `tempfile::tempdir()`, no env mutation
- [ ] `tests/phase19_security_cli.rs` — created RED-first by 19-06 (binary-level SEC-R01/R02 policy)
- [ ] Update existing assertions in `tests/full_suite_battle_and_perf.rs` (builder signatures) and `src/pane/history_store.rs` tests (export path) — 19-02 / 19-03
- Framework: none to install (Rust test harness + `tempfile` dev-dep already present)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Foreign-owner rejection of a pre-existing private dir | SEC-R03 | creating a dir owned by another uid needs root; tests cover mode + symlink | as root: create `$D/bilterm` mode 0700, `chown nobody` it, run `XDG_RUNTIME_DIR=$D bilterm --headless` as the normal user; expect a PermissionDenied error and no socket/files created |
| Windows/macOS cfg gates | SEC-R01, SEC-R03 | toolchains not available here | `cargo check --target x86_64-pc-windows-gnu` / macOS build if available |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** {pending / approved YYYY-MM-DD}
