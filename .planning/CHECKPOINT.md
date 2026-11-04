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
- 46-04b MCP part done (h-scroll in McpConsoleState.h_scroll, clip_row). Open for 46-04: drawer h-scroll (src/ui/drawer.rs rows), drawer wheel viewport scroll, MCP resize (+ `:mcp size`), drawer clamp 16..60, font-scale decision (46-04d, needs user OK).

## NEXT SESSION START HERE (2026-11-02, after 7a35f65)
User decision (verbatim): "yes, build GUI-only per-panel scaling, then finish 46-04". Standing rule: "we are taking NOTHING away from the old looks." agy is user-run; Claude does Phase 46 serially. Commits end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`. After every step: scripts/verify.sh, update the PLAN status line + ROADMAP + STATE.md + this file in the same commit.
Done on main: 46 steps :appearance, :mode, ShellLayout, Zen/Minimal, navigator pills (explicit profile only), MCP h-scroll. verify PASS 32/32, tree clean.
1. DONE: 46-04d GUI panel scale (`:panel scale 75..200`; wm.panel_scale_pct() is 100 off-GUI; renderer draws drawer/MCP via a virtual workspace area; mouse remapped by wm.panel_space). Drawer h-scroll (h/l, </>, wheel L/R) and clamp 16..100 done. NOT verified in a live GUI.
2. DONE (drawer also scrolls via Left/Right and Shift+wheel like MCP): 46-04, 46-08 (title/focus_ring, :chrome), 46-09 (pane action strip). Phase 46 code complete. Next: user live `bilterm --gui` pass with Obsidian (panel scale, Zen, pills, strip, font), then release decision. Deferred: [appearance] padding.
3. Then 46-08 pane chrome options (title/focus_ring/padding), 46-09 contextual pane actions + snapshot tests (candidate for interactive agy).
Open notes: resize_all has its own offset copy (pill row added); WorkspaceFrameSnapshot::capture ignores top menubar (pre-existing); Phase 45 GAPS.md (profile management, 8 vs 5 profiles); optional borderless Zen prompt; CodeGraph 1.6.2 upgrade available (user runs `codegraph upgrade`).

## 2026-11-02 release state
- v2.6.0 tagged, released (15 assets, CI mac/win OK). main pushed. Installed to ~/.local/bin/bilterm (backup bilterm.bak-2.6.0). Tree clean.
- :appearance now restyles palette/glyphs/chrome (6f847e1). Midnight/Arctic palettes are my own picks; Arctic readability UNVERIFIED (user unsure; wait for their live-GUI feedback, do not change blindly).
- Palette (Ctrl+X :) is PALETTE_CATALOG in src/ui/help_catalog.rs; Tab completion is COLON_COMMANDS in src/pane/completion.rs.
- Next: user live `bilterm --gui` pass; deferred: [appearance] padding, persistence of :appearance/:mode/:panel, Phase 45 gaps (GAPS.md), Phase 47/48 agy tasks.

## 2026-11-02 Phase 49 planned
- Plan: .planning/phases/49-command-parity-api/49-PLAN.md (audit done, no code). Next: 49-02 colon parity for 7 key-only Actions; open questions at the end of the plan need the user's answers before 49-06.
- Note: plain `cargo test` fails 1 test on machines with real saved sessions; use scripts/verify.sh (isolates XDG_STATE_HOME).

## 2026-11-02 end of session: Phase 49 state
- main is ahead of origin by ~14 commits, NOTHING pushed. v2.6.1 tag already exists (user-made); install on this machine is still v2.6.0 (~/.local/bin/bilterm); bigmac.local also v2.6.0.
- Phase 49 done: 49-01..49-06 (see 49-PLAN.md progress log). Pending: 49-07 conformance tests over Unix/TCP/HTTP/MCP + `bilterm ctl` for all endpoints + docs + release; user-run agy packet .planning/dispatch/49-doc.md (docs/API.md; run `.planning/dispatch/RUN.sh 49-doc apidoc`; then correct its "planned" section: HTTP+SSE, MCP resources and :get/:set are now implemented).
- User decisions: HTTP+SSE AND JSON-RPC both; config write over API behind `[api] allow_config_write` (default off); persistence only on explicit `:appearance save`/`:config save`.
- Test-coverage request (/eng) NOT done: `cargo llvm-cov` was started with isolated XDG_STATE_HOME and was killed by the restart. Rerun: `export XDG_STATE_HOME=$(mktemp -d); cargo llvm-cov --summary-only --json --output-path /tmp/cov.json` (takes >10 min, run in background), rank files changed since v2.5.0, then add tests (hunting-silent-failures lens). Known weak tests: focus-ring (phase46), pane.send_keys gate.
- Plain `cargo test` fails 1 drawer test when real saved sessions exist; use scripts/verify.sh (35/35 at last run).
- Process: worktree subagents branch from origin/main, NOT local HEAD (they lacked the plan file) - run in place or push first. Agents never commit; orchestrator reviews and commits. Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>.
- Still deferred: 49-07; Phase 45 gaps (GAPS.md); `[appearance] padding`; minimize-pane/MCP-console freeze+move need Actions; HTTP token generator is Unix-only; `/events` shares the events.poll queue; live `bilterm --gui` pass for Phases 44-46; user runs `codegraph upgrade` (v1.6.2).

## 2026-11-02 late: test-coverage push in flight
- Committed: 49-07 (ed7780a), design tests (67be7f3). verify.sh PASS 36/36 at ed7780a. Nothing pushed; main ahead of origin by ~16.
- llvm-tools-preview now installed (user did it). Measured coverage 75.6% lines (cov json was in $CLAUDE_JOB_DIR/tmp/cov.json). Weak: wm/drawer.rs 76%, wm/layout_profile.rs 77%, ui/design/config.rs 48%, api http/server/rpc/playbook 84-88%, gui_font/linux_x11.rs 0% (needs display).
- IN FLIGHT (user said: "commit all of it when they finish"): two subagents still writing NEW test files, uncommitted: tests/api_error_paths.rs + tests/config_commands_edge.rs (API error paths/config edges), tests/drawer_behaviour_coverage.rs + tests/layout_profile_coverage.rs (drawer/layout profiles). When both report: run `export XDG_STATE_HOME=$(mktemp -d); scripts/verify.sh`, review any #[ignore = "bug: ..."] tests and reported production bugs, then `git add tests/*.rs` for those four files and commit ("test: ..." + Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>). If the session was restarted and the agents are gone: `git status -s` shows the partial files; finish or delete them.
- Then: rerun coverage (`cargo llvm-cov --summary-only --json --output-path /tmp/cov.json </dev/null` in background, ~12 min) to confirm the numbers moved.
- Noted gaps: Materials does not clamp corner_radius_px (only AppearanceConfig::sanitized does); no mutation check was run on design tests; focus-ring and pane.send_keys gate tests are weak.
- Still user-run: agy API-doc packet (.planning/dispatch/49-doc.md; run `.planning/dispatch/RUN.sh 49-doc apidoc`), live `bilterm --gui` pass, install v2.6.1+ builds (this machine and bigmac.local both at 2.6.0), `codegraph upgrade`. No release has been requested for Phase 49.

## 2026-11-02 latest: test-gap bugs fixed
- Committed 6b82dc4 (tests for API error paths, config edges, drawer, layout profiles) and the fixes below. verify PASS 41/41 incl. clippy. No `#[ignore = "bug` tests remain. Nothing pushed (main ahead of origin ~20).
- Fixed: JSON parser (src/api/json.rs): floats via new `JsonValue::Float(f64)` (Eq derive dropped), RFC 8259 numbers (no leading zeros), surrogate pairs, unknown escapes and raw control chars in strings now REJECTED (behaviour change for lax clients; bilterm's own writer escapes them). Config (src/config/dotted.rs): `:config save` keeps comments above/inline on changed keys and CRLF; `:get theme.sub` => unknown key.
- Coverage run after fixes: pending (`cargo llvm-cov --summary-only --json --output-path $CLAUDE_JOB_DIR/tmp/cov.json </dev/null`, ~12 min, background).
- Remaining small gaps: Materials does not clamp corner_radius_px; no mutation check on design tests; focus-ring + pane.send_keys gate tests weak; JSON-RPC uses -32000 where MCP uses -32601/-32602.

## latest: input coverage tests (modal.rs, mouse_wm.rs)
- 173 new tests in 5 files (tests/input_modal*_coverage.rs, tests/input_mouse_wm_coverage*.rs); verify PASS 46/46. modal.rs ~98.7% and mouse_wm.rs ~80% from these alone.
- Oddities, not pinned: MCP console `>` graft reports success but tree unchanged; MCP `K` at top is a silent no-op (no status, unlike `J`).
- Not yet re-measured overall (cargo llvm-cov, ~12 min). Nothing pushed.

## restart point (after 6b1d82f)
- main @ 6b1d82f, tree clean, ~21 commits ahead of origin, nothing pushed. verify PASS 46/46.
- No subagents running. Nothing in flight.
- Pending (user-run): agy `.planning/dispatch/RUN.sh 49-doc apidoc` (then fix docs/API.md "planned" section); live `bilterm --gui` pass; install builds >2.6.0 here and on bigmac.local; `codegraph upgrade`.
- Optional next: re-run cargo llvm-cov (</dev/null, background, ~12 min); investigate the two MCP console oddities (graft no-op success, silent K at top); api/server.rs 84%.
- Ask before: push, release, `[appearance] padding`, Phase 45 gaps.

## restart point 2 (after daded18, 2026-11-02)
- main @ daded18, tree clean (after this doc commit), ~23 commits ahead of origin, nothing pushed. verify PASS 47/47.
- Done this session: coverage re-measured = 79.2% lines (31727/40064). Biggest gaps: gui_font/linux_x11.rs 0% (512 missed), pane/typography.rs 46%, ui/canvas_raster.rs 47%, main.rs 41%, ui/theme_import.rs 62%.
- 7d41e69: MCP oddities fixed. `graft` onto current parent is now an Err ("already a child of"); tests/layout_profile_coverage.rs `graft_errors_and_success_relink` updated (was pinning idempotent success). J/K at session-list edge now sets status "already at the top/bottom" (no test for the message yet).
- daded18: tests/api_server_edges.rs (7 tests) for listen-config validation and token-gated RPC handshake. api/server.rs was 83.9% before; not re-measured. Remaining misses are mostly unix socket probe / peer-UID mismatch.
- Pending (user-run): agy `.planning/dispatch/RUN.sh 49-doc apidoc` (then fix docs/API.md "planned" section); live `bilterm --gui` pass; install builds >2.6.0 here and on bigmac.local; `codegraph upgrade` (server v1.6.0, v1.6.2 available).
- Optional next: test for J/K edge status message; cover typography.rs / theme_import.rs / main.rs; re-run `cargo llvm-cov` (XDG_STATE_HOME=$(mktemp -d), </dev/null, background, ~12 min).
- Ask before: push, release, `[appearance] padding`, Phase 45 gaps.

## 2026-11-02 end: v2.7.0 released
- v2.7.0 tagged, pushed, released (15 assets; mac/win via CI workflow, Linux built locally from the tag in a clean worktree; SHA256SUMS merged). main pushed through 2d1abfd (API docs). Installed 2.7.0 on this machine and bigmac.local (backups `bilterm.bak-2.6.0`).
- New tests: tests/mcp_list_edge_status.rs, pane_typography_coverage.rs, theme_import_coverage.rs. Fixed: Base16 TOML mixed-case keys; FontFamilyPreset::from_name accepts display names.
- docs/API.md written by agy with `--dangerously-skip-permissions` (user approved once, for this run). Headless RUN.sh allow-list is still insufficient for 49-doc (denied command not logged). RUN.sh now branches from HEAD (BASE= overrides), not stale ui-api-frozen.
- codegraph upgraded to 1.6.2 (restored source-ui/package-lock.json, pull, npm install, build). Running MCP server needs a restart to pick it up.
- Linux release recipe: scripts/build-release-assets.sh also tries darwin (cannot link on Linux); instead use zigbuild for x86_64-unknown-linux-gnu.2.28, x86_64/aarch64 musl from a `git worktree` of the tag, then `gh release upload` with merged SHA256SUMS after the CI workflow finishes. aarch64 Linux archive not run (no ARM host).
- Untracked, not ours: docs/mcp-usecases/.
- Still pending: live `bilterm --gui` pass (Phases 44-46); Phase 45 gaps; `[appearance] padding`; persistence of :appearance/:mode/:panel; re-measure coverage (llvm-cov ~12 min, XDG_STATE_HOME=$(mktemp -d), </dev/null); local swe-skills-2 diverged from remote (1 ahead, 3 behind), untouched.
- Ask before: push, release, `[appearance] padding`, Phase 45 gaps.

## 2026-11-02 late: panel pin + interactive scale (20ad5a0, pushed; unreleased)
- MCP window pin: `McpConsoleState.pinned/focused`, `captures_input()`; `:mcp pin|unpin`, `*` key, Esc/outside-click drops focus, inside-click regains, `:mcp close` unpins. Title shows `[PIN]`.
- Drawer: `:drawer unpin` added; `*` in focused drawer toggles Fixed/OnDemand (`:drawer pin` already existed).
- Scale: Ctrl+wheel over drawer/MCP, `[`/`]` keys, `:drawer scale N`, `:mcp scale N`; `WindowManager::step_panel_scale`. Scale is now independent per panel (PanelTarget::{Drawer,Mcp}); `:panel scale` sets both.
- Tests: tests/panel_pin_scale.rs (7). verify.sh PASS 51/51. CHANGELOG has `[Unreleased]` entry.
- NOT verified: live `bilterm --gui`. No clickable pin button (title-bar hit targets are fixed column offsets). Pin/scale are session-only. `[`/`]` not active in MCP file browser.
- Open question for user: per-panel independent scale?

## 2026-11-02 night: v2.8.0 released; Milestone v2.9 planned
- v2.8.0 tagged, pushed, released with 15 assets (Linux local zigbuild; Windows x64 via local win-devvm VM; Windows ARM64 + macOS on bigmac.local; CI not used by user's choice). Local win-devvm vm.sh lacks push/pull; used a wrapper copy in /tmp. SHA256SUMS regenerated over all 14 archives. NOT yet done: install 2.8.0 here and on bigmac.
- Panel pin (MCP `:mcp pin`, `*`; drawer `:drawer pin|unpin`, `*`) and independent drawer/MCP scale (Ctrl+wheel, `[` `]`, `:drawer scale`, `:mcp scale`, `:panel scale` both) shipped in 2.8.0.
- Milestone v2.9 Agent Fleet added to ROADMAP/REQUIREMENTS (8c92254): Phase 50 herdr-style left sidebar (FLT-01), Phase 51 agent-to-agent messaging (AGT-01..03). Next: 50-01 research/design (needs user input on what herdr's left side does), then sign-off before code.
- Ask before: push, release, install.

---

## Phase 50 execution - IN PROGRESS (2026-11-03)
6/16 plans done on branch `gsd/phase-50-navigator` (unpushed). Full detail, decisions, remaining order and PR steps:
`.planning/phases/50-agent-aware-workspace-navigator/EXEC-CHECKPOINT.md`. Next: execute plan 50-07.
