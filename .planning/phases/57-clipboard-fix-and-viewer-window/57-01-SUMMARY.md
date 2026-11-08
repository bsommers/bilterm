# Phase 57-01 Summary: Herdr/TUI `Ctrl+V` Fix (`CLIP-01`) & `ClipboardRegistry` (`CLIP-02`)

## Delivered
- **`CLIP-01` Smart `Ctrl+V` Paste in `RawPtyPassthrough` / `ALT_SCREEN`**:
  - Added `ClipboardMimeProbe` (`Text`, `Media { mime, bytes_len }`, `Empty`), `probe_system_clipboard_mime()`, and `set_test_clipboard_mime_override()` in `src/clipboard.rs` across Wayland (`wl-paste --list-types`), X11 (`xclip -selection clipboard -t TARGETS -o`), macOS (`osascript`), and Windows (PowerShell `Get-Clipboard`).
  - Updated `handle_wm_key_event` in `src/input/mod.rs` so pressing `Ctrl+V` in `RawPtyPassthrough` checks `probe_system_clipboard_mime()`: when the OS clipboard contains image/video media (`ClipboardMimeProbe::Media`), forwards raw `0x16` (`\x16`) to the PTY and records a `ClipboardMime::MediaRef` entry; when the clipboard contains text (or the yank buffer is non-empty), routes through `paste_clipboard_into_pane` (`DECSET 2004` bracketed paste `\x1b[200~...\x1b[201~`).
  - Added `:paste raw-ctrl-v` (`:paste raw`) escape hatch to explicitly send raw `0x16` to the active pane PTY.
  - Added `Shift+RightClick` / `Shift+MiddleClick` SGR mouse tracking (`?1000h`/`?1006h`) paste bypass in `src/input/mouse.rs`.
  - Added inbound `OSC 52` base64 clipboard decoding (`\x1b]52;c;<base64>\x07`) in `src/vt/scan_osc_dcs.rs` and `src/pane/mod.rs`.
- **`CLIP-02` Multi-Session & Multi-Window `ClipboardRegistry` + Secret Redaction**:
  - Created `src/wm/clipboard_registry.rs` with `ClipboardSource`, `ClipboardMime`, `ClipboardFilterScope` (`All`, `Session`, `Window`, `Pinned`, `Media`), `ClipboardEntry`, and `ClipboardRegistry` (`MAX_CLIPBOARD_ENTRIES = 200`).
  - Integrated `crate::pane::redact::classify` so secret-matching entries are flagged `redacted = true`, masked in UI/RPC previews (`••• [REDACTED SECRET ...]`), and never written to disk (`src/pane/history_store.rs`).
