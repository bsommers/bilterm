---
gsd_state_version: "1.0"
milestone: v3.3.0
current_phase: 66
current_phase_name: foreground-rpc-and-instance-discovery
status: planned
stopped_at: "Registered Milestone v3.3.0 (Phases 66–70, LIVE/HERDR/BEYOND/SWARM/HUD); Phases 61–65 marked Complete"
last_updated: "2026-11-09T13:08:00Z"
last_activity: 2026-11-09
last_activity_desc: "Marked Phases 61–65 complete; registered Milestone v3.3.0 (Phases 66–70) with 20 requirements and 66–70 PLAN files"
state_head: feat/phase-65-guardrails
progress:
  total_phases: 70
  completed_phases: 65
  total_plans: 160
  completed_plans: 155
milestone_name: "Bilterm v3.3.0 — Live Foreground Agent Control Plane & Swarm Orchestration (Beyond Herdr)"
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-10-31)

**Core value:** A responsive composite terminal pane (movable/resizable floating command prompt over a full-PTY output canvas that scrolls underneath) that can be split horizontally/vertically, stacked/shuffled in z-order via keyboard or mouse, styled with classic IBM 3270 mainframe & DOS Borland Turbo Vision or Non-Retro Modern Minimal / Liquid Glass aesthetics, and integrated seamlessly with `tmux` and the persistent `Bilterm` (`Herdr` + TRON `MCP`) multiplexer daemon.
**Current focus (2026-11-09):** Milestone v3.3.0 registered (Phases 66–70): Live Foreground Agent Control Plane & Swarm Orchestration — Beyond Herdr. Phases 61–65 complete.

## Current Position

Phase: 66 (foreground-rpc-and-instance-discovery) — PLANNED (alongside planned phases 67–70)
Plan: 65/70 phases complete (155/160 plans complete).
Status: 65/70 Phases complete.
Last activity: 2026-11-09 — Marked Phases 61–65 Complete; registered Milestone v3.3.0 (Phases 66–70) and 66–70 PLAN files.

Progress: [█████████░] 92.9% (65/70 Phases)

## Performance Metrics

**Velocity:**

- Total plans completed: 143
- Average duration: 0 min
- Total execution time: 0.0 hours

**Recent Trend:**

- Last 5 plans: 60-01, 60-02, 60-03, 59-01, 59-02
- Trend: Stable

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Init]: Composite Window Pane (`OutputCanvas` + `FloatingPrompt` + `OiaStatusBar`) is the atomic layout unit across all H/V splits and z-order stacks.
- [Init]: Child PTY dimensions stay fixed to full `OutputCanvas` rect so moving/resizing/hiding `FloatingPrompt` never triggers `SIGWINCH` reflow storms.
- [Init]: Three-signal input routing (`OSC 133` + `tcgetpgrp` + `termios !ECHO` + `DECSET 1049`) prevents `sudo` password echo leaks and interactive REPL lockouts.
- [Init]: Custom `occlude_rect` buffer pass sanitizes split 2-column wide graphemes at floating prompt, stacked pane, Guake drop-down overlay, and Turbo Vision drop-shadow boundaries.
- [Phase 50]: Classic drawer goldens generated on untouched tree; never regenerate in Phase 50
- [Phase 51]: 51-13: final gate green (verify.sh, 11 Legacy goldens, doc gates); UIT-01..04 and FND-R01..04 ticked.
- [Phase 51 Followups]: Resolved all 11 items; split `src/wm/drawer.rs` and `src/wm/agents_window.rs` into focused submodules to adhere to 800-1000 line ceilings (`ARCH-R04`).
- [Phase 53]: Added in-place `RenameDialog` modal for keyboard/mouse renaming of windows, sessions, and drawer entries with Esc cancel and Enter confirm.
- [Phase 53]: Added universal right-click context menu across Classic, Modern Minimal, and Liquid Glass modes for instant window management.
- [Phase 45]: Design system profile engine verified across 8 profiles with persistent `:appearance` lifecycle.
- [Verification]: Composable runner `scripts/verify.sh` supports 8 tiers (smoke, unit, integration, system, perf, stress, torture, security), `--match`, `--coverage` (`cargo-llvm-cov`), `--xvfb`, `report.md`, and `junit.xml`.
- [Phase 56]: Live and snapshot drawer peek preview pane designed with zero-copy, zero-`SIGWINCH` compositing and transient Esc/click-to-focus semantics (`56-RESEARCH.md`, `56-CONTRACT.md`).
- [Phase 57]: Smart MIME-aware clipboard paste fix for Herdr/Claude Code in `RawPtyPassthrough` wrapping text in `DECSET 2004` bracketed paste; multi-session tagged `ClipboardRegistry` and first-class `CLIPBOARD` viewer window designed (`57-RESEARCH.md`, `57-CONTRACT.md`).
- [Phase 56 & 57]: Marked COMPLETE (`80994fb` / `2f069bf`, `PASS 129/129`, `GOLDENS OK 11`, PR #31).
- [Phase 58]: Local AI STT optional module (`src/stt/`) spike complete: zero-bloat two-tier audio capture, OpenAI-compatible Whisper + Ollama + CLI protocols, prompt buffer staging, and OIA status bar telemetry (`58-RESEARCH-SPIKE.md`, `58-ARCHITECTURE-ADR.md`).
- [Phase 59]: Embedded Chrome engine browser canvas (CDP Screencast frame-sink) and 60fps RGBA video player canvas (`ffmpeg` pipe + audio sync) spike complete across GUI (`GuiTextureQuad`) and TUI (`Kitty`/`Sixel`/`HalfBlock`) (`59-RESEARCH-SPIKE.md`, `59-ARCHITECTURE-ADR.md`).
- [Phase 60]: Completed Phase 60: Multi-Surface Web Frontend Backend & `@agent-manager/protocol` Bridge (`WEB-FE-01..04`).
- [Phase 65]: Registered Phase 65: Architectural Guardrails & Type-Safe Domain Hardening (`GUARD-01..03`): type-safe `SignalablePid` newtype preventing broadcast/init signaling, `PaneTransport` sum type separating local PTY from remote bridge, and mouse focus isolation in `src/input/mouse.rs`.

- [Phases 61–65]: Marked COMPLETE (FIX-01..03, COPY-01..02, KEY-01..03, TRAF-01..04, GUARD-01..03).
- [Milestone v3.3.0]: Registered Phases 66–70 (LIVE-01..04, HERDR-01..04, BEYOND-01..04, SWARM-01..04, HUD-01..04): always-on foreground RPC, agent/pane/workspace CLI with zero-polling wait, semantic introspection & context bus, worktree swarm DAG runner, Mission Control HUD.

### Pending Todos

None.

### Blockers/Concerns

None.

## Deferred Items

Items acknowledged and carried forward:

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| *(none)* | | | |

## Session Continuity

Last session: 2026-11-08T18:20:00Z
Stopped at: Registered Milestone v3.3.0 (Phases 66–70) in roadmap, requirements, and state.
Resume file: None
