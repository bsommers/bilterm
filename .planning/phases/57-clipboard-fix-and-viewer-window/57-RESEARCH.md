# Phase 57: Clipboard Paste Bug Fix ("No image or video found in clipboard") & Multi-Session Filterable Clipboard Viewer Window — Research

**Researched:** 2026-11-07
**Domain:** PTY Clipboard Passthrough, DECSET 2004 Bracketed Paste, OS Clipboard MIME Detection, Multi-Session Clipboard History Registry, Non-Modal Floating/Dockable TUI & GUI Windowing
**Confidence:** HIGH (grounded in direct codebase inspection)

---

## 1. Root-Cause Diagnosis: Why Pasting in Herdr / Claude Code / Nested TUIs Displays `"No image or video found in clipboard"`

When a user runs `herdr`, Claude Code, Codex CLI, or another nested TUI inside a Bilterm window and attempts to paste text, the child application displays:
> `"No image or video found in clipboard"`

Codebase inspection across `src/input/`, `src/clipboard.rs`, `src/vt/`, and `src/ui/` identified **four compounding root causes**:

### Root Cause 1: Raw `Ctrl+V` (`0x16` / `CSI 118;5u`) Passthrough Trap in `RawPtyPassthrough` & `ALT_SCREEN`
- **Where:** `src/input/mod.rs:138-197`, `src/input/mod.rs:330`, `src/input/keymap.rs:629-637`, `src/input/encode.rs:50-100`.
- **Mechanism:**
  1. When a pane is in `CommandPrompt` mode (`src/input/mod.rs:330`), pressing `Ctrl+V` (`KeyCode::Char('v')` with `KeyModifiers::CONTROL`) calls `paste_clipboard_into_pane(pane)` — teaching users that `Ctrl+V` is Bilterm's standard paste shortcut.
  2. When `herdr`, Claude Code, Codex CLI, or another TUI starts, it enters `DECSET 1049` (`ALT_SCREEN`) or triggers raw passthrough, switching `pane.effective_routing_mode()` to `InputRoutingMode::RawPtyPassthrough`.
  3. In `handle_wm_key_event` (`src/input/mod.rs:635`), `RawPtyPassthrough` bypasses normal WM shortcuts and routes the key to `handle_key_event(wm.active_pane_mut(), raw_key)`.
  4. Inside `handle_key_event` (`src/input/mod.rs:172-196`), **only** `Ctrl+Shift+V` (`KeyCode::Char('V' | 'v')` with `CONTROL | SHIFT`) and `Shift+Insert` call `paste_clipboard_into_pane(pane)`. Plain `Ctrl+V` falls through to `encode_key_to_pty_bytes_full` (`src/input/encode.rs:50-100`), which sends literal byte `0x16` (`\x16`, ASCII `SYN`) — or Kitty keyboard protocol `\x1b[118;5u` when `vt.kitty_keyboard_flags() > 0` — directly to the child PTY!
  5. **Why Herdr / Claude Code prints `"No image or video found in clipboard"`:** Modern AI TUIs (Herdr, Claude Code, Codex CLI) rely on the host terminal emulator to send **text** pastes wrapped in **DECSET 2004 Bracketed Paste** (`\x1b[200~<text>\x1b[201~`). Because terminal emulators traditionally reserve `Ctrl+Shift+V` / `Cmd+V` for text paste, Herdr and Claude Code bind incoming `0x16` (`Ctrl+V`) to **"Paste Image or Video Attachment from OS Clipboard"**. Upon receiving `0x16`, Herdr/Claude Code shells out to `xclip -selection clipboard -t image/png -o` / `wl-paste --type image/png` / `osascript` / PowerShell to read an image or video from the OS clipboard. When the user's clipboard actually contains **text** (or only exists in Bilterm's internal yank buffer), that image probe fails and prints `"No image or video found in clipboard"`!

### Root Cause 2: X11 / Keymap `SHIFT` Modifier Normalization Conflating `Ctrl+Shift+V` with `Ctrl+V`
- **Where:** `src/ui/gui_font/linux_x11.rs:631-641`, `src/input/keymap.rs:28-50`.
- **Mechanism:**
  - In `linux_x11.rs:631-635`, `translate_x11_keysym_and_bytes` translates `0x20..=0x7E` when `ctrl || alt` is held using `if shift && !ctrl { ch.to_ascii_uppercase() } else { ch.to_ascii_lowercase() }`. Although `KeyModifiers::SHIFT` is preserved in `modifiers`, any code path or user keybinding table that normalizes `KeyCode::Char('V')` without preserving `KeyModifiers::SHIFT` (such as `default_global_action` in `src/input/keymap.rs:34-43` stripping `SHIFT` on `Char`) risks ambiguity between `Ctrl+V` and `Ctrl+Shift+V`.

### Root Cause 3: Right-Click & Middle-Click Paste Swallowed by SGR Mouse Tracking in `RawPtyPassthrough`
- **Where:** `src/input/mouse.rs:912-928` vs `src/input/mouse.rs:162-172`.
- **Mechanism:**
  - In `handle_mouse_event` (`src/input/mouse.rs:162-172`), `MouseEventKind::Down(MouseButton::Right | MouseButton::Middle)` pastes the clipboard into the pane.
  - However, in `handle_wm_mouse_event` (`src/input/mouse.rs:912-926`), when `pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough` and `pane.vt().is_mouse_sgr_or_tracking()` is true (which `herdr` and TUI apps enable via `DECSET 1000` / `1006`), `handle_wm_mouse_event` intercepts the click and sends raw SGR mouse report bytes (`\x1b[<2;col;rowM` or `\x1b[<1;col;rowM`) to the child PTY **before** `handle_mouse_event` is ever reached!
  - Consequently, neither `Shift+RightClick` / `Shift+MiddleClick` nor Middle-Click paste works when a TUI has mouse tracking enabled unless mouse bypass modifiers or smart paste routing are checked before SGR mouse forwarding.

### Root Cause 4: Inbound `OSC 52` & Internal Yank Desync with OS Clipboard Targets
- **Where:** `src/vt/scan_osc_dcs.rs:158-173`, `src/clipboard.rs:316-468`.
- **Mechanism:**
  1. When a child TUI (`herdr`, `tmux`, `neovim`, `helix`) copies text using `OSC 52` (`\x1b]52;c;<base64>\x07`), `VtParser::scan_osc_sequence` (`src/vt/scan_osc_dcs.rs:158-173`) only appends the raw escape sequence to `self.pending_osc52`. In TUI mode this is echoed to the outer terminal, and in native GUI mode (`--gui`) it is **completely discarded** — never decoded, never written to `crate::clipboard::set_system_clipboard`, and never recorded in Bilterm's clipboard history!
  2. In `src/clipboard.rs`, `get_system_clipboard()` only queries text and has no helper (`probe_system_clipboard_mime()`) to distinguish whether the OS clipboard currently holds **image/video data** (`image/png`, `image/jpeg`, `image/webp`, `image/gif`, `video/mp4`, `video/webm`, macOS `public.png`/`public.tiff`, Windows `CF_DIB`/`CF_HDROP`) versus **plain text** (`UTF8_STRING`, `text/plain`).

---

## 2. Solution Architecture for `CLIP-01`: Smart Clipboard Paste & OS MIME Detection

### 2.1 OS Clipboard MIME Probe (`crate::clipboard::ClipboardMimeKind`)
Add `probe_system_clipboard_mime() -> ClipboardMimeProbe` in `src/clipboard.rs`:
- **Linux Wayland (`WAYLAND_DISPLAY`):** `wl-paste --list-types` (bounded 80ms timeout). Checks for `image/*` or `video/*` vs `text/*` / `UTF8_STRING`.
- **Linux X11 (`DISPLAY`):** `xclip -selection clipboard -t TARGETS -o` (bounded 80ms timeout). Checks whether `TARGETS` advertises `image/png`, `image/jpeg`, `image/webp`, `image/gif`, `video/*`, or `text/uri-list` pointing to an image/video file vs `UTF8_STRING` / `STRING` / `TEXT` / `text/plain`.
- **macOS:** `osascript -e 'clipboard info'` or `NSPasteboard` inspection checking for `«class PNGf»` / `TIFF picture` / `public.png` vs `Unicode text` / `string`.
- **Windows:** PowerShell `[Windows.Forms.Clipboard]::ContainsImage()` / `ContainsFileDropList()` vs `ContainsText()`.
- **Test/Headless Override:** `set_test_clipboard_mime_override(Option<ClipboardMimeProbe>)` for deterministic unit and integration testing without requiring a live X11/Wayland server.

### 2.2 Smart `Ctrl+V` Routing in `RawPtyPassthrough` (`src/input/mod.rs`)
When `Ctrl+V` (`KeyCode::Char('v')` with `KeyModifiers::CONTROL`, without `ALT`) is pressed while `pane.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough`:
1. Probe the OS clipboard MIME via `crate::clipboard::probe_system_clipboard_mime()`.
2. **Case A — OS Clipboard Holds Image or Video (`ClipboardMimeProbe::Media { mime, .. }`):**
   - Record a metadata entry in `WindowManager::clipboard_registry` (`ClipboardMime::Image` / `ClipboardMime::Video`, source `OsClipboardSync`).
   - Forward raw `0x16` (`\x16` or Kitty `CSI 118;5u`) to the child PTY so `herdr` / Claude Code's native image/video attachment handler reads the media from the OS clipboard and succeeds!
3. **Case B — OS Clipboard or Bilterm Clipboard Holds Text (Default Smart Paste):**
   - Instead of sending `0x16` (which triggers Herdr's bogus `"No image or video found in clipboard"` error), call `paste_clipboard_into_pane(pane)`!
   - `paste_clipboard_into_pane(pane)` syncs text from `crate::clipboard::get_system_clipboard()` (falling back to `pane.prompt().yank_history.front()`), syncs it back to the OS clipboard if it only existed internally, and sends it to the child PTY wrapped in **DECSET 2004 Bracketed Paste** (`\x1b[200~<sanitized_text>\x1b[201~`) whenever `pane.vt().is_bracketed_paste()` is true (which Herdr, Claude Code, and modern shells enable).
4. **Escape Hatch for Literal `0x16` (`SYN` / Vim verbatim insert):**
   - Pressing `Ctrl+Q` followed by `Ctrl+V`, or `:paste raw-ctrl-v`, or setting `config.smart_ctrl_v_paste = false` forwards literal `0x16` to the child PTY.
5. **Mouse Paste Bypass in `RawPtyPassthrough` (`src/input/mouse.rs`):**
   - In `handle_wm_mouse_event`, `Shift+RightClick`, `Shift+MiddleClick`, or `MiddleClick` when `Shift` is held (plus `MiddleClick` when SGR mouse tracking is off) always invokes `paste_clipboard_into_pane` with `DECSET 2004` bracketed paste rather than being swallowed by SGR mouse reporting.
6. **Inbound `OSC 52` Decoding (`src/vt/scan_osc_dcs.rs`):**
   - Decode base64 payloads from `\x1b]52;<selection>;<base64>\x07` (when `<base64>` is not `?`), call `crate::clipboard::set_system_clipboard(&decoded)`, and push to the pane's yank history and `WindowManager::clipboard_registry` with `ClipboardSource::Osc52Inbound`.

---

## 3. Architecture for `CLIP-02..04`: Multi-Session Filterable `CLIPBOARD` Viewer Window

### 3.1 Existing State vs Required State
- **Current limitation (`src/colon_command/execute.rs:382-486`):** `:clipboard` currently reads `wm.active_pane().prompt().yank_history` (`VecDeque<String>`) and opens a static read-only `open_mcp_popup_window("CUT & PASTE BUFFER HISTORY", lines)`. It has no session or window attribution, no live filter tabs, no search query, no entry pinning, no preview pane, and no docking/pinning window lifecycle.
- **Required Architecture (`CLIP-02`, `CLIP-03`, `CLIP-04`):**
  - Follow the exact non-modal floating/pinnable/dockable window architecture established by `AgentsList` (`src/wm/agents_list.rs`), `CommandWindow` (`src/wm/command_window.rs`), and `SessionMap` (`src/wm/session_map.rs`).

### 3.2 Workspace-Wide `ClipboardRegistry` (`src/wm/clipboard_registry.rs`)
Owned by `WindowManager`, tracking up to `MAX_CLIPBOARD_ENTRIES = 200` entries across all sessions and panes:
- **Per-Entry Metadata (`ClipboardEntry`):**
  - `id: u64` (monotonic ID)
  - `created_at_ms: u64` & `timestamp_label: String` (`HH:MM:SS`)
  - `session_name: String` (the active or owning session name, e.g., `"main"`, `"herdr-dev"`)
  - `pane_id: Option<PaneId>` & `window_title: String` (e.g., `"#2 herdr"`, `"#1 zsh"`)
  - `source: ClipboardSource` (`PromptCopy`, `PromptCut`, `PromptKill`, `CanvasCopyMode`, `CanvasRectSelect`, `CanvasMouseSelect`, `UrlYank`, `HintYank`, `Osc52Inbound`, `OsClipboardSync`, `ManualCopy`)
  - `mime: ClipboardMime` (`TextPlain`, `Image { mime: String, bytes_len: usize }`, `Video { mime: String, bytes_len: usize }`)
  - `content: String` (raw text or media descriptor; deduplicated by moving matching `(content, session_name, pane_id)` to the front while preserving `pinned`)
  - `redacted: bool` & `redacted_kind: Option<&'static str>` (checked via `crate::pane::redact::classify(&content)` — if `Secret`, UI masks the preview by default unless toggled, and disk persistence in `clipboard.log` NEVER writes secret entries per `SEC-R01..04`)
  - `pinned: bool` (pinned entries survive `clear` and ring eviction)
  - `line_count: usize`, `char_count: usize`, `byte_len: usize`

### 3.3 Filter Scopes & Interactive Controls (`src/wm/clipboard_window.rs`)
The `CLIPBOARD` Viewer Window (`[ CLIPBOARD · <scope> · <count> items ]`) provides:
1. **Filter Scope Tabs (`ClipboardFilterScope`):**
   - `All`: Every entry across all sessions, windows, and OS clipboard syncs.
   - `Session`: Entries filtered to a target session (`target_session: Option<String>`, defaulting to the current `wm.session_name()`, with `s` cycling through all known session names in the registry + active sessions).
   - `Window`: Entries filtered to a target window/pane (`target_pane_id: Option<PaneId>`, defaulting to `wm.active_pane_id()`, with `w` cycling through all pane IDs present in the workspace/registry).
   - `Pinned`: Starred/pinned entries (`entry.pinned == true`).
   - `Media`: Image/video clipboard events and `OSC 52` / OS-synced entries.
2. **Live Search Filter (`search_query: String`, `search_focused: bool`):**
   - Press `/` to focus the inline filter input; filters across `content`, `session_name`, `window_title`, and `source.label()` (case-insensitive substring + fuzzy matching).
3. **Split List + Preview Layout:**
   - **Header Row 1:** Title + `[•]` Pin / `[□]` Dock / `[×]` Close buttons.
   - **Header Row 2 (Filter Bar):** Clickable filter tabs `[1:All]` `[2:Session(<name>)]` `[3:Window(#<id>)]` `[4:Pinned]` `[5:Media]` + `[s:Cycle Sess]` `[w:Cycle Win]`.
   - **Header Row 3 (Search Bar):** `Filter (/): <search_query>█` + `[Paste]` `[Copy OS]` `[Pin *]` `[Del]` `[Clear]` action chips.
   - **Body Upper (Entry Table):** Columns `#`, `Pin`, `Time`, `Session`, `Win`, `Src`, `Size`, `Snippet (1-line sanitized)`.
   - **Body Lower (Multi-Line Preview Pane):** Shows full multi-line preview of the selected entry (or masked secret warning if `redacted`), plus footer hint bar.
4. **Full TUI + Native GUI Parity (`UNI-R01`):**
   - TUI renderer in `src/ui/clipboard_window_ui.rs` + GUI vector card renderer in `src/ui/clipboard_window_gui.rs` (`CLIPBOARD_WINDOW_TEXTURE_ID: u64 = 980_000`).
   - Dockable into `PanelDock` (`DockPanel::Clipboard`) and scalable via `PanelTarget::Clipboard` (`75%..=200%`).
