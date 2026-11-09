---
gsd_state_version: "1.0"
milestone: v3.2.0
current_phase: 65
current_phase_name: architectural-guardrails-and-type-hardening
status: planned
stopped_at: "Registered Phase 65: Architectural Guardrails & Type-Safe Domain Hardening (GUARD-01..03)"
last_updated: "2026-11-09T13:08:00Z"
last_activity: 2026-11-09
last_activity_desc: "Registered Phase 65 in roadmap and requirements; generated 65-PLAN.md covering SignalablePid, PaneTransport decoupling, and mouse focus isolation"
state_head: feat/phase-65-guardrails
progress:
  total_phases: 65
  completed_phases: 60
  total_plans: 155
  completed_plans: 143
milestone_name: "Bilterm v3.2.0 — Regressions, Copy Mode, Keymaps, Traffic Monitor & Architectural Guardrails"
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-10-31)

**Core value:** A responsive composite terminal pane (movable/resizable floating command prompt over a full-PTY output canvas that scrolls underneath) that can be split horizontally/vertically, stacked/shuffled in z-order via keyboard or mouse, styled with classic IBM 3270 mainframe & DOS Borland Turbo Vision or Non-Retro Modern Minimal / Liquid Glass aesthetics, and integrated seamlessly with `tmux` and the persistent `Bilterm` (`Herdr` + TRON `MCP`) multiplexer daemon.
**Current focus (2026-11-09):** Active roadmap extended through Phase 65: Architectural Guardrails & Type-Safe Domain Hardening (`GUARD-01..03`).

## Current Position

Phase: 65 (architectural-guardrails-and-type-hardening) — PLANNED (alongside planned phases 61–64)
Plan: 60/65 phases complete (143/155 plans complete).
Status: 60/65 Phases complete.
Last activity: 2026-11-09 — Registered Phase 65 in ROADMAP.md, created 65-PLAN.md, and updated STATE.md.

Progress: [█████████░] 92.3% (60/65 Phases)

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
Stopped at: Registered Phase 65 (GUARD-01..03) in roadmap, requirements, and state.
Resume file: None
