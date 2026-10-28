# Phase 36 Plan: Interactive Mouse & Keyboard Navigation + Live Telemetry in `:mcp` Console Window (`v2.1`)

## Objective
Upgrade the `:mcp` floating/push-out console window (`:mcp status`, `:mcp tree`, `:session tree`, `:theme list`) from a read-only scrollable viewer into an interactive Borland/TRON control deck supporting `Up`/`Down` cursor selection, `Enter` and mouse-click row activation (focusing panes, attaching/freezing/branching session nodes, or applying themes live), action hotkeys (`r`, `f`, `n`, `m`, `d`), and live telemetry refresh.

## Architectural & Security Guardrails
- **`ARCH-R04`**: All `src/**/*.rs` files MUST remain `<= 800` lines. Note that `src/ui/mod.rs` is currently `799` lines! Extract `render_mcp_console_overlay` from `src/ui/mod.rs` into a dedicated `src/ui/mcp_ui.rs` submodule so `src/ui/mod.rs` drops to `~700` lines and `src/ui/mcp_ui.rs` has ample room for interactive row highlighting and mouse hit-testing.
- **`ARCH-R01`**: Route all triggered mutations through `WindowManager` methods and `ColonCommand`/`Action`.

---

## Wave Breakdown

### Wave 1 (`36-01`): Interactive Row Metadata & Selection Cursor (`src/mcp_console/mod.rs`, `src/wm/v2_runtime.rs`)
1. In `src/mcp_console/mod.rs`:
   - Define `#[derive(Clone, Debug, PartialEq, Eq)] pub enum McpRowAction`:
     - `None`
     - `FocusPane(usize)`
     - `AttachSession(String)`
     - `ApplyTheme(String)`
   - Add `pub selected_row: usize` and `pub row_actions: Vec<McpRowAction>` to `McpConsoleState`.
   - Add `pub fn select_prev(&mut self)`, `pub fn select_next(&mut self)`, `pub fn select_row(&mut self, idx: usize)`, and `pub fn selected_action(&self) -> Option<&McpRowAction>`.
2. In `src/wm/v2_runtime.rs`:
   - Populate `mcp_console.row_actions` automatically when building `:mcp status`, `:mcp tree`, `:session tree`, and `:theme list` lines:
     - Pane rows (`Pane #<id>`) -> `McpRowAction::FocusPane(id)`
     - Session tree rows (`◈ <node>` / `├─ <node>`) -> `McpRowAction::AttachSession(node)`
     - Theme preset rows (`• <name>`) -> `McpRowAction::ApplyTheme(name)`
   - Add `pub fn activate_selected_mcp_row(&mut self) -> bool` and `pub fn refresh_mcp_console_view(&mut self)`.

### Wave 2 (`36-02`): Extracted `src/ui/mcp_ui.rs` Renderer, Keyboard Hotkeys & Mouse Row Clicks (`src/ui/mcp_ui.rs`, `src/ui/mod.rs`, `src/input/modal.rs`, `src/input/mouse_wm.rs`)
1. Extract `render_mcp_console_overlay` from `src/ui/mod.rs` into `src/ui/mcp_ui.rs`:
   - Render the `selected_row` with a high-contrast Borland/TRON selection bar (`► ` prefix + inverted cyan/gold highlight) and display interactive footer hints (`Up/Down=Select | Enter/Click=Activate | r=Refresh | f=Freeze | n=Branch | m=Float/Push | d=Edge | Esc=Close`).
2. In `src/input/modal.rs`:
   - When `wm.mcp_console().active`:
     - `Up` / `k`: `select_prev()` (and auto-scroll viewport to keep `selected_row` visible)
     - `Down` / `j`: `select_next()` (and auto-scroll viewport)
     - `Enter`: `wm.activate_selected_mcp_row()`
     - `r` / `R`: `wm.refresh_mcp_console_view()`
     - `f` / `F`: toggle freeze/thaw on selected `AttachSession(node)` (or active session)
     - `n` / `N`: spawn child branch under selected `AttachSession(node)` (or active session)
3. In `src/input/mouse_wm.rs`:
   - Clicking a content row inside `mcp_rect` selects that row (`scroll_offset + rel_row`) and immediately activates its `McpRowAction` (`FocusPane`, `AttachSession`, or `ApplyTheme`).

### Wave 3 (`36-03`): Verification Suite (`tests/mcp_interactive_navigation.rs`)
1. Write `tests/mcp_interactive_navigation.rs` testing:
   - Keyboard `Up`/`Down`/`Enter` inside `:mcp tree` to focus a split pane (`McpRowAction::FocusPane`).
   - Keyboard `Up`/`Down`/`Enter` and mouse click inside `:theme list` to live-switch the active `ThemePreset` (`McpRowAction::ApplyTheme`).
   - Pressing `f` (freeze/thaw), `n` (branch child session), and `r` (refresh live telemetry) inside `:mcp tree`.
2. Verify `wc -l src/**/*.rs <= 800`, `cargo clippy -q --all-targets -- -D warnings`, and `cargo test -q`.
