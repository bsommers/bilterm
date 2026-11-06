---
phase: 51-ui-tweaks-and-command-parity
plan: 13
subsystem: gates, docs, planning
tags: [final-gate, doc-coverage, bookkeeping, UIT-01, UIT-02, UIT-03, UIT-04]
requires: ["51-01..12"]
provides:
  - "scripts/check_phase50_doc_coverage.sh covers Phase 51 sub-forms (DOCS OK 91)"
  - "AGENTS.md: 13 MCP tools, Phase 50.1 and Phase 51 source files in the architecture map"
  - "ROADMAP/REQUIREMENTS/STATE: Phase 50.1 (15/15) and Phase 51 (13/13) complete; FND-R01..04, UIT-01..04 ticked"
  - "51-FOLLOWUPS.md"
key-files:
  modified:
    - scripts/check_phase50_doc_coverage.sh
    - src/colon_command/execute_agents.rs
    - AGENTS.md
    - .planning/ROADMAP.md
    - .planning/REQUIREMENTS.md
    - .planning/STATE.md
  created:
    - .planning/phases/51-ui-tweaks-and-command-parity/51-FOLLOWUPS.md
decisions:
  - "UNI-R01 stays open: its 50.1 and 51 parity matrices are delivered, but the rule applies from 50.1 on and 51.1 has not shipped; traceability row reads In progress"
  - "Phase base for the dependency gate checked against both 23b5940 (plan text) and 31ad21b (orchestrator brief): both clean"
status: complete
plan_head_before: 580fbca3e59eeeac948e8d8efa15d7ec57f42516
plan_head_after: 3e0e61b
commits: 2
actuals:
  tokens: 6000
  tasks: 2
  commits: 2
---
# Phase 51 Plan 13: Final gate Summary

Whole tree is green, Legacy goldens are byte-identical, the doc gate covers the Phase 51 sub-forms, and Phase 50.1 and Phase 51 are closed in ROADMAP, REQUIREMENTS and STATE. The SUMMARY commit is a third, separate docs commit after `plan_head_after`.

## Commits
| Commit | What |
|--------|------|
| 170d672 | doc gate phrases, stale messaging notice (Phase 51 -> Phase 52), AGENTS.md tool count and file map |
| 3e0e61b | ROADMAP, REQUIREMENTS, STATE, 51-FOLLOWUPS.md |

## Gate evidence (verbatim, `CARGO_TARGET_DIR=/tmp/bt-target-integ`, run after all code edits)
- `scripts/verify.sh`: `PASS 89/89 59s`
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`
- `bash scripts/check_phase50_doc_coverage.sh | tail -1`: `DOCS OK 91` (was 86 before this plan; 5 phrases added)
- `bash scripts/check_phase50_1_doc_coverage.sh | tail -1`: `DOCS OK 33`
- `cargo clippy -q --all-targets -- -D warnings`: no output, exit 0
- `git diff --exit-code 31ad21b -- Cargo.toml Cargo.lock`: exit 0; same against `23b5940`: exit 0
- `ls target/verify | grep -c phase51`: 12 stages
- `handle_wm_key_event`: 53 lines (<= 80)
- `src/**/*.rs` over 1000 lines: `src/wm/drawer.rs` 1071, `src/wm/agents_window.rs` 1177 (both pre-existing, noted); none over 1200; no newly added src file over 1000 (largest new: see `git diff --name-status 31ad21b HEAD -- src`, all `A` files under 1000)
- `tests/snapshots/drawer_classic_parity__*` unchanged vs `31ad21b`

## Deviations from Plan
1. **[Doc] Doc gate count.** The plan expected more than 77 (57 + 20). The count at the start of this plan was already 86 (the 51-11 chapter work), so the phrase additions take it to 91; the 20 new `:agents` entries are derived from the verb list and were already counted.
2. **[Scope, orchestrator request] AGENTS.md** MCP line 202 `11 tools` -> `13 tools` (names `bilterm_agents_list` / `bilterm_agents_demo`), architecture section gained two bullets for the Phase 50.1 Find files and the Phase 51 files; convention 6 (checklist item 6) untouched. The "48-file" wording was not changed (173 files now; recorded in follow-ups).
3. **[Doc] Stale string.** `Agent messaging is not enabled (Phase 51)` -> `(Phase 52)`; no test or doc pinned it (`grep -rn` over src, tests, docs).
4. **[Bookkeeping] UNI-R01** is not ticked; its traceability row now says `In progress (50.1 and 51 parity matrices delivered; 51.1+ pending)`. Checkbox unchanged.
5. **STATE.md** was far behind (current phase 46); updated `current_phase`, focus line, one decision and session fields, plus a metric row via `gsd-tools`. The tool left progress counters at stored values (milestone-heading mismatch warning); they were not hand-edited.
6. No residual integration failures: full run was green on first execution, so no catalog-sweep skip rows, no other fixes.

## Follow-ups
Full list in `.planning/phases/51-ui-tweaks-and-command-parity/51-FOLLOWUPS.md`: docked traffic window effective-area (`agents_workspace`, `move_agents_window`), `:arrange` overwriting the no-room dock notice, `drawer.rs` 1071 / `agents_window.rs` 1177 lines, tests reading the real `XDG_STATE_HOME` outside `verify.sh`, `:ssh-tmux` pane labelled with the remote host (51.1), palette Enter prefill (D-02), const `Arrange` Action for a chord, optional data-only `:agents status`, stale "48-file" count and `docs/architecture.md:87`.

Contract deviations recorded by plans: exception 6 is stale (bare `:agents`, `ls`, `status`, `list` all open the window; `:agents list status` is a usage error), 14 (not 13) theme-preset exclusions, `:window theme prev` has a palette entry (51-04, 51-11).

## Known Stubs
None.

## Threat Flags
None. T-51-31 (no check weakened: no fixes needed), T-51-32 (Cargo.toml and Cargo.lock unchanged vs both bases), T-51-SC (no install tasks).

## Self-Check: PASSED
Commits 170d672 and 3e0e61b are ancestors of HEAD; `51-FOLLOWUPS.md` exists; ROADMAP shows 13 ticked `51-NN` plan lines, REQUIREMENTS shows `UIT-01..04` ticked and `Complete` in the traceability table.
