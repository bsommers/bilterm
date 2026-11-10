---
phase: 70-agent-mission-control-hud-and-time-travel
plan: 01
type: execute
wave: 1
depends_on:
  - 64-terminal-traffic-monitor
  - 66-foreground-rpc-and-instance-discovery
  - 67-first-class-agent-pane-workspace-cli
  - 68-semantic-agent-introspection-and-context-bus
  - 69-ephemeral-worktree-swarm-and-dag-runner
files_modified:
  - src/wm/agents_hud.rs
  - src/wm/agents_hud_render.rs
  - src/agent/replay.rs
  - src/agent/watchdog.rs
  - src/config/agents.rs
  - src/keymap/leader.rs
  - src/action.rs
  - src/colon_command/agents.rs
  - src/mcp/tools.rs
  - tests/phase70_agent_swarm_parity.rs
autonomous: true
requirements:
  - HUD-01
  - HUD-02
  - HUD-03
  - HUD-04
recommended_model: sonnet
must_haves:
  truths:
    - "The HUD renders every agent pane as a live mini viewport with velocity sparkline, queue depth, and traffic arc, in both TUI and GUI, without triggering SIGWINCH on child PTYs."
    - "One-click [Approve], [Prompt], [Diff], [Zoom] on a HUD tile lower to the same Actions as their CLI/colon/RPC/MCP equivalents."
    - "agent replay scrubs per-turn history read-only; it never writes to the live PTY."
    - "The watchdog detects stalls (no output beyond threshold while Working) and loops (repeating output signature) and raises OIA/toast/RPC-event alerts per [agents.watchdog]."
    - "tests/phase70_agent_swarm_parity.rs proves every v3.3.0 capability is reachable from key, colon, palette, menu, mouse, CLI, RPC, and MCP (UNI-R01)."
  artifacts:
    - path: "src/wm/agents_hud.rs"
      provides: "Mission Control HUD model and interaction"
    - path: "src/agent/replay.rs"
      provides: "Per-turn time-travel scrubber"
    - path: "src/agent/watchdog.rs"
      provides: "Stall and loop detection"
    - path: "tests/phase70_agent_swarm_parity.rs"
      provides: "Cross-surface parity matrix for Phases 66-70"
---

# Phase 70 Plan: Agent Mission Control HUD & Time-Travel (HUD-01..04)

## Objective

Provide a single live control surface for the fleet and close the milestone with parity proof:
1. Agent Mission Control HUD (`HUD-01`).
2. Per-turn time-travel scrubber (`HUD-02`).
3. Autonomous stall/loop watchdog (`HUD-03`).
4. Full `UNI-R01` surface parity (`HUD-04`).

---

## Detailed Requirements

### HUD-01: Mission Control HUD
- `:agents hud` and `Leader A` open a utility window (Phase 51.2 pattern): grid of tiles, one per agent pane, each with live mini viewport (read-only downscaled snapshot of `OutputCanvas`, zero-copy, no resize of the PTY), velocity sparkline (bytes/s from Phase 64 traffic layer), queue depth (Phase 68), traffic arc, state glyph, and swarm node/worktree badge (Phase 69).
- Click/keys per tile: `[Approve]` (when Blocked), `[Prompt]` (opens injection prompt), `[Diff]` (opens `:swarm review` for its worktree), `[Zoom]`.
- Design-system tokens for Classic/Modern/Liquid Glass; keyboard navigable with which-key hints; refresh rate capped.

### HUD-02: Time-Travel Scrubber
- `:agent replay <pane>`: left/right steps turns (Phase 68 turn store), up/down scrolls within a turn; shows prompt, response, code blocks, files touched; read-only; `Esc` exits.

### HUD-03: Watchdog
- Config:
  ```toml
  [agents.watchdog]
  enabled = true
  stall_secs = 120
  loop_window = 20
  loop_repeat = 4
  alert = ["oia", "toast", "event"]
  ```
- Stall: Working with no output for `stall_secs`. Loop: normalized output-line signature repeats `loop_repeat` times in `loop_window`. Alerts deduped with cooldown; never auto-kills.

### HUD-04: UNI-R01 Parity
- Matrix test enumerating each Phase 66–70 capability against: keybinding, colon command, palette, menu, mouse, CLI, RPC, MCP; each must lower to the same `Action`. Parity gaps fail the test.

---

## Threat Model & Security Considerations

| Threat ID | Threat Category | Description | Mitigation Strategy |
|-----------|-----------------|-------------|---------------------|
| **T-HUD-01** | Misclick Approval | Mis-targeted one-click Approve on a stale tile. | Approve button only enabled while state is Blocked and classifier confident; action carries pane generation token. |
| **T-HUD-02** | Information Disclosure | Mini viewports expose secrets (password prompts). | Respect `!ECHO`/secret-input masking; mask tiles for panes in no-echo mode. |
| **T-HUD-03** | Performance / DoS | Many tiles redraw continuously. | Refresh cap, dirty-flag rendering, tile count cap with paging. |
| **T-WD-01** | Alert Storm | Flapping watchdog floods toasts/events. | Per-pane cooldown and dedupe. |
| **T-REP-01** | State Corruption | Replay writes into live PTY. | Replay view has no PTY handle; compile-time separation. |

---

## Technical Design & Wave Breakdown

### Wave 1: Watchdog & replay (`HUD-02`, `HUD-03`)
1. `src/agent/replay.rs`, `src/agent/watchdog.rs`, `[agents.watchdog]` config + validation.

### Wave 2: HUD (`HUD-01`)
1. `src/wm/agents_hud*.rs`, TUI + GUI renderers, `Leader A`, tile actions, OIA alert segment.

### Wave 3: Parity & verification (`HUD-04`)
1. `tests/phase70_agent_swarm_parity.rs` parity matrix; docs update (`docs/DEVELOPER_AND_AGENT_GUIDE.md`, AGENTS.md); `scripts/verify.sh`.

---

## Verification Gates

- [ ] **Gate 1 (HUD)**: Headless render tests for tile states; no `SIGWINCH` emitted (resize counter assertion).
- [ ] **Gate 2 (Replay)**: Read-only invariant and turn navigation tests.
- [ ] **Gate 3 (Watchdog)**: Stall, loop, cooldown, disabled-config tests with fake clock.
- [ ] **Gate 4 (Parity)**: Parity matrix green across all eight surfaces for Phases 66–70.
- [ ] **Gate 5 (Quality)**: `scripts/verify.sh` PASS; `GOLDENS OK 11`; files `<= 1000` lines (`ARCH-R04`).
