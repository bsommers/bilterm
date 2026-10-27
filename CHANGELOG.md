# Changelog

All notable changes to **Bilterm** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v1.12.0] - 2026-10-26

### Added
- **Native Linux/Unix X11 + Xft/Fontconfig Desktop GUI Window (`src/ui/gui_font/linux_x11.rs`, `src/ui/gui_font/mod.rs`, `src/ui/gui_window.rs`)**:
  - Added runtime `dlopen`-loaded `libX11`, `libXft`, and `libfontconfig` native desktop GUI backend for `bilterm --gui` (`-G`) on Linux/Unix with zero hard build-time C library dependencies.
  - Supports anti-aliased TrueType/OpenType monospace font rendering (`XftFontOpenName` / `XftDrawStringUtf8`), custom `.ttf`/`.otf` registration (`FcConfigAppFontAddFile` / `FcConfigAppFontAddDir`), crisp geometric box-drawing/Braille/Sextant rasterization, full keyboard/mouse/wheel event routing, and live window resize (`ConfigureNotify`).
- **Independent Per-Pane Font Scaling & Inverse PTY/VT Screen Resizing (`src/ui/typography.rs`, `src/ui/renderer.rs`, `src/wm/mod.rs`, `src/input/keymap.rs`)**:
  - `Ctrl++` / `Ctrl+=`, `Ctrl+-` / `Ctrl+_`, `Ctrl+0`, and `Ctrl+Wheel` now scale each pane's font independently (`PaneFontOptions::scale_pct`), including when focused on interactive `RawPtyPassthrough` / `DECSET 1049` TUI apps (`nmon`, `htop`, `vim`) or after a `Ctrl+X`/`Ctrl+B` leader prefix.
  - Each pane's PTY (`SIGWINCH`) and `VtParser` viewport `(rows, cols)` dynamically scales inversely to its font scale (`pane_scaled_inner_rect`), and `GuiWorkspaceRenderer` + `X11GuiCanvas` / `Win32GuiCanvas` render each pane at its own scaled cell dimensions and cached TrueType point size (`GuiGlyphRun::font_size_pt`).
- **All-Border & All-Corner Multi-Pane Frame Mouse Resizing (`src/input/mouse.rs`, `src/wm/split_tree.rs`, `src/wm/z_stack.rs`)**:
  - Added `WindowResizeEdge::TopLeft` and `TopRight` and enabled dragging all 4 borders (`Top`, `Bottom`, `Left`, `Right`) and all 4 corners across both `Tiled` and `Stacked` multi-pane layouts.
  - Added `SplitTree::set_split_ratio_for_border` and outer-border delta mapping so dragging top borders in `Tiled` mode resizes the horizontal divider instead of swapping panes, and dragging outer borders or corners resizes the corresponding split without collapsing sibling axes.
- **Extended CLI Flags, 10-Chapter User Guide, SVG Screen Renders & 5 Starter Packs (`src/main.rs`, `docs/user-guide/`, `starter-packs/`, `tests/full_suite_battle_and_perf.rs`)**:
  - Added `--gui-font`, `--gui-font-size`, `--gui-font-file`, `-T`/`--theme`, and `bilterm run <PLAYBOOK.toml>` CLI aliases in `src/main.rs`.
  - Added Chapters 08 (`08-extended-cli-and-gui-examples.md`), 09 (`09-use-cases-and-workflows.md`), and 10 (`10-security-and-best-practices.md`) plus two new SVG workspace screen renders (`screenshot-per-pane-scaling.svg` and `screenshot-multi-agent-swarm.svg`) and 5 ready-to-run starter packs (`starter-packs/01..05`).

### Fixed
- **Small-Canvas Floating Prompt Clamp Panic (`src/pane/prompt_editor.rs`)**:
  - Guarded `FloatingCommandPrompt::effective_height` with `.clamp(3, canvas_height.max(3)).min(canvas_height)` so panes shrunk below 3 canvas rows never panic in `core::cmp::Ord::clamp`.

## [v1.11.1] - 2026-10-26

### Fixed
- **Non-Blocking Windows ConPTY Session Reaper (`src/pty/mod.rs`)**:
  - Updated `PtySession::drop` to close and drain `output_rx` immediately and offload `writer`, `child.kill()`/`child.wait()`, and `master` handle teardown to a background `bilterm-pty-reaper` thread so Windows ConPTY pipe closures never deadlock the UI or test runner thread.
- **Windows `~` Home Directory Tab Completion Fallback (`src/pane/completion.rs`)**:
  - Added `USERPROFILE` fallback when `HOME` is unset on Windows in `complete_filesystem_path` so `cd ~<Tab>` and `~/...` path completions work natively across Windows shells.
- **Windows Integration Test Suite Parity (`tests/pty_lifecycle.rs`, `tests/terminal_torture_and_stress.rs`, `tests/v110_v120_v130_automation_api_mcp_playbooks.rs`)**:
  - Updated `pty_lifecycle` on Windows to spawn `cmd.exe`, submit `\r`-terminated commands, and flush DSR query responses back to the ConPTY master.
  - Updated `terminal_torture_and_stress` environment variable tab completion check to account for Windows `${PATHEXT}` alongside `${PATH}`.
  - Switched playbook `.ppm` export path interpolation in `v110_v120_v130_automation_api_mcp_playbooks` to a single-quoted TOML literal string (`path = '{}'`) so Windows `\` path separators are not misinterpreted as `\U` Unicode escapes.

## [v1.11.0] - 2026-10-25

### Added
- **Standalone Native OS GUI Window & 1:1 HiDPI ClearType TrueType/OpenType Font Engine (`GPU-02`, `FONT-02`, `src/ui/gui_window.rs`, `src/ui/gui_font/`, `src/ui/bitmap_font.rs`, `src/pane/typography.rs`, `src/main.rs`)**:
  - **Native Desktop Window (`bilterm --gui` / `-G`)**: Drives `App` + `GuiWorkspaceRenderer` + `GuiFrameSurface` in a native Win32 (`user32.dll` + `gdi32.dll`) desktop window with 1:1 `CreateDIBSection` + `BitBlt`, `SetProcessDpiAwarenessContext` (`DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`), full keyboard (`WM_KEYDOWN` / `WM_SYSKEYDOWN` / `WM_CHAR`), mouse drag/click/scroll (`WM_LBUTTONDOWN`, `WM_MOUSEMOVE`, `WM_MOUSEWHEEL` + `Ctrl+Wheel` font zoom), and live pixel-to-cell window resizing (`WM_SIZE`).
  - **1:1 ClearType TrueType/OpenType Font Engine & Runtime `.ttf`/`.otf` Loader (`src/ui/gui_font/win32_gdi.rs`, `src/pane/typography.rs`)**: Probes and renders installed monospace fonts (`Cascadia Mono`, `Cascadia Code`, `Consolas`, `JetBrains Mono`, `Fira Code`, `Iosevka Term`, `IBM Plex Mono`, `Hack`, `IBM 3270`, `PxPlus IBM VGA8`, `Lucida Console`, `Courier New`) via `CreateFontW(CLEARTYPE_NATURAL_QUALITY)` and loads custom `.ttf`/`.otf`/`.ttc` font files on the fly via `--font-file <PATH>`, `-F/--font <FAMILY>`, `--font-size <PT>`, `:font add <path.ttf>`, `:font cycle`, and `~/.config/bilterm/fonts/`.
  - **Geometric Box-Drawing & IBM VGA 5×7 Bitmap Fallback (`src/ui/gui_font/geometric.rs`, `src/ui/bitmap_font.rs`)**: Draws edge-to-edge Borland double/single/rounded box borders (`U+2500..U+257F`), block elements (`U+2580..U+259F`), Braille (`U+2800..U+28FF`), and Sextant (`U+1FB00..U+1FB3B`) sub-cells with zero inter-cell gaps alongside a crisp 5×7 IBM VGA / CP437 bitmap glyph fallback for headless `.ppm` snapshots.
- **OS Desktop Launcher Integration & Non-TTY Auto-Relaunch (`src/launcher.rs`, `scripts/install.sh`, `scripts/install.ps1`)**:
  - Added `--desktop-file` flag and installer integration for freedesktop.org XDG `.desktop` entries (`~/.local/share/applications/bilterm.desktop`), macOS `.app` bundle `Info.plist`, and Windows Start Menu shortcuts (`Bilterm.lnk`), plus automatic terminal-emulator detection and relaunch when started from a desktop icon without an attached TTY.

### Fixed
- **Windows ConPTY 3270 Floating Prompt Command Submission (`src/pane/mod.rs`)**:
  - Normalized submitted floating prompt command line endings to `\r` (`0x0D`) on Windows (`#[cfg(windows)]`) so `powershell.exe` and `cmd.exe` inside Windows ConPTY execute commands immediately on `Enter`.
- **Linux Cross-Platform Parity & Per-Pane Font Size Isolation (`src/ui/gui_window.rs`, `src/ui/typography.rs`, `tests/vt_worker_and_snapshot_seam.rs`)**:
  - Scoped Windows-only mouse/event imports in `src/ui/gui_window.rs` and imported `WorkspaceFrameRenderer` under `#[cfg(not(windows))]`.
  - Preserved workspace-wide `config.font_size_pt` and `config.font_scale_pct` when running single-pane `:font` commands so per-pane font sizes/scales remain independent across split panes.

## [v1.10.2] - 2026-10-25

### Changed
- **Strict Acyclic Core Domain DAG (`error/secure_fs -> pty -> vt -> pane`, `src/pty/line_discipline.rs`, `src/vt/snapshot.rs`, `src/pane/typography.rs`, `tests/vt_worker_and_snapshot_seam.rs`)**:
  - Moved `Osc133Phase` into `src/pty/line_discipline.rs`, `LineSizeAttr` and `scan_dec_line_size_escapes` into `src/vt/snapshot.rs`, and `FontFamilyPreset`, `PaneFontOptions`, and `CanvasResolution` into `src/pane/typography.rs`, eliminating all upward `crate::ui` and `crate::config` imports from `pty`, `vt`, and `pane`.
  - Added automated CI layer DAG acyclicity tests (`core_domain_layers_pty_vt_pane_form_strict_acyclic_dag` and `ui_layer_never_imports_api_layer`).
- **`FloatingPromptState` Encapsulation & `ARCH-R04` Module Decompression (`src/pane/prompt_editor.rs`, `src/colon_command/parse_visual.rs`, `src/vt/scan_osc_dcs.rs`)**:
  - Restricted internal buffer/cursor fields on `FloatingPromptState` (`input`, `lines`, `cursor_row`, `cursor_col`, `cursor_char_idx`, `scroll_offset`, `h_scroll_offset`, `history_index`) to `pub(crate)` visibility and added read-only getters.
  - Split `src/colon_command/parse.rs` (`412` LOC) + `src/colon_command/parse_visual.rs` (`338` LOC) and `src/vt/scan.rs` (`352` LOC) + `src/vt/scan_osc_dcs.rs` (`378` LOC) so all `75` source files remain well below the `800`-line `ARCH-R04` ceiling.
- **Dirty-Driven Redraw, Cached `PixelCanvasBuffer` Rasterization & Single-Pass `ESC` Scanner (`src/app.rs`, `src/ui/canvas_raster.rs`, `src/vt/scan.rs`)**:
  - Updated `CompositePane::poll_pty_events`, `CompositePane::ingest_pty_events`, and `WindowManager::poll_all_ptys` to return `bool`, gating `App::run` terminal redraws on `self.needs_redraw`.
  - Added generation-keyed `Arc<PixelCanvasBuffer>` caching (`RASTER_CACHE`) in `render_graphical_canvas` so idle frames reuse rasterized pixel buffers without re-allocating RGBA framebuffers.
  - Replaced the 14 separate `text.match_indices(...)` passes in `VtParser::scan_control_sequences` with a single-pass `text.match_indices('\x1b')` dispatcher (with a fast `!bytes.contains(&0x1b)` early return).

## [v1.10.1] - 2026-10-25

### Security
- **Unified Automation File-Write Confinement & Atomic Private Exports (`SEC-R01`, `src/secure_fs.rs`, `src/wm/mod.rs`, `src/colon_command/execute_wm.rs`, `src/ui/hires_subcell.rs`, `src/api/rpc_handlers.rs`)**:
  - Added `WindowManager::apply_automation(action, confine)` and `AutomationControlPlane::apply_with_policy(action, policy)` so all JSON-RPC (`wm.command`, `canvas.command`), MCP, and confined Playbook actions route through a single policy gate.
  - Confined both `:canvas export <name>` and `:record start <name>` / `:record workspace <name>` under `SnapshotPolicy::Confined` to `snapshot_dir` via `secure_fs::validate_plain_file_name`, rejecting path traversal (`..`, `/`, `\`, NUL) and symlinks, and writing canvas exports atomically (`0600`) via `secure_fs::write_private_file`.
  - Added checked `u16::try_from` bounds validation for `size` and `scale_pct` in `workspace.fontsize` (`src/api/rpc_handlers.rs`).
- **Supply-Chain & Dependency Hardening (`Cargo.toml`, `Cargo.lock`, `docs/DEPENDENCY_AUDIT.md`)**:
  - Disabled unused `default = ["tui-term"]` feature on `vt100-ctt = { version = "0.17.1", default-features = false }`, pruning 13 unused transitive dependencies (including duplicate `ratatui 0.29.0`, unmaintained `paste 1.0.15` `RUSTSEC-2024-0436`, and `lru 0.12.5` `RUSTSEC-2026-0002`) for `0` CVEs across `247` lockfile crates.

### Fixed
- **Live `CanvasResolution` Wiring, Dynamic Canvas Export Dimensions & Cached `TIOCGWINSZ` Probing (`src/ui/canvas_raster.rs`, `src/ui/hires_subcell.rs`, `src/app.rs`)**:
  - Wired `scene.resolution.resolve_px_per_cell(canvas.width, canvas.height)` into `render_graphical_canvas` (capped at `4..=16 × 8..=32` px/cell for 60 FPS on-screen `HalfBlock`/`Octant` rendering while preserving full uncapped resolution on `:canvas export`).
  - Cached `ioctl(TIOCGWINSZ)` terminal pixel dimensions in `CACHED_TERMINAL_PIXELS` refreshed on `Event::Resize` (`refresh_terminal_pixel_size`), avoiding per-frame syscalls.
  - Updated `:canvas export` and `:canvas encode` to use the active pane's exact `self.vt.size()` dimensions instead of hardcoded `(80, 24)` and surfaced `wm.split_focused(...)` errors in the pane status bar.

### Changed
- **Module DAG Decoupling & `ARCH-R04` Ceiling Decompression (`src/api/rpc_handlers.rs`, `src/colon_command/execute_wm.rs`, `src/pane/tests.rs`, `src/wm/tests.rs`, `docs/ARCHITECTURE_REVIEW.md`, `docs/REFACTOR_LOG.md`)**:
  - Eliminated the `ui <-> api` circular dependency by moving `handle_rpc_canvas` and `handle_rpc_fontsize` into `src/api/rpc_handlers.rs`.
  - Split `src/colon_command/execute.rs` into `execute.rs` (`298` LOC) and `execute_wm.rs` (`505` LOC) and extracted unit tests into `src/pane/tests.rs` and `src/wm/tests.rs` so all source files remain comfortably under the `800`-line `ARCH-R04` ceiling.

## [v1.10.0] - 2026-10-25

### Added
- **High-Resolution Anti-Aliased Pixel/GPU Canvas & Adaptive SSH/GUI Graphics Engine (`CANV-02`, `SSH-GFX-01`, `CLI-GFX-01`, `src/ui/pixel_canvas.rs`, `src/pane/canvas_scene.rs`, `src/ui/renderer.rs`, `src/main.rs`)**:
  - **`PixelCanvasBuffer` Anti-Aliased RGBA Framebuffer (`CANV-02`)**: Renders `PaneCanvasScene` and `TektronixState` with Xiaolin Wu anti-aliased lines and cubic Bezier curves, signed-distance-field anti-aliased circles/discs, high-DPI plots with gridlines and gradient area fill, bilinear RGBA image resampling, smooth pixel-space radial focal zoom lens (`apply_focal_lens`), and direct `.ppm` / `.svg` export (`:canvas export <path.ppm|path.svg>`).
  - **Adaptive Pure-SSH & Native GUI Graphics Negotiation (`SSH-GFX-01`)**: `CanvasRenderBackend` (`Auto`, `Kitty`, `Sixel`, `ITerm2`, `HalfBlock`, `Braille`, `Sextant`, `Quadrant`) inspects `$TERM`, `$TERM_PROGRAM`, `$LC_TERMINAL`, `$KITTY_WINDOW_ID`, `$GHOSTTY_RESOURCES_DIR`, `$WEZTERM_EXECUTABLE`, and `$WT_SESSION`, encodes `Kitty APC G`, `DEC Sixel DCS q`, or `iTerm2 OSC 1337` streams with automatic `\x1bPtmux;\x1b...\x1b\\` DCS passthrough (`:canvas encode [kitty|sixel|iterm2]`), renders `1×2` 24-bit TrueColor `HalfBlock` (`▀`/`▄`) bitmaps (`:canvas hires` / `:canvas mode halfblock`) over any SSH session with `FloatingCommandPrompt` occlusion, and attaches full-resolution `GuiImageTexture` layers in `GuiWorkspaceRenderer`.
  - **Ultra-High-Resolution Presets (`1x`..`4K`), Hardware `TIOCGWINSZ` Probing & 2-Color TrueColor `Octant` Supersampling (`CANV-03`, `src/ui/hires_subcell.rs`)**: Added `CanvasResolution` (`Auto` querying OS `ioctl(TIOCGWINSZ)` `ws_xpixel × ws_ypixel` with `2x` Retina `16×32` px/cell `1280×768` default, `1x`, `2x`, `4x`, `720p`, `1080p`, `4k` `3840×2160`, and custom `<W>x<H>` via `:canvas res <mode>`) plus `CanvasSubcellMode::Octant` (`2×4` 8-subpixel per cell 2-color `fg`+`bg` 24-bit TrueColor partitioned supersampling, `4×` the spatial resolution of `HalfBlock`).
  - **Selectable Per-Pane & Per-Window Font Options (`FONT-02`, `src/ui/typography.rs`)**: Added `FontFamilyPreset` (`Ibm3270`, `BorlandDosVga`, `Tektronix4014`, `JetBrainsMono`, `FiraCode`, `IosevkaTerm`, `CascadiaCode`, `Hack`, `Custom`) and `PaneFontOptions` per `CompositePane` (`:font <family|size|scale|bold|italic|double|pane <id> ...|all ...|reset|list>`), emitting per-pane font metadata in `WorkspaceFrameSnapshot` and rendering per-pane `bold`, `italic`, and `2×1` `double_width` character expansion in TUI mode.
  - **`bilterm plot` CLI Subcommand (`CLI-GFX-01`)**: Plot `line`, `bar`, `scatter`, `candlestick`, `heatmap`, or `hires` demo scenes directly from the CLI or SSH shell (`bilterm plot line 10,25,18,42 -o chart.svg` or `-b kitty|sixel|iterm2|halfblock`).

## [v1.9.1] - 2026-10-24

### Fixed
- **WSL & Linux 3270 Command Prompt Password Input Lock (`src/pane/mod.rs`, `src/pane/modes.rs`, `src/pty/line_discipline.rs`, `src/vt/mod.rs`, `src/secure_fs.rs`)**:
  - Reset `force_command_prompt = false` in `CompositePane::submit_command` so cycling terminal modes (`F4` / `Alt+E` / `Alt+T` / `Alt+B` / `Alt+S`) never prevents subsequent `sudo` / `ssh` password prompts from auto-hiding the 3270 command prompt and routing keystrokes to `RawPtyPassthrough`.
  - Transitioned `VtParser::record_command_submission` immediately to `Osc133Phase::CommandExecuted` so the 16ms `refresh_line_discipline` tick does not clear `running_command` while `OSC 133;A` (`PromptStart`) is still active before `PS0` (`\e]133;C\a`).
  - Updated `LineDisciplineState::is_password_prompt` and `inspect_pty_fd` to recognize non-canonical `!ECHO && !ICANON` password prompts (`sudo 1.9.15+` with `use_pty`, `sudo-rs`, and subshells) and added a zero-allocation `cur_c == 0` fast path to `canvas_cursor_line_looks_like_password_prompt`.
  - Allowed symlinked parent `XDG_RUNTIME_DIR` (`/run/user/1000 -> /mnt/wslg/runtime-dir`) on WSL2 in `private_dir_in` while preserving `O_NOFOLLOW` / `symlink_metadata` validation on target runtime directories.
- **Windows PowerShell Build, Clippy & ConPTY Test Suite (`src/api/server.rs`, `src/app.rs`, `src/pty/mod.rs`, `src/pane/history_store.rs`, `tests/full_suite_battle_and_perf.rs`)**:
  - Eliminated non-Unix unused-import and unused-variable warnings under `cargo clippy --all-targets -- -D warnings`.
  - Fixed `App::new` to propagate `pane.is_headless()` so headless tests do not spawn a live `powershell.exe` ConPTY inside `QuickTerminal`, ordered `PtySession::writer` before `master` for clean ConPTY teardown, and kept headless `HistoryStore` instances disabled in `apply_settings` unless a custom `dir` is configured.

## [v1.9.0] - 2026-10-24

### Added
- **Interactive Mouse & Keyboard Window/Pane Moving, Resizing, Minimize & Maximize (`src/wm/z_stack.rs`, `src/wm/split_tree.rs`, `src/input/mouse.rs`, `src/ui/chrome.rs`)**:
  - **Z-Stack & Tiled Window Moving & Resizing**: Drag any window's top title bar (`╔═╗`) to move it freely across the workspace (or swap panes in Tiled mode; `Alt+Drag` pops a tiled pane into Z-Stack mode). Drag any window border or lower-right `◢` corner resize handle to resize windows in Z-Stack mode or adjust split ratios in Tiled mode.
  - **Keyboard & `:colon` Geometry Commands**: Added `Alt+Shift+Arrows` (`<Leader> M`) and `:move <left|right|up|down|x y> [n]` (`Action::WindowMove`), `Ctrl+Alt+Arrows` (`<Leader> <`/`>`/`+`/`-`) and `:resize <left|right|up|down|w h> [n]` (`Action::WindowResize`), and `:cascade` / `:equalize` (`<Leader> =`, `Action::WindowResetGeometry`).
  - **Resizable Modals (`F1` Help, `F9` Inspector, `Ctrl+P` Palette, `F10` Quake) + Title-Bar `[_][↕][▪]` Controls**: Added `[_]` Minimize (`minimize_badge`) alongside `[↕]` Maximize/Zoom and `[▪]` Close on Borland window frames. Help, Inspector, and Command Palette modals can be moved by dragging their title bar, resized via their lower-right `◢` corner or borders, maximized/restored via `[↕]`, and closed via `[_]`/`[▪]`.
- **Conventional Unix/Linux Short & Long CLI Switches (`src/main.rs`, `docs/bilterm.1`)**:
  - Added full short and long flag parity (`-f <VAL>`, `--flag <VAL>`, `-f=<VAL>`, `--flag=<VAL>`): `-v`/`-V`/`--version`, `-h`/`--help`, `-m`/`--mcp`, `-d`/`--headless`/`--daemon`, `-l`/`--listen`, `-r`/`--allow-remote`, `-t`/`--token-file`, `-S`/`--snapshot-dir`, `-p`/`--playbook`, `-g`/`--gui-dump`, `-s`/`--session`, `-c`/`--config`, `-C`/`--completions`, and `-M`/`--man`.
- **Cross-Platform Developer Setup & Plain Binary Installer Scripts (`scripts/setup.{sh,ps1}`, `scripts/install.{sh,ps1}`)**:
  - Added `scripts/setup.sh` (Linux/macOS) and `scripts/setup.ps1` (Windows PowerShell) to check/install system C compilers, `rustup`, Rust `1.90`, `rustfmt`, `clippy`, and optional dev tools (`cargo-nextest`, `cargo-insta`, `cargo-llvm-cov`).
  - Added `scripts/install.sh` and `scripts/install.ps1` supporting `--local` (`~/.local/bin`), `--cargo` (`~/.cargo/bin`), `--global` (`/usr/local/bin`), `--prefix <DIR>`, `--binary-only` (GitHub Release download), `--from-source`, `--dry-run`, and `--uninstall`.
- **Text-Mode Font Scaling & Sub-Pixel Graphical Canvas Lens for `:focalzoom` (`src/ui/typography.rs`, `src/ui/cursor.rs`, `src/ui/canvas_raster.rs`)**:
  - `:focalzoom` (`Alt+Shift+O`) now scales `font_scale_pct` (`125%..=300%`) and emits host terminal font-scaling sequences in text mode without inserting blank spaces between characters, while applying a smooth sub-pixel radial lens (`SubcellFocalLens`) to Tektronix 4014 vectors and `PaneCanvasScene` graphics.

### Fixed
- **Interactive `:tek` / `:canvas` Immediate Rendering & Terminal Text Passthrough (`src/vt/tektronix.rs`, `src/ui/canvas_raster.rs`, `src/pane/mod.rs`)**:
  - Separated `TektronixState::stream_capture` (only enabled by in-band `DECSET ?38h` `\x1b[?38h` PTY escape streams) from `active` so interactive `:tek demo` never swallows subsequent VT100 shell output into Tektronix labels, preserved non-empty terminal text cells during `SubcellBuffer::flush_to_ratatui`, and auto-dismissed the demo wireframe when the next shell command is submitted.

## [v1.8.0] - 2026-10-24

### Added
- **Tektronix 4010/4014 Vector Storage-Tube Emulation & Modern Graphical Canvas (`TEK-01`, `CANV-01`, `src/vt/tektronix.rs`, `src/pane/canvas_scene.rs`, `src/ui/canvas_raster.rs`)**:
  - **Tektronix 4010/4014 Vector State Machine (`TEK-01`)**: Decodes `\x1b[?38h` / `\x1b[?38l` / `:canvas tek [on|off|erase]` / `:tek`, 10-bit (`1024×780` scaled to `4096×3120`) and 12-bit (`4096×3120`) vector coordinates, `GS` (`0x1D`) dark-move & bright-vector drawing, `FS` (`0x1C`) point plot, `RS` (`0x1E`) 8-direction incremental plot, `US` (`0x1F`) alpha text labels, `ESC `` ` / `a` / `b` / `c` / `d` solid/dotted/dot-dash/short-dash/long-dash line styles, and `\x1b\x0c` screen erase with authentic green 3278 (`#00FF66`) / amber 3279 (`#FFB000`) phosphor persistence decay.
  - **Modern Retained Scene-Graph, 2D Sprite Engine, Declarative Plotting & Image Inspector (`CANV-01`)**: Supports vector primitives (`Line`, `Polyline`, `CubicBezier`, `Circle`, `RectShape`, `Polygon`, `TextLabel`), 2D sprites (`id`, position, velocity, multi-frame animation, z-index, AABB collision detection), declarative live plots (`Line`, `Scatter`, `Bar`, `Candlestick`, `Heatmap`), and an interactive image viewer (`pan`, `zoom`, `inspect_pixel`), controllable via in-band `OSC 1337;Canvas=<cmd>\x07` sequences, `:canvas <subcommand>`, JSON-RPC `canvas.command` / `canvas.status`, and MCP `bilterm_canvas`.
  - **Sub-Cell Unicode Rasterizer**: Renders Tektronix 4010/4014 vectors and `PaneCanvasScene` into Unicode `2×4` Braille (`U+2800..U+28FF`), `2×3` Sextant (`U+1FB00..U+1FB3B`), or `2×2` Quadrant sub-cell blocks with 24-bit TrueColor in Pass 1 (`render_output_canvas`), keeping the floating command prompt (`Pass 2`) and OIA status bar (`Pass 3`) cleanly composited on top.
- **`herdr`-Style Slide-Out Sessions/Windows/Panes & Fleet Drawer (`DRW-01`, `DRW-02`, `src/wm/drawer.rs`, `src/ui/drawer.rs`)**:
  - Supports 4-edge docking (`Left`, `Right` vertical tree; `Top`, `Bottom` horizontal ribbon), 3 visibility modes (`OnDemand`, `Fixed`, `AutoHide` triggered by leader chord, mouse edge hover, or background pane notification/exit alert), and 2 layout modes (`Overlay` floating over the workspace with Borland drop shadow and zero `SIGWINCH`, or `Push` carving out `drawer_rect` from `workspace_area` and reflowing panes).
  - Renders hierarchical tree connectors (`├─`, `╰─`) and live per-pane telemetry badges: focus & Z-order rank (`► [1] Z:1/N`), activity braille spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) + throughput sparkline (`▁▂▃▅▆▇`), OSC 133 command status (`⚙ RUN`, `✓ 0`, `✗ <code>`), mode/security badges (`🖥 ALT`, `🔑 !ECHO`, `🔒 PROT`, `⏺ REC`, `📡 BCAST`, `🔔 N`), and transport badges (`🌐 SSH`, `🔗 TMUX`).
  - Controllable via `F7`, `Alt+Shift+D`, `<Leader> d`, `:drawer [toggle|focus|left|right|top|bottom|fixed|autohide|ondemand|float|push|size <n>]`, interactive keyboard navigation (`j`/`k`/`h`/`l`/`Enter`/`Space`/`x`/`z`/`d`/`m`/`p`), and mouse edge hover/click/drag.
- **Dynamic Terminal Text & Font Size Scaling (`FONT-01`, `src/ui/typography.rs`, `src/ui/renderer.rs`)**:
  - Wired `Action::FontZoomIn`, `Action::FontZoomOut`, `Action::FontZoomReset`, and `Action::FontSetSize(u16)` to `Ctrl++`/`Ctrl+=`, `Ctrl+-`, `Ctrl+0`, `<Leader> 0`, `:fontsize [+|-|reset|<pt>|<pct>%]`, JSON-RPC `workspace.fontsize`, MCP `bilterm_workspace_fontsize`, and `config.toml` (`font_size_pt`, `font_scale_pct`, `cell_width_px`, `cell_height_px`, `host_font_passthrough`).
  - `GuiWorkspaceRenderer` and `GuiFrameSurface` dynamically scale cell pixel dimensions (`50%..=300%`), SVG `<text font-size="...">`, and `.ppm` raster glyph dimensions, and recalculate `(rows, cols)` character grids for a given pixel window size (`resize_workspace_for_pixel_window`).
  - `VtParser` and `render_output_canvas` parse and render DEC VT100/VT220 line-size attributes (`\x1b#5` `DECSWL`, `\x1b#6` `DECDWL` double-width, `\x1b#3`/`\x1b#4` `DECDHL` double-height top/bottom) across 2 horizontal cells per character in TUI mode, paired with `OSC 50` host-terminal font passthrough (`encode_host_font_passthrough`).
- **Built-in Whole-Workspace Asciicast v2 (`.cast`) Compositor Frame Recorder (`REC-02`, `src/vt/recording.rs`)**:
  - `:record start [path]` / `:record workspace [path]` / `:record stop` (`Ctrl+X R`) diffs rendered multi-pane `ratatui::buffer::Buffer` frames into ANSI escape sequences so splits, Z-order stacks, floating prompts, Borland menus, 3270 cross-hairs, and modals are captured into a single playable `.cast` stream.
- **Unified `Action` Command Vocabulary & Declarative Keymap Tables (`ARCH-R01`, `src/action.rs`, `src/input/keymap.rs`)**:
  - Every key chord, `:colon` command, playbook step, and JSON-RPC/MCP operation lowers into `crate::action::Action` executed by `WindowManager::apply`.
  - Dual-leader prefix (`Ctrl+B` and `Ctrl+X` when standalone; automatic fallback to `Ctrl+X` inside outer `$TMUX`), held-`Ctrl` leader chord normalization (`Ctrl+X Ctrl+<key>`), `tmux`/`herdr` chord parity (`"`, `%`, `n`, `p`, `z`, `o`, `[`), and `asciinema` PTY-wrapper `!ECHO` override harmony.
- **`TerminalBackend` Port Trait & Reference `MockTerminalBackend` (`ARCH-R03`, `ARCH-R05`, `src/vt/backend.rs`, `docs/adr/007-unwired-subsystems.md`)**:
  - Isolated all `vt100_ctt` types inside `src/vt/`, added `trait TerminalBackend` implemented by `VtParser`, `MockTerminalBackend`, and `GhosttyVtEngine`, wired `TmuxControlSubprocess` into `WindowManager`, and added `docs/DEVELOPER_AND_AGENT_GUIDE.md`.

### Changed
- **Modular Architecture & Encapsulated State Invariants (`ARCH-R02`, `ARCH-R04`)**:
  - Decomposed `src/vt/`, `src/pane/`, `src/input/`, `src/api/`, `src/ui/`, and `src/wm/` into focused submodules (all `src/**/*.rs` files `<= 800` lines; `handle_wm_key_event <= 80` lines) with zero `pane ↔ wm` dependency cycles.
  - Encapsulated `WindowManager` and `CompositePane` fields behind invariant-preserving methods (`WindowManager::remove_pane`, `CompositePane::submit_command`, exclusive `enum Modal`, and `enum Focus { Quick, Pane(PaneId) }`).

## [v1.7.0] - 2026-10-23

### Security
- **Headless Daemon Listen Policy & Bearer Auth Gate (SEC-R01)**: The headless JSON-RPC automation daemon (`bilterm --headless`) now defaults to a private `0600` Unix domain socket with peer UID verification. Non-loopback TCP listeners strictly require `--allow-remote` and `--token-file <path>` (enforcing `0600` mode). Token-protected listeners enforce an initial `auth.login` handshake before dispatching any command; unauthenticated requests receive error code `-32001` and are immediately closed. Secret tokens are never logged or leaked in error messages.
- **Workspace Snapshot Confinement (SEC-R02)**: Direct RPC and MCP `workspace.snapshot` commands and RPC-submitted playbooks are strictly confined to the designated snapshot directory (default `$XDG_STATE_HOME/bilterm/snapshots` or `--snapshot-dir <path>`). Path traversal, absolute paths, and planted symlinks are rejected via `O_NOFOLLOW` open and parent directory canonicalization.
- **Private Temporary Files & Recordings (SEC-R03)**: All Bilterm-managed temporary and state files (shell integration rc scripts, command history exports, history store fallback directory, asciicast recordings, and SSH control paths) reside in per-user private `0700` directories with race-free `O_CREAT|O_EXCL` `0600` creation.
- **SSH Destination Injection Guards (SEC-R04)**: Remote SSH host destinations reject options beginning with `-` across all input vectors (`:ssh`, `:ssh-tmux`, `:remote add`, `fleet.register`), and argument vectors separate options with `--` before host destinations.

### Changed (breaking)
- `bilterm ctl` default target endpoint changed from `127.0.0.1:7370` to the local private Unix domain socket.
- `bilterm --headless` with `--listen <addr>` refuses non-loopback addresses unless `--allow-remote` is supplied.
- `workspace.snapshot` `ppm_path` parameter over RPC/MCP must be a bare file name within the snapshot directory.
- `RemoteHostProfile::build_ssh_args` and `build_ssh_tmux_cc_args` now take `control_dir: Option<&Path>` and return a `Result<Vec<String>>`.

## [v1.6.0] - 2026-10-23

### Added
- **Persistent, session-segmented command history**: named sessions (`--session <name>` / `history_session`) stored in `<history_dir>/sessions/<name>/history.log` (`0700`/`0600`); each run is a segment (`:history runs`); the last `history_load_limit` commands are restored into Up / `Ctrl+R` / the history popup on startup; bounded by `history_max_entries` compaction.
- **Dynamic, RAM-scaled scrollback** (`[scrollback]`, `src/vt/scrollback.rs`): global budget defaults to 10% of physical RAM (`memory_budget = "auto" | "25%" | "8GiB"`), panes start at `initial_lines` and double on demand into unused budget, and contract toward a fair share under budget overrun or low system memory (`low_memory_pct`). New `:scrollback` / `:sb` report and `:scrollback clear`.
- **`ghostty-vt` feature now builds** against `libghostty-vt` 0.2.2 (opt-in; requires Rust 1.90+ and Zig 0.15.2; `zig_0_15` added to the flake devShell).

### Fixed
- PTY output beyond 32 KB per frame was silently discarded, splitting escape sequences and losing mode changes (alt-screen, bracketed paste, OSC 133).
- Width changes (splits/resizes) rebuilt the terminal from only the last 64 KB of output, discarding older scrollback; scrollback is now preserved and reflowed.
- `~/.config/bilterm/config.toml` was never loaded; it is now read at startup and parse errors are shown in the OIA bar.
- Headless JSON-RPC daemon: one client's connection reset terminated the daemon, and deeply nested JSON overflowed the stack. Connections are now isolated per task, request lines are capped at 1 MiB, and JSON nesting is limited to 128.
- `GhosttyCAbiTerminal` exposed undefined behaviour through a safe public constructor; construction is now restricted and the invariant documented.

### Changed
- MSRV raised to **Rust 1.88**.
- History file escaping now also escapes backslashes; legacy per-run history logs are no longer written (existing ones are left untouched).

### Security
- `tokio` 1.54.0 → 1.54.1 (RUSTSEC-2026-0384; 1.54.0 yanked).
- `time` 0.3.45 → 0.3.55 (RUSTSEC-2026-0009).

## [v1.5.0] - 2026-10-22

### Added
- **Toggleable, Resizable Focal Spotlight Shader (`SpotlightMode`, `render_spotlight_cursor`, `src/ui/cursor.rs`, `src/config/mod.rs`, `src/ui/mod.rs`, `src/pane/colon_command.rs`, `src/input/mod.rs`)**:
  - Supports **Circular** (aspect-corrected `2:1` for terminal character cells) and **Elliptical** (`spotlight_rx`, `spotlight_ry`) spotlight shapes centered at the active cursor (`Alt+O` / `Ctrl+X L` / `:spotlight`).
  - **Smooth Fuzzy Falloff (`spotlight_pixelated = false`)**: Hermite `smoothstep` ($3t^2 - 2t^3$) radial luminance attenuation and phosphor core bloom.
  - **Pixelated Character-Block Mode (`spotlight_pixelated = true`)**: Quantizes spotlight coordinates to `2×1` character-cell blocks (`x & !1`) and renders stepped block dither rings (`▒` -> `░` -> ` `) on background cells.
  - Live dynamic resizing via `Alt+Shift+]` (`Alt+}`) / `Alt+Shift+[` (`Alt+{`) or `:spotlight [+|-|size <rx> [ry]|feather <pct>|full|pane|off]`.
- **Focal Zoom (Fisheye Magnifying Lens) Shader (`render_focal_zoom_cursor`, `src/ui/cursor.rs`, `src/config/mod.rs`, `src/ui/mod.rs`, `src/pane/colon_command.rs`, `src/input/mod.rs`)**:
  - Real-time radial inverse-mapped magnifying lens centered at the cursor with multiple smooth magnification levels (`1.25x`, `1.50x`, `2.00x`, `3.00x`, or custom `1.05x..=4.00x` via `Alt+Shift+O` / `:focalzoom` / `:lens`).
  - Uses Hermite `smoothstep` radial magnification falloff to `1.00x` at the lens rim, anti-stutter primary-cell glyph placement with `Modifier::BOLD` + phosphor clarity boost, continuous box-drawing line stretching (`─`, `═`, `│`, `║`), and optional `2×1` pixelated character-block magnification (`:focalzoom pixel`).
- **Universal Leader Prefix (`Ctrl+X`), macOS `Option` Key Normalization & Readline Word Navigation (`src/input/mod.rs`, `src/config/mod.rs`)**:
  - Added configurable `leader_key` (`"Ctrl+X"` default) that works identically in both `CommandPrompt` and `RawPtyPassthrough` modes without colliding with macOS window manager shortcuts or outer `tmux` (`Ctrl+B`).
  - Added `normalize_macos_option_key` (`macos_option_as_alt = true`) translating macOS `Option+<key>` Unicode symbols (`˙`, `√`, `∫`, `ƒ`, `†`, `∂`, `∑`, `Ω`, `≈`, `π`, `≤`, `≥`, `÷`, `` ` ``) back into `Alt+<key>` chords.
  - Added Readline word-wise navigation and deletion (`Alt+Left`/`Alt+B`, `Alt+Right`/`Alt+F`, `Alt+Backspace`/`Ctrl+W`, `Alt+D`/`Alt+Delete`) in `FloatingPromptState`.
- **3-Way Command Prompt Mode Toggle (`PromptMode`, `F4` / `Alt+E` / `Ctrl+X e` / `:mode [float|fixed|terminal|cycle]`)**:
  - Cycles seamlessly between **Floating Command Prompt**, **Fixed (Non-Overlapping Docked) Command Prompt**, and **Regular Terminal (`RawPtyPassthrough`)**, or via clicking the bottom OIA status bar.

### Fixed
- **`sudo` / `sudo-rs` / `ssh` Non-Canonical `!ECHO` Password Prompt Input Capture (`src/pty/line_discipline.rs`, `src/pane/mod.rs`)**:
  - Modern `sudo` (`sudo-rs` on Ubuntu 25.10+) and `ssh` disable both `ECHO` and `ICANON` (`!ECHO && !ICANON`) while running in a foreground child process group (`fg_pgrp != shell_pgid`). `inspect_pty_fd` and `LineDisciplineState::is_password_prompt` now distinguish shell Readline/ZLE (`fg_pgrp == shell_pgid`) from foreground child `!ECHO` prompts (`fg_pgrp != shell_pgid`) and include defense-in-depth canvas password prompt detection (`[sudo`, `Password:`, `passphrase`), immediately switching to `RawPtyPassthrough` (`🔒 !ECHO LOCK`) so password prompts capture keyboard input directly.
- **Eliminated Command Prompt Flashing Under Readline / ZLE / Oh-My-Bash (`src/pty/line_discipline.rs`)**:
  - Distinguishes interactive shell line-editor states (`!ICANON && !ECHO` with foreground `tcgetpgrp == shell_pid`) from genuine password prompts (`ICANON && !ECHO` or child subcommand `sudo`/`ssh`), preventing the floating prompt from hiding between commands.

## [v1.4.0] - 2026-10-21

### Added
- **First-Class SSH Domains, Remote `tmux -CC` Persistence & `herdr`-Style Central Fleet Federation (`REM-01`, `src/remote/mod.rs`, `src/wm/mod.rs`, `src/pane/colon_command.rs`, `src/api/mod.rs`, `src/ui/mod.rs`)**:
  - **SSH Composite Panes (`:ssh <user@host> [port]`)**: Spawns remote SSH sessions inside local Composite Panes with automatic `OSC 133` + `OSC 7` shell-integration bootstrap (`remote_osc133_bootstrap_command`), full local `FloatingCommandPrompt` editing, `!ECHO LOCK` passphrase/sudo protection, and a live `🌐 SSH:<user@host>` badge in the IBM 3270 OIA status bar.
  - **Remote Persistent `tmux -CC` over SSH (`:ssh-tmux <user@host> [session]`)**: Bridges remote `tmux -CC new-session -A -s <session>` streams over SSH (`build_ssh_tmux_cc_args`) into `TmuxControlBridge`.
  - **`herdr`-Style Central Fleet Registry & Tag Fan-Out (`FleetRegistry`, `:remote add|list|exec|disconnect`)**: Manages local and remote nodes with `[remotes.<name>]` profiles in `config.toml` and tag-filtered command fan-out (`all`, `local`, `remote`, `@tag`, `<node_id>`).
  - **JSON-RPC 2.0 & MCP Fleet Tools**: Added `remote.connect_ssh`, `remote.connect_tmux`, `fleet.register`, `fleet.list`, `fleet.exec`, `fleet.disconnect`, and MCP tools `bilterm_remote_connect` and `bilterm_fleet_exec`.

## [v1.3.0] - 2026-10-21

### Added
- **Structured Automation Event Subscription Bus & Declarative Robot Playbooks (`AUTO-03`, `src/api/mod.rs`, `src/main.rs`)**:
  - `AutomationEvent` tracks `command_submitted`, `command_completed` (with `OSC 133;D` exit code and duration), `trigger_matched`, `notification_received` (`OSC 9`/`OSC 777`), `echo_lock_changed`, and `alternate_screen_changed` via `events.poll`.
  - `Playbook` (`bilterm --playbook <file.toml>` / `playbook.run`) executes declarative multi-step TOML automation playbooks (`colon`, `exec`, `send_keys`, `feed_output`, `assert_screen_contains`, `assert_trigger_fired`, `assert_exit_code`, `export_ppm`).

## [v1.2.0] - 2026-10-21

### Added
- **Native Model Context Protocol (MCP) Server for AI Agents (`AUTO-02`, `bilterm --mcp`, `src/api/mod.rs`, `src/main.rs`)**:
  - Zero-wrapper JSON-RPC 2.0 MCP (`2024-11-05`) server over `stdio` exposing `initialize`, `ping`, `tools/list`, and `tools/call` for 6 agent-native tools: `bilterm_workspace_status`, `bilterm_exec_command`, `bilterm_inspect_pane`, `bilterm_send_keys`, `bilterm_colon_command`, and `bilterm_run_playbook`.

## [v1.1.0] - 2026-10-21

### Added
- **JSON-RPC 2.0 Automation Control Plane, `--headless` Daemon & `bilterm ctl` CLI (`AUTO-01`, `src/api/mod.rs`, `src/main.rs`)**:
  - `AutomationControlPlane`, `run_headless_tcp_server` (`bilterm --headless --listen <addr>`), and `send_rpc_request` (`bilterm ctl <endpoint> <method> [params]`) exposing 100% of Bilterm's workspace, pane, `:colon` command, key-chord, and snapshot capabilities over JSON-RPC 2.0 with built-in `!ECHO LOCK` and `🔒 PROT` safety guards.

## [v1.0.0] - 2026-10-21

### Added
- **CLI Shell Completions, Manpage Generator & Nix Flake Packaging (`PKG-01`, `src/main.rs`, `docs/bilterm.1`, `flake.nix`)**:
  - Added `bilterm --completions bash|zsh|fish` and `bilterm --man` CLI flags alongside the standalone `roff(7)` manual page (`docs/bilterm.1`) and reproducible Nix flake (`flake.nix`).
  - Promoted crate version to `1.0.0` with full documentation synchronization across `README.md`, `docs/`, and `docs/user-guide/`.

## [v0.7.0] - 2026-10-21

### Added
- **`OSC 9` / `OSC 777` Desktop Notifications & OIA Bell Telemetry (`NTF-01`, `src/vt/mod.rs`, `src/ui/mod.rs`, `src/pane/colon_command.rs`)**:
  - Parses iTerm2 `OSC 9;<message>ST` and rxvt-unicode `OSC 777;notify;<title>;<body>ST` notifications alongside `BEL` (`0x07`), displaying a live `🔔 <count>:<title>` badge in the IBM 3270 OIA status bar and clearing via `:notifications clear` (`:notify clear`).
- **Custom Output Highlight Triggers (`TRG-01`, `src/config/mod.rs`, `src/ui/mod.rs`, `src/pane/colon_command.rs`)**:
  - Highlights user-defined pattern matches across the `OutputCanvas` in real time via `:trigger add <pattern>`, `:trigger clear`, or `triggers = [...]` in `config.toml`.
- **Asciicast v2 (`.cast`) Terminal Session Recorder (`REC-01`, `src/vt/mod.rs`, `src/ui/mod.rs`, `src/pane/colon_command.rs`)**:
  - Records live PTY output streams into standard Asciicast v2 JSONL `.cast` files (`:record start [path]` / `:record stop`) with a live `⏺ REC` indicator in the OIA status bar.

## [v0.6.0] - 2026-10-21

### Added
- **Interactive Fuzzy Command Palette (`CMD-01`, `Ctrl+P` / `Alt+Space` / `:palette`, `src/wm/mod.rs`, `src/input/mod.rs`, `src/ui/mod.rs`)**:
  - Borland Turbo Vision modal command palette powered by `nucleo-matcher` that searches and executes all `:colon` commands, switches panes, and applies themes with instant fuzzy filtering.
- **Custom Keybinding Remapping Engine (`CFG-02`, `src/config/mod.rs`, `src/input/mod.rs`)**:
  - Supports `[keybindings]` in `~/.config/bilterm/config.toml` (e.g. `"Ctrl+T" = ":split v"`) via `format_key_chord` and `execute_colon_command` dispatch.
- **Retro CRT Phosphor Scanline Shader (`CRT-01`, `:crt [on|off]`, `src/config/mod.rs`, `src/ui/mod.rs`, `src/ui/renderer.rs`)**:
  - Optional even-row scanline luminance modulation across both the Ratatui TUI compositor and the native `GuiFrameSurface` RGBA rasterizer.

## [v0.5.0] - 2026-10-21

### Added
- **Bi-Directional `tmux -CC` Session Daemon & Layout Topology Bridge (`TMUX-03`, `src/tmux/mod.rs`)**:
  - `parse_tmux_layout_panes()` parses `tmux` layout strings (`<checksum>,<WxH>,<X>,<Y>{...}[...]`) and automatically hydrates 1:1 local `CompositePane` instances on `%layout-change`.
  - Handles `%window-pane-changed`, `%window-close` / `%unlinked-window-close`, and `%exit [reason]`, and routes `FloatingCommandPrompt` submissions back to `tmux` via `TmuxControlBridge::route_prompt_submission` (`send-keys -t %<id> -H <hex>`), `queue_split`, and `queue_resize_client`.
  - `TmuxControlSubprocess::spawn_attach_or_new` spawns `tmux -CC new-session -A -s <session>` with asynchronous `stdin`/`stdout` Tokio channels.
- **Kitty Graphics Protocol (`APC G`) & DEC Sixel (`DCS q`) Support (`GFX-01`, `src/vt/mod.rs`, `src/ui/mod.rs`, `src/ui/renderer.rs`)**:
  - Decodes Kitty Graphics Protocol (`\x1b_G<k=v,...>;<base64>\x1b\\`) queries (`a=q` -> `\x1b_Gi=<id>;OK\x1b\\`), RGB/RGBA transmissions & placements (`a=t`, `a=T`, `a=p`), and deletions (`a=d`).
  - Decodes DEC Sixel (`\x1bP<params>q<sixel>\x1b\\`) raster attributes (`"Pan;Pad;Ph;Pv`), RGB color registers (`#<reg>;2;<r>;<g>;<b>`), RLE repeat (`!<count><char>`), and 6-pixel vertical bands into RGBA pixel buffers.
  - Renders `TerminalGraphicPlacement` regions onto TUI `OutputCanvas` cells using 24-bit TrueColor Unicode half-blocks (`▀`) and attaches full-resolution `GuiImageTexture` buffers to `GuiFrameSurface` in GPU mode.

## [v0.4.0] - 2026-10-21

### Added
- **Native GPU/GUI Workspace Renderer & Framebuffer Pipeline (`GPU-01`, `src/ui/renderer.rs`, `src/main.rs`)**:
  - `GuiWorkspaceRenderer` and `GuiFrameSurface` implement `WorkspaceFrameRenderer` for `GPUI` / `winit + egui` integration, emitting `GuiQuadPrimitive` backgrounds/underlines, `GuiGlyphRun` text runs, `GuiImageTexture` quads, and a deterministic RGBA / Netpbm `P6` `.ppm` software rasterizer (`--gui-dump <path.ppm>`).
- **`libghostty-vt` C-ABI Vtable & FFI Handle (`GHOSTTY-01`, `src/vt/ghostty_backend.rs`)**:
  - Added `GhosttyCAbiVtable` (`terminal_new`, `terminal_free`, `terminal_vt_write`, `terminal_resize`) and safe `GhosttyCAbiTerminal` wrapper alongside the `--features ghostty-vt` binding.

## [v0.3.5] - 2026-10-21

### Changed
- **Living Documentation & User Guide Synchronization (`README.md`, `docs/architecture.md`, `docs/user-guide/`)**:
  - Synchronized all documentation tables, architecture diagrams, keyboard/mouse/`:command` references (`F9`, `Alt+F`, `Alt+L`, `Alt+/`, `Alt+C`, `:inspector`, `:hints`, `:lock`, `:broadcast`, `:menubar`, `:session`, `:search`, `:links`, `:copy`), and multi-pane session guides with `v0.2.0`–`v0.5.0`.

## [v0.3.0] - 2026-10-21

### Added
- **Live 3270 Terminal Inspector Modal (`BEL-01`, `F9` / `:inspector`, `src/vt/mod.rs`, `src/ui/mod.rs`, `src/input/mod.rs`)**:
  - Real-time Borland Turbo Vision diagnostic overlay inspecting the focused `CompositePane`'s active `DECSET` modes (`?1049`, `?2004`, `?1006`, `?1007`, `?2026`, `?2027`, `?2031`, `?2048`), `DECCKM`/`DECBKM`, Kitty keyboard protocol flags (`CSI > flags u`), cursor coordinates, cell-under-cursor grapheme + `U+XXXX` codepoints, extended SGR underline/overline state, 3-signal line-discipline status, and a bounded 16-entry ring buffer of recent CSI/OSC escape sequences.
- **`OSC 9;4` OIA Progress Bar Gauge (`BEL-02`, `src/vt/mod.rs`, `src/ui/mod.rs`)**:
  - Parses ConEmu/Ghostty `\x1b]9;4;<state>;<pct>(\x07|\x1b\\)` progress sequences (`Normal`, `Error`, `Paused`, `Indeterminate`, and reset `0`) and renders a live 8-cell block progress bar (`[████░░░░] 50%`, `![████░░░░] 50%`, `⏸[████░░░░] 50%`, `[▓▓░░░░▓▓] BUSY`) inside the IBM 3270 Operator Information Area (OIA) status bar.
- **Color Scheme Change Notifications (`BEL-03`, `DECSET ?2031` & `CSI ? 996 n`, `src/vt/mod.rs`, `src/wm/mod.rs`, `src/pane/colon_command.rs`)**:
  - Replies to `CSI ? 996 n` color-scheme DSR queries with `CSI ? 997 ; 1 n` (dark) or `CSI ? 997 ; 2 n` (light) computed from the active theme background luminance, and automatically dispatches unsolicited `CSI ? 997 ; 1|2 n` notifications to all panes with `DECSET ?2031` enabled whenever `:theme` changes.
- **Unicode Grapheme Cluster Mode (`FND-07`, `DECSET ?2027`, `src/ui/mod.rs`)**:
  - Preserves multi-codepoint Zero-Width Joiner (`U+200D`) and Variation Selector-16 (`U+FE0F`) grapheme clusters on the `OutputCanvas` when `DECSET ?2027` is active.
- **QuickSelect 3270 Field Hints (`ECO-01`, `Alt+F` / `:hints`, `src/vt/mod.rs`, `src/pane/mod.rs`, `src/ui/mod.rs`, `src/input/mod.rs`)**:
  - Scans the visible `OutputCanvas` for URLs (`https://`, `http://`, `file://`), file paths (`/path`, `./path`, `../path`, `~/path`, `dir/file`), IPv4 addresses (`a.b.c.d[:port]`), and Git commit SHAs (`7..40` hex chars), overlays high-contrast `[a]..[z]` jump badges, and inserts the selected token into the floating command prompt while yanking via `OSC 52`.
- **Workspace Layout & Session Serialization (`ECO-02`, `:session save <name>` / `:session restore <name>` / `:session list`, `src/wm/mod.rs`, `src/pane/colon_command.rs`)**:
  - Saves and restores complete multi-pane workspace topologies (`WorkspaceLayoutMode`, `SplitTree`, `ZStack` z-order, zoom state, pane titles, working directories, `PromptAnchor`, prompt dimensions, read-only locks, and command histories) to declarative TOML files in `$XDG_STATE_HOME/bilterm/sessions/<name>.toml`.
- **Interactive Top Borland Turbo Vision Menu Bar (`ECO-03`, `:menubar [on|off]`, `src/ui/mod.rs`, `src/input/mod.rs`, `src/config/mod.rs`)**:
  - Optional 1-row top pull-down menu bar (`≡ System  Window  Prompt  View  Theme  Tmux  Help`) with mouse click activation, keyboard arrow navigation, and Borland shadow-framed pull-down menus.
- **Read-Only Protected 3270 Field Lock & Multi-Pane Input Broadcast (`ECO-04`, `Alt+L` / `:lock` & `:broadcast`, `src/pane/mod.rs`, `src/wm/mod.rs`, `src/input/mod.rs`, `src/ui/mod.rs`)**:
  - Per-pane Read-Only lock (`Alt+L` / `:lock [on|off]`) displays a `🔒 PROT` OIA badge and blocks accidental shell command submissions or raw PTY keystrokes while still permitting `:colon` commands.
  - Synchronized Multi-Pane Input Broadcast (`:broadcast [on|off]`) displays a `📡 BCAST` badge and fans out submitted commands to all unlocked workspace panes simultaneously.

## [v0.2.5] - 2026-10-21

### Added
- **Renderer-Agnostic `GridSnapshot` & `CellSnapshot` Seam (`ARCH-01`, `src/vt/snapshot.rs`, `src/vt/mod.rs`)**:
  - Point-in-time `GridSnapshot`, `RowSnapshot`, `CellSnapshot`, `CursorSnapshot`, `UnderlineKind`, and `RgbTriplet` structures that decouple the UI rendering layer (`Ratatui`, `GPUI`, or `winit + egui`) from the underlying virtual terminal state machine.
- **Dedicated Emulation Worker Thread Pipeline (`ARCH-02`, `src/vt/worker.rs`, `src/vt/ghostty_backend.rs`)**:
  - `VtWorkerHandle` and `SharedGridSnapshot` spawn a dedicated OS worker thread (`bilterm-vt-worker`) paired with a `portable-pty` reader pipe (`bilterm-pty-pipe`), feeding raw VT/ANSI byte chunks into `libghostty-vt` (`GhosttyVtEngine` behind `--features ghostty-vt`) or `VtParser`, answering terminal queries (`DSR`, `CPR`, `DA1`, `DECRPM`, `OSC 4/10/11/12`, `XTGETTCAP`) directly on the worker thread, and publishing atomic `GridSnapshot` frames to the UI layer.
- **Abstract UI & Window Layer Renderer Seam (`ARCH-03`, `src/ui/renderer.rs`)**:
  - `WorkspaceFrameSnapshot`, `PaneFrameSnapshot`, and `WorkspaceFrameRenderer` (`RatatuiWorkspaceRenderer`) allow `Ratatui`, Zed's `GPUI`, or `winit + egui` to render the entire multi-pane workspace (`SplitTree`, `ZStack`, `QuickTerminal`, `FloatingPrompt`, `OiaStatusBar`) from immutable snapshots.
- **Soft-Wrapped Scrollback Reflow (`FND-08`), Extended SGR Attributes (`FND-09`) & In-Band Size Reports (`VAL-05`, `src/vt/mod.rs`)**:
  - Reflows soft-wrapped primary-screen lines when horizontal pane columns shrink or expand (`VtParser::set_size`), while leaving Alternate Screen (`?1049`) grids un-reflowed.
  - Parses extended SGR sub-parameter underlines (`4:0` none, `4:1` single, `4:2` double, `4:3` curly, `4:4` dotted, `4:5` dashed), `SGR 53`/`55` overline, and `SGR 58;2;r;g;b` / `58:2::r:g:b` RGB underline colors.
  - Emits in-band resize notifications (`CSI 48 ; rows ; cols ; 0 ; 0 t`) when `DECSET ?2048` is enabled.
- **`vttest` / `esctest` & 17-Session Torture Suite (`tests/vttest_esctest_fuzz_torture.rs`, `tests/vt_worker_and_snapshot_seam.rs`)**:
  - Added 1-byte/3-byte/7-byte split-boundary escape sequence fuzzing, parameter bomb tests, 16-workspace-pane + 1-Quake-panel simultaneous tiling/stacking/shuffling stress tests, and live `portable-pty` -> `VtWorkerHandle` -> `GridSnapshot` integration tests (74 total tests passing).

## [v0.2.0] - 2026-10-21

### Added
- **Paste Injection Sanitization & Unsafe Multi-Line Paste Guard (`FND-01`, `src/vt/mod.rs`, `src/input/mod.rs`, `src/ui/mod.rs`)**:
  - `sanitize_paste_bytes()` strips dangerous C0/DEL control bytes (`NUL`, `ESC`, `DEL`, `VINTR 0x03`, `VQUIT 0x1C`, `VSUSP 0x1A`, `VWERASE 0x17`, `VLNEXT 0x16`) from all clipboard pastes.
  - `is_unsafe_paste()` intercepts unbracketed multi-line pastes and embedded `\x1b[201~` bracketed-paste breakout payloads in `INPUT: RAW PTY` mode and opens a Borland Turbo Vision `[!] UNSAFE PASTE GUARD` confirmation modal (`y`/`Enter` to confirm, `n`/`Esc` to abort).
- **Mouse Alternate Scroll (`DECSET ?1007`, `FND-02`, `src/vt/mod.rs`, `src/input/mod.rs`)**:
  - Tracks `\x1b[?1007h`/`l` (enabled by default) and translates mouse wheel scroll events on the `OutputCanvas` into `Up`/`Down` arrow keys (`\x1bOA`/`\x1bOB` or `\x1b[A`/`\x1b[B`) when in Alternate Screen (`?1049`) without mouse tracking enabled (`less`, `man`, `git log`).
- **Modern Terminal Protocol Queries (`DECRPM`, Kitty Keyboard `CSI u`, Dynamic `OSC 4/10/11/12`, `XTGETTCAP` & `DECRQSS`, `FND-03..06`, `src/vt/mod.rs`, `src/input/mod.rs`)**:
  - `DECRPM` (`CSI ? Pd $ p` & `CSI Pd $ p`) mode state reporting (`CSI ? Pd ; Ps $ y`) for `?1`, `?7`, `?25`, `?47`, `?67`, `?1000..1007`, `?1047`, `?1049`, `?2004`, `?2026`, `?2027`, `?2031`, `?2048`.
  - Per-screen Kitty Keyboard Protocol flag stacks (`CSI > flags u`, `CSI < count u`, `CSI = flags ; mode u`, `CSI ? u`) and `CSI u` key encoding (`encode_key_to_pty_bytes_full`).
  - Dynamic palette color query responders for `OSC 4`, `OSC 10` (fg), `OSC 11` (bg), and `OSC 12` (cursor) (`\x1b]10;rgb:RRRR/GGGG/BBBB\x1b\\`).
  - `XTGETTCAP` (`DCS + q <hex> ST` -> `DCS 1 + r <hex>=<hex> ST` for `TN`, `Co`, `RGB`, `Tc`, `Smulx`, `Setulc`, `Ms`) and `DECRQSS` (`DCS $ q <pt> ST`).
- **Zero-Config Shell Integration Auto-Injection (`VAL-01`, `src/pty/shell_integration.rs`, `src/pty/mod.rs`)**:
  - Automatically injects `OSC 133;A/B/C/D;<exit>` and `OSC 7` working directory reporting hooks into interactive `bash` (`--rcfile`), `zsh` (`ZDOTDIR`), and `fish` (`XDG_DATA_DIRS`) sessions without modifying user dotfiles.
- **Command Exit-Code Badges & Execution Duration Telemetry (`VAL-06`, `src/vt/mod.rs`, `src/ui/mod.rs`)**:
  - Records per-command execution duration (`duration_ms`) and exit code (`exit_code`) on `CommandBlockRecord` and renders `✓ 0 [12ms]` / `✗ 1 [45ms]` telemetry badges in the floating prompt's `LAST CMD` banner.
- **Output Canvas Scrollback Search & Match Highlighting (`VAL-02`, `Alt+/` / `:search <query>`, `Alt+n` / `Alt+N`)**:
  - Case-insensitive search across the `OutputCanvas` grid with bright yellow/amber match cell highlighting and `next`/`prev` navigation.
- **`OSC 8` Explicit Hyperlinks & URL Auto-Detection (`VAL-03`, `Ctrl+Click` / `:links`)**:
  - Parses explicit `OSC 8` hyperlinks and auto-detects `https://`, `http://`, and `file://` URLs on the `OutputCanvas` with cyan underline styling and `Ctrl+LeftClick` / `:links` `OSC 52` clipboard yanking.
- **Vi Copy Mode & Rectangular Block Selection (`VAL-04`, `Alt+C` / `:copy`, `Alt+LeftDrag`)**:
  - Keyboard-driven Vi Copy Mode (`h/j/k/l/0/$/w/b`, `v` character selection, `V` line selection, `b`/`Ctrl+V` rectangular block selection, `y`/`Enter` yank via `OSC 52` and prompt yank buffer) plus `Alt+LeftDrag` mouse rectangular block selection on the `OutputCanvas`.

## [v0.1.0] - 2026-10-20

### Added
- **Core PTY & VT100/VT220/Xterm Emulation Engine (`src/pty/`, `src/vt/`)**:
  - Cross-platform non-blocking PTY process manager (`portable-pty` + `tokio`) with dynamic `TIOCSWINSZ` resize propagation and bounded 64KB chunk streaming for zero-freeze flood resilience (`tail -f /dev/random`).
  - Full control sequence state machine (`VtEngine`): Alternate Screen (`DECSET ?1049/?1047/?47`), Application Cursor Keys (`DECCKM ?1`), Bracketed Paste (`DECSET ?2004`), SGR/X10/Button/Any Mouse Tracking (`?1000/?1002/?1003/?1006`), Focus Reporting (`?1004`), Synchronized Output (`?2026`), `OSC 133` (`A`/`B`/`C`/`D`) semantic shell prompt markers, `OSC 52` clipboard capture, `OSC 0`/`OSC 2` window titles, `OSC 7` working directory tracking, and `DSR`/`CPR`/`DA1` bidirectional terminal query replies.
  - 3-Signal Line-Discipline Detector (`src/pty/line_discipline.rs`): Combines `DECSET 1049`, `OSC 133`, and `tcgetattr` (`ECHO`/`ICANON`) with printable heuristic fallback so binary output floods never hide or lock out the floating command prompt.
- **Composite Terminal Pane & Floating Command Prompt (`src/pane/`)**:
  - 4-mode Command Prompt Layout (`PromptAnchor`): Floating Top (`Top`), Floating Bottom (`Bottom`), and non-overlapping Split Command/Output Panes (`SplitTop`, `SplitBottom`).
  - Resizable command prompt width (`Alt+]`/`Alt+[`, `:prompt width <cols|full>`, left/right/corner mouse drag) and vertical/horizontal prompt viewport scrolling (`Shift+PageUp`/`Shift+PageDown`, mouse wheel, right-border scrollbar indicator).
  - Running/Last Command indicator banner (`⟳ RUNNING:` / `● LAST CMD:`) displayed inside the command prompt box after submitting a command (`Enter`).
  - Context-aware `<Tab>` completion (`src/pane/completion.rs`) for `$PATH` executables (first token) and relative、`~/`、or directory paths (subsequent tokens) with longest-common-prefix expansion and candidate cycling.
  - Multi-line shell editor with live syntax highlighting, fuzzy history search (`Ctrl+R`), and `OSC 133` command block jump (`Alt+Up`/`Alt+Down`).
- **Persistent Session Command History & History Pop-up (`src/pane/history_store.rs`)**:
  - Automatic per-session history persistence (`$XDG_STATE_HOME/bilterm/history/history-<terminal_id>-<session_id>.log`) with ISO-8601 UTC timestamps, unique session IDs, pane IDs, and configurable file rotation (`history_max_files`, `:history on|off`, `:history max-files <N>`).
  - Temporary shell script export (`:history save [path]`) and interactive Borland-style Command History Pop-up Window (`Alt+P` / `:history`).
- **Multi-Pane Window Manager, Z-Order Deck & Quake Drop-Down (`src/wm/`)**:
  - Binary Split Tree (`SplitNode`) for arbitrary horizontal/vertical composite pane splits (`Alt+H`, `Alt+V`) and directional navigation.
  - Overlapping Z-Order Window Stack (`ZOrderStack`) with cycle-raise, shuffle, and mouse click-to-focus.
  - Persistent Quake/Guake drop-down quick terminal (`F12` / `` Ctrl+` ``).
  - Interactive `<F1>` / `:help` Borland Turbo Vision Quick Reference Help Modal (`Up`/`Down`/`PageUp`/`PageDown` scrollable).
- **Borland Turbo Vision Chrome, IBM 3270 Cross-Hairs & Configuration (`src/ui/`, `src/config/`)**:
  - 5 built-in retro palettes (`borland-classic`, `turbo-vision-dark`, `ibm-3270-green`, `ibm-3270-amber`, `cyberpunk-neon`), 8 border outline styles (`double`, `single`, `rounded`, `thick`, `dotted`, `dashed`, `ascii`, `none`), and 4 DOS drop-shadow styles (`classic`, `solid`, `tinted`, `none`).
  - Software IBM 3270 Cross-Hair Cursor (`Alt+X`, `:crosshair full | pane | off`) with pane-local and full-screen span modes plus 4 caret glyphs (`Block`, `HollowBlock`, `Underscore`, `Bar`).
  - Rich IBM 3270 OIA Status Bar with 4 glyph profiles (`unicode`, `cp437`, `nerdfont`, `ascii`), live `⌖ R:xx C:xxx` cursor coordinates, and `≣ OUT:<lines>L/<bytes>` output length telemetry.
  - Persistent TOML configuration (`~/.config/bilterm/config.toml`) with live `:set` and `:save` commands.
- **Dual-Mode `tmux` Integration (`src/tmux/`)**:
  - Host `tmux` detection (`$TMUX`, `DCS tmux;` passthrough wrapping) and `tmux -CC` control-mode parser (`%output`, `%layout-change`, `%exit`).
- **Testing & Documentation (`tests/`, `docs/`)**:
  - 55 automated unit, integration, `insta` snapshot, and terminal torture/stress tests (`81.83%` line coverage across `src/`).
  - Architecture guide (`docs/ARCHITECTURE.md`), coverage report (`docs/TEST_COVERAGE.md`), and 7-chapter User Manual (`docs/user-guide/`).
