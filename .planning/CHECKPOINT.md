# Bilterm Session Checkpoint (2026-11-01, post-freeze + verify.sh)

## User decisions (verbatim)
- Phase 44 sign-off: direction approved; default Obsidian; bundle mixed-case mono font; Zen keeps one-line input (see .planning/phases/44-*/DECISIONS.md)
- "I want to be able to select between the Legacy/retro looks and Obsidian, even if Obsidian is the default. we are taking NOTHING away from the old looks. just adding new looks and capabilities. EXTENDING what we have."
- "go straight through to the freeze, stop there for review" (done); design/*.rs approved by user.
- Ownership: Claude does NOT run agy-owned work; agy work is not run as Claude subagents unless user overrides.

## State
- Branch main, clean. Commits: c1da655, 67b9c64, ec30c3d, 9f7f9c3, e101949 (decisions+DISPATCH_PLAN), 7a1dfa9 (design API, tag ui-api-frozen), 286c709 (agy task texts).
- Artifact (proposal): https://claude.ai/artifact/DHtLf6cfQAUwST9vKqRL8v
- Unset profile resolves to Legacy for now; Phase 46 makes Obsidian default for fresh installs only.

## Waiting on user
- User runs 5 agy tasks: `.planning/dispatch/RUN.sh 45-03a oia | 45-03b status | 45-03c modals-tok | 45-03d cursor | 45-03e theme-import` (branches ui/<slug> in ../bilterm-<slug>). agy never commits.

## Next (Claude)
1. When user reports branches done: review each diff (spec then quality), run `scripts/verify.sh` (prints one line: `PASS n/n Ns` or `FAIL ...`; on FAIL read only `target/verify/<stage>/fail.txt`, then `log` if needed; never paste full logs), commit per branch, merge to main.
2. 45-04 bundled font; Phase 46 shell/Zen/`:appearance` command + Obsidian default for fresh installs (Claude, serial, sonnet; opus only for shell design).
3. Then agy: 47-01/02/03, Phase 48 tests (see .planning/DISPATCH_PLAN.md).

## Added after freeze
- `scripts/verify.sh` (7201ad1): one-line PASS/FAIL gate; on FAIL read only `target/verify/<stage>/fail.txt`. Flags --quick/--failed/--only/--list/--status. fmt opt-in (legacy drift). Documented in AGENTS.md + CLAUDE.md; used in all agy tasks.
- swe-skills-2 (1665094): layered-verification-gates, orchestrating-subagents, writing-agent-context-files updated for scripted gates.
- `.planning/dispatch/RUN.sh` has user's uncommitted edit (--model gemini-3.8-flash-medium); leave it.
- Still waiting: user reports which agy branches (ui/oia, ui/status, ui/modals-tok, ui/cursor, ui/theme-import) are done.
