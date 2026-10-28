# Bilterm Session Checkpoint (`v2.0.0`)

- **Timestamp**: 2026-10-27
- **Milestone**: `v2.0` (`Phases 1–34` 100% complete — tagged `v2.0.0`)
- **Installed Binary**: `~/.local/bin/bilterm` & `~/.cargo/bin/bilterm` (`bilterm 2.0.0`)

## Completed in `v2.0.0`
1. **Phases 29–34 (`v2.0` Milestone)**:
   - Phase 29: Mainframe Block/Page-Hold (`MORE...`) & Smooth Kinetic Scroller (`SCRL-01..02`)
   - Phase 30: Root Shell Exit Guard & Remain-on-Exit Pane Protection (`SAFE-01..02`)
   - Phase 31: Universal Open Color Scheme & Theme Typography Engine (`THM-01..02`)
   - Phase 32: Persistent Background `Herdr` Multiplexer Daemon & Multi-Client Attach (`DAEM-01..02`)
   - Phase 33: Hierarchical `SessionTree` & Crash-Safe `WorkspaceJournal` (`TREE-01..02`, `PERS-01..02`)
   - Phase 34: TRON-Inspired Master Control Program (`MCP`) Root Terminal (`MCP-01..02`)
2. **Floating vs Push-Out `:mcp` Console Window & OIA Status Bar Mouse Control Isolation**:
   - `:mcp status`, `:mcp tree`, `:session tree`, and `:theme list` open in a scrollable Floating (`Overlay`) or Push-Out (`Push`, shrinking pane geometry on `Top`, `Bottom`, `Left`, or `Right`) window (`:mcp float`, `:mcp push [edge]`, `m` key, `d` key, `Esc`).
   - Mouse clicks on the 3270-OIA status bar row (`status_rect`) are strictly isolated from `OutputCanvas` selection/copy and act exclusively as interactive status bar controls.
3. **Full Multi-Pane Window Layout Detach & Re-Attach**:
   - `Ctrl+X D` / `:detach` saves the complete multi-pane `SplitTree`, `ZStack` z-order + custom `FloatingWindowGeometry`, `layout_mode`, `zoomed_pane`, per-pane `cwd`, `title`, prompt state, font scale, scroll mode, and screen output lines.
   - `bilterm attach [session]`, `bilterm -a [session]`, `:attach [session]`, or re-running `bilterm` after a detach automatically restores the exact multi-pane window layout.
4. **Comprehensive `<F1>` / `:help` Reference, Command Palette & `<Tab>` Completion**:
   - `src/ui/help_catalog.rs`, `src/ui/modals.rs`, and `src/pane/completion.rs` document and tab-complete every `:colon` command and `:mcp` subcommand.
