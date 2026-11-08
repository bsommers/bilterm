# Phase 57 Contract: Clipboard Paste Bug Fix & Multi-Session Filterable Clipboard Viewer Window

**Phase:** 57 (`57-clipboard-fix-and-viewer-window`)
**Requirements:** `CLIP-01`, `CLIP-02`, `CLIP-03`, `CLIP-04`
**Architecture Gates:** `ARCH-R01` (Unified `Action` Dispatch), `ARCH-R02` (Encapsulated State Invariants), `ARCH-R04` (Module Size Ceiling `<= 800` lines), `SEC-R01..04` (`secure_fs` + `pane::redact` secret masking), `UNI-R01` (Surface Uniformity across Mouse, Keys, Colon, Palette, Top Menu, JSON-RPC, MCP).

---

## 1. Requirement Definitions (`CLIP-01..04`)

- **`CLIP-01` — Smart Clipboard Paste & Bracketed Paste Fix for Herdr / Claude Code / TUIs:**
  1. Pressing `Ctrl+V` while a pane is in `InputRoutingMode::RawPtyPassthrough` (including `DECSET 1049` `ALT_SCREEN` apps such as `herdr`, Claude Code, and Codex CLI) MUST inspect the OS clipboard MIME type via `crate::clipboard::probe_system_clipboard_mime()`:
     - When the OS clipboard holds **text** (or no image/video MIME target is present and Bilterm's clipboard holds text), `Ctrl+V` MUST paste the text into the pane via `paste_clipboard_into_pane` wrapped in **DECSET 2004 Bracketed Paste** (`\x1b[200~<text>\x1b[201~` when `vt.is_bracketed_paste()` is true), preventing `herdr` / Claude Code from receiving raw `0x16` and printing `"No image or video found in clipboard"`.
     - When the OS clipboard holds an **image or video** (`image/png`, `image/jpeg`, `image/webp`, `image/gif`, `video/*`, or media file drop), `Ctrl+V` in `RawPtyPassthrough` MUST record a media entry in `ClipboardRegistry` and forward `0x16` (or Kitty `CSI 118;5u`) to the child PTY so the child TUI's native image/video paste handler succeeds.
     - Literal `0x16` passthrough remains accessible via `:paste raw-ctrl-v` (`Action::ClipboardWindow(ClipboardWindowOp::SendRawCtrlV)`) or when `config.smart_ctrl_v_paste == false`.
  2. In `RawPtyPassthrough` when `vt.is_mouse_sgr_or_tracking()` is enabled, `Shift+RightClick` and `Shift+MiddleClick` (and unshifted `RightClick`/`MiddleClick` when mouse tracking is off) MUST paste text into the pane rather than being swallowed by SGR mouse encoding.
  3. Inbound `OSC 52` clipboard sequences (`\x1b]52;...;<base64>\x07`) emitted by child PTY applications MUST be base64-decoded, written to the OS clipboard (`set_system_clipboard`), pushed to the pane's yank history, and recorded in `ClipboardRegistry` in both TUI and GUI modes.

- **`CLIP-02` — Multi-Session / Multi-Window Tagged `ClipboardRegistry` with Secret Redaction:**
  1. `WindowManager` owns a workspace-wide `ClipboardRegistry` (`src/wm/clipboard_registry.rs`, bounded at `MAX_CLIPBOARD_ENTRIES = 200` unpinned entries) that automatically captures every copy/cut/yank/kill/OSC-52/OS-sync event across all panes and sessions.
  2. Every `ClipboardEntry` records: `id`, `created_at_ms`, `timestamp_label`, `session_name`, `pane_id`, `window_title`, `source` (`ClipboardSource`), `mime` (`ClipboardMime`), `content`, `redacted` (classified via `crate::pane::redact::classify`), `pinned`, `line_count`, `char_count`, and `byte_len`.
  3. Entries classified as `RedactionClass::Secret` by `crate::pane::redact::classify` are marked `redacted: true`, masked in the UI list/preview by default, and excluded from disk persistence (`clipboard.log`) per `SEC-R01..04`.

- **`CLIP-03` — First-Class Floating, Pinnable, Dockable `CLIPBOARD` Viewer Window:**
  1. A non-modal floating window (`src/wm/clipboard_window.rs`, rendered in TUI via `src/ui/clipboard_window_ui.rs` and in native GUI via `src/ui/clipboard_window_gui.rs` with `CLIPBOARD_WINDOW_TEXTURE_ID = 980_000`) displaying the full clipboard history list and a multi-line / media-metadata preview pane.
  2. Filterable interactively by:
     - **Scope tabs (`ClipboardFilterScope`):** `All`, `Session` (active session or any selected session name), `Window` (active pane/window `#id` or any selected pane ID), `Pinned`, and `Media`.
     - **Live search query:** Substring/fuzzy matching across `content`, `session_name`, `window_title`, and `source`.
  3. Supports titlebar drag-move, 8-edge/corner resize, `[•]` Pin (`pinned`), `[□]` Dock (`DockPanel::Clipboard` in `PanelDock`), `[×]` Close, per-panel typography scale (`PanelTarget::Clipboard`, `75%..=200%`), entry paste (`Enter`), prompt-insert (`p`), OS clipboard sync (`y`), entry star/pin (`*`/`Space`), entry delete (`d`/`Delete`), and clear unpinned (`C`).

- **`CLIP-04` — `UNI-R01` Full Surface Uniformity:**
  - Every `CLIPBOARD` viewer and clipboard operation is reachable with identical semantics across Mouse (GUI & TUI), Keyboard (`Ctrl+Alt+V` reserved global shortcut + Leader `;` + in-window keys), `:clipboard` / `:clips` colon commands, Command Palette, Top Menu (`Edit -> Clipboard Viewer...`), JSON-RPC (`clipboard.*`), and MCP (`bilterm_clipboard`).

---

## 2. Frozen Types & `Action` Variants

### 2.1 `src/action.rs`
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardFilterScope {
    All,
    Session,
    Window,
    Pinned,
    Media,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardWindowOp {
    Open,
    Close,
    Toggle,
    Focus,
    Blur,
    PinToggle,
    PinSet(bool),
    DockToggle,
    DockSet(bool),
    SelectNext,
    SelectPrev,
    SelectIndex(usize),
    FilterScope(ClipboardFilterScope),
    FilterNextScope,
    FilterPrevScope,
    CycleSessionFilter,
    SetSessionFilter(Option<String>),
    CycleWindowFilter,
    SetWindowFilter(Option<PaneId>),
    SearchFocus,
    SearchInput(char),
    SearchBackspace,
    SearchClear,
    SearchSet(String),
    PasteSelected,
    PasteIndex(usize),
    PasteToPromptSelected,
    SyncSelectedToOs,
    ToggleStarSelected,
    ToggleStarIndex(usize),
    ToggleRevealSecret,
    DeleteSelected,
    DeleteIndex(usize),
    ClearHistory,
    CopyText(String),
    PopLatest,
    PersistSet(bool),
    SendRawCtrlV,
    MoveBy { dx: i16, dy: i16 },
    ResizeBy { dw: i16, dh: i16 },
    ScaleSet(u16),
    ScaleDelta(i16),
    ScaleReset,
}
```
And in `pub enum Action`:
```rust
    /// Operate or toggle the non-modal floating/dockable CLIPBOARD Viewer Window (`CLIP-01..04`).
    ClipboardWindow(ClipboardWindowOp),
```

### 2.2 `src/wm/clipboard_registry.rs`
```rust
pub const MAX_CLIPBOARD_ENTRIES: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardSource {
    PromptCopy,
    PromptCut,
    PromptKill,
    CanvasCopyMode,
    CanvasRectSelect,
    CanvasMouseSelect,
    UrlYank,
    HintYank,
    Osc52Inbound,
    OsClipboardSync,
    ManualCopy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardMime {
    TextPlain,
    Image { mime: String, bytes_len: usize },
    Video { mime: String, bytes_len: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardEntry {
    pub id: u64,
    pub created_at_ms: u64,
    pub timestamp_label: String,
    pub session_name: String,
    pub pane_id: Option<PaneId>,
    pub window_title: String,
    pub source: ClipboardSource,
    pub mime: ClipboardMime,
    pub content: String,
    pub redacted: bool,
    pub pinned: bool,
    pub line_count: usize,
    pub char_count: usize,
    pub byte_len: usize,
}
```

---

## 3. Surface Uniformity Matrix (`UNI-R01` / `CLIP-04`)

| Capability | Mouse (TUI & GUI) | Keyboard (TUI & GUI) | Colon Command | Palette / Top Menu | JSON-RPC / MCP |
|------------|-------------------|----------------------|---------------|--------------------|----------------|
| Toggle / Open `CLIPBOARD` Window | Top Menu `Edit -> Clipboard Viewer...` or OIA / Dock pill click | `Ctrl+Alt+V` (reserved global, works in `RawPtyPassthrough`) or Leader `;` | `:clipboard` / `:clips` / `:clipboard toggle` / `:clipboard open` | Palette: `Clipboard Viewer: Toggle Window` | RPC `clipboard.window` (`op: "toggle"` / `"open"`), MCP `bilterm_clipboard` |
| Close / Blur `CLIPBOARD` Window | Click `[×]` on titlebar | `Esc` / `q` (when focused) | `:clipboard close` / `:clipboard blur` | Palette: `Clipboard Viewer: Close Window` | RPC `clipboard.window` (`op: "close"`) |
| Pin / Unpin `CLIPBOARD` Window | Click `[•]` on titlebar | `P` (in window) | `:clipboard pin [on\|off\|toggle]` | Palette: `Clipboard Viewer: Toggle Pin` | RPC `clipboard.window` (`op: "pin"`) |
| Dock / Undock `CLIPBOARD` Window | Click `[□]` on titlebar | `D` (in window) | `:clipboard dock [on\|off\|toggle]` | Palette: `Clipboard Viewer: Toggle Dock` | RPC `clipboard.window` (`op: "dock"`) |
| Filter by Scope (`All`/`Session`/`Window`/`Pinned`/`Media`) | Click `[1:All]` `[2:Session]` `[3:Window]` `[4:Pinned]` `[5:Media]` tabs | `Tab` / `BackTab` or `1`..`5` (in window) | `:clipboard filter <all\|session\|window\|pinned\|media>` | Palette: `Clipboard Viewer: Filter by Current Session` / `Window` | RPC `clipboard.list` / `clipboard.window` (`filter`) |
| Cycle Specific Session / Window Filter | Click `[s:Sess ▾]` / `[w:Win ▾]` pill | `s` (cycle session), `w` (cycle window) | `:clipboard session <name\|active\|all>`, `:clipboard window <id\|active\|all>` | Palette: `Clipboard Viewer: Cycle Session Filter` | RPC `clipboard.window` (`session` / `window`) |
| Live Search / Query Filter | Click search bar `Filter (/):` | `/` + type query, `Esc` clears search focus | `:clipboard search <query>` | — | RPC `clipboard.list` (`query`) |
| Paste Entry into Active Pane (Smart Bracketed Paste) | Double-click row or click `[Paste]` chip | `Enter` (in window), `Ctrl+V` / `Ctrl+Shift+V` (in pane) | `:clipboard paste [1_based_idx]` or `:clipboard <idx>` | Palette: `Paste from System Clipboard` | RPC `clipboard.paste` (`index` / `id`), MCP `bilterm_clipboard` |
| Pin/Star Entry / Delete Entry / Clear | Click `[*]` on row / `[Pin *]` / `[Del]` / `[Clear]` chips | `*` or `Space` (star), `d`/`Delete` (delete), `C` (clear) | `:clipboard star [idx]`, `:clipboard delete [idx]`, `:clipboard clear` | Palette: `Clipboard Viewer: Clear Unpinned History` | RPC `clipboard.pin_entry`, `clipboard.delete`, `clipboard.clear` |

---

## 4. Backward Compatibility Guarantees

All existing `:clipboard` tests and behaviors MUST continue to pass unchanged:
- `:clipboard persist on|off` -> sets `wm.clipboard_persist` and writes/removes `clipboard.log` via `secure_fs`.
- `:clipboard clear` -> clears unpinned entries in `ClipboardRegistry` AND clears `active_pane().prompt_mut().yank_history`.
- `:clipboard copy <text>` -> sets OS clipboard, pushes to `active_pane().prompt_mut().yank_history`, and records in `ClipboardRegistry`.
- `:clipboard pop` -> pops front of `yank_history` & `ClipboardRegistry` and syncs new front to OS clipboard.
- `:clipboard paste <1_based_idx>` or `:clipboard <1_based_idx>` -> pastes the Nth item into the active pane.
