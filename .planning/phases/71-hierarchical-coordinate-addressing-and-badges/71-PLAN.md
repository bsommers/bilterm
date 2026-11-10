---
phase: 71-hierarchical-coordinate-addressing-and-badges
plan: 01
type: execute
wave: 1
depends_on:
  - 66-foreground-rpc-and-instance-discovery
  - 67-first-class-agent-pane-workspace-cli
files_modified:
  - src/coords/mod.rs
  - src/coords/parse.rs
  - src/coords/resolve.rs
  - src/coords/format.rs
  - src/selector.rs
  - src/ui/window_bar.rs
  - src/config/coords.rs
  - src/action.rs
  - src/colon_command/coords.rs
  - src/api/rpc_coords.rs
  - src/mcp/tools.rs
  - src/cli/coords.rs
  - tests/phase71_coords.rs
autonomous: true
requirements:
  - COORD-01
  - COORD-02
  - COORD-03
  - COORD-04
recommended_model: sonnet
must_haves:
  truths:
    - "Every addressable object has exactly one canonical Coord { daemon, workspace, session, window, pane }; formatting then parsing round-trips."
    - "All accepted input forms parse to the same Coord; malformed, out-of-range, or ambiguous input returns a typed error, never a panic or a silent best guess."
    - "Omitted levels resolve against the caller context (from_pane / focused) and never against another daemon implicitly."
    - "The title-bar badge is right-aligned, truncates before existing bar info, is off by default in golden fixtures, and leaves the 11 classic goldens byte-identical."
    - ":coords, palette, menu, RPC coords.resolve/coords.format, MCP bilterm_coords_resolve and CLI all lower to the same Action/resolver (UNI-R01)."
  artifacts:
    - path: "src/coords/parse.rs"
      provides: "Universal coordinate grammar and parser"
    - path: "src/coords/resolve.rs"
      provides: "Coord -> object resolver with context defaults"
    - path: "src/coords/format.rs"
      provides: "Verbose, Compact, DaemonTuple formatters"
    - path: "tests/phase71_coords.rs"
      provides: "Grammar, resolution, badge, and parity tests"
---

# Phase 71 Plan: Hierarchical Coordinate Addressing & Window Bar Coordinate Badge (COORD-01..04)

## Objective

1. Canonical 5-level addressing `D:(W,S,Y,P)` (`COORD-01`).
2. One universal parser/resolver shared by CLI, RPC, MCP, and `:colon` (`COORD-02`).
3. Right-aligned title-bar coordinate badge in three styles (`COORD-03`).
4. `:coords` command family with full `UNI-R01` parity (`COORD-04`).

---

## Detailed Requirements

### COORD-01: Hierarchy
- `Daemon (D) -> Workspace (W) -> Session (S) -> Window (Y) -> Pane (P)`; indices are 0- or 1-based consistently (decision recorded in `71-PLAN` wave 1; default 1-based for display, matching Herdr `w12:p6`).
- `P` may carry a sub-pane suffix, a tmux `%id`, or a Herdr pane number; the foreground instance is `D = 0` unless the registry (Phase 66) maps another daemon.
- Coordinates are positional and may change when layout changes; stable ids (pane generation token) are returned alongside for scripting.

### COORD-02: Parser & Resolver
- Accepted forms (all equivalent when levels match):
  - `D:(W,S,Y,P)`, `(W,S,Y,P)`, `(W,S,Y)` (window level)
  - `(Wksp: W, Sess: S, Win: Y, Pane: P)` (any key order, case-insensitive keys)
  - `d0:w1:s1:y2:p1`, partial paths `w1:s1`, and Herdr `w12:p6`
- Whitespace-tolerant; length-capped input; numeric overflow rejected.
- Resolution: missing leading levels filled from context; ambiguous Herdr `wN:pM` (no session/window) resolved via the Phase 67 selector rules; returns `NotFound` / `Ambiguous{candidates}` / `OutOfRange`.
- `src/selector.rs` delegates `w<N>:p<M>` to this parser to avoid two grammars.

### COORD-03: Title-Bar Badge
- Styles: `Verbose` `(Wksp: 1, Sess: 2, Win: 3, Pane: 4)`, `Compact` `(1,2,3,4)` or `(1,2,3)`, `DaemonTuple` `(D0:(1,2,3,4))`.
- Right-aligned on the window bar, to the right of existing info; drops (lowest priority first) when width is insufficient; wide-grapheme safe via `occlude_rect`.
- Config `[coords] badge = "off|compact|verbose|daemon"`; default `off` in test/golden config so the 11 classic goldens are unchanged; new goldens added for badge-on only.
- Rendered in Classic, Modern Minimal, and Liquid Glass title bars (TUI + GUI).

### COORD-04: Surfaces
- `:coords [verbose|compact|daemon|off|locate <coord>|copy]`: style switch, focus/flash the located object, copy focused coordinate to the clipboard registry (Phase 57).
- Command Palette entries, Menu Bar (View -> Coordinates), RPC `coords.resolve` / `coords.format`, MCP `bilterm_coords_resolve`, CLI `bilterm coords resolve|format`.
- All lower to `Action::Coords(..)` executed by `WindowManager::apply`.

---

## Threat Model & Security Considerations

| Threat ID | Threat Category | Description | Mitigation Strategy |
|-----------|-----------------|-------------|---------------------|
| **T-CRD-01** | Misdirected Input | Positional coordinate shifts so a send/approve hits the wrong pane. | Mutating commands accept optional generation token; ambiguity fails closed. |
| **T-CRD-02** | Parser DoS | Pathological or huge input. | Length cap, linear-time hand-written parser, fuzz tests (`parsing-untrusted-text-robustly`). |
| **T-CRD-03** | Cross-Daemon Confusion | Omitted `D` silently targets a remote daemon. | Default to caller's daemon only; explicit `D` required for others. |
| **T-CRD-04** | Layout Regression | Badge changes golden output / overdraws bar info. | Off by default in goldens; priority-based truncation; golden gate. |

---

## Technical Design & Wave Breakdown

### Wave 1: Model, parser, formatter (`COORD-01`, `COORD-02`)
1. `src/coords/{mod,parse,format}.rs`; round-trip and fuzz-style table tests; decide index base.

### Wave 2: Resolver & selector integration (`COORD-02`)
1. `src/coords/resolve.rs` against `WindowManager` (headless); migrate `w<N>:p<M>` handling in `src/selector.rs`.

### Wave 3: Badge & surfaces (`COORD-03`, `COORD-04`)
1. Title-bar badge in `src/ui/window_bar.rs`; `[coords]` config; `Action::Coords`, `:coords`, palette, menu, RPC, MCP, CLI.
2. `tests/phase71_coords.rs`; add to the Phase 70 parity matrix; `scripts/verify.sh`.

---

## Verification Gates

- [ ] **Gate 1 (Grammar)**: Every listed form parses to the same `Coord`; invalid and overflow inputs return typed errors; format->parse round-trip.
- [ ] **Gate 2 (Resolution)**: Context defaults, not-found, ambiguous, out-of-range, cross-daemon cases.
- [ ] **Gate 3 (Badge)**: Three styles, truncation priority, right alignment, badge-off leaves `GOLDENS OK 11`.
- [ ] **Gate 4 (Parity)**: `:coords`, palette, menu, CLI, RPC, MCP reach the same resolver (`UNI-R01`).
- [ ] **Gate 5 (Quality)**: `scripts/verify.sh` PASS; files `<= 1000` lines (`ARCH-R04`).
