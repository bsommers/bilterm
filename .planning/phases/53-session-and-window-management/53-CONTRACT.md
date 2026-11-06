# Phase 53 Frozen Contract: Session & Window Management (`SES-01..05`, `UNI-R01`)

Authored for Phase 53 of Bilterm under `AGENTS.md` conventions:
- `ARCH-R01`: Unified Action dispatch (every key, menu, colon command, RPC, MCP, or CLI path lowers to typed `Action` and dispatches via `WindowManager::apply`).
- `ARCH-R02`: Encapsulated state on `WindowManager` (never expose internal topologies; mutations happen through dedicated methods).
- `ARCH-R04`: Modular source files targeting 800–1000 lines (soft goal; test backstop 1200 lines).
- `UNI-R01`: Surface uniformity across keyboard, mouse, colon commands, RPC/MCP APIs, and `bilterm ctl`.
- `SEC-R01..04`: Filesystem and credential confinement via `secure_fs` (`validate_safe_name`, `0700`/`0600` permissions).

Every Phase 53 plan reads this contract. Plans 53-01, 53-02, and 53-03 build against these frozen definitions.

---

## 1. Architectural Decisions

- **D-01 Hierarchy Definition (`SES-01`)**:
  - The Bilterm workspace hierarchy is formalized as: `Workspace > Session (supports arbitrary nesting via SessionTree) > Window (CompositePane) > Pane`.
  - A "Window" in Bilterm corresponds to a top-level composite pane unit (having its own PTY, VTE terminal canvas, floating command prompt, 3270 OIA status bar, chrome styling, and z-order index).
  - Nested sessions are addressed by hierarchical path: `parent/child` or leaf name when unambiguous.

- **D-02 Unified Action Dispatch (`ARCH-R01`, `SES-05`)**:
  - All window management operations lower to `Action::WindowManage(WindowManageOp)`.
  - All session management operations lower to `Action::SessionManage(SessionManageOp)`.
  - Both dispatch through `WindowManager::apply(&mut self, action: Action) -> Result<AppAction>`, ensuring identical behavior whether invoked via keyboard chord, mouse click/menu, colon command, JSON-RPC, MCP, or `bilterm ctl`.

- **D-03 Non-Destructive Window Migration (`SES-01`)**:
  - Moving a window between sessions transfers the live `CompositePane` instance intact.
  - The underlying PTY process, file descriptors, VTE screen buffer, ring scrollback history, active running command (`running_command`), prompt input, and history are preserved without process termination, respawn, or data truncation.
  - Moving a window into the current active session is a no-op that returns an explicit error (`"Window <id> is already in session '<name>'"`); moving into a non-existent session returns an explicit error (`"Target session '<name>' not found"`).
  - Post-move topology invariants:
    - Source session: The moved window is removed from the source `split_tree`, `z_stack`, and `zoomed_pane`. If the moved window was focused, focus falls back to the top-most remaining window in `z_order`. If the source session would become empty, a fresh default shell window is created or the move is prevented if configured to keep sessions non-empty.
    - Target session: The window is inserted into the target session's `z_stack` (placed at the top of z-order) and tiled split tree, and gains focus if the user switches with `--switch` or if specified by policy.

- **D-04 Name Validation & Deterministic Disambiguation (`SES-02`)**:
  - All window and session names must pass `validate_safe_name` (no path traversal `..`, slashes `/` or `\`, control chars, NUL; max length 64 chars). For nested session paths (`a/b/c`), each segment must individually satisfy `validate_safe_name`.
  - Duplicate rejection: If a rename conflicts with an existing window or session name in the same parent scope, the operation returns an explicit error unless disambiguation is requested, in which case deterministic suffix incrementation (`<name>-2`, `<name>-3`) is applied.
  - Display rendering: All names rendered in TUI chrome, title bars, status bar, and Navigator drawer are sanitized via `sanitize_display` to prevent terminal escape injection.

- **D-05 Multi-Selection & All-or-Nothing Bulk Semantics (`SES-03`)**:
  - `WindowManager` maintains an active selection set per session: `selected_windows: BTreeSet<PaneId>`.
  - Selection is non-modal: selecting windows does not lock out normal terminal input unless explicitly operating on the selection.
  - Bulk operations:
    - `Move`: Moves all selected windows to a target session in their current z-order.
    - `Close`: Closes all selected windows, strictly honoring `ExitProtectionPolicy` (`SAFE-01`). If any window has a protected running process, the bulk close is halted before closing any protected windows and prompts the user or returns an error.
    - `RenamePrefix`: Prepends a sanitized prefix to all selected windows (`prefix + original_name`).
    - `Tile`: Rearranges all selected windows into an optimal binary split layout.
    - `Pin`: Toggles or sets pin status for all selected windows.
  - All-or-Nothing: If any window in a bulk operation fails validation (e.g. invalid target, last window orphan constraint, exit guard refusal), the entire bulk operation is aborted and rolled back.

- **D-06 Single-Step Undo Stack (`SES-04`)**:
  - `WindowManager` maintains a single-step undo slot `topology_undo: Option<WorkspaceTopologyUndo>`.
  - Captured before any mutating move, rename, or bulk action:
    - Source and target session placements.
    - Window names and IDs.
    - Z-order sequences and split tree topologies.
    - Active selection set.
  - Executing `:window undo` / `:session undo` or `Action::WindowManage(WindowManageOp::Undo)` restores the exact prior topology and selection.
  - The undo stack explicitly rejects operations that would orphan a pane or leave a session in an invalid state.

- **D-07 Persistence & Round-Trip Compatibility (`SES-04`)**:
  - Saved workspace sessions (`SavedWorkspaceSession`) in TOML and daemon session journals persist window names, custom order, and multi-selection state.
  - Backwards compatibility: Existing `SavedWorkspaceSession` TOML files without window name or selection fields deserialize cleanly using `#[serde(default)]` without breaking older setups.

- **D-08 Full Surface Uniformity (`SES-05`, `UNI-R01`)**:
  - Keys: Global chords (leader `Ctrl+X m`, `Ctrl+X r`), Navigator tree bindings (`Space` to toggle selection, `Shift+Up/Down` for range selection, `Ctrl+A` for select-all, `r` for rename, `m` for move, `x` for close).
  - Mouse: Left click, `Ctrl+Click` multi-select toggle, right-click context menu (with `Move to session...`, `Rename`, `Multi-select` sub-items).
  - Colon commands: Complete `:window` and `:session` command grammar.
  - JSON-RPC & MCP: `window.move`, `window.rename`, `session.rename`, `window.select`, `window.bulk`, `window.undo` methods; MCP tool `bilterm_window_manage`.
  - CLI: `bilterm ctl` subcommands matching RPC methods.

---

## 2. Core Enums and Data Structures

### 2.1 `WindowManageOp`
```rust
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WindowManageOp {
    /// Moves a window (default: active window) to target_session.
    Move {
        window_id: Option<PaneId>,
        target_session: String,
        switch: bool,
    },
    /// Renames a window (default: active window).
    Rename {
        window_id: Option<PaneId>,
        new_name: String,
    },
    /// Updates the multi-selection state for windows.
    Select(WindowSelectTarget),
    /// Executes an atomic bulk operation on currently selected windows.
    Bulk(WindowBulkOp),
    /// Reverts the last window/session topology mutation.
    Undo,
}
```

### 2.2 `SessionManageOp`
```rust
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SessionManageOp {
    /// Renames a session (default: active session if old_name is None).
    Rename {
        old_name: Option<String>,
        new_name: String,
    },
    /// Moves session position in user-defined ordering.
    MoveOrder {
        name: Option<String>,
        target_pos: String,
    },
    /// Reverts the last session topology mutation.
    Undo,
}
```

### 2.3 `WindowSelectTarget`
```rust
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WindowSelectTarget {
    /// Toggles selection of a specific window.
    Toggle(PaneId),
    /// Selects a contiguous range of windows between from and to in visual order.
    Range { from: PaneId, to: PaneId },
    /// Selects all windows in the current session.
    All,
    /// Clears window selection.
    None,
    /// Inverts window selection in the current session.
    Invert,
    /// Sets selection explicitly to the given set of window IDs.
    Explicit(Vec<PaneId>),
}
```

### 2.4 `WindowBulkOp`
```rust
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WindowBulkOp {
    /// Moves all selected windows to target_session atomically.
    Move { target_session: String },
    /// Closes all selected windows, honoring ExitProtectionPolicy.
    Close,
    /// Prepends prefix to all selected window names.
    RenamePrefix { prefix: String },
    /// Tiles all selected windows into the active workspace split layout.
    Tile,
    /// Sets or toggles pinned status for all selected windows.
    Pin(Option<bool>),
}
```

### 2.5 `WorkspaceTopologyUndo`
```rust
#[derive(Clone, Debug)]
pub struct WorkspaceTopologyUndo {
    pub description: String,
    pub timestamp: std::time::Instant,
    pub active_session: String,
    pub affected_sessions: Vec<String>,
    pub snapshot_before: std::collections::BTreeMap<String, crate::wm::SavedWorkspaceSession>,
    pub selection_before: std::collections::BTreeSet<PaneId>,
}
```

---

## 3. Colon Command Grammar

### 3.1 `:window` Commands
- `:window move <session>`: Move active window to `<session>`.
- `:window move <id> <session>`: Move window `<id>` to `<session>`.
- `:window move --switch <session>`: Move active window to `<session>` and switch active session to `<session>`.
- `:window rename <new_name>`: Rename active window.
- `:window rename <id> <new_name>`: Rename window `<id>` to `<new_name>`.
- `:window select toggle <id>`: Toggle selection of window `<id>`.
- `:window select all`: Select all windows in active session.
- `:window select none` / `:window select clear`: Clear window selection.
- `:window select invert`: Invert window selection in active session.
- `:window select range <from_id> <to_id>`: Select range from `<from_id>` to `<to_id>`.
- `:window bulk move <session>`: Bulk-move selected windows to `<session>`.
- `:window bulk close`: Bulk-close selected windows (honors exit guard).
- `:window bulk close --force`: Force bulk-close bypassing exit guard.
- `:window bulk rename-prefix <prefix>`: Prefix names of selected windows.
- `:window bulk tile`: Tile selected windows.
- `:window bulk pin [on|off]`: Set pin status of selected windows.
- `:window undo`: Revert last window or session operation.

### 3.2 `:session` Commands
- `:session rename <new_name>`: Rename active session.
- `:session rename <old_name> <new_name>`: Rename session `<old_name>` to `<new_name>`.
- `:session move [name] <up|down|top|bottom|1..N>`: Reorder session (preserves existing Phase 38 syntax).
- `:session undo`: Revert last session or window operation.

---

## 4. Navigator Drawer Integration

### 4.1 Key Bindings in Navigator Style
- `Space`: Toggle selection of the highlighted window row.
- `Shift+Down` / `Shift+Up`: Extend selection range downward / upward.
- `Ctrl+A`: Select all windows in the current session.
- `r` / `R`: Inline rename prompt prefilled with current item name.
- `m`: Move prompt prefilled with `:window move `.
- `x` / `Delete`: Close window (or bulk close if multi-selection active).
- `Esc`: Clear multi-selection if active; otherwise close menu or unfocus drawer.

### 4.2 Right-Click Context Menu
Adds the following entries to `NavigatorMenuEntry`:
1. `MoveWindow`: "Move to session..." (prompts or opens session picker submenu).
2. `Rename`: "Rename" (triggers inline rename or `:window rename`).
3. `BulkActions` (shown when multi-selection > 1):
   - "Bulk move..."
   - "Bulk close"
   - "Bulk tile"

---

## 5. JSON-RPC & MCP Surface Parity (`SES-05`)

### 5.1 JSON-RPC Methods
- `window.move`:
  - Request: `{"window_id"?: number, "target_session": string, "switch"?: boolean}`
  - Response: `{"status": "ok", "window_id": number, "source_session": string, "target_session": string}`
- `window.rename`:
  - Request: `{"window_id"?: number, "new_name": string}`
  - Response: `{"status": "ok", "window_id": number, "name": string}`
- `session.rename`:
  - Request: `{"old_name"?: string, "new_name": string}`
  - Response: `{"status": "ok", "old_name": string, "new_name": string}`
- `window.select`:
  - Request: `{"action": "toggle"|"range"|"all"|"none"|"invert"|"explicit", "window_id"?: number, "from"?: number, "to"?: number, "windows"?: number[]}`
  - Response: `{"status": "ok", "selected": number[], "count": number}`
- `window.bulk`:
  - Request: `{"action": "move"|"close"|"rename_prefix"|"tile"|"pin", "target_session"?: string, "prefix"?: string, "pinned"?: boolean, "force"?: boolean}`
  - Response: `{"status": "ok", "action": string, "affected_windows": number[], "count": number}`
- `window.undo`:
  - Request: `{}`
  - Response: `{"status": "ok", "reverted": string}`

### 5.2 MCP Tool
- `bilterm_window_manage`: Unified MCP tool taking action (`"move"`, `"rename"`, `"select"`, `"bulk"`, `"undo"`) with validated parameters.

### 5.3 CLI (`bilterm ctl`)
- Direct invocation matching JSON-RPC:
  - `bilterm ctl window.move '{"window_id": 1, "target_session": "dev"}'`
  - `bilterm ctl window.rename '{"window_id": 1, "new_name": "editor"}'`
  - `bilterm ctl session.rename '{"old_name": "main", "new_name": "primary"}'`
  - `bilterm ctl window.select '{"action": "all"}'`
  - `bilterm ctl window.bulk '{"action": "tile"}'`
  - `bilterm ctl window.undo '{}'`

---

## 6. Verification and Acceptance Gates

- **Unit and Seam Suite (`tests/phase53_seam.rs`)**:
  - `WindowManageOp` and `SessionManageOp` serialization/deserialization.
  - Colon command parser round-trip for all `:window` and `:session` variants.
  - `validate_safe_name` enforcement against traversal and injection tokens.
  - Action reachability test in `src/action.rs`.
- **Core Logic Suite (`tests/phase53_window_manage.rs`)**:
  - Move active window from active session to background session preserving PTY state, scrollback lines, and running process.
  - Move window to nested session (`parent/child`).
  - Error when moving to current session or unknown target.
  - Focus and z-order fallback on window departure.
  - Window rename and deterministic disambiguation.
  - Multi-select toggle, range, all, none, invert.
  - Bulk move, bulk close with exit guard protection refusal, bulk rename-prefix, bulk tile.
  - Single-step undo of move, rename, and bulk actions.
  - TOML and journal round-trip persistence.
- **Surface Parity Suite (`tests/phase53_parity.rs`)**:
  - Parity across key sequence, mouse gesture, colon command, and JSON-RPC for every operation.
  - MCP tool invocation.
  - Navigator row badge and selection rendering.
