# Phase 50: herdr-Style Left Sidebar - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in 50-CONTEXT.md.

**Date:** 2026-11-03
**Areas discussed:** Scope split with Phase 51; Glyphs, state and comms indicators; Traffic window layout and snoop; Text vs GUI rendering

---

## Scope split
Options: UI on traffic-event bus (chosen) / move graph+snoop to Phase 51 / new Phase 52.

## Snoop
Options: read-only / read-only + send-as-me (chosen) / read-only + pause-replay.

## Graph layout
Options: automatic force/layered (chosen) / fixed ring-grid / you decide.

## Comms glyphs
Options: per-row glyph + partner name (chosen) / glyph only / workspace summary.

## State source
Options: heuristics / heuristics + per-agent profiles (chosen) / explicit only.

## Glyph style
Unicode with ASCII fallback (chosen) / Nerd Font / Unicode only.

## Zoom window
Key + click, resizable/pinnable/scalable like MCP (chosen) / full-screen / dock as pane.

## GUI graphics
Smooth vector via pixel canvas (chosen) / add vector crate / basic lines. User note: GUI should have high-resolution, smooth graphics and good graph layout algorithms.

## Claude's Discretion
Layout algorithm per mode, glyph choices, animation, keys, buffer sizes.

## Deferred Ideas
None.
