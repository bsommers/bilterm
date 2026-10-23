# Session Checkpoint (Phase 19 planning IN PROGRESS)

**Updated:** `2026-10-23` (Phase 19 `/gsd-plan-phase` session)
**Branch:** `main` @ `8083618` (local, not pushed). All Phase 19 planning docs committed. No background jobs.

## 0. Phase 19 planning state (RESUME HERE)
- **GSD runtime:** GSD is only installed for Gemini. The user chose "Adapt Gemini install":
  - tools: `node ~/.gemini/config/plugins/gsd-core/gsd-core/bin/gsd-tools.cjs`
  - workflow: `~/.gemini/config/plugins/gsd-core/gsd-core/workflows/plan-phase.md`
  - roles: `~/.gemini/config/plugins/gsd-core/agents/gsd-*.md`, run as general-purpose subagents that are told to read their role file
- **Done:**
  - research (`19-RESEARCH.md`), validation (`19-VALIDATION.md`), context (`19-CONTEXT.md`, D-01..D-06 plus two user amendments), all committed in 59c65cf
  - patterns (`19-PATTERNS.md`)
  - 7 plans: 19-01 secure_fs (W1); 19-02 ssh, 19-03 temp files, 19-04 listener/auth (W2); 19-05 ppm confinement (W3); 19-06 CLI (W4); 19-07 docs/REQUIREMENTS (W5)
  - checker iteration 1 found 2 blockers and 5 warnings. All are fixed, and a planner revision pass is done.
- **User decisions this session (verbatim choices):**
  - token: "auth.login handshake"
  - playbooks: "Playbooks unconfined", later amended to "Confine RPC playbooks" (CLI `--playbook` stays trusted)
  - snapshot dir: "$XDG_STATE_HOME + flag"
  - loopback: "Token optional on loopback"
  - ppm names: "Bare file names only"
- **Things to tell the user at the end:**
  - Breaking changes: `ctl` defaults to the Unix socket instead of `127.0.0.1:7370`, and the SSH builders take `control_dir` and return `Result`.
  - Accepted risk T-19-13: RPC `wm.command` can still write to arbitrary paths via `:record start <path>` and `:history save <path>`. This is out of scope.
- **Planner contributions file:** the session scratchpad copy is gone. Regenerate it with `node <gsd-tools> loop render-hooks plan:pre --raw`, taking the `activeHooks` entries where kind is contribution and into is planner. This matters only if a revision pass needs it.
- **Skipped gates (tooling missing):** the UI gate (`ui plan-gate` command doesn't exist; the phase has no UI); the spec-less §A probe was run by the planner.
- **Next:**
  1. Spawn the plan-checker for iteration 2 of 3 (haiku). The prompt is the same as iteration 1: plans plus ROADMAP, REQUIREMENTS, CONTEXT, RESEARCH and VALIDATION; SEC-R01..04; ASVS L1 block-on-high.
  2. Then workflow steps 13 (requirements and decision coverage), 13b (STATE.md), 13c, and 13d (commit the plans).
  3. Then present the status and `/gsd-execute-phase 19`.
- **Context-guard fix:** `.claude/settings.local.json` sets `SWE_SKILLS_CHECKPOINT_GLOBS` to include `.planning/CHECKPOINT.md`. The default globs had missed it. The nudge fired at 71%, which shows the hook works.

---

## (prior) v1.6.0 release checkpoint

**Updated:** `2026-10-23`
**Branch:** `main` @ `ac7d82b` (= tag `v1.6.0`, synced with `origin/main`)
**Release:** https://github.com/bsommers/bilterm/releases/tag/v1.6.0 (PR #1 merged by the user after review)
**Baseline verification:** `cargo clippy --all-targets -- -D warnings` clean; `cargo test` 127 passed, 0 failed; `cargo audit` 0 vulnerabilities (4 inherited warnings: `lru`, `paste` via `vt100-ctt`/`ratatui-core`).

## 1. User decisions (verbatim, this session)
- Architecture review → "yes. do all 1,2,3" (fix P0s, dependency audit, P1 → GSD phases 19–23).
- "1 - yes" (MSRV → 1.88), "2 - wire-up ghostty-vt feature…", "3 - commit and push".
- "execute this as autonomously as possible … conserve tokens, pass minimal sufficient context to subagents".
- Added persistent session-segmented history and dynamic RAM-scaled scrollback (both shipped in v1.6.0).
- "do 1 - open the PR then I'll review it" → PR #1; the user merged it. Then the tag, push, and `gh release create v1.6.0`.
- "add binaries for linux" and asked whether macOS Apple Silicon can be cross-compiled from Linux x86 (see §3).
- **Next:** the user will run `/gsd-plan-phase 19`.

## 2. Phase 19 context (Security Hardening). Source: `.planning/ROADMAP.md` and `.planning/reviews/ARCHITECTURE_REVIEW_PLAN.md` §5.1
- `--headless --listen` binds any address, with no auth (`src/api/mod.rs` `run_headless_tcp_server` / `serve_jsonrpc`). Target: a `0600` Unix socket in `$XDG_RUNTIME_DIR` by default; non-loopback TCP only with `--allow-remote` plus a bearer token from a `0600` file. (`--help` already claims Unix socket support, but it isn't implemented.)
- `workspace.snapshot` `ppm_path` allows an arbitrary file write (`dispatch_method`). Confine it to an output directory and test that traversal is rejected.
- Predictable `/tmp` paths: shell-integration rc files (`src/pty/shell_integration.rs` `shell_integration_root_dir`), history temp exports (`src/pane/history_store.rs` `export_to_temp_file`), `.cast` recordings (`src/pane/colon_command.rs`), and SSH `ControlPath=/tmp/bilterm-ssh-%r@%h:%p` (`src/remote/mod.rs`). Move them to a per-user `0700` directory and use `O_EXCL` writes.
- SSH option injection: `build_ssh_args` / `build_ssh_tmux_cc_args` push the destination without `--`. Reject a leading `-` and emit `--` before the destination.
- The P0 hardening is already done: per-connection RPC tasks, 1 MiB line cap, JSON depth 128.

## 3. Release binaries (DONE: 4 tarballs + SHA256SUMS uploaded to the v1.6.0 release)
- Built with `cargo zigbuild --release`, using Zig 0.15.2 from the session scratchpad (not persistent). Targets: `x86_64-unknown-linux-gnu.2.28`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `aarch64-apple-darwin`.
- The macOS binary is ad-hoc signed by zig's linker (`LC_CODE_SIGNATURE` present) but not notarized, so users need `xattr -d com.apple.quarantine`. It hasn't been run on a real Mac yet.

## 4. Known open items (carried)
- `cargo fmt --check` already failed on `main` before this work (pre-existing).
- Scrollback:
  - Rebuild runs on the UI thread, so a pane of about 1M lines can stall it.
  - A fast flood can drop the oldest lines before the 2 s rebalance.
  - DECSTBM isn't restored after a rebuild.
  - The worker and ghostty paths keep fixed capacity.
- History: old per-run logs aren't migrated; two processes sharing one session can lose a line during compaction.
- Ghostty: `zig_0_15` in `flake.nix` hasn't been checked with nix; palette-indexed underline colours map to `None`.
- The test suite rewrites `docs/assets/screenshot-3278-inspector.svg`. Revert it before committing (`git checkout -- docs/assets/screenshot-3278-inspector.svg`).
- Local and remote branch `fix/architecture-review-p0` still exist (merged); safe to delete.
- The repo has no CI workflows.

## 5. Resume command
`git switch main && git pull --ff-only && cargo test && cargo clippy --all-targets -- -D warnings` → then `/gsd-plan-phase 19`
