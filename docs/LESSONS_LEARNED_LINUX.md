# Bilterm Engineering Lessons Learned (Sourced from Linux)

> **Source Environment Provenance:**
> - **Sourced From:** **Linux** (`x86_64-unknown-linux-gnu`, POSIX PTY + Native X11 / Xft / Fontconfig `--gui` + Ratatui TUI)
> - **Scope:** Cross-platform engineering, architecture, debugging, UI/UX, typography, mouse window management, security, and testing lessons learned across the development of Bilterm (`v0.1.0` → `v1.12.0+`) and the Linux interactive debugging/documentation sessions.
> - **Companion Documents:** See **[`docs/LESSONS_LEARNED_COMPOUNDED.md`](LESSONS_LEARNED_COMPOUNDED.md)** (unified cross-platform synthesis & Milestone `v2.0` roadmap mapping) and **[`docs/LESSONS_LEARNED.md`](LESSONS_LEARNED.md)** (macOS-sourced retrospective).

[← Back to Documentation Hub](README.md) | [← Back to Project README](../README.md)

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Lesson 1: Guarding `Ord::clamp(min, max)` Against Dynamic Viewport & Font-Scale Inversion](#2-lesson-1-guarding-ordclampmin-max-against-dynamic-viewport--font-scale-inversion)
3. [Lesson 2: Per-Pane Independent Font Scaling Requires Inverse PTY/VT Grid Resizing (`TIOCSWINSZ`)](#3-lesson-2-per-pane-independent-font-scaling-requires-inverse-ptyvt-grid-resizing-tiocswinsz)
4. [Lesson 3: Intercepting WM Font-Zoom & Focus Chords Before `RawPtyPassthrough` (`nmon`, `htop`, `vim`)](#4-lesson-3-intercepting-wm-font-zoom--focus-chords-before-rawptypassthrough-nmon-htop-vim)
5. [Lesson 4: All-Border & All-Corner Multi-Pane Mouse Resizing (`SplitTree` vs. `ZStack`)](#5-lesson-4-all-border--all-corner-multi-pane-mouse-resizing-splittree-vs-zstack)
6. [Lesson 5: Zero-Header-Dependency Linux X11 + Xft/Fontconfig Native GUI via Runtime `dlopen`](#6-lesson-5-zero-header-dependency-linux-x11--xftfontconfig-native-gui-via-runtime-dlopen)
7. [Lesson 6: 3-Signal `!ECHO LOCK`, `asciinema` Harmony & `/dev/random` Escape-Trap Immunity](#7-lesson-6-3-signal-echo-lock-asciinema-harmony--devrandom-escape-trap-immunity)
8. [Lesson 7: Powerline / Nerd Font PUA Geometric Rasterization & Wide-Grapheme Occlusion](#8-lesson-7-powerline--nerd-font-pua-geometric-rasterization--wide-grapheme-occlusion)
9. [Lesson 8: Race-Free `0700`/`0600` Filesystem Confinement, Unix `SO_PEERCRED` & Non-Blocking PTY Reaping](#9-lesson-8-race-free-07000600-filesystem-confinement-unix-so_peercred--non-blocking-pty-reaping)
10. [Lesson 9: Verifiable Documentation, Starter-Pack Playbook Gates & Context-Window Discipline](#10-lesson-9-verifiable-documentation-starter-pack-playbook-gates--context-window-discipline)
11. [Summary Checklist for Future Contributors & Agents](#11-summary-checklist-for-future-contributors--agents)

---

## 1. Executive Summary

Building **Bilterm** — a composite-pane TUI multiplexer (`OutputCanvas` + `FloatingCommandPrompt` + `3270 OIA Bar`), native desktop GUI terminal (`--gui`), remote SSH/`tmux -CC` fleet manager, and MCP/JSON-RPC agent platform — surfaced subtle interactions between terminal line disciplines, binary space-partitioned (`SplitTree`) vs. stacked (`ZStack`) window geometry, per-pane typography, and OS graphics/PTY APIs.

Every bug discovered during live user testing on Linux led to both an architectural root-cause fix and a deterministic regression test in [`tests/linux_session_regression_and_gui_scaling.rs`](../tests/linux_session_regression_and_gui_scaling.rs).

---

## 2. Lesson 1: Guarding `Ord::clamp(min, max)` Against Dynamic Viewport & Font-Scale Inversion

### The Incident
Running `bilterm --gui --gui-font "JetBrains Mono" --gui-font-size 16` and repeatedly pressing `Ctrl+X` followed by `Ctrl+-` triggered a panic:
```text
Message: assertion failed: min <= max
Location: library/core/src/cmp.rs:1082
```

### Root Cause
In `src/pane/prompt_editor.rs`, `FloatingCommandPrompt::effective_height(canvas_height)` computed:
```rust
// BUG: panics when canvas_height is 1 or 2 because min (3) > max (1 or 2)!
self.height.clamp(3, canvas_height.max(1))
```
When multiple horizontal splits or font-scale changes reduced a pane's `canvas_height` below `3` rows, `3 > canvas_height.max(1)` violated `Ord::clamp`'s precondition (`min <= max`). Similar hazards exist anywhere a minimum dimension constant (`MIN_W = 16`, `MIN_H = 3` or `5`) is clamped against a dynamic container size (`canvas_width`, `bounds.width`, `bounds.height`).

### The Fix & Architectural Invariant
Always ensure `max >= min` before calling `.clamp(min, max)`, then cap to the actual container bound if necessary:
```rust
pub fn effective_height(&self, canvas_height: u16) -> u16 {
    if canvas_height == 0 {
        return 0;
    }
    self.height.clamp(3, canvas_height.max(3)).min(canvas_height)
}
```
- **Regression Test:** `small_canvas_repeated_ctrl_x_ctrl_minus_never_panics_on_clamp` in [`tests/linux_session_regression_and_gui_scaling.rs`](../tests/linux_session_regression_and_gui_scaling.rs).

---

## 3. Lesson 2: Per-Pane Independent Font Scaling Requires Inverse PTY/VT Grid Resizing (`TIOCSWINSZ`)

### The Incident
Users expected `Ctrl++` and `Ctrl+-` to independently scale **each pane's** font size and dynamically change that pane's terminal screen size (`cols × rows`) to fit its frame. Initially, `font_scale_pct` lived only on the workspace-global `BiltermConfig`, scaling all panes together and leaving individual pane grids static.

### Root Cause & Architectural Solution
For each composite pane in a multi-pane workspace to have its own independent font size while occupying a fixed tile or Z-Stack rectangle on screen:
1. **Per-Pane State (`PaneFontOptions`)**: `font_zoom_in()`, `font_zoom_out()`, `font_zoom_reset()`, and `font_set_size()` on `WindowManager` must mutate `active_pane_mut().font_options_mut().scale_pct` (`50%..=300%`) rather than only global config.
2. **Inverse Grid Scaling (`WindowManager::pane_scaled_inner_rect`)**:
   - If a pane's visual frame is `(W_cells, H_cells)` at `100%` base cell size `(cw, ch)`, and the pane's font is scaled to `S = scale_pct / 100.0`, each character cell in that pane is `(cw * S, ch * S)` pixels.
   - Therefore, the number of columns and rows that fit inside the pane's pixel rectangle scales **inversely** (`cols = round(W_cells * 100 / scale_pct)`, `rows = round(H_cells * 100 / scale_pct)`).
   - Zooming a pane's font **out** to `80%` gives its shell/TUI (`nmon`, `htop`) **`1.25×` more columns and rows**; zooming a pane's font **in** to `125%` renders larger glyphs with **`0.8×` columns and rows**, triggering `TIOCSWINSZ` (`SIGWINCH`) on that pane's PTY alone.
3. **Per-Run Font Size in `GuiWorkspaceRenderer`**:
   - `GuiWorkspaceRenderer` renders each pane into a temporary Ratatui `Buffer` sized to its `pane_scaled_inner_rect`, blits its cells into the pixel framebuffer using that pane's scaled `(cw_i, ch_i)`, and tags every `GuiGlyphRun` with `font_size_pt: pane_font_pt`.
   - Native GUI backends (`X11GuiCanvas` on Linux, `Win32GuiCanvas` on Windows, `MacOsGuiCanvas` on macOS) cache font handles keyed by `(family, font_size_pt, bold, italic)` so multiple font sizes render crisp TrueType glyphs simultaneously in a single frame.
4. **Mouse Coordinate Projection**:
   - In `handle_wm_mouse_event`, mouse `(column, row)` inside a scaled pane's `inner` rect must be proportionally mapped into `pane_scaled_inner_rect` before hitting the floating prompt or encoding SGR mouse sequences for the child PTY.

---

## 4. Lesson 3: Intercepting WM Font-Zoom & Focus Chords Before `RawPtyPassthrough` (`nmon`, `htop`, `vim`)

### The Incident
In a 3-way split (Pane #1 on the left, Pane #2 running `nmon` in the top-right, Pane #3 in the bottom-right), focusing the `nmon` pane and pressing `Ctrl++` or `Ctrl+-` did nothing.

### Root Cause
1. `nmon` switches the terminal to the alternate screen (`DECSET ?1049h`) and disables echo (`!ECHO`), placing Pane #2 into `InputRoutingMode::RawPtyPassthrough`.
2. In `src/input/mod.rs::handle_wm_key_event`, the `if in_raw_passthrough` branch forwarded all keys directly to the child PTY *before* `handle_wm_shortcut_key` (`DEFAULT_GLOBAL_BINDINGS`) was reached!
3. Additionally, on many Linux X11 and terminal keyboards, pressing `Ctrl+-` emits `KeyCode::Char('_')` or `KeyCode::Char('\x1f')` with `KeyModifiers::CONTROL`, and users also press `Ctrl+X Ctrl+-` (leader prefix + held `Ctrl`).

### The Fix & Architectural Invariant
1. In `src/input/keymap.rs::handle_wm_user_binding_or_leader_entry` (which executes **before** `in_raw_passthrough`), explicitly route `Action::FontZoomIn`, `Action::FontZoomOut`, `Action::FontZoomReset`, `Action::PaneResize`, `Action::PaneMove`, `Action::FocusPane`, `Action::DeckNext`, and `Action::DeckPrev` from `DEFAULT_GLOBAL_BINDINGS`.
2. Bind all terminal encodings of `Ctrl++` (`'+'`, `'='`), `Ctrl+-` (`'-'`, `'_'`, `'\x1f'`), and `Ctrl+0` (`'0'`, `')'`) in both `DEFAULT_GLOBAL_BINDINGS` and `handle_wm_leader_key`.
3. In `src/input/mouse.rs::handle_wm_mouse_event`, intercept `KeyModifiers::CONTROL` + `MouseEventKind::ScrollUp`/`ScrollDown` to focus the hovered pane and zoom its font before SGR mouse passthrough.
- **Regression Test:** `three_way_split_nmon_raw_passthrough_per_pane_font_scaling_and_pty_sigwinch` in [`tests/linux_session_regression_and_gui_scaling.rs`](../tests/linux_session_regression_and_gui_scaling.rs).

---

## 5. Lesson 4: All-Border & All-Corner Multi-Pane Mouse Resizing (`SplitTree` vs. `ZStack`)

### The Incident
Users reported that in multi-pane layouts, only bottom-right corners resized reliably, dragging top borders swapped panes instead of resizing, and dragging outer borders corrupted tiled split layouts.

### Root Causes & Fixes
1. **State Contamination Between `SplitTree` and `ZStack`**:
   - When `set_split_ratio_at` returned `false` (because the user dragged an outer border rather than an interior shared divider), the fallback code called `z_stack_mut().set_window_rect(...)` even while in `WorkspaceLayoutMode::Tiled`. Because `ZStack` had a custom rect stored, subsequent layout passes ignored `SplitTree` ratios for that pane!
   - **Rule**: Never write to `ZStack` custom window rects when `layout_mode == WorkspaceLayoutMode::Tiled`, and ignore `ZStack` custom rects in `Tiled` mode.
2. **Top-Border Resize vs. Title-Bar Move/Swap**:
   - Clicking `row == outer.y` in `Tiled` mode previously initiated `WmMouseDrag::MoveWindow` across almost the entire top border, so dragging the top border of a bottom pane (`Pane #3` under `Pane #2`) swapped the panes instead of moving the horizontal split divider between them!
   - **Rule**: In `Tiled` mode, `row == outer.y` initiates `WmMouseDrag::ResizeWindow` (`WindowResizeEdge::Top` / `TopLeft` / `TopRight`) unless `Alt` is held (`Alt+Drag` moves/swaps windows).
3. **Subtree-Aware `SplitTree::set_split_ratio_for_border`**:
   - In a 3-way split (`Vertical(Pane #1, Horizontal(Pane #2, Pane #3))`), `set_split_ratio_at` using only cursor proximity could grab the wrong axis.
   - We implemented `SplitTree::set_split_ratio_for_border(pane_id, direction, is_first_or_second_border, col, row, area)` which locates the exact ancestor `SplitNode::Split` along `direction` where `pane_id` borders the divider. When dragging an *outer* border (where no interior divider exists on that side), the drag delta is mapped onto the opposite side's divider so all 4 edges (`Top`, `Bottom`, `Left`, `Right`) and all 4 corners (`TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`) resize smoothly.
- **Regression Test:** `all_four_borders_and_all_four_corners_mouse_drag_resize_in_three_pane_tiled_and_stacked_layouts` in [`tests/linux_session_regression_and_gui_scaling.rs`](../tests/linux_session_regression_and_gui_scaling.rs).

---

## 6. Lesson 5: Zero-Header-Dependency Linux X11 + Xft/Fontconfig Native GUI via Runtime `dlopen`

### Context & Trade-Off
`bilterm --gui` needs native desktop windowing and anti-aliased TrueType/OpenType font rendering (`JetBrains Mono`, `IBM 3270`, `Fira Code`, custom `.ttf` files) on Linux, macOS, and Windows. However, adding build-time `pkg-config` + `-lX11 -lXft -lfontconfig` dependencies on Linux would break `cargo build` in minimal Docker/CI containers that do not have X11 development headers installed.

### Solution (`src/ui/gui_font/linux_x11.rs`)
- Load `libX11.so.6`, `libXft.so.2`, and `libfontconfig.so.1` dynamically at runtime via POSIX `dlopen` / `dlsym`.
- Register custom font files and `~/.config/bilterm/fonts/` via `FcConfigAppFontAddFile` / `FcConfigAppFontAddDir`.
- Render box-drawing (`U+2500..=U+257F`), block elements (`U+2580..=U+259F`), Braille (`U+2800..=U+28FF`), Sextant (`U+1FB00..=U+1FB3B`), and Powerline/Nerd Font PUA (`U+E0A0..=U+E0D7`, `U+E000..=U+F8FF`) procedurally with exact pixel geometry (`src/ui/gui_font/geometric.rs`) into the 32-bit BGRA `XImage` backbuffer first, then draw anti-aliased UTF-8 text runs on top via `XftDrawStringUtf8` over an X11 `Pixmap` before a single tear-free `XCopyArea` present.

---

## 7. Lesson 6: 3-Signal `!ECHO LOCK`, `asciinema` Harmony & `/dev/random` Escape-Trap Immunity

1. **Password Safety (`!ECHO LOCK`)**:
   - Inspecting `tcgetattr(master_fd)` alone is necessary but must be paired with `DECSET 1049` tracking and terminal prompt heuristics (`Password:`, `passphrase`, `[sudo]`) because wrappers like `asciinema rec` or `script` place their outer PTY in raw `!ECHO` mode permanently. Giving the user a manual `F4` / `F12` (`force_command_prompt`) override that still yields to active `Password:` regex matches achieves both `asciinema` usability and `sudo`/`ssh` password safety.
2. **Stream Trap Sanitization (`tail -f /dev/random`)**:
   - Random binary streams inevitably emit byte sequences matching `DECSET ?1049h` (enter alternate screen) or `DECSTBM` (`\x1b[<t>;<b>r` scroll regions). `VtParser::sanitize_stream_traps` neutralizes those sequences whenever the child process is in canonical shell mode (`ICANON && ECHO`), preventing binary output from hiding the floating prompt or locking the scroll region.

---

## 8. Lesson 7: Powerline / Nerd Font PUA Geometric Rasterization & Wide-Grapheme Occlusion

1. **Procedural Powerline & PUA Glyphs (`src/ui/gui_font/geometric.rs`)**:
   - Many system monospace fonts lack Powerline triangles (`U+E0B0..=U+E0B3`), rounded caps (`U+E0B4..=U+E0B7`), flame/pixel/ice separators (`U+E0C0..=U+E0D7`), or Nerd Font icons, causing OS font engines to render ugly `.notdef` / `LastResort` `[?]` boxes or leave 1px gaps between status-bar segments.
   - Rasterizing these codepoints procedurally to the exact `cell_w × cell_h` pixel box guarantees zero-gap status ribbons across every font and OS.
2. **Wide-Character (`unicode-width == 2`) Border Occlusion (`src/ui/occlusion.rs`)**:
   - Whenever a floating prompt, Z-Stack window, or drop shadow overlays an `OutputCanvas` containing 2-column CJK or Emoji graphemes, `occlude_rect` must clear any bisected wide character at `rect.x - 1` or `rect.x + rect.width` with `" "` so host terminals never shift border columns out of alignment.

---

## 9. Lesson 8: Race-Free `0700`/`0600` Filesystem Confinement, Unix `SO_PEERCRED` & Non-Blocking PTY Reaping

1. **Atomic Private File Writes (`src/secure_fs.rs`)**:
   - Never implement `write_private_file` by unlinking the target path and recreating it in place — concurrent panes or shell-integration hooks reading the file will fail with `ENOENT`. Always create a uniquely named sibling temp file with mode `0600` (`O_CREAT | O_EXCL | O_NOFOLLOW`), write + flush, and `fs::rename` atomically over the target path.
2. **Stale Unix Socket Detection (`src/api/server.rs`)**:
   - When binding `$XDG_RUNTIME_DIR/bilterm/bilterm.sock`, only unlink an existing socket file if a probe connect returns `ErrorKind::ConnectionRefused` or `ErrorKind::NotFound` — never unlink on `PermissionDenied`.
3. **Background PTY Reaper (`src/pty/mod.rs`)**:
   - `Child::kill()` and `Child::wait()` on PTY handles (especially on Windows ConPTY and heavily loaded POSIX shells) can block for tens of milliseconds. Offloading teardown to a detached `bilterm-pty-reaper` thread in `PtySession::drop` after closing the output channel keeps pane closure instantaneous.

---

## 10. Lesson 9: Verifiable Documentation, Starter-Pack Playbook Gates & Context-Window Discipline

1. **Test-Verified Starter Packs & Screenshots**:
   - Every TOML playbook under `starter-packs/*/playbook.toml` is executed by [`tests/linux_session_regression_and_gui_scaling.rs`](../tests/linux_session_regression_and_gui_scaling.rs) during `cargo test`, ensuring documentation examples never rot when `:colon` syntax evolves.
   - Watch out for test side-effects: `generates_real_workspace_screenshots_for_documentation` writes `docs/assets/screenshot-*.svg` on every `cargo test` run. Revert unchanged SVG timestamps (`git checkout -- docs/assets/screenshot-*.svg`) before committing non-documentation changes.
2. **Module Size Ceiling (`ARCH-R04 <= 800` lines) & Subagent Delegation (`PERF-CTX-03`)**:
   - Enforcing `<= 800` lines per `.rs` file via automated `awk` gates forced clean splits (`mouse.rs` + `mouse_wm.rs`, `parse.rs` + `parse_visual.rs`, `execute.rs` + `execute_wm.rs`, `gui_font/mod.rs` + `linux_x11.rs` / `win32_gdi.rs` / `macos_ct.rs` / `geometric.rs`).
   - Running `cargo clippy -q` and `cargo test -q` inside a `flash` subagent keeps raw compiler/test output out of the primary planning context window.

---

## 11. Summary Checklist for Future Contributors & Agents

- [ ] **Clamp Safety**: Every `.clamp(min, max)` on layout/canvas geometry enforces `min <= max` even on `0×0` or `1×1` viewports.
- [ ] **Per-Pane Scaling**: Font zoom (`Ctrl++`/`Ctrl+-`/`Ctrl+0`/`Ctrl+Wheel`) updates the target pane's `PaneFontOptions`, recalculates `pane_scaled_inner_rect`, and sends `TIOCSWINSZ` to that pane's PTY.
- [ ] **`RawPtyPassthrough` Escape Hatch**: Global window-manager resize/focus/font-zoom shortcuts execute before `in_raw_passthrough` so interactive TUIs (`nmon`, `htop`, `vim`) never trap window controls.
- [ ] **`Tiled` vs. `Stacked` Separation**: Mouse border drags in `Tiled` mode mutate only `SplitTree` ratios (`set_split_ratio_for_border`), never `ZStack` custom window rects.
- [ ] **File Budget (`ARCH-R04`)**: Run `awk 'ENDFILE { if (FNR > 800) { print FILENAME ": " FNR; bad=1 } } END { exit bad }' src/**/*.rs` before every commit.
