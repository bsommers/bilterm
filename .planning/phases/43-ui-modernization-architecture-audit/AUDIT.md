# Phase 43 Audit — UI Modernization Step 1 (read-only)

Date: 2026-11-01. Baseline commit: c1da655 (v2.5.4), clippy -D warnings + cargo test -q green.

## Key findings
- GUI is **native Rust**: ratatui cell `Buffer` -> `GuiWorkspaceRenderer` (src/ui/renderer.rs) software rasterizer -> Win32 GDI / X11 (gui_font/linux_x11.rs) / macOS CoreText (gui_macos.rs). No web/frontend framework. No backdrop blur; glass = tint/bevel/halo/vector corners with opaque fallback.
- VT engine: `vt100-ctt` default behind `TerminalBackend` (src/vt/backend.rs); `libghostty-vt` optional experimental feature `ghostty-vt` (src/vt/ghostty_backend.rs). Additive path only; default untouched.
- Config: `BiltermConfig` (src/config/mod.rs, TOML). Style flags `snazzy_mode`, `glass_mode`, `modern_mode`, `liquid_glass` (70 refs / 10 files). Per-window `PaneChromeStyle` (src/wm/chrome_style.rs). `ThemePalette` (src/ui/theme.rs).
- Entry points for styles: `--modern/--liquid-glass/--glass`, `:style|:modern|:liquid`, F11. (`:ui-style` does not exist.)
- All commands lower to `Action` -> `WindowManager::apply`.
- Hard-coded `Color::Rgb` literals scattered (visual_style 37, oia 27, cursor_cmd 26, theme_import 24, modals 17, status_bar 16).
- No settings UI/live preview, no animation clock, no display modes yet.
- Docs drift: CLAUDE.md listed alacritty_terminal/tui-textarea-2/tree-sitter-bash/rustix (not in Cargo.toml) — fixed.

## Decisions (user, 2026-11-01)
- Commit v2.5.4 baseline first (done).
- ARCH-R04 relaxed: soft ~800-1000 lines, 1200 test backstop; modularity over fixed threshold.
- libghostty may be used where useful, as an additional path; do not remove/rewire existing paths; paths coexist.

## Plan
Phases 43-48 in .planning/ROADMAP.md map 1:1 to spec/Recommended execution sequence.
