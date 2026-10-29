# Bilterm Session Checkpoint (`v2.1.2` Release)

- **Timestamp**: 2026-10-28T22:38:00-04:00
- **Branch & Tag**: `main` at `v2.1.2` (pushed to `origin/main` with annotated tag `v2.1.2`)
- **Installed Binary**: `~/.local/bin/bilterm` (`bilterm 2.1.2`)

## 1. Completed in This Session
1. **Multi-Profile Layout Manager & Auto-Save (`4740534`)**:
   - Implemented `:layout save [name]`, `:layout load [name]`, `:layout default <name>`, `:layout autosave [on|off|toggle]`, `:layout list`, `:layout delete <name>` with startup auto-restore (`default_layout_profile = "default"`) and exit/periodic auto-save (`layout_autosave = true`).
2. **`.itermcolors` Crash Fix, Interactive Browser & RAW PTY Paste (`a32484c`, `v2.1.1` `9d64132`)**:
   - Fixed `nucleo-matcher 0.3.1` `fuzzy_optimal.rs:37` panic (`should have been caught by prefilter`) via case-consistent `Atom` matching in `src/wm/mod.rs` and `src/pane/prompt.rs`.
   - Added interactive `.itermcolors` picker modal (`:theme iterm2`, `:theme list`) + `Ctrl+Shift+V`, `Shift+Insert`, `Ctrl+X p`, `Ctrl+X v`, and `:paste` with bracketed-paste (`?2004h`) framing.
3. **WCAG 2.1 Highlighted Text Contrast Engine (`a60010a`, `src/ui/contrast.rs`)**:
   - Enforced `>= 5.2:1` contrast on highlighted/inverse cells and `>= 4.5:1` on standard text across all themes (including Borland Turbo Blue `#000084`) with per-theme ANSI 16-color RGB lookup tables and cross-hair contrast preservation (`src/ui/cursor.rs`).
4. **General Claude Code & CLI Agent Marker Rendering Fix (`a60010a`)**:
   - Added geometric + 5x7 bitmap + Xft fallback rendering for `❯` (`U+276F`), `❮`, `⏺` (`U+23FA`), `⎿` (`U+23BF`), `✓`, `✔`, `✗`, `✘`, `✢..✽`, `●`, `○`, `◆`, `◇`, `▸`, `▹`, `•`, `→`, `←`, `↑`, `↓`, `↳`, `↪` (`src/ui/gui_font/geometric.rs`, `src/ui/bitmap_font.rs`, `src/ui/gui_font/linux_x11.rs`).
   - Fixed `is_safe_terminal_cluster` Variation Selector (`U+FE0E`/`U+FE0F`) handling (`src/ui/mod.rs`), TrueColor SGR parameter skipping (`src/vt/scan.rs`), and split multi-byte UTF-8 chunk reassembly (`utf8_carry` in `src/vt/mod.rs`).
5. **Bounded Non-Blocking Clipboard I/O (`a60010a`, `src/clipboard.rs`)**:
   - Isolated integration test processes (`is_automated_test_process`) to the in-process clipboard and bounded `wl-copy`/`wl-paste`/`xclip`/`xsel` subprocess calls to 120ms/200ms.

## 2. Verification State
- `cargo clippy -q --all-targets -- -D warnings`: `0` warnings.
- `191` lib unit tests + integration suites (`full_suite_battle_and_perf`, `itermcolors_and_paste_fixes`, `layout_profiles_and_autosave`) passing.
- All `src/**/*.rs` files `<= 800` lines (`ARCH-R04`).

## 3. Exact Next Steps (When Resuming)
- Resume from `.planning/CHECKPOINT.md` (`git status --short` clean on `main` at `a60010a`).
