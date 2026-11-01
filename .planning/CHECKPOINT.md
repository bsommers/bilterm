# Bilterm Session Checkpoint (2026-11-01 evening)

## User decisions (verbatim, still in force)
- Default Obsidian; bundled mixed-case mono font; Zen keeps one-line input (see .planning/phases/44-*/DECISIONS.md)
- "I want to be able to select between the Legacy/retro looks and Obsidian, even if Obsidian is the default. we are taking NOTHING away from the old looks. just adding new looks and capabilities. EXTENDING what we have."
- agy work is user-run unless user overrides (user overrode once, for the 45-03d/e rerun).

## State
- main @ 34ae639 (+ this commit). Tag ui-api-frozen. Version 2.6.0.
- 45-03a/b/c merged by agy; reviewed by Claude: oia.rs had a Legacy-parity regression, fixed in b64bbe3 (tokens only for palettes naming a non-Legacy profile).
- 34ae639: scripts/verify.sh sets XDG_STATE_HOME to a temp dir (tests used to read the real ~/.local/state/bilterm/sessions). verify = PASS 31/31.
- 45-03d/e: audited by Claude, zero migrations (see .planning/dispatch/45-03de-RESULT.md). 45-03 is DONE.
- RUN.sh now passes a session-only allow-list via --settings (literal prefixes `command(cat)` etc., NOT regex) and exits 1 on "no output produced". agy headless still denies some unknown command on larger tasks (theme-import hit it); the denied command is not logged. Probe tasks with `cat/grep/git/scripts/verify.sh` work.

## Next (Claude)
1. 45-04 bundled font; Phase 46 shell/Zen/`:appearance` + Obsidian default for fresh installs only (serial, sonnet; opus only for shell design).
2. Then agy: 47-01/02/03, Phase 48 tests (.planning/DISPATCH_PLAN.md). Before dispatching, find the missing allow-rule (try adding cd/echo/awk/python3/true) or run agy interactively.
3. Always review agy diffs for Legacy byte-identical parity before merge.
