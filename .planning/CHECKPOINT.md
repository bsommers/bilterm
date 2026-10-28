# Session Checkpoint — 2026-10-27 (`v2.0` + MCP Pop-Up/Push Window + Status Bar Mouse Isolation)

## 1. Completed Objectives
1. **Milestone `v2.0` Phases 30–34 (`34/34` Phases Complete)**:
   - Phase 30 (`src/pane/exit_guard.rs`, `src/ui/exit_guard_ui.rs`, `src/wm/v2_runtime.rs`): Interactive Exit-Protection Modal & Final-Pane Hold-on-Exit Guard (`:exit-guard`, `:respawn-pane`).
   - Phase 31 (`src/ui/theme.rs`, `src/ui/theme_import.rs`): Universal Open Theme Engine (`14` built-in themes including `TronGrid`, iTerm2 `.itermcolors` / Base16 YAML / Ghostty / Alacritty TOML importer, `:theme import <path>`).
   - Phase 32 (`src/daemon/mod.rs`, `src/cli.rs`): Persistent Background Daemon & Multi-Client Attach/Detach (`bilterm daemon`, `bilterm attach`, `bilterm detach`, `:detach`).
   - Phase 33 (`src/wm/session_tree.rs`): `herdr`-Inspired Hierarchical Session Tree (`:session tree`, `:session branch`, `:session graft`, `:session promote`, `:session broadcast`, `:session freeze`, `:session thaw`).
   - Phase 34 (`src/mcp_console/mod.rs`): TRON-Inspired Master Control Program (`MCP`) Root Terminal (`bilterm mcp`, `:mcp status`, `:mcp tree`, `:mcp derez`, `:mcp checkpoint`).
2. **Agent Autonomy & Super-Permissive Configuration**:
   - Saved [`docs/AGENT_AUTONOMY_AND_PERMISSIONS.md`](file:///home/bill/src/ai/bilterm/docs/AGENT_AUTONOMY_AND_PERMISSIONS.md) in `bilterm` and mirrored to `/home/bill/src/swe-skills-2/docs/AGENT_AUTONOMY_AND_PERMISSIONS.md` and `/home/bill/src/swe-skills-2/skills/supervising-autonomous-sessions/references/agent-autonomy-and-permissions.md`.
   - Configured `~/.gemini/antigravity-cli/settings.json`, `.claude/settings.local.json`, and `.planning/config.json`.
3. **Floating (`Overlay`) vs. Push-Out (`Push`) Window for `:mcp tree`, `:mcp status`, `:session tree`, and `:theme list`**:
   - Implemented in [`src/mcp_console/mod.rs`](file:///home/bill/src/ai/bilterm/src/mcp_console/mod.rs), [`src/wm/v2_runtime.rs`](file:///home/bill/src/ai/bilterm/src/wm/v2_runtime.rs), [`src/wm/drawer.rs`](file:///home/bill/src/ai/bilterm/src/wm/drawer.rs), [`src/ui/mod.rs`](file:///home/bill/src/ai/bilterm/src/ui/mod.rs), [`src/input/modal.rs`](file:///home/bill/src/ai/bilterm/src/input/modal.rs), and [`src/input/mouse_wm.rs`](file:///home/bill/src/ai/bilterm/src/input/mouse_wm.rs).
   - Supports `:mcp float` (`DrawerLayoutMode::Overlay`) and `:mcp push [top|bottom|left|right]` (`DrawerLayoutMode::Push`), plus interactive keys (`m` to toggle `Float <-> Push`, `d` to cycle dock edge, `Up`/`Down`/`j`/`k`/`PageUp`/`PageDown` to scroll, `Esc`/`q` to close) and mouse clicks/scroll inside the MCP window without polluting the shell's PTY output canvas.
4. **Status Bar Mouse Click Isolation & Item Controls**:
   - Implemented in [`src/input/mouse.rs`](file:///home/bill/src/ai/bilterm/src/input/mouse.rs) and tested in [`src/input/tests.rs`](file:///home/bill/src/ai/bilterm/src/input/tests.rs).
   - Fixed font-scaling coordinate rounding (`font_scale_pct < 100`) and `RawPtyPassthrough` SGR mouse interception so clicks on the bottom 1-row OIA status bar (`pane.status_bar_rect`) and bottom DOS Function-Key bar (`render_dos_fkey_bar`) never start a canvas copy selection and never right-click/middle-click paste into the pane.
   - Left-clicking OIA status bar badges controls the corresponding status bar items (`3270-OIA` version toggle, `MORE...`/`SCROLL` page advance/tail reset, `INPUT: CMD BOX`/`RAW PTY`/`CANVAS` mode cycle, `PIN:TOP`/`BOT`/`SPL` prompt anchor cycle, `🔒 PROT` lock toggle, `SCRL:SMOOTH`/`PAGE` scroll mode toggle).

## 2. Verification & Binary State
- `ARCH-R04 (<= 800 lines per src/**/*.rs)`: **PASS**
- `cargo clippy -q --all-targets -- -D warnings`: **PASS**
- `cargo test -q`: **PASS**
- Installed release binary (`v1.13.0`) to `~/.local/bin/bilterm` and `~/.cargo/bin/bilterm`.
