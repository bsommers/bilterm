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

## 45-04 bundled font: DONE (committed)
- JetBrains Mono v2.304 (OFL) bundled in assets/fonts/ (download worked on retry). Loader: src/ui/gui_font/bundled.rs; called from all three platform canvases (linux_x11/win32_gdi/macos_ct `new()`) via `install_if_wanted(wm.config())`; installs to private runtime dir only when AppearanceProfile::resolve(cfg) != Legacy. Legacy font resolution unchanged.
- Verified: 3 new unit tests, verify.sh PASS 31/31. NOT verified: Windows/macOS compile of the one-line call sites, and live GUI rendering (needs `bilterm --gui` with [appearance] profile=obsidian).
- Next: Phase 46 (shell/Zen/`:appearance`, Obsidian default for fresh installs only). Make non-Legacy profiles select the "JetBrains Mono" family explicitly in sync_font (currently only resolves via existing candidate list / Ibm3270 default).

## Phase 46 progress (2026-11-02)
- Done+committed: c5a95f9 font default, 2456157 `:appearance`, 1fe1227 fresh-install Obsidian. verify PASS 31/31.
- Design doc written (uncommitted until review): .planning/phases/46-shell-display-modes/DESIGN.md. Awaiting user answers to its section 8 (5 questions) before building 46-04..46-09.
- `:appearance` is session-only (no config write path exists). User's ~/.config/bilterm/config.toml exists, so fresh-install default does not apply to them.
- Commits end with: Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
- 46-04 done: `:mode standard|minimal|zen` extends the EXISTING `:mode` (prompt mode float/fixed/terminal/cycle, unchanged) in parse.rs. DesignDoc answers: all defaults accepted. Session-only; renders nothing yet (46-05/06 do layout). Next: 46-05 ShellLayout extraction with Legacy parity tests.
- 46-05 done: src/ui/shell.rs `ShellLayout::compute` is the single offset source for ui/mod.rs, renderer.rs (GUI) and input/mouse.rs. Parity oracle test over sizes/flags. NOT touched: WorkspaceFrameSnapshot::capture (renderer.rs ~54) applies only the bottom bar and ignores the top menubar (pre-existing quirk). GSD plan files 46-01..03 written. Next: 46-06 (Standard/Minimal/Zen layouts, Zen input row + notices).
- 46-06 done: shell::effective_display_mode (Legacy => Standard). Zen (non-Legacy): compute_workspace_rects returns only the active pane at full area (also drives PTY resize + mouse), is_framed()=false, bottom row = shell::render_zen_row (notices only, no OIA). Minimal(Focus): frame title text blank. tests/phase46_display_modes.rs (4 tests incl. Legacy byte-identical across modes). verify PASS 32/32.
- GAP vs Phase 44 decision "Zen keeps a one-line input": prompt box keeps its normal 3+ row bordered look in Zen. 46-06b = compact borderless 1-row prompt (needs PromptEditor effective_height/prompt_rect/canvas_rect changes + render). Also resize_all in wm/mod.rs has a 5th copy of the offset math (different thresholds) not migrated to ShellLayout. Top bar absent until 46-07.
- GSD tracking (2026-11-02, user request): backfilled PLAN+SUMMARY for 43-01, 44-01, 45-01..04 (phases/45-design-system/), GAPS.md for Phase 45, ROADMAP/REQUIREMENTS (UIM-00/01 ticked; UIM-02 open), STATE.md now v2.6.0 / Phase 46. RULE: after each Phase 46+ step, update its PLAN status line, ROADMAP counts and STATE.md stopped_at in the same commit.
- 46-06b: no code. Default prompt height 3 = one text row in a border, so Zen already has a one-line input; earlier "GAP" note was wrong. Borderless row = optional polish, needs mouse.rs/pin/resize/render changes. Next: 46-07 Navigator + top-bar pills.
- 2026-11-02 user request inserted as phases/46-shell-display-modes/46-04-PLAN.md (drawer + :mcp resize/scale/h-scroll/v-scroll). 46-04d (font scale) needs a user decision on GUI per-panel scaling. Order: 46-07 first, then 46-04a-c.
- 46-07 done: src/ui/navigator.rs (pills from wm.panes(); click -> focus_and_raise; +n overflow), ShellLayout::compute(area,cfg,pane_count) with TopBarKind. Pills ONLY for an explicit [appearance].profile != Legacy (old --modern/--liquid-glass flags keep their layout: a drawer test caught this). resize_all adds the pill row. Not yet: pill state dots for Busy are colour only; right-side badges; session/drawer items in pills. Next: 46-04a (vertical scroll) per 46-04-PLAN.md, then 46-08.
