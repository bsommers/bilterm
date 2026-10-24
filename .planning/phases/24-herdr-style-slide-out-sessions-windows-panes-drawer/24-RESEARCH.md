# Phase 24 Research: `herdr`-Style Slide-Out Sessions/Windows/Panes Drawer with Status Glyphs & Float/Push Modes

**Phase:** 24 (`DRW-01`, `DRW-02`)
**Date:** 2026-10-24

## 1. Existing Seams & File-Size Constraints (`ARCH-R04`)

Every `src/**/*.rs` file must remain `<= 800` lines:
- `src/wm/mod.rs` is `788` lines — `WorkspaceDrawer` types, state, geometry calculation, telemetry builder, and `impl WindowManager` drawer methods MUST live in a dedicated submodule `src/wm/drawer.rs` (`pub mod drawer;`).
- `src/ui/mod.rs` is `705` lines — drawer rendering (`render_workspace_drawer` for `Left`/`Right` vertical tree and `Top`/`Bottom` horizontal ribbon) MUST live in `src/ui/drawer.rs` (`pub mod drawer;`), invoked from `WorkspaceWidget::render` in `src/ui/mod.rs`.
- `src/colon_command/execute.rs` is `743` lines — `Action::Drawer*` execution delegates to a single helper method `wm.execute_drawer_action(...)` defined in `src/wm/drawer.rs` so `execute.rs` adds `< 10` lines.

## 2. Geometry & `SIGWINCH` Invariant (`DRW-01`)

- **`DrawerEdge`**: `Left`, `Right` (vertical tree drawer, default width `30` cols, clamped `18..=60` and `<= area.width / 2`), `Top`, `Bottom` (horizontal ribbon/bar drawer, default height `5` rows, clamped `3..=14` and `<= area.height / 2`).
- **`DrawerVisibilityMode`**:
  - `OnDemand`: Hidden until explicitly opened via hotkey (`Alt+D` / `Ctrl+X d` / `Ctrl+B d` / `:drawer`), or focused.
  - `Fixed`: Always visible (pinned open).
  - `AutoHide`: Automatically reveals when:
    1. Leader key is pending (`wm.is_leader_pending()`),
    2. Mouse cursor hovers within 1 cell of the docked screen edge (`hover_visible`), or
    3. Any background (non-focused) pane has unread `OSC 9`/`777` notifications (`!pane.vt().notifications().is_empty()`) or has exited (`pane.has_exited()`).
- **`DrawerLayoutMode`**:
  - `Overlay`: Renders on top of `workspace_area` using `render_drop_shadow_styled` + `occlude_rect` + `render_borland_frame_styled`. Crucially, `compute_workspace_rects` and `resize_all` use the full `workspace_area`, so opening/closing/resizing an `Overlay` drawer emits **zero `SIGWINCH`** to child PTYs (`PANE-01` invariant).
  - `Push`: Subtracts `drawer_rect` from `workspace_area` (`effective_workspace_area(area)`) before `compute_workspace_rects` and `resize_all`, cleanly reflowing tiled/stacked panes into the remaining region.

## 3. High-Density Telemetry Status Glyphs (`DRW-02`)

Each row in the drawer represents a hierarchical node (`Session` -> `Layout/Window` -> `Pane` / `QuickTerminal` / `FleetNode`):
- **Tree connectors**: `├─` for intermediate children, `╰─` for last child.
- **Focus & Z-order rank**: `► [1] Z:1/4` for the focused pane (or `  [2] Z:2/4` for background panes, `[Q]` for Guake QuickTerminal).
- **Activity Braille Spinner & Throughput Sparkline**:
  - 10-frame Braille spinner (`['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏']`) indexed by `pane.vt().total_bytes_processed()` / tick counter.
  - 6-bar sparkline (`['▁', '▂', '▃', '▅', '▆', '▇']`) derived from recent byte throughput history per pane.
- **OSC 133 Command State**:
  - `⚙ RUN` when `pane.prompt().running_command.is_some()` or `Osc133Phase::CommandExecuted`
  - `✓ 0` when last `Osc133Phase::CommandFinished(Some(0))` (or last command block exit `Some(0)`)
  - `✗ <code>` when last `Osc133Phase::CommandFinished(Some(code))` with `code != 0`
- **Mode & Security Badges**:
  - `🖥 ALT` when `pane.vt().is_alternate_screen()`
  - `🔑 !ECHO` when `pane.is_password_prompt()`
  - `🔒 PROT` when `pane.is_read_only()`
  - `⏺ REC` when `pane.vt().is_recording()` or `wm.is_workspace_recording()`
  - `📡 BCAST` when `wm.is_broadcast_enabled()`
  - `🔔 N` when `pane.vt().notifications().len() > 0`
- **Transport Badges**:
  - `🌐 SSH` when `pane.remote_target().is_some()` or `FleetNodeKind::SshPane` / `SshTmux`
  - `🔗 TMUX` when `pane.title().contains("TMUX")` or `wm.is_tmux_control_attached()`

## 4. Interactive Navigation (`DRW-01`, `DRW-02`)

- **Keyboard (when drawer is focused)**:
  - `j` / `Down`, `k` / `Up`, `h` / `Left`, `l` / `Right`: move selection across drawer items
  - `Enter`: focus/raise selected pane and close drawerfocus (if `OnDemand`/`AutoHide`, also hide)
  - `Space`: preview/raise selected pane while keeping keyboard focus inside the drawer
  - `x`: close selected pane (`wm.remove_pane(id)`)
  - `z`: toggle zoom on selected pane
  - `d`: cycle docking edge (`Left` -> `Right` -> `Top` -> `Bottom` -> `Left`)
  - `m`: toggle layout mode (`Overlay` <-> `Push`)
  - `p`: toggle pin (`Fixed` <-> `AutoHide` / `OnDemand`)
  - `Esc` / `q`: unfocus/close drawer
- **Mouse**:
  - Moving cursor to docked screen edge triggers `AutoHide` hover reveal
  - Clicking a pane row focuses/raises that pane
  - Clicking header action badges (`[D:Left]`, `[M:Float/Push]`, `[P:Pin]`, `[▪]`) cycles edge, layout mode, pin state, or closes
  - Dragging the inner border of the drawer resizes its width (`Left`/`Right`) or height (`Top`/`Bottom`)
