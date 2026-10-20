# Pitfalls Research

**Domain:** Rust + Ratatui Terminal Emulator, Multiplexer, and Floating-Prompt Shell Wrapper (`Bilterm`)
**Researched:** 2026-10-20
**Confidence:** HIGH

## Critical Pitfalls

### Pitfall 1: Echo & Line-Discipline Desync Between the Floating Command Prompt and the Underlying PTY

**What goes wrong:**
When decoupling command input into a Ratatui floating prompt widget (`FloatingCommandPrompt`) above a PTY output canvas (`OutputCanvas`), input handling desynchronizes from the child process's actual terminal state:
1. **Double Echoing & Prompt Clutter:** The user types a command into the floating editor and presses `Enter`. Bilterm writes `cmd\n` to the PTY master. Because the shell (`bash`/`zsh`/`fish`) has readline/line-editor echo enabled (`ECHO` / `ICANON` or custom raw-mode line editing), the shell echoes `PS1` + `cmd` into the `OutputCanvas`, duplicating what the user just submitted or corrupting multi-line paste blocks.
2. **Password Leaks in `sudo`, `ssh`, `gpg`, or `pass`:** A child process disables `ECHO` on its slave PTY (`tcsetattr` clearing `ECHO`) to read a secret. If Bilterm buffers keystrokes in the floating prompt widget instead of detecting the non-echoing prompt, the user's password is rendered in cleartext inside the floating command box and stored in Bilterm's command history.
3. **Interactive REPL & Sub-Shell Lockout (`python`, `node`, `psql`, ` gdb`, `git add -p`):** When a foreground command enters its own interactive read loop (either canonical line-by-line input or raw character-by-character input like `[y/n/q]` in `git add -p`), keeping focus trapped in the shell's floating command editor—or waiting for `Enter` before forwarding bytes—makes single-keystroke prompts and REPL tab-completion unresponsive.
4. **False Raw-Mode Detection Against Modern Shells:** Relying strictly on `termios.c_lflag & ICANON` to detect whether the user is "at the shell prompt" vs "running a program" fails immediately because `zsh` (ZLE), `bash` (readline), and `fish` all put the PTY into raw/non-canonical mode (`!ICANON && !ECHO`) while waiting for normal command input!

**Why it happens:**
Developers assume the PTY is in canonical mode (`ICANON | ECHO`) at the shell prompt and raw mode (`!ICANON`) only during full-screen or interactive programs. In reality, modern shells manage their own line editing in raw mode and only restore canonical mode *after* forking a child process.

**How to avoid:**
Implement a **Three-Signal Input Routing State Machine** per `CompositePane`:
1. **Shell Integration Markers (Primary — OSC 133 / FinalTerm):** Inject lightweight shell integration scripts (`PROMPT_COMMAND` / `precmd` / `preexec`) emitting `OSC 133;A` (prompt start), `OSC 133;B` (command start / ready for input), `OSC 133;C` (command executed / foreground job running), and `OSC 133;D;exit_code` (command finished). Only route typing to the `FloatingCommandPrompt` between `133;B` and `133;C`.
2. **Foreground Process Group + Termios Inspection (Fallback / Security Guard):** Before rendering a keystroke in the floating editor or saving on `Enter`, query the PTY master file descriptor:
   - Check `tcgetpgrp(pty_master_fd)` vs the root shell's PID. If a foreground child process group owns the TTY (`tcgetpgrp != shell_pgid`), auto-route keystrokes directly to the PTY stream (passthrough mode) unless explicitly overridden.
   - Inspect `tcgetattr(pty_master_fd)`: if `(c_lflag & ICANON) != 0 && (c_lflag & ECHO) == 0`, the child is in **password/secret entry mode** (`sudo`, `ssh`, `cryptsetup`). Immediately switch input to masked direct-PTY passthrough and bypass history recording.
3. **Clean Shell Submission Protocol:** When submitting from the `FloatingCommandPrompt` while the shell is at `OSC 133;B`, use bracketed paste (`\x1b[200~...\x1b[201~\r`) for multi-line commands so the shell executes the block atomically without intermediate newline execution.

**Warning signs:**
- Typing `sudo ls` shows no password prompt until after hitting `Enter`, or displays the password in plaintext inside the floating box.
- Running `git add -p` or `rm -i` requires pressing `Enter` in the floating box instead of responding immediately to `y`/`n`.
- Pasting a 5-line `for` loop in the floating editor executes line 1 prematurely.

**Phase to address:**
Phase 1 (Core PTY & Composite Pane Architecture) and Phase 2 (Floating Command Editor & Shell Integration).

---

### Pitfall 2: Alternate Screen (`DECSET 1049/1047/47`) & `SIGWINCH` Geometry Thrashing

**What goes wrong:**
Bilterm's core value proposition is that `OutputCanvas` spans the full composite pane height while `FloatingCommandPrompt` floats on top (pinned top like IBM 3270, pinned bottom, or free-floating), auto-hiding when full-screen TUI apps (`vim`, `htop`, `less`) enter the Alternate Screen (`\x1b[?1049h`). Two severe sizing failures occur:
1. **Viewport Occlusion vs. Resize Loops:** If the PTY is given the *full* composite pane dimensions `(W, H)` while the floating prompt is pinned to the top (occupying 3 rows), the first 3 rows of output or cursor positions are hidden behind the floating prompt. Conversely, if Bilterm shrinks the PTY `TIOCSWINSZ` to `(W, H - 3)` when the floating prompt is visible and expands it to `(W, H)` on `DECSET 1049h`, every transition between normal shell and `vim`/`less` triggers a `SIGWINCH` storm, reflowing scrollback lines and causing `less -RFX` or `git diff` to miscalculate page height.
2. **Incomplete Alternate Screen Escape Coverage & Missed Teardowns:** Checking only `\x1b[?1049h` / `\x1b[?1049l` misses legacy `DECSET 47` (`\x1b[?47h`) and `DECSET 1047` (`\x1b[?1047h`) used by older `ncurses`/`terminfo` entries (or inside nested `screen`/`tmux` with `TERM=screen`). Furthermore, if a full-screen program crashes (`kill -9`), `1049l` is never emitted, leaving the floating prompt permanently hidden until manual recovery.

**Why it happens:**
Treating the floating prompt as a layout split during normal mode and an overlay during alternate screen mode causes `TIOCSWINSZ` geometry changes on mode switches, or ignoring vertical scroll padding when the prompt overlays the canvas.

**How to avoid:**
1. **Keep PTY `TIOCSWINSZ` Equal to the `OutputCanvas` Rect `(W, H_canvas)` (minus only fixed OIA status bar):** Never resize the PTY when showing, hiding, or dragging the `FloatingCommandPrompt`. This guarantees zero `SIGWINCH` thrashing when entering/exiting `vim`, `htop`, or `less`, and allows live output to genuinely scroll underneath the floating window.
2. **Virtual Scroll Margin / Viewport Offset Compensation:** When the floating prompt is pinned at the top (3270 mode) or bottom of the composite pane, reserve virtual scroll padding in the `OutputCanvas` renderer (e.g., auto-scrolling output down by `prompt_height` rows when at bottom, or rendering top-pinned prompt with translucent/toggleable peek and `scroll_offset`) so the active shell line is never permanently trapped behind the floating box.
3. **Track Full VT State + Shell Return Reset:** Detect `47`, `1047`, and `1049` directly from the `vt100`/`alacritty_terminal` parser state machine (`screen.alternate_screen()`), not via regex scanning on raw PTY bytes. Additionally, whenever `OSC 133;A` (shell prompt return) fires or the foreground process group returns to the root shell PID while `alternate_screen()` is still true, provide an optional auto-reset or prominent OIA indicator (`ALT-SCR LOCKED - PRESS <KEY> TO RESET`).

**Warning signs:**
- Exiting `vim` causes the shell scrollback history to reflow and wrap weirdly.
- Running `git log` (which invokes `less`) hides the last 3 lines of the diff behind the floating prompt before `less` triggers alternate screen (or when `LESS=-X` suppresses `1049h`).
- Killing a hung TUI app leaves the pane permanently stuck without its floating command prompt.

**Phase to address:**
Phase 1 (PTY & VT100 Emulation) and Phase 3 (Composite Overlay & Alternate Screen Auto-Hide).

---

### Pitfall 3: Z-Order Compositing & Ratatui Buffer Bleed-Through on Wide Graphemes and Styles

**What goes wrong:**
Bilterm supports both **floating command prompts inside each composite pane** and **z-order stacked composite panes** overlapping one another on a single Ratatui `Buffer`. When an upper layer (a floating prompt or a raised composite pane) renders on top of a lower layer (`OutputCanvas` or background pane), severe visual corruption occurs at the boundaries:
1. **Split Wide Characters (CJK / Emoji / Nerd Fonts):** Suppose the background PTY canvas has a 2-column wide character (e.g., `🦀` or `漢`) at columns `(x=9, x=10)`. An overlapping floating pane or stacked composite pane starts its left border at `x=10`. Overwriting cell `(10, y)` with `│` leaves the leading half of `🦀` at `(9, y)`. When Crossterm flushes the diff to the host terminal, `🦀` at `x=9` still occupies 2 physical columns on the host terminal, pushing the border character `│` and every subsequent cell on row `y` right by 1 column—shattering the entire TUI layout!
2. **Right-Edge Orphan Continuation Cells:** Conversely, if the upper pane ends at `x=9`, overwriting `(9, y)` leaves the trailing empty continuation cell of the wide character at `(10, y)`. Ratatui's buffer representation stores a zero-width spacer symbol at `x=10`, which either renders as a glitchy artifact or fails to clear the second half of the host terminal's wide glyph.
3. **Style / Background Color Bleed (`Color::Reset` vs Explicit Background):** Using `Clear` in Ratatui resets cell symbols to `" "` and styles to `Style::default()` (`Color::Reset`). If the lower pane had an explicit ANSI background color or reverse-video (`INVERSE`) attribute, or if the user enables "translucent/dimmed background pane" themes, `Clear` strips attributes inconsistently or lets underline/strikethrough/italic modifiers bleed into the upper widget's border.

**Why it happens:**
Ratatui's built-in `Clear` widget simply loops over `Rect` cells and calls `cell.reset()`, completely unaware of multi-column Unicode character boundaries (`unicode-width` == 2) immediately outside the `Rect`'s left (`rect.left() - 1`) and right (`rect.right()`) edges.

**How to avoid:**
Build a dedicated **`CompositePaneCompositor` (`occlude_rect`)** helper invoked before rendering any floating prompt or stacked pane:
1. **Left-Boundary Wide-Char Sanitization:** For every row `y` in `rect.top()..rect.bottom()`, if `rect.left() > 0`, inspect cell `(rect.left() - 1, y)`. If `cell.symbol().width() > 1`, replace `(rect.left() - 1, y)`'s symbol with `" "` (preserving its background style) so it cannot overflow into `rect.left()`.
2. **Right-Boundary Spacer Sanitization:** Before clearing `rect`, check cell `(rect.right() - 1, y)`. If it contains the first half of a wide character (`width() > 1`) and `rect.right() < buf.area.right()`, explicitly set `(rect.right(), y)` to `" "` so an orphaned zero-width continuation cell doesn't remain outside the right border.
3. **Explicit Theme Background & Modifier Reset:** After clearing `rect`, explicitly set `Modifier::Reset` and the active 3270 theme background color on every cell in `rect` so `UNDERLINED`, `REVERSED`, or `BLINK` attributes from the underlying PTY canvas never bleed into the floating prompt or top-stacked pane.
4. **Hardware Cursor Ownership:** Only the single topmost focused widget (either the focused pane's `FloatingCommandPrompt` or, in Alternate Screen/passthrough mode, its `OutputCanvas`) may set `Frame::set_cursor_position`. Background/occluded panes must render their cursors as software-styled cells (e.g., hollow block or dimmed block) clipped against upper z-order rects.

**Warning signs:**
- Dragging a floating command box or shuffling stacked panes over `ls` output containing Nerd Font icons, emojis, or CJK filenames causes right-hand pane borders to zig-zag by 1 character.
- Blinking or underlined text in the background PTY makes the floating command prompt's border blink or underline.

**Phase to address:**
Phase 3 (Z-Order Compositing, Floating Overlay & Split/Stack Layout Engine).

---

### Pitfall 4: Tmux Prefix Collisions, Mouse-Coordinate Translation, and Escape Passthrough

**What goes wrong:**
Bilterm requires first-class `tmux` support (running smoothly inside `tmux`, or synchronizing with `tmux` sessions/panes). Four classic multiplexer-in-multiplexer failures break usability:
1. **Prefix Key & Shortcut Deadlocks:** If Bilterm binds `Ctrl-b` (or `Ctrl-a`) or `Alt+Arrow` for splitting/shuffling composite panes, running Bilterm inside `tmux` causes `tmux` to intercept the keystrokes first, splitting the outer `tmux` window instead of the inner Bilterm composite pane (or vice versa when running `tmux` inside a Bilterm pane).
2. **SGR Mouse (DECSET 1000/1002/1003/1006) Coordinate Mismatch & Interception:** When a TUI app (`vim`, `htop`, or nested `tmux`) inside a split or z-stacked Bilterm composite pane enables mouse reporting (`\x1b[?1000h\x1b[?1006h`), Bilterm intercepts host mouse events for its own border-dragging, z-order click-to-raise, and floating prompt dragging. If Bilterm forwards untranslated `(screen_x, screen_y)` coordinates to the child PTY, clicking at `(50, 20)` inside a right-hand split pane sends `(50, 20)` instead of pane-local `(1, 1)` to `vim`! Worse, if a stacked pane or floating prompt occludes part of that pane, mouse clicks on the floating prompt accidentally trigger clicks inside the underlying `vim` buffer.
3. **Truncated Passthrough Escape Sequences (`OSC 52`, `DCS tmux;`):** When Bilterm runs inside `tmux`, sequences like `OSC 52` (system clipboard copy) or synchronized output (`DECSET 2026`) emitted by programs inside a Bilterm PTY are swallowed by Bilterm's `vt100` parser unless explicitly parsed and re-emitted (wrapped in `\x1bPtmux;\x1b...\x1b\\` when `$TMUX` is present and `allow-passthrough` is needed).
4. **Recursive `$TMUX` / `$TERM` Environment Pollution:** If Bilterm sets `TERM=xterm-256color` unconditionally inside `tmux` or inherits `$TMUX` into child panes when managing independent PTYs, child commands running `tmux` CLI helpers target the wrong socket/pane or corrupt capabilities.

**Why it happens:**
Terminal emulators usually assume they own the top-level window, while multiplexers assume child panes don't have overlapping z-order widgets (`FloatingCommandPrompt`) inside each pane.

**How to avoid:**
1. **Modal / Non-Colliding Prefix & Tmux-Aware Keymap:** Default Bilterm's command prefix / window-managementchord to a non-`Ctrl-b` chord (e.g., `Ctrl-Space` or `Alt-g` / configurable 3270 `PF` keys + dedicated "Tmux Passthrough Toggle" mode, similar to Zellij's locked mode), and support `CSI u` (Kitty keyboard protocol / Crossterm `PushKeyboardEnhancementFlags`) when the host terminal supports it.
2. **Hit-Tested Z-Order Mouse Routing & Coordinate Translation:**
   - Perform top-down z-order hit testing on every mouse event: `FloatingCommandPrompt` (if visible and hit) $\rightarrow$ Pane Border/Titlebar $\rightarrow$ `OutputCanvas`.
   - If the event hits `OutputCanvas` and the child PTY has mouse tracking enabled (`vt100` mouse mode flags `1000`/`1002`/`1003`), subtract `canvas_rect.x` and `canvas_rect.y` to compute 1-based local coordinates `(local_col = mouse_x - canvas_rect.x + 1, local_row = mouse_y - canvas_rect.y + 1)` and re-encode the SGR `1006` sequence (`\x1b[<btn;local_col;local_rowM` or `m`) written to the child PTY.
   - Never forward mouse events to a child PTY if the coordinate falls inside that pane's `FloatingCommandPrompt` or an overlapping higher-z-order composite pane.
3. **Explicit `OSC 52` & Host Tmux Detection:** Detect `$TMUX` at Bilterm startup. Intercept `OSC 52` clipboard sequences from child PTYs and forward them to the host terminal (wrapping in DCS passthrough if inside `tmux` or using ` tmux set-buffer -w`). Set `TERM=xterm-256color` (or `tmux-256color`) accurately in child PTYs and strip or namespace `$TMUX` unless explicitly attaching to the outer tmux server.

**Warning signs:**
- Clicking in the right-hand split pane while running `nvim` places the cursor on the far right of the `nvim` buffer or does nothing.
- Clicking inside the floating command prompt while `htop` or `less --mouse` runs underneath scrolls or clicks the background app.
- Copying text with `y` in `nvim` inside Bilterm inside `tmux` fails to reach the system clipboard.

**Phase to address:**
Phase 4 (Triple-Modality Controls & Mouse Hit-Testing) and Phase 5 (Tmux Integration & Passthrough).

---

### Pitfall 5: Cross-Platform PTY Gotchas (Linux `/proc` vs macOS `ioctl` vs Windows ConPTY)

**What goes wrong:**
Code that works cleanly on Linux breaks on macOS or behaves erratically on Windows ConPTY:
1. **Windows ConPTY Invisible Frame Rendering & Fake Cursor Moves:** Unlike POSIX PTYs (which are raw byte pipes where the child writes ANSI directly), Windows **ConPTY** (`CreatePseudoConsole`) runs an internal screen buffer inside `conhost.exe` and translates the buffer diff into its own synthesized VT100 stream! ConPTY frequently emits `\x1b[2J` (clear screen), explicit cursor jumps `\x1b[row;colH`, and trailing spaces on resize—even for simple `dir` output. This breaks naive line-by-line scrollback append logic and can prematurely trigger false full-screen heuristics.
2. **Windows Missing `termios` & `tcgetpgrp`:** Windows has no `tcgetattr(fd)` (`ICANON`/`ECHO`) and no POSIX process groups (`tcgetpgrp`). Code that relies strictly on `nix::sys::termios` to detect password prompts or foreground child processes will not compile or work on Windows.
3. **macOS `/dev/ttys*` Master vs Slave FD `tcgetattr` Errors:** On Linux, calling `tcgetattr` or `tcgetpgrp` on the PTY *master* FD returned by `portable-pty` works directly. On macOS (XNU/Darwin), certain `termios` and `TIOCGPGRP` `ioctl` calls on the master FD return `ENOTTY` or stale flags unless called on the *slave* FD (or resolved via `sysctl` `KERN_PROC` for the session's foreground process group).
4. **Blocking PTY Reads & Zombie Shell Processes on Pane Close:** `portable-pty`'s `MasterPty::try_clone_reader()` returns a blocking `std::io::Read`. Wrapping it naively in `tokio::io::AsyncRead` blocks Tokio worker threads, and on Windows ConPTY, dropping the child handle without closing the `PseudoConsole` handle in the exact required order deadlocks the reader thread until the next byte is output.

**Why it happens:**
`portable-pty` abstracts *spawning* and *resizing* a PTY across OSes, but does **not** abstract line-discipline inspection (`ECHO`/`ICANON`), foreground process group detection, or ConPTY's synthesized VT stream behavior.

**How to avoid:**
1. **Isolate OS-Specific TTY Inspection Behind a `PtyIntrospector` Trait:**
   - **Linux:** Use `tcgetattr` and `tcgetpgrp` on the master FD + `/proc/<fg_pgid>/comm` for fast foreground process/echo inspection.
   - **macOS:** Keep a duplicated non-controlling handle to the slave path (`ttyname` / `open(slave_path, O_RDONLY | O_NOCTTY)`) or use `sysctl([CTL_KERN, KERN_PROC, KERN_PROC_PGRP, ...])` so `tcgetattr` never fails with `ENOTTY`.
   - **Windows:** Rely primarily on shell integration (`OSC 133` in PowerShell / `cmd` clink) plus Win32 Console API / child process tree inspection (`CreateToolhelp32Snapshot` checking if `powershell.exe` has spawned a child process like `ssh.exe` or `vim.exe`).
2. **Dedicated OS Blocking Reader Thread per Pane:** Spawn a dedicated `std::thread` (or `tokio::task::spawn_blocking`) per `CompositePane` reading into a reusable `[u8; 65536]` buffer and sending chunks over a bounded `tokio::sync::mpsc::channel` (for backpressure when `cat huge_file.log` runs).
3. **Full VT Parser for All Output (Never Line-Split Raw PTY Bytes):** Always feed PTY bytes into the `vt100` / `alacritty_terminal` state machine first, even on Windows ConPTY, so ConPTY's cursor positioning sequences render accurately onto the `OutputCanvas`.

**Warning signs:**
- `tcgetattr` works in Linux CI tests but returns `ENOTTY` (os error 25) on macOS.
- Closing a split pane on Windows hangs the UI or leaks `conhost.exe` processes.
- Resizing a pane on Windows duplicates the command prompt 3 times in scrollback.

**Phase to address:**
Phase 1 (PTY Abstraction & Cross-Platform Introspection) and Phase 6 (macOS & Windows ConPTY Hardening).

---

### Pitfall 6: Retro 3270 Cross-Hair Cursor & Blinking Caret Clashing with Host Terminal Cursor

**What goes wrong:**
Bilterm requires authentic IBM 3270 mainframe aesthetics: a **cross-hair cursor** (spanning the full row and column of the active caret), plus solid block, blinking hollow block, underscore, and vertical bar carets. Two visual bugs commonly occur:
1. **Cross-Hair Ruining Syntax Highlighting & Text Legibility:** If the cross-hair cursor overwrites the foreground/background colors of every cell in `cursor_row` and `cursor_col` with a flat solid color, syntax highlighting in the floating prompt or ANSI colors in the underlying canvas become unreadable along the cross-hair axes, and the intersection cell `(cursor_col, cursor_row)` becomes indistinguishable from the cross-hair arms.
2. **Host Terminal Cursor Fighting Software Cursor:** Standard terminal emulators only support DECSCUSR (`\x1b[1 q` through `\x1b[6 q`: blinking/steady block, underline, bar). They do **not** natively support a "blinking hollow block" (`☐`) or a cross-hair cursor. If Bilterm enables the host terminal's hardware cursor while *also* drawing a software hollow-block or cross-hair intersection cell, the host terminal's solid cursor paints directly over the hollow block, erasing the hollow interior!

**Why it happens:**
Mixing Ratatui cell-level styling with host terminal hardware cursor (`Frame::set_cursor_position` + `DECSCUSR`) without explicitly hiding the hardware cursor during custom software caret modes.

**How to avoid:**
1. **Dual-Mode Cursor Renderer (Hardware vs Software Caret):**
   - For standard modes (`SolidBlock`, `Underscore`, `VerticalBar`), emit the corresponding `DECSCUSR` escape sequence (`\x1b[2 q`, `\x1b[4 q`, `\x1b[6 q`) and call `frame.set_cursor_position((x, y))` so screen readers, IME composition popups, and low-latency hardware blinking work natively.
   - For custom 3270 modes (`HollowBlock`, `BlinkingHollowBlock`), hide the hardware cursor (`terminal.hide_cursor()`) or render the cell with `Modifier::REVERSED` / Unicode box border / custom 3270 glyph on tick intervals (`250ms`–`500ms` timer) while still updating cursor position only when IME is active.
2. **Non-Destructive Cross-Hair Compositing:** Render the cross-hair cursor as a **post-render pass clipped strictly to the active pane's bounds**:
   - On `row == cursor_y` (excluding `cursor_x`), apply `Modifier::UNDERLINED` (or subtle background tint/dimming in TrueColor 3270 phosphor theme) while preserving each cell's existing foreground text and symbol.
   - On `col == cursor_x` (excluding `cursor_y`), apply a subtle background tint (`rgb(0, 45, 0)` on phosphor green theme) while preserving the foreground glyph.
   - At `(cursor_x, cursor_y)`, apply high-contrast `Modifier::REVERSED | Modifier::BOLD` so the actual insertion point stands out unmistakably from the cross-hair lines.

**Warning signs:**
- Selecting "Hollow Block" cursor still shows a solid filled block in Alacritty, Kitty, or Gnome Terminal.
- Enabling the 3270 cross-hair cursor extends across *all* split panes instead of clipping to the focused composite pane, or hides text along the active column.

**Phase to address:**
Phase 4 (IBM 3270 Aesthetics, OIA Status Bar, and Cursor Engine).

---

## Technical Debt Patterns

Shortcuts that seem reasonable early on but create severe architectural rewrites later.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Storing panes as a flat `Vec<CompositePane>` instead of a hybrid **Split-Tree + Z-Order Stack Layer** | Fast to prototype 2 panes | Impossible to combine H/V tiled splits with z-order stacking/shuffling cleanly; mouse hit-testing and resize propagation become spaghetti | Never — define the layout tree + z-stack data model in Phase 1 |
| Unbounded PTY reader channel (`mpsc::unbounded_channel`) | Never blocks the PTY reader thread | Running `yes` or `cat /dev/urandom` consumes gigabytes of RAM in seconds and freezes the Ratatui UI loop as it processes millions of queued chunks | Never — always use bounded channels (`capacity: 64..256`) so OS PTY backpressure throttles runaway child processes |
| Calling `vt100::Parser::process()` on the main Ratatui UI render thread for every single byte chunk before drawing | Simple single-threaded state | A pane running `cargo build` or `find /` stalls keyboard input in the floating prompt and drops framerate below 10 FPS | Only in initial spike; move to coalesced 16ms frame batching (up to budget per tick) immediately in Phase 1 |
| Scanning raw PTY output bytes with regexes for `\x1b[?1049h` | Avoids inspecting parser state | Fails whenever an escape sequence is split across two `read()` buffer boundaries (e.g., `\x1b[?10` in chunk 1, `49h` in chunk 2) | Never — always query the state machine (`vt100` / `vte` parser) after feeding bytes |
| Hardcoding Linux `nix::sys::termios` calls directly inside widget event handlers | Quick echo detection on Linux | Breaks compilation on Windows and fails with `ENOTTY` on macOS master PTY FDs | Never — abstract behind a platform `PtyIntrospector` trait from day one |

## Integration Gotchas

Common mistakes when integrating external crates and protocols in the Rust TUI ecosystem.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| `tui-term` + `vt100` crate | Allocating a new `String` per cell on every frame and ignoring scrollback offset (`set_scrollback`) | Cache or render directly from `vt100::Screen::cell(row, col)` using zero-allocation `Cell::set_symbol(cell.contents())` and synchronize `screen.set_scrollback(offset)` with `OutputCanvas` scroll state |
| `crossterm` EventStream | Leaving `EnableBracketedPaste`, `EnableMouseCapture`, or `PushKeyboardEnhancementFlags` active when Bilterm panics | Install a `std::panic::set_hook` in Phase 1 that restores raw mode, exits alternate screen, disables mouse capture, and pops keyboard enhancement flags before printing the panic backtrace |
| `tmux` (Nested or Control Mode `-CC`) | Assuming `tmux` passes through extended keys (`Ctrl+Shift+...`) and `OSC 52` by default | Detect `$TMUX`, document/configure `extended-keys on` and `allow-passthrough on` (or use `tmux set-buffer -w`), and provide conflict-free prefix keys |
| Shell Integration (`bash` / `zsh` / `fish`) | Modifying the user's `~/.bashrc` or `~/.zshrc` on disk | Inject `OSC 133` hooks ephemerally via `--rcfile` / `ZDOTDIR` wrapper or `ENV` env vars at PTY spawn time without touching user dotfiles |

## Performance Traps

Patterns that work during manual typing but fail under real-world terminal workloads.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Rendering on every PTY `read()` notification instead of vsync/tick coalescing | 100% CPU usage and laggy floating prompt typing while compiling code in a background pane | Decouple PTY ingestion from rendering: drain PTY channels continuously, mark pane `dirty = true`, and call `terminal.draw()` at most once every `16ms` (60 FPS) or immediately on user keystroke | Any command outputting >5,000 lines/sec (`yes`, `cat`, `ripgrep`, `cargo test`) |
| Re-cloning full scrollback buffers on every frame for z-stacked background panes | Memory churn and frame drops when 4+ composite panes are open | Render only the visible `(W, H)` viewport slice of `vt100::Screen`; skip rendering `OutputCanvas` cells that are 100% occluded by an opaque full-screen stacked pane above them | >4 panes with `10,000` lines of scrollback history each |
| Naive multi-line syntax highlighting (`syntect` / `tree-sitter`) on every keystroke in `FloatingCommandPrompt` | Noticeable input latency (>30ms per key) when editing a 20-line shell script or SQL query in the floating prompt | Debounce or incrementally highlight only the visible lines of the floating prompt, and cap synchronous highlighting at 4KB of input | Pasting a large JSON payload or multi-line script into the floating command editor |

## Security Mistakes

Domain-specific security risks in a shell wrapper and terminal multiplexer.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Echoing or logging input in `FloatingCommandPrompt` when the slave PTY has `ECHO` disabled (`!ECHO && ICANON`) | **CRITICAL:** Passwords typed into `sudo`, `ssh`, `gpg`, `su`, or `mysql -p` are displayed on screen in plaintext and written to command history | Always check PTY `ECHO` state + foreground process group before buffering input in the floating editor; automatically switch to masked/direct PTY passthrough and disable history recording when `!ECHO` is detected |
| Honoring unrestricted `OSC 52` clipboard *read* requests (`\x1b]52;c;?\x07`) from child PTYs | Malicious remote SSH server or `curl` output can silently exfiltrate the user's system clipboard contents (passwords, keys) | Allow `OSC 52` clipboard *write* (`\x1b]52;c;<base64>\x07`), but **block** `OSC 52` clipboard *query/read* (`?`) by default |
| Failing to sanitize `OSC` / `DCS` title and status strings rendered in the 3270 OIA status bar | Malicious file names or `cat` output containing control characters (`\r`, `\x1b`) can spoof OIA status indicators or inject host terminal escape sequences | Strip all C0/C1 control characters (`0x00..=0x1F`, `0x7F..=0x9F`) from any window title (`OSC 0`/`OSC 2`) or CWD (`OSC 7`) before rendering in Ratatui widgets |

## UX Pitfalls

Common user experience mistakes specific to floating-prompt shells and stacked/tiled multiplexers.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Ambiguous Focus Between `FloatingCommandPrompt` and `OutputCanvas` | User types `j`/`k` expecting to scroll output (or interact with `python` REPL), but characters appear in the floating box—or vice versa | Show unmistakable visual focus in both the pane border and the 3270 OIA status bar (`[INPUT: CMD BOX]` vs `[INPUT: DIRECT PTY]` vs `[ALT SCREEN]`), with a single instant toggle key (`Esc` or `Ctrl-Space`) |
| Top-Pinned Floating Prompt (3270 Mode) Covering Fresh Output of Short Commands | User runs `pwd` or `date` at the top of a fresh shell; the output prints on rows 1–2 directly underneath the top-pinned floating prompt and is invisible | Add a top virtual scroll margin equal to the top-pinned prompt's height, or auto-anchor fresh output below the top-pinned prompt |
| Losing Track of Hidden Stacked Panes | User stacks 4 composite panes in z-order, shuffles them, and forgets that 3 panes are running behind the active pane | Render a compact **Z-Stack Deck / Tab Indicator** in the composite pane's titlebar or OIA status bar (`[Stack: 1/4 ▲▼]`) showing stacked background panes and their running processes |
| Intercepting `Up`/`Down` Arrows in Multi-Line Floating Editor vs Shell History | Pressing `Up` to recall the previous command only moves the cursor up one line inside a multi-line command, or vice versa | When cursor is on the **first line** of the floating prompt, `Up` recalls previous command history; on subsequent lines, `Up` moves the cursor up within the multi-line buffer (matching Warp / Fish / IPython ergonomics) |

## "Looks Done But Isn't" Checklist

Critical verification checks that separate a toy prototype from a daily-driver terminal emulator.

- [ ] **Password Prompts (`sudo` / `ssh`):** Often missing `termios` `!ECHO` detection — verify `sudo -k && sudo echo ok` never displays the password in the floating prompt or saves it to history.
- [ ] **Interactive Pagers Without Alternate Screen (`less -X` / `git log`):** Often missing foreground process group (`tcgetpgrp`) detection — verify `git log` routes `q`, `j`, `k`, and `/` directly to `less` even when the user has `LESS=-FRX` (which disables `DECSET 1049`).
- [ ] **Wide CJK & Emoji Occlusion:** Often missing 2-column boundary sanitization — verify dragging the floating prompt or a stacked pane across a screen filled with `🦀` or `日本語` produces zero border shift on the right edge.
- [ ] **Mouse Selection vs App Mouse Capture:** Often missing `Shift+Mouse` bypass — verify holding `Shift` while dragging selects text in Bilterm even when `vim` or `tmux` has mouse reporting enabled (`DECSET 1000`).
- [ ] **Orphaned Process Cleanup:** Often missing `SIGHUP` on pane close — verify closing a split or stacked composite pane cleanly terminates the child shell and its background subshells without leaking zombie processes or hung reader threads.
- [ ] **Resize Under Heavy Output:** Often missing synchronized rendering — verify resizing the terminal window while `htop` and `cargo build` run in split/stacked panes neither panics with out-of-bounds Ratatui `Buffer` coordinates nor corrupts ANSI parser state.

## Recovery Strategies

When pitfalls occur at runtime despite prevention, Bilterm must provide instant user recovery without losing the session.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Pane stuck in "Direct PTY / Alternate Screen" mode after a child crash (`kill -9 vim`) | LOW | Provide a manual **Toggle Floating Prompt / Reset VT Modes** hotkey (`Ctrl-Space r` or OIA command) that forces `\x1b[?1049l\x1b[?1000l`, restores the `FloatingCommandPrompt`, and sends `SIGWINCH` to the child shell |
| Shell integration (`OSC 133`) unavailable (e.g., user SSHs into a remote server without Bilterm shell hooks) | LOW | Automatically fall back to `tcgetpgrp` + `termios` inspection + manual "Passthrough vs Floating Prompt" toggle hotkey so remote SSH sessions remain 100% usable |
| Visual corruption from unrecognized escape sequence or host terminal desync | LOW | Bind a **Full Redraw (`Ctrl-Space Ctrl-l`)** action that calls `terminal.clear()` on the host Ratatui `Terminal` (forcing Crossterm to repaint every cell from scratch) and sends `SIGWINCH` to active PTYs |

## Pitfall-to-Phase Mapping

How roadmap phases should sequence and verify pitfall prevention.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| **Pitfall 1:** Echo & Line-Discipline Desync (`sudo`, REPLs, double echo) | Phase 1 (PTY Core) & Phase 2 (Floating Prompt + Shell Integration) | Automated PTY integration test running `bash`/`zsh`, `sudo -S`, `python3 -i`, and `git add -p` verifying input routing and zero cleartext password exposure |
| **Pitfall 2:** Alternate Screen (`1049`/`1047`/`47`) & `SIGWINCH` Geometry Thrashing | Phase 1 (VT100 Emulation) & Phase 3 (Alternate Screen Auto-Hide) | Verify `stty size` inside the PTY remains constant when toggling/moving the floating prompt, and `vim` / `htop` / `less` auto-hide and restore the prompt cleanly |
| **Pitfall 3:** Z-Order Compositing & Wide-Char Bleed-Through | Phase 3 (Composite Overlay & Split/Stack Layout Engine) | Unit test `occlude_rect` on a Ratatui `Buffer` populated with 2-cell wide emojis (`🦀`) and `Modifier::BLINK \| Modifier::UNDERLINED` cells |
| **Pitfall 4:** Tmux Collisions, SGR 1006 Mouse Offset, & Escape Passthrough | Phase 4 (Triple-Modality Controls) & Phase 5 (Tmux Integration) | Verify clicking inside a bottom-right split running `nvim` places the cursor at the exact clicked character, and running Bilterm inside `tmux` has zero prefix conflict |
| **Pitfall 5:** Cross-Platform PTY Gotchas (Linux / macOS / Windows ConPTY) | Phase 1 (`PtyIntrospector` trait) & Phase 6 (Cross-Platform Polish) | CI matrix tests spawning shells, resizing PTYs, and closing panes cleanly across Linux, macOS, and Windows |
| **Pitfall 6:** Retro 3270 Cross-Hair & Blinking Hollow Block Cursor Clashes | Phase 4 (IBM 3270 Aesthetics, OIA, & Cursor Engine) | Visual/buffer test verifying cross-hair preserves underlying glyphs/colors, clips to active pane, and hides hardware cursor when drawing hollow block |

## Sources

- **POSIX & Linux TTY Line Discipline:** `termios(3)`, `tty_ioctl(4)` (`TIOCGWINSZ`, `TIOCSWINSZ`, `TIOCGPGRP`), and how modern shells (`zsh` ZLE, `bash` readline) operate in non-canonical (`!ICANON && !ECHO`) mode at the prompt while restoring `ICANON` prior to `execve`.
- **FinalTerm / Semantic Prompts Specification (`OSC 133`):** Standard escape sequences (`133;A`, `133;B`, `133;C`, `133;D`) used by Warp, WezTerm, Kitty, and iTerm2 to separate prompt rendering, command input, and command execution output.
- **Ratatui Buffer & Wide-Character Rendering Architecture:** `ratatui::buffer::Buffer` cell storage model, `Clear` widget behavior (`cell.reset()`), and Unicode East Asian Width / emoji continuation cell artifacts on overlapping rect borders.
- **XTerm Control Sequences & Tmux Passthrough:** `DECSET 47 / 1047 / 1049` (Alternate Screen Buffer), `DECSET 1000 / 1002 / 1003 / 1006` (SGR Extended Mouse Reporting), `DECSCUSR` (Cursor Style), `OSC 52` (Clipboard), and `tmux` DCS passthrough (`\x1bPtmux;...`).
- **WezTerm `portable-pty` & Microsoft Windows ConPTY Documentation:** Architectural differences between POSIX byte-stream PTYs and Windows `CreatePseudoConsole` (`conhost.exe` screen-buffer synthesis and resize redraw behavior).

---
*Pitfalls research for: Bilterm (Rust + Ratatui Terminal Emulator, Multiplexer, and Floating-Prompt Shell Wrapper)*
*Researched: 2026-10-20*
