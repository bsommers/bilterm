# Session Checkpoint — `bilterm`

**Updated:** 2026-11-09T19:00:00-05:00  
**Status:** COMPLETE — all media/browser/video/voice TUI & URL bar enhancements, menu fixes, Phase 65 GUARD-01 (`SignalablePid`), and v3.2.0 verified and merged to `main`.

## 1. Goal & User Decisions (Verbatim)

1. *"looking at all of the bugs and bug fixes, let's write a report of bugs. Find those that were introduced by poor specification others that could otherwise be corrected by better skills or design things"* -> Committed `docs/BUG_AUDIT_REPORT.md` (PR #42).
2. *"let's do a /release"* -> Released `v3.2.0` (`ec942fb`, PR #41, GitHub Release `v3.2.0`).
3. *"let's take those Actionable Recommendations & Guardrails and turn them into a plan - improved skills, improved planning skills, bug fixing behavior during agent builds and in skills, and write a report. save that report, put it into the milestones on the plan. start executing the plan in parallel subagents using the cheapest model suited to the tasks"* -> Updated `AGENTS.md`, `swe-skills-2` (`93accf7`), registered Phase 65 in `.planning/ROADMAP.md`, and implemented `GUARD-01` (`SignalablePid` in `src/daemon/types.rs` + `tests/signalable_pid_invariants.rs`, PR #43).
4. *"these aren't right on Linux. double check and fix as needed: ... Leader m ... Window Menu doesn't grab the mouse clicks, they go below the window to the drawer window or other window below."* -> Fixed `Leader m` (`Action::MenuOpen`) across all profiles in `src/input/keymap_tables.rs` and reordered `handle_modal_mouse_event` before `handle_drawer_mouse_event` in `src/input/mouse.rs` (`tests/menubar_mouse_and_leader_m.rs`, PR #44).
5. *"in subagents: find out and fix: - is the \":browser\" canvas supposed to work? I see that it says i toggled it on, but I don't see a web browser inside of bilterm. same with video canvas. don't see that. am I in tui mode? also what about the voice mode>"*
6. *"dispatch the subagents, one into each the three claude sessions through herdr here in this same repos on the windows on the right. make sure the webbrowser mode lets us edit the URL bar."*

## 2. Completed Work in This Increment

- **Browser Canvas TUI Rendering, Prompt Occlusion Fix & Interactive Editable URL Bar (`w12:p7`, `a317666`)**:
  - `src/pane/browser_canvas/{mod,chrome_ui}.rs`, `src/input/{mod,mouse_media,mouse}.rs`: Clicking `BrowserNavHit::UrlBar` on the 1-row nav bar or pressing `Ctrl+L` enters interactive URL editing mode (`is_editing_url()`, `url_input`, `url_cursor`) supporting `Char`, `Backspace`, `Delete`, `Left`/`Right`/`Home`/`End`, `Ctrl+U`/`Ctrl+A`/`Ctrl+E`, `Enter` (navigates, auto-prepending `https://` when scheme is omitted), and `Esc` (cancels edit).
  - `src/pane/mod.rs`: Offsets `prompt_rect` by `+1` row when `browser_canvas.is_some()` and `PromptAnchor::Top` so Pass 2 `occlude_rect` never hides the 1-row browser navigation bar.
  - `src/ui/mod.rs`: Rasterizes `browser.cached_frame()` (`canvas.y + 1 .. canvas.bottom()`) and `video.cached_frame()` (`canvas.y .. canvas.bottom() - 1`) in TUI mode using TrueColor half-block (`▀`) cells.
  - `tests/browser_canvas_tui_and_url_edit.rs` (4/4 passing).
- **Video Canvas Real `FfmpegPipeBackend` Frame Reader & Diagnostics (`w12:p8`, `cdea0bf`)**:
  - `src/pane/video_canvas/{ffmpeg,mod,osd}.rs`: Background reader thread decoding `width * height * 4` RGBA frames from `ffmpeg` stdout pipe; `render_half_block_frame` helper.
  - `src/colon_command/execute_media.rs`: Clear backend (`CDP` vs `synthetic`, `ffmpeg` vs `synthetic`) and surface (`[TUI half-block; run bilterm --gui for GPU canvas]`) status messages.
  - `tests/video_canvas_tui_and_ffmpeg.rs` (5/5 passing).
- **OIA `[TUI]` / `[GUI]` Surface Indicator & `:voice` Pre-Flight Diagnostics (`w12:pA`, `a26bf07`)**:
  - `src/ui/oia.rs`: Surface mode badge `[TUI]` / `[GUI]` in the OIA status bar (`GOLDENS OK 11` preserved).
  - `src/stt/{mod,audio,worker}.rs`, `src/colon_command/execute_voice.rs`: Pre-flight audio recorder (`pw-record`/`parecord`/`arecord`/`ffmpeg`) and STT endpoint reachability diagnostics with actionable status hints.
  - `tests/voice_and_oia_surface_badge.rs` (4/4 passing).

## 3. Verification Baseline

- `scripts/verify.sh --quick`: `PASS 135/135`
- `scripts/check_classic_goldens.sh`: `GOLDENS OK 11`
- New suites: `browser_canvas_tui_and_url_edit` (4/4), `video_canvas_tui_and_ffmpeg` (5/5), `voice_and_oia_surface_badge` (4/4), `menubar_mouse_and_leader_m` (2/2), `signalable_pid_invariants` (12/12).

## 4. Next Steps (If Resuming)

- Execute remaining Phase 65 waves (`GUARD-02` `PaneTransport` enum refactor in `src/pane/mod.rs` and `GUARD-03` focus-isolation verification in `.planning/phases/65-architectural-guardrails-and-type-hardening/65-PLAN.md`) when requested.
