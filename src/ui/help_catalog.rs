//! Comprehensive `<F1>` / `:help` reference lines and `Ctrl+P` / `:palette` command catalog.

/// Complete keyboard shortcut & `:colon` command reference displayed inside the `<F1>` / `:help` modal.
pub const HELP_LINES: &[(&str, &str)] = &[
    ("═══ LEADER PREFIX (Ctrl+X), DETACH & GLOBAL HOTKEYS ═══", ""),
    ("Ctrl+X <key>", "Universal Leader Prefix (works in Command Box & Raw PTY)"),
    ("Ctrl+X ? / F1 / :help", "Toggle this interactive Help & :colon Command reference"),
    ("Ctrl+X D / :detach [name]", "Detach client & preserve full multi-pane window layout intact"),
    (":attach [name] / bilterm -a", "Re-attach & restore saved/detached multi-pane window layout"),
    ("bilterm ls / bilterm sessions", "List live background daemon sockets & saved .toml session layouts"),
    ("Ctrl+P / :palette", "Open Fuzzy Command Palette (nucleo-matcher search)"),
    ("Ctrl+X - / F2 / Alt+H", "Split focused composite pane horizontally (Top/Bottom)"),
    ("Ctrl+X | / Alt+V", "Split focused composite pane vertically (Left/Right)"),
    (":ssh <user@host> [port]", "Spawn a new CompositePane connected via SSH (or :ssh list)"),
    (":ssh-tmux <user@host> [session]", "Connect pane to remote persistent tmux -CC session over SSH"),
    ("F11 / :modern / :liquid / :style", "Switch UI between Classic Retro, Snazzy, Glass, Modern Minimal, & Liquid Glass"),
    ("Ctrl+X z / F5 / Alt+Z", "Toggle maximize/zoom on focused pane"),
    ("Ctrl+X o / F6 / Alt+Tab", "Shuffle pane deck top-to-bottom (cycle focus)"),
    ("Ctrl+X 1..9 / Alt+1..9", "Selectively raise & focus pane #1..#9"),
    ("Ctrl+X M / Alt+M", "Toggle Tiled Splits vs Z-Order Stacked Windows"),
    ("Ctrl+X d / Alt+D / F7 / :drawer", "Toggle Slide-Out Sessions / Windows / Panes Drawer (Push by default)"),
    ("Ctrl+X q / Ctrl+\\ / F10", "Toggle persistent Guake Quick Terminal (macOS-safe)"),
    ("Ctrl+X e / F4 / Alt+E", "Cycle Floating Prompt -> Fixed Prompt -> Regular Terminal"),
    ("Ctrl+X g / F12 / Ctrl+G", "Toggle Command Box vs Regular Terminal (Raw PTY)"),
    ("Ctrl+X x / Alt+X", "Cycle IBM 3270 Cross-Hair (Pane -> Full -> Off)"),
    ("Alt+O / :spotlight", "Cycle Focal Spotlight (Circle/Ellipse × Fuzzy/Pixelated)"),
    ("Alt+Shift+O / :focalzoom", "Cycle Focal Zoom Lens (1.25x -> 1.50x -> 2.00x -> 3.00x -> Off)"),
    ("Alt+Shift+] / Alt+Shift+[", "Grow / shrink Focal Spotlight or Focal Zoom radius"),
    ("Ctrl+Q / :quit / :quit!", "Exit Bilterm (:quit honors Exit Guard; :quit! forces exit)"),
    ("═══ MASTER CONTROL PROGRAM (:mcp) & EXIT GUARD ═══", ""),
    (":mcp / :mcp status", "Open MCP Root Console (↑/↓/Enter, J/K=Move, </>=Parent, +=Win, n=Branch, f=Freeze, m/d, Esc)"),
    (":mcp tree", "Show hierarchical SessionTree in MCP floating/push-out window"),
    (":mcp float", "Switch MCP console window to Floating Borland overlay mode"),
    (":mcp push [top|bottom|left|right]", "Switch MCP console window to Push-Out mode (shrinks panes)"),
    (":mcp attach <node>", "Attach client to session node in SessionTree"),
    (":mcp spawn <parent/child>", "Spawn child session branch or root session in SessionTree"),
    (":mcp graft <src> <dst>", "Graft session subtree onto a new parent node"),
    (":mcp detach-subtree <node>", "Promote session subtree to an independent root session"),
    (":mcp broadcast <node> <cmd>", "Fan-out shell command across an entire session subtree"),
    (":mcp freeze <node> / thaw <node>", "Freeze (read-only lock) or thaw a session subtree"),
    (":mcp derez <node> --confirm", "Terminate a session subtree (requires --confirm)"),
    (":mcp checkpoint", "Persist atomic WorkspaceJournal checkpoint to disk"),
    (":mcp shutdown --confirm", "Shut down background multiplexer daemon (requires --confirm)"),
    (":exit-guard [guard|respawn|detach|off]", "Configure final/root shell exit protection policy"),
    (":respawn-pane / :respawn", "Respawn exited shell in-place inside held pane"),
    ("═══ COLON (:) WINDOW, LAYOUT, DRAWER & SESSION COMMANDS ═══", ""),
    (":split h | :split v (:vsplit)", "Split focused pane horizontally or vertically"),
    (":zoom / :close / :focus <id>", "Maximize/restore pane, close pane, or focus pane #<id>"),
    (":deck next | :deck prev", "Cycle Z-Order window deck forward or backward"),
    (":layout tile | :layout stack", "Switch between Binary Split Tree & Z-Order Stacked Windows"),
    (":arrange [grid|columns|rows|main-side|cascade|stack-grid]", "Auto-arrange all open windows in-place (preserving live PTYs)"),
    (":layout save-pos [name] | restore-pos [name]", "Save or restore window positions/geometry mid-flight without respawning PTYs"),
    (":profile list | :layout list", "List all layout profiles ([ACTIVE], [DEFAULT], autosave=ON/OFF)"),
    (":profile switch <name> | next | prev", "Dynamically switch layout profile (single/dual-v/dual-h/main-side/quad/cascade/<custom>)"),
    (":profile save [name] | :layout save", "Save current multi-pane window layout to profile [name]"),
    (":profile default <name>", "Set <name> as the active default layout profile on startup"),
    (":layout autosave [on|off] / :autosave", "Toggle automatic saving of pane/window layout changes (ON by default)"),
    (":resize <left|right|up|down|reset> [n]", "Resize split ratio or Z-Stack floating window dimensions"),
    (":move <left|right|up|down|reset> [n]", "Move Z-Stack floating window or swap Tiled panes"),
    (":cascade / :equalize", "Reset custom Z-Stack window positions & equalize split ratios"),
    (":quake / :guake", "Toggle drop-down Quake Quick Terminal"),
    (":drawer [toggle|focus|float|push]", "Control Slide-Out Drawer (n/c/+=NewWin, N=NewSess, J/K=Move, </>=Parent, b=Branch)"),
    (":drawer [left|right|top|bottom|size <n>]", "Dock Slide-Out Drawer on any screen edge or set size"),
    (":drawer [fixed|autohide|ondemand]", "Set Slide-Out Drawer visibility behavior"),
    (":drawer attention", "Focus the next pane needing attention (blocked, then errored, then done)"),
    (":drawer fold", "Navigator drawer: fold or unfold the selected session (key e)"),
    (":drawer enter", "Navigator drawer: focus the selected row (keys Enter / Space)"),
    (":drawer next", "Navigator drawer: select the next row (keys j / Down)"),
    (":drawer prev", "Navigator drawer: select the previous row (keys k / Up)"),
    (":drawer row <n>", "Navigator drawer: select row n (1-based; what a mouse click applies)"),
    (":drawer scroll <+-n>", "Navigator drawer: move the selection n rows (keys PgUp / PgDn = -5 / +5)"),
    (":drawer widen <+-n>", "Navigator drawer: grow or shrink the drawer by n (keys { / } = -2 / +2)"),
    (":drawer new-session", "Navigator drawer: create a new session and switch to it (key N)"),
    (":drawer new-pane", "Navigator drawer: split the focused pane into a new window (keys n c + w)"),
    (":drawer rename [name]", "Navigator drawer: rename the selected session, or open the prompt (key r)"),
    (":drawer close-pane", "Navigator drawer: close the selected pane (keys x / Delete)"),
    (":drawer expand all", "Navigator drawer: unfold every workspace and session (key E; also :navigator expand all)"),
    (":drawer collapse all", "Navigator drawer: fold everything to workspace rows; again unfolds (key C; also :navigator collapse all)"),
    (":navigator expand all", "Navigator tree: unfold every workspace and session (alias of :drawer expand all)"),
    (":navigator collapse all", "Navigator tree: fold everything to workspace rows (alias of :drawer collapse all)"),
    (":drawer add window [session]", "Navigator drawer: add a window to the selected (or named) session (key a; also :navigator add window)"),
    (":drawer add pane [h|v]", "Navigator drawer: split the selected pane horizontally or vertically (key A; also :navigator add pane)"),
    (":drawer menu [close|pick <n>]", "Navigator drawer: open the right-click menu on the selected row, close it, or run entry n (mouse: right-click a row)"),
    (":navigator add window [session]", "Navigator tree: add a window to the selected (or named) session (alias of :drawer add window)"),
    (":navigator add pane [h|v]", "Navigator tree: split the selected pane (alias of :drawer add pane)"),
    ("Navigator drawer keys", "j/k Enter ! e E C N a A n r x PgUp/PgDn { } (every key has a :drawer command twin)"),
    ("Navigator drawer mouse", "click row/fold marker/[+], right-click row = menu, footer * ! N + r x @, wheel, drag edge (each gesture = the key or :drawer twin)"),
    (":drawer style [classic|navigator]", "Switch the drawer between the classic telemetry rows and the compact navigator tree"),
    (":agents [list]", "Agent state per pane: working / blocked / idle / done / errored (data over RPC)"),
    (":agents zoom [pane-id]", "Open the agent traffic window (graph + conversation)"),
    (":agents close", "Close the agent traffic window"),
    (":agents dismiss", "Esc: close the window, or release the keyboard when pinned"),
    (":agents select <left|right|up|down|node-key>", "Move the node cursor (arrow keys), or select a node by key such as pane:2 (click)"),
    (":agents edge <next|prev|a b>", "Cycle the selected node's edges (Tab / Shift+Tab), or select the edge between two nodes (click)"),
    (":agents enter", "Focus the selected pane node and leave the traffic window (Enter)"),
    (":agents move <dx> <dy>", "Move the traffic window by cells (Shift+arrows; drag the title bar)"),
    (":agents resize <dw> <dh>", "Grow or shrink the traffic window by cells (Alt+arrows; drag an edge or corner)"),
    (":agents rect <x> <y> <w> <h>", "Set the traffic window rect, clamped to the workspace (min 40x12)"),
    (":agents size <cols>x<rows>|auto", "Set the traffic window size, or auto for the default centred size"),
    (":agents max", "Toggle maximize for the traffic window (m; title badge)"),
    (":agents split <20..80|+n|-n>", "Set or step the graph/conversation splitter (< and >; drag the divider)"),
    (":agents grab [on|off]", "Grab the selected node: arrows nudge it (g toggles; Esc, Enter or g drop)"),
    (":agents nudge <left|right|up|down>", "Move the selected node one cell and pin it (arrows in grab mode)"),
    (":agents node-pin <node-key> <x> <y>", "Pin a node at a permille position 0..1000 of the graph (node drag)"),
    (":agents node-unpin [node-key]", "Release a pinned node to the automatic layout (u; right-click a node)"),
    (":agents pin [toggle]", "Pin the traffic window open: Esc then only releases the keyboard (*; click the pin badge in the title)"),
    (":agents unpin", "Unpin the traffic window so Esc closes it again"),
    (":agents scale <75..200>|+n|-n", "Set or step the traffic window's own panel scale (GUI only; [ and ]; Ctrl+wheel over the window)"),
    (":session new [name] / :new-session", "Create and switch to a new named workspace session"),
    (":session blank [name]", "Start a clean, blank single-pane workspace session"),
    (":session rename [old] <new>", "Rename active or specified workspace session"),
    (":session move [name] <up|down|top|bottom>", "Reorder sessions in the session list & F7 Drawer"),
    (":session save [name]", "Save layout, shell, running software/commands & CWD per pane to <name>.toml"),
    (":session restore [name]", "Restore layout, shell, CWD & auto-relaunch saved commands from <name>.toml"),
    (":session processes [on|off]", "Toggle automatic relaunch of each pane's saved running software/command"),
    (":session cmd [<command>|clear]", "Inspect, set, or clear the saved running command for the focused pane"),
    (":session shell [<prog> [args]]", "Inspect or set the shell program & arguments saved for the focused pane"),
    (":session cwd [<path>]", "Inspect or set the working directory saved for the focused pane"),
    (":session relaunch / :session info", "Relaunch saved pane commands or inspect all panes' shell/CWD/command"),
    (":session list / :session tree", "List saved sessions or open SessionTree pop-up/push window"),
    (":session branch <parent>/<child>", "Branch a new child session under <parent>"),
    (":session graft <src> <dst>", "Re-parent session <src> under <dst>"),
    (":session detach-subtree <node>", "Promote <node> subtree to a top-level root session"),
    (":session checkpoint [slot]", "Write crash-safe WorkspaceJournal checkpoint"),
    (":ssh <user@host> [port]", "Spawn a new CompositePane connected via SSH"),
    (":ssh-tmux <user@host> [session]", "Connect pane to remote persistent tmux -CC session over SSH"),
    (":remote list | add | exec | disconnect", "Manage & fan-out commands across SSH / tmux-CC fleet"),
    ("═══ COLON (:) PROMPT, READLINE, HISTORY & BLOCKS ═══", ""),
    (":prompt top | bottom | float <row>", "Pin Floating Command Prompt to Top, Bottom, or row offset"),
    (":prompt split-top | split-bottom", "Dock Fixed (Non-Overlapping) Command Prompt at Top or Bottom"),
    (":prompt height <3..12>", "Set Command Prompt box height in rows"),
    (":prompt width <cols|full>", "Set Command Prompt box width in columns or full pane width"),
    (":mode [float|fixed|terminal|cycle|standard|minimal|zen]", "Switch Floating/Fixed/Raw prompt; standard|minimal|zen sets display chrome (non-Legacy looks)"),
    (":raw", "Toggle manual Raw PTY Passthrough override"),
    (":history [popup|runs|on|off]", "Open History Pop-up, inspect run segments, or toggle disk log"),
    (":history save [path]", "Export session command history to a shell script"),
    (":history max-files <N>", "Set maximum retained session history files"),
    (":block prev | :block next", "Jump between prior commands in scrollback (OSC 133)"),
    (":yank / :paste / :clear", "Yank via OSC 52, paste OS clipboard into PTY, or clear OutputCanvas"),
    ("Ctrl+Shift+V / Shift+Ins / Ctrl+X p/v", "Paste OS clipboard directly into active PTY (even in RAW PTY)"),
    ("Shift+Arrows / Ctrl+C/V / Alt+B/F", "Select text, Copy/Paste, or move/delete by word in Prompt"),
    ("Tab / Shift+Tab / Ctrl+R", "Tab-complete paths/commands/:colon or Ctrl+R fuzzy history"),
    ("═══ COLON (:) CANVAS, SCROLLER, COPY, SEARCH & RECORDING ═══", ""),
    (":scroll [smooth|page|instant|toggle]", "Configure Smooth Kinetic, Mainframe Page-Hold (MORE...), or Instant"),
    (":scroll next | :scroll prev | :more", "Advance or step back 1 full page in Page-Hold (MORE...) mode"),
    (":scrollback [clear|status]", "Report adaptive scrollback memory budget or clear scrollback"),
    (":copy / :vi (F8 / Alt+C)", "Enter OutputCanvas Vi/Cursor Copy & Rectangular Block mode"),
    (":rect / :select (Alt+R / Alt+Drag)", "Enter Rectangular Block Select mode (r/b/Ctrl+V/Alt+Shift+Arrows)"),
    (":rect mode [on|off|toggle]", "Toggle persistent Rectangular Mouse Drag mode (plain LeftDrag = block)"),
    (":rect <r1> <c1> <r2> <c2> [copy]", "Highlight (or copy) a 1-based rectangular region on OutputCanvas"),
    (":rect copy | :rect cut | :rect off", "Copy active rectangular selection (OSC 52 + clipboard), send to prompt, or clear"),
    (":search <query|next|prev|off>", "Search OutputCanvas & jump between highlighted matches"),
    (":links / :url (Ctrl+Click)", "Yank visible or OSC 8 hyperlink to clipboard via OSC 52"),
    (":hints / :fields (F3 / Ctrl+X f)", "Activate QuickSelect 3270 Field Hints ([a]..[z]) for URLs/paths/IPs"),
    (":lock [on|off] (Ctrl+X l / Alt+L)", "Toggle Read-Only Protected 3270 Field Lock (🔒 PROT)"),
    (":broadcast [on|off]", "Toggle Synchronized Multi-Pane Input Broadcast (📡 BCAST)"),
    (":record [start|workspace|pane] [path]", "Start Asciicast v2 (.cast) pane or whole-workspace recording"),
    (":record stop", "Stop active Asciicast v2 (.cast) recording"),
    (":trigger add <pattern> | clear", "Register output regex/substring alert trigger or clear triggers"),
    (":notifications", "Clear OSC 9 / OSC 777 desktop notifications"),
    ("═══ COLON (:) THEMES, FONTS, SHADERS, GRAPHICS & CHROME ═══", ""),
    (":theme <preset> | :theme global <preset>", "Switch active window theme (or all windows via :theme global <preset>)"),
    (":theme list | :theme import [path]", "Open Universal Theme browser or import iTerm2/TOML/YAML scheme"),
    (":modern [on|off] / :liquid [on|off]", "Toggle Non-Retro Modern Minimal or Liquid Glass UI mode"),
    (":browse [dir] / :files [dir]", "Open interactive Text File & Theme Directory Browser (Enter, Backspace, type filter)"),
    (":theme config", "Initialize & inspect ~/.config/bilterm/config.toml & themes/ directory"),
    (":fontsize [+|-|reset|<pt>|<pct>%]", "Zoom terminal/GUI font size in/out or set exact size/scale"),
    (":font <preset|custom <ttf>|list|reset>", "Configure per-pane/global font family, .ttf file, bold/italic"),
    (":outline [window|prompt] <style>", "double | single/thin | rounded | thick/bold | dotted | dashed | quadruple | ascii"),
    (":outline bold [on|off]", "Render all window outline frames in bold (or normal) weight"),
    (":window / :window menu", "Open interactive Per-Window Style, Theme & Element Color Customizer"),
    (":window theme [list|browse|import]", "Open Per-Window Theme Browser (14 presets + .itermcolors) for focused window"),
    (":window theme <name|next|prev|reset>", "Apply per-window theme preset or imported .itermcolors to focused window"),
    (":window <elem> <color|reset>", "Per-window color: font-color|bg|prompt-bg|prompt-color|cursor-color|frame-color|controls-color|status-color|title-color|badge-color"),
    (":window outline <style|next|reset>", "Per-window outline frame style (double, thin, dashed, thick, ...)"),
    (":window bold [on|off] / :window reset", "Per-window bold border, or reset window to global theme, colors & style"),
    (":shadows [on|off|translucent|tinted|solid]", "Configure Borland Turbo Vision window & prompt drop shadows"),
    (":appearance <legacy|obsidian|midnight|arctic|liquid>", "Switch the visual look; legacy restores the classic retro style"),
    (":chrome title all|focused|none|auto", "Non-Legacy looks: which pane frames show a title"),
    (":chrome ring on|off", "Non-Legacy looks: accent-colored border on the focused pane"),
    (":panel scale <75..200>", "GUI only: scale the font of the side drawer, :mcp console and agent traffic window (100 = normal)"),
    (":glyphs <unicode|cp437|nerd-font|ascii>", "Switch box-drawing & status indicator glyph pack"),
    (":cursor [window|global] [shape|blink|color]", "Customize active window or global cursor (block|hollow|underline|bar|custom, cursor-color)"),
    (":crosshair [on|off|full|pane]", "Configure IBM 3270 full-screen or pane-local cross-hair ruler"),
    (":spotlight <circle|ellipse|pixel|fuzzy|off>", "Configure focal spotlight shape, feathering, or radius (+/-)"),
    (":focalzoom <1.25x|1.5x|2x|3x|pixel|fuzzy|off>", "Configure focal zoom magnification lens"),
    (":crosshair cycle / :layout cycle", "Cycle crosshair (off/pane/full) or workspace layout mode (Alt+X / Alt+M)"),
    (":focal radius <+N|-N> [+N|-N]", "Grow/shrink focal spotlight & zoom radius (Alt+} / Alt+{)"),
    (":prompt move up|down / resize-height|resize-width <+N|-N>", "Move or resize the floating prompt (Alt+Up/Down, Alt++/-, Alt+[/])"),
    (":search prefill", "Open the prompt pre-filled with `:search ` (Alt+/)"),
    (":get <key> / :set <key> <value>", "Read or change any config.toml key by dotted path (e.g. appearance.profile)"),
    (":config list / :config save / :appearance save", "List config keys; persist changed keys (comments kept) to config.toml"),
    (":state", "Show appearance, mode, layout, active pane, pane count and drawer state"),
    (":crt [on|off]", "Toggle retro CRT phosphor scanline shader effect"),
    (":menubar [on|off] / :fkeybar [on|off]", "Toggle top Borland Menu Bar or bottom DOS F-Key Bar"),
    (":version [on|off] / :inspector [on|off]", "Toggle OIA version badge or F9 3270 Terminal Inspector"),
    (":tek [on|off|demo|erase|clear]", "Tektronix 4010/4014 vector storage-tube mode & demo"),
    (":canvas [hires|demo|mode|plot|image|clear]", "High-resolution Braille/Sextant/Kitty/Sixel/iTerm2 canvas"),
];

/// Complete Fuzzy Command Palette catalog (`Ctrl+P` / `:palette`) paired with shortcuts and descriptions.
pub const PALETTE_CATALOG: &[(&str, &str, &str)] = &[
    (":split h", "Ctrl+X - / F2", "Split focused pane horizontally (Top/Bottom)"),
    (":split v", "Ctrl+X | / Alt+V", "Split focused pane vertically (Left/Right)"),
    (":ssh ", "—", "Connect a new CompositePane to a remote SSH host (:ssh <user@host> [port])"),
    (":ssh-tmux ", "—", "Attach remote persistent tmux -CC session over SSH (:ssh-tmux <user@host>)"),
    (":remote list", "—", "List active SSH / remote tmux -CC fleet nodes & profiles"),
    (":modern", "F11", "Toggle Non-Retro Modern Minimal UI mode (rounded chrome, obsidian slate)"),
    (":liquid", "F11", "Toggle Liquid Glass UI mode (refractive glass borders + ambient halo)"),
    (":mcp status", "—", "Open TRON Master Control Program (MCP) Root Console window"),
    (":mcp tree", "—", "Show hierarchical SessionTree in MCP floating/push-out window"),
    (":mcp float", "—", "Switch MCP console window to Floating Borland overlay mode"),
    (":mcp push right", "—", "Dock MCP console window in Push-Out mode (shrinks pane area)"),
    (":mcp checkpoint", "—", "Persist atomic WorkspaceJournal checkpoint to disk"),
    (":detach", "Ctrl+X D", "Detach client & save full multi-pane window layout intact"),
    (":attach", "bilterm -a", "Re-attach & restore saved/detached multi-pane window layout"),
    (":exit-guard guard", "—", "Enable Exit Guard modal on final/root shell exit"),
    (":exit-guard respawn", "—", "Auto-respawn shell in-place on final/root shell exit"),
    (":exit-guard detach", "—", "Auto-detach client on final/root shell exit"),
    (":exit-guard off", "—", "Disable exit guard interception"),
    (":respawn-pane", "—", "Respawn exited shell in-place inside held pane"),
    (":zoom", "Ctrl+X z / F5", "Toggle zoom/maximize on focused pane"),
    (":close", "Ctrl+X x", "Close focused composite pane"),
    (":layout stack", "Ctrl+X M / Alt+M", "Switch workspace to Z-Order Stacked Cascade"),
    (":layout tile", "Ctrl+X M / Alt+M", "Switch workspace to Tiled Binary Split Tree"),
    (":profile list", "—", "List all layout profiles ([ACTIVE], [DEFAULT], autosave state)"),
    (":profile next", "—", "Dynamically switch to the next layout profile"),
    (":profile prev", "—", "Dynamically switch to the previous layout profile"),
    (":profile switch dual-v", "—", "Switch dynamically to 2-pane side-by-side layout profile"),
    (":profile switch dual-h", "—", "Switch dynamically to 2-pane top/bottom layout profile"),
    (":profile switch main-side", "—", "Switch dynamically to 3-pane IDE (main + 2 side) layout profile"),
    (":profile switch quad", "—", "Switch dynamically to 4-pane 2x2 grid layout profile"),
    (":profile switch cascade", "—", "Switch dynamically to 3-pane Borland Z-Stack cascade profile"),
    (":profile save", "—", "Save current pane/window layout to active profile"),
    (":profile default", "—", "Set current profile as the active default startup layout"),
    (":layout autosave", "—", "Toggle automatic saving of pane/window layouts (ON by default)"),
    (":resize right", "Ctrl+Alt+Right / Ctrl+X >", "Widen active pane split or Z-Stack floating window"),
    (":resize left", "Ctrl+Alt+Left / Ctrl+X <", "Narrow active pane split or Z-Stack floating window"),
    (":resize down", "Ctrl+Alt+Down", "Grow height of active pane split or Z-Stack window"),
    (":resize up", "Ctrl+Alt+Up", "Shrink height of active pane split or Z-Stack window"),
    (":move right", "Ctrl+Shift+Right / Ctrl+Alt+L", "Move Z-Stack window right or swap Tiled panes"),
    (":move left", "Ctrl+Shift+Left / Ctrl+Alt+H", "Move Z-Stack window left or swap Tiled panes"),
    (":move down", "Ctrl+Shift+Down / Ctrl+X J", "Move Z-Stack window down or swap Tiled panes"),
    (":move up", "Ctrl+Shift+Up / Ctrl+X K", "Move Z-Stack window up or swap Tiled panes"),
    (":cascade", "Ctrl+X =", "Reset custom Z-Stack window positions & equalize splits"),
    (":deck next", "Ctrl+X o / F6", "Shuffle Z-order window deck top-to-bottom"),
    (":quake", "Ctrl+X q / Ctrl+\\", "Toggle persistent Guake drop-down quick terminal"),
    (":drawer toggle", "F7 / Ctrl+X d", "Toggle Slide-Out Sessions/Windows/Panes Drawer"),
    (":drawer float", "—", "Set Slide-Out Drawer to Floating overlay mode"),
    (":drawer push", "—", "Set Slide-Out Drawer to Push-Out tiled mode"),
    (":drawer attention", "Ctrl+X !", "Focus the next pane needing attention (blocked, then errored, then done)"),
    (":drawer fold", "e (Navigator drawer)", "Fold or unfold the selected session in the navigator drawer"),
    (":drawer enter", "Enter / Space (Navigator drawer)", "Focus the selected row in the navigator drawer"),
    (":drawer next", "j / Down (Navigator drawer)", "Select the next row in the navigator drawer"),
    (":drawer prev", "k / Up (Navigator drawer)", "Select the previous row in the navigator drawer"),
    (":drawer row 1", "Click (Navigator drawer)", "Select row n (1-based) in the navigator drawer"),
    (":drawer scroll 5", "PgDn (Navigator drawer)", "Move the navigator drawer selection by n rows (negative goes up)"),
    (":drawer widen 2", "} (Navigator drawer)", "Widen the navigator drawer by n (negative narrows)"),
    (":drawer new-session", "N (Navigator drawer)", "Create a new session from the navigator drawer"),
    (":drawer new-pane", "n / c / + / w (Navigator drawer)", "Split the focused pane into a new window from the navigator drawer"),
    (":drawer rename", "r (Navigator drawer)", "Rename the selected session (opens the prompt without a name)"),
    (":drawer close-pane", "x / Delete (Navigator drawer)", "Close the selected pane in the navigator drawer"),
    (":drawer expand all", "E (Navigator drawer)", "Unfold every workspace and session in the navigator drawer"),
    (":drawer collapse all", "C (Navigator drawer)", "Fold the navigator tree down to workspace rows (again unfolds)"),
    (":navigator expand all", "E (Navigator drawer)", "Unfold every workspace and session (alias of :drawer expand all)"),
    (":navigator collapse all", "C (Navigator drawer)", "Fold the navigator tree down to workspace rows (alias of :drawer collapse all)"),
    (":drawer add window", "a (Navigator drawer)", "Add a window to the selected session in the navigator drawer"),
    (":drawer add pane v", "A (Navigator drawer)", "Split the selected pane (h or v) in the navigator drawer"),
    (":drawer menu", "Right-click row (Navigator drawer)", "Open the navigator context menu: new session, new window, split, rename, close"),
    (":drawer menu pick 1", "Click menu entry (Navigator drawer)", "Run context-menu entry n on the selected row and close the menu"),
    (":navigator add window", "a (Navigator drawer)", "Add a window to the selected session (alias of :drawer add window)"),
    (":navigator add pane", "A (Navigator drawer)", "Split the selected pane (alias of :drawer add pane)"),
    (":agents list", "—", "Agent state per pane: working / blocked / idle / done / errored (data over RPC)"),
    (":agents zoom", "Leader a", "Open the agent traffic window (graph + conversation)"),
    (":agents close", "—", "Close the agent traffic window"),
    (":agents dismiss", "Esc (agent traffic window)", "Close the window, or release the keyboard when pinned"),
    (":agents select left", "Arrows (agent traffic window)", "Move the node cursor in the traffic graph"),
    (":agents edge next", "Tab / Shift+Tab (agent traffic window)", "Cycle the selected node's edges"),
    (":agents enter", "Enter (agent traffic window)", "Focus the selected pane node and leave the window"),
    (":agents move -1 0", "Shift+arrows (agent traffic window)", "Move the traffic window one cell"),
    (":agents resize 1 0", "Alt+arrows (agent traffic window)", "Grow or shrink the traffic window one cell"),
    (":agents size auto", "—", "Return the traffic window to its default centred size"),
    (":agents max", "m (agent traffic window)", "Toggle maximize for the traffic window"),
    (":agents split 55", "< / > (agent traffic window)", "Set the graph/conversation splitter in percent"),
    (":agents grab", "g (agent traffic window)", "Toggle node grab mode; arrows then nudge the selected node"),
    (":agents nudge right", "Arrows (grab mode)", "Move the selected node one cell and pin it"),
    (":agents node-unpin", "u (agent traffic window)", "Release the selected pinned node"),
    (":agents pin", "* (agent traffic window)", "Pin the traffic window open; Esc releases only the keyboard"),
    (":agents unpin", "* (agent traffic window)", "Unpin the traffic window"),
    (":agents scale +10", "] / [ / Ctrl+wheel (agent traffic window)", "Step the traffic window's own panel scale (GUI only)"),
    (":inspector", "Ctrl+X i / F9", "Open Live 3270 Terminal Inspector modal"),
    (":hints", "Ctrl+X f / F3", "Activate QuickSelect 3270 Field Hints ([a]..[z])"),
    (":copy", "F8 / Alt+C / Ctrl+Up", "Focus OutputCanvas to move cursor & select text"),
    (":rect", "Alt+R / Alt+Drag", "Start 2D Rectangular Block Selection & Highlight on OutputCanvas"),
    (":rect mode", "—", "Toggle persistent Rectangular Mouse Drag mode (LeftDrag selects rectangle)"),
    (":rect copy", "Enter / y / Ctrl+C", "Copy highlighted rectangular block to OS clipboard & OSC 52"),
    (":rect cut", "x / Ctrl+X", "Insert highlighted rectangular block into Floating Command Prompt"),
    (":rect clear", "Esc", "Clear rectangular highlight & exit Copy Mode"),
    (":scroll page", "Ctrl+X PgDn", "Toggle IBM 3270 / Timeshare Block/Page-Hold Scroller (MORE...)"),
    (":scroll smooth", "—", "Switch pane to Smooth Sub-Cell / Kinetic Velocity Scroller"),
    (":scroll instant", "—", "Switch pane to Discrete Line-by-Line Scroller"),
    (":more", "PF8 / PA2 / Space", "Advance 1 full page in Mainframe Block/Page-Hold Mode"),
    (":scrollback status", "—", "Show adaptive scrollback memory usage & budget"),
    (":scrollback clear", "—", "Clear active pane scrollback buffer"),
    (":version", "—", "Toggle version number badge in 3270-OIA status bar"),
    (":lock", "Ctrl+X l / Alt+L", "Toggle Read-Only Protected 3270 Field Lock"),
    (":broadcast", "—", "Toggle Synchronized Multi-Pane Input Broadcast"),
    (":menubar", "—", "Toggle Top Borland Turbo Vision Menu Bar"),
    (":fkeybar", "—", "Toggle Bottom DOS Function-Key Bar"),
    (":crt", "—", "Toggle Retro CRT Phosphor Scanline Effect"),
    (":mode cycle", "Ctrl+X e / F4", "Cycle Floating Prompt -> Fixed Prompt -> Regular Terminal"),
    (":mode fixed", "Ctrl+X s / Alt+S", "Switch to Fixed (Non-Overlapping Docked) Command Prompt"),
    (":mode float", "Ctrl+X t / Alt+T", "Switch to Floating Command Prompt over Terminal Canvas"),
    (":mode terminal", "F12 / Ctrl+G", "Switch to Regular Terminal (Hide Command Prompt Box)"),
    (":prompt split-top", "Ctrl+X s / Alt+S", "Non-overlapping Split Command Prompt at Top"),
    (":prompt top", "Ctrl+X t / Alt+T", "Pin Floating Command Prompt to Top"),
    (":prompt bottom", "Ctrl+X b / Alt+B", "Pin Floating Command Prompt to Bottom"),
    (":history popup", "Alt+P", "Open Command History Pop-up Window"),
    (":history runs", "—", "Inspect recorded command history run segments"),
    (":block prev", "—", "Jump to previous OSC 133 command block in scrollback"),
    (":block next", "—", "Jump to next OSC 133 command block in scrollback"),
    (":crosshair full", "Ctrl+X x / Alt+X", "Enable Full-Screen IBM 3270 Cross-Hair Cursor"),
    (":spotlight", "Alt+O", "Cycle Focal Spotlight (Circle/Ellipse × Fuzzy/Pixelated)"),
    (":spotlight ellipse fuzzy", "Alt+O", "Enable Elliptical Fuzzy Focal Spotlight"),
    (":spotlight ellipse pixel", "Alt+O", "Enable Elliptical Pixelated Character-Block Spotlight"),
    (":spotlight circle pixel", "Alt+O", "Enable Circular Pixelated Character-Block Spotlight"),
    (":spotlight +", "Alt+Shift+]", "Grow Focal Spotlight Radius (+4 cols, +1 row)"),
    (":spotlight -", "Alt+Shift+[", "Shrink Focal Spotlight Radius (-4 cols, -1 row)"),
    (":spotlight off", "—", "Disable Focal Spotlight"),
    (":focalzoom", "Alt+Shift+O", "Cycle Focal Zoom Lens (1.25x -> 1.50x -> 2.00x -> 3.00x -> Off)"),
    (":focalzoom 1.5x fuzzy", "Alt+Shift+O", "Enable 1.50x Smooth Fuzzy Focal Zoom Lens"),
    (":focalzoom 2x fuzzy", "Alt+Shift+O", "Enable 2.00x High Magnification Focal Zoom Lens"),
    (":focalzoom 2x pixel", "—", "Enable 2.00x Pixelated Character-Block Focal Zoom Lens"),
    (":focalzoom +", "—", "Step Focal Zoom Magnification Up (+0.25x)"),
    (":focalzoom -", "—", "Step Focal Zoom Magnification Down (-0.25x)"),
    (":focalzoom off", "—", "Disable Focal Zoom Lens"),
    (":theme list", "—", "Open Universal Theme & .itermcolors Catalog in floating/push window"),
    (":browse", "—", "Open interactive Text File & Theme Directory Browser"),
    (":theme import", "—", "Open interactive File & Directory Browser to import theme schemes"),
    (":theme config", "—", "Initialize & show ~/.config/bilterm/config.toml & themes/ directory"),
    (":itermcolors", "—", "View & apply discovered iTerm2 .itermcolors schemes (iterm2colors)"),
    (":cursor", "—", "Open interactive Cursor Style, Blink & Color Selector"),
    (":cursor block blink", "—", "Set cursor to Blinking Block (█)"),
    (":cursor hollow blink amber", "—", "Set cursor to Blinking Hollow Block (▯) in Amber (#FFB000)"),
    (":cursor underline solid cyan", "—", "Set cursor to Solid Underline (_) in Cyan (#00FFFF)"),
    (":cursor bar blink", "—", "Set cursor to Blinking Vertical Bar (▏)"),
    (":cursor custom ◆ solid #00ff66", "—", "Set cursor to Solid Custom Character (◆) in Phosphor Green"),
    (":cursor reset", "—", "Reset cursor shape, blink mode, and color to theme defaults"),
    (":paste", "Ctrl+Shift+V / Ctrl+X p", "Paste OS clipboard directly into active pane or Raw PTY"),
    (":window menu", "—", "Open interactive Per-Window Style, Theme & Element Color Customizer"),
    (":window theme list", "—", "Open Per-Window Theme Browser (14 presets + .itermcolors) for focused window"),
    (":window theme browse", "—", "Open interactive File & .itermcolors Browser for focused window"),
    (":window theme next", "—", "Cycle focused window's own theme (independent of other windows)"),
    (":window theme reset", "—", "Reset focused window to the global theme"),
    (":window font-color green", "—", "Set focused window's terminal & prompt font color to Phosphor Green"),
    (":window cursor-color amber", "—", "Set focused window's cursor color to Gas-Plasma Amber"),
    (":window frame-color cyan", "—", "Set focused window's active border frame color to Turbo Cyan"),
    (":window controls-color yellow", "—", "Set focused window's [_][□][X] & ◢ controls color to Yellow"),
    (":window status-color navy", "—", "Set focused window's 3270-OIA status bar background to Royal Navy"),
    (":window outline next", "—", "Cycle focused window's outline style (double/thin/rounded/thick/dashed/...)"),
    (":window outline dashed", "—", "Set focused window's outline frame to dashed"),
    (":window bold toggle", "—", "Toggle bold border on focused window"),
    (":window reset", "—", "Reset all per-window theme, outline, and element color overrides"),
    (":outline bold on", "—", "Render all window outline frames in bold"),
    (":theme turbo-blue", "—", "Switch palette to Borland Turbo Vision Blue"),
    (":theme 3278", "—", "Switch palette to IBM 3278 Phosphor Green"),
    (":theme 3279", "—", "Switch palette to IBM 3279 Amber CRT"),
    (":session new", "—", "Create and switch to a new workspace session"),
    (":session blank", "bilterm -B", "Start a clean, blank single-pane workspace session"),
    (":session move up", "Shift+Up (Drawer)", "Move active session up in the session list"),
    (":session move down", "Shift+Down (Drawer)", "Move active session down in the session list"),
    (":session save default", "—", "Save layout, shell, running software/commands & CWD per pane to TOML"),
    (":session restore default", "—", "Restore saved layout, shell, CWD & auto-relaunch running commands"),
    (":session processes", "—", "Toggle auto-relaunch of saved running software/commands on session restore"),
    (":session info", "—", "Inspect each pane's saved shell, working directory (CWD), and running command"),
    (":session relaunch", "—", "Relaunch saved running commands across all writable session panes"),
    (":session list", "—", "List all saved workspace sessions"),
    (":session tree", "—", "Show hierarchical SessionTree in floating/push window"),
    (":session checkpoint", "—", "Persist crash-safe WorkspaceJournal checkpoint"),
    (":record start", "Ctrl+X R", "Start Asciicast v2 (.cast) terminal recording"),
    (":record workspace", "—", "Start whole-workspace Asciicast v2 (.cast) recording"),
    (":record stop", "—", "Stop Asciicast v2 (.cast) terminal recording"),
    (":fontsize +", "Ctrl++", "Zoom terminal font & text size in (+10%)"),
    (":fontsize -", "Ctrl+-", "Zoom terminal font & text size out (-10%)"),
    (":fontsize reset", "Ctrl+0", "Reset terminal font & text size to 100%"),
    (":font cycle", "—", "Cycle terminal & GUI font family preset"),
    (":font cascadia", "—", "Switch font family to Microsoft Cascadia Mono / Code"),
    (":font consolas", "—", "Switch font family to ClearType Consolas"),
    (":font jetbrains", "—", "Switch font family to JetBrains Mono"),
    (":font firacode", "—", "Switch font family to Fira Code"),
    (":font iosevka", "—", "Switch font family to Iosevka Term"),
    (":font plex", "—", "Switch font family to IBM Plex Mono"),
    (":font hack", "—", "Switch font family to Hack"),
    (":font ibm3270", "—", "Switch font family to IBM 3270 / 3270 Nerd Font"),
    (":font vga", "—", "Switch font family to Borland DOS PxPlus IBM VGA8"),
    (":font list", "—", "List available font presets and loaded .ttf/.otf font files"),
    (":tek", "—", "Toggle Tektronix 4010/4014 storage-tube vector mode"),
    (":tek demo", "—", "Render Tektronix 4014 storage-tube wireframe vector demo"),
    (":tek erase", "—", "Erase Tektronix 4014 screen with phosphor afterglow decay"),
    (":tek off", "—", "Exit Tektronix 4010/4014 mode"),
    (":canvas demo", "—", "Render modern Braille/Sextant vector + live plot demo"),
    (":canvas hires", "—", "Render high-resolution sub-cell & pixel graphics demo"),
    (":canvas mode braille", "—", "Set canvas rasterizer to 2x4 Unicode Braille sub-cells"),
    (":canvas mode sextant", "—", "Set canvas rasterizer to 2x3 Unicode Sextant sub-cells"),
    (":canvas clear", "—", "Clear all Tektronix vectors and modern canvas primitives"),
    (":notifications clear", "—", "Clear OSC 9 / OSC 777 desktop notifications"),
    (":remote list", "—", "List local and remote SSH / tmux-CC fleet sessions"),
    (":help", "Ctrl+X ? / F1", "Open Borland Turbo Vision Quick Reference Help"),
    (":quit", "Ctrl+Q", "Cleanly exit Bilterm (honors Exit Guard)"),
    (":quit!", "—", "Force immediate exit bypassing Exit Guard"),
    (":appearance obsidian", "—", "Look: Obsidian (modern dark default)"),
    (":appearance midnight", "—", "Look: Midnight"),
    (":appearance arctic", "—", "Look: Arctic (light)"),
    (":appearance liquid", "—", "Look: Liquid glass"),
    (":appearance legacy", "—", "Look: Legacy retro style (the classic 3270/Borland look)"),
    (":mode standard", "—", "Display mode: standard chrome (non-Legacy looks)"),
    (":mode minimal", "—", "Display mode: minimal chrome, no pane titles (non-Legacy looks)"),
    (":mode zen", "—", "Display mode: Zen, focused pane plus one input row (:mode standard to leave)"),
    (":panel scale 125", "—", "GUI only: scale drawer, :mcp and agents window text (75..200, 100 = normal)"),
    (":chrome title focused", "—", "Non-Legacy looks: title on the focused pane only (all|focused|none|auto)"),
    (":chrome ring on", "—", "Non-Legacy looks: accent border on the focused pane (on|off)"),
    (":autosave", "—", "Layout profile autosave: :autosave on|off (restore your layout on next launch)"),
    (":bell", "—", "Clear notifications / bell indicators (alias of :notifications)"),
    (":buffers", "—", "List the clipboard / yank history buffers (alias of :clipboard list)"),
    (":commands", "—", "Open this command palette (alias of :palette)"),
    (":equalize", "—", "Reset window geometry: equalize sizes / re-cascade (alias of :cascade)"),
    (":find", "—", "Search the active pane's scrollback: :find <text>, then :find next|prev|off (alias of :search)"),
    (":fkeys", "—", "Cycle F-key mode (Bilterm / app passthrough / auto); :fkeys status, :fkeys send F5"),
    (":inspect", "—", "Toggle the 3270 Terminal Inspector (alias of :inspector, F9)"),
    (":lens", "—", "Focal-zoom lens over the cursor area (alias of :focalzoom): :lens 2x|circle|off"),
    (":notify", "—", "Clear notifications / bell indicators (alias of :notifications)"),
    (":palette", "—", "Toggle the fuzzy command palette (Ctrl+X : / Ctrl+P)"),
    (":profiles", "—", "List saved layout profiles (alias of :profile list)"),
    (":respawn", "—", "Respawn an exited shell in place inside a held pane (alias of :respawn-pane)"),
    (":resume", "—", "Restore a saved session: :resume [name] (alias of :attach; default session if no name)"),
    (":save", "—", "Save the current workspace as a named session: :save <name> (alias of :session save)"),
    (":scanlines", "—", "Toggle CRT scanlines: :scanlines on|off (alias of :crt)"),
    (":sessions", "—", "Session commands: :sessions save|restore|tree ... (alias of :session)"),
    (":rename ", "—", "Rename a session: :rename [old_name] <new_name> (alias of :session rename)"),
    (":triggers add ", "—", "Add an output trigger: :triggers add <pattern> (also :triggers clear; alias of :trigger)"),
    (":urls", "—", "Copy/yank the links found in the active pane (alias of :links)"),
    (":guake", "—", "Toggle the drop-down quick terminal (alias of :quake)"),
    (":crosshair cycle", "Alt+X", "Cycle crosshair: off / pane / fullscreen"),
    (":layout cycle", "Alt+M", "Cycle workspace layout mode (tiled / stacked)"),
    (":focal radius +4 +1", "Alt+}", "Grow focal spotlight/zoom radius"),
    (":focal radius -4 -1", "Alt+{", "Shrink focal spotlight/zoom radius"),
    (":prompt move up", "Alt+Up", "Move the floating prompt up"),
    (":prompt move down", "Alt+Down", "Move the floating prompt down"),
    (":prompt resize-height +1", "Alt++", "Increase prompt height"),
    (":prompt resize-width +4", "Alt+]", "Widen the prompt"),
    (":search prefill", "Alt+/", "Open prompt pre-filled with :search"),
    (":get ", "—", "Read a config value by dotted key (:get <key>)"),
    (":set ", "—", "Set a config value in the running session (:set <key> <value>)"),
    (":state", "—", "Show workspace state (appearance, mode, layout, panes, drawer)"),
    (":config list", "—", "List every config key and value"),
    (":config save", "—", "Persist changed config keys to config.toml"),
    (":appearance save", "—", "Persist the appearance settings to config.toml"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colon_command::{execute_wm_colon_command, parse_colon_command};
    use crate::pane::completion::compute_completions;
    use crate::wm::split_tree::SplitDirection;
    use crate::wm::{WindowManager, WorkspaceLayoutMode};

    #[test]
    fn every_palette_entry_with_complete_args_parses_ok() {
        for &(cmd, _, _) in PALETTE_CATALOG {
            if cmd.ends_with(' ') || cmd.contains('<') || cmd.contains('[') {
                continue; // needs an argument typed by the user
            }
            match parse_colon_command(cmd) {
                Some(Ok(_)) => {}
                other => panic!("palette entry `{cmd}` does not parse: {other:?}"),
            }
        }
    }

    #[test]
    fn appearance_mode_panel_chrome_are_in_palette_completion_and_work() {
        let cwd = std::path::Path::new(".");
        for prefix in [":appear", ":mod", ":pane", ":chro"] {
            let (_, _, c) = compute_completions(prefix, prefix.len(), cwd);
            assert!(!c.is_empty(), "no completion for {prefix}");
        }
        let (_, _, c) = compute_completions(":appearance ", 12, cwd);
        assert_eq!(c.len(), 6); // + `save` (Phase 49-04)
        let (_, _, c) = compute_completions(":mode ", 6, cwd);
        assert!(["standard", "minimal", "zen"].iter().all(|m| c.iter().any(|x| x.display == *m)));
        for cmd in [":appearance obsidian", ":mode standard", ":mode minimal", ":mode zen", ":panel scale 125", ":chrome ring on"] {
            assert!(PALETTE_CATALOG.iter().any(|e| e.0 == cmd), "{cmd} missing from palette");
        }
        let mut wm = WindowManager::new_headless(24, 80);
        for (cmd, check) in [
            (":appearance midnight", "midnight"),
            (":mode zen", "zen"),
        ] {
            let action = parse_colon_command(cmd).unwrap().unwrap();
            execute_wm_colon_command(&mut wm, action);
            match check {
                "midnight" => assert_eq!(crate::ui::design::AppearanceProfile::resolve(wm.config()).label(), "midnight"),
                _ => assert_eq!(wm.config().appearance.display_mode.label(), "zen"),
            }
        }
    }

    #[test]
    fn ctrl_x_colon_palette_finds_appearance_and_mode_by_typing() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut wm = WindowManager::new_headless(24, 80);
        let key = |c| KeyEvent::new(c, KeyModifiers::NONE);
        crate::input::handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)).unwrap();
        crate::input::handle_wm_key_event(&mut wm, key(KeyCode::Char(':'))).unwrap();
        assert!(wm.is_palette_visible(), "Ctrl+X : opens the palette");
        for (typed, want) in [("appearance", ":appearance obsidian"), ("zen", ":mode zen"), ("chrome", ":chrome ring on"), ("panel", ":panel scale 125")] {
            wm.open_palette_with_query(typed);
            assert!(wm.filtered_palette_items().iter().any(|e| e.0 == want), "`{typed}` should list {want}");
        }
    }

    #[test]
    fn detach_and_attach_preserve_full_multi_pane_window_layout_and_geometries() {
        let tmp = tempfile::tempdir().unwrap();
        let mut wm = WindowManager::new_headless(24, 80);
        wm.config_mut().session_dir = Some(tmp.path().to_string_lossy().into_owned());
        wm.config_mut().history_session = "work".to_string();

        // Split into 2 panes, customize window geometry & prompt, and write canvas output
        let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
        assert_eq!(wm.pane_count(), 2);
        wm.set_layout_mode(WorkspaceLayoutMode::Stacked);
        wm.move_focused_pane_or_window(6, 3);
        wm.resize_focused_pane_or_window(4, 2);
        let saved_geom = wm
            .z_stack()
            .geometries()
            .get(&p2)
            .copied()
            .unwrap_or_default();
        assert!(saved_geom.is_custom());
        wm.active_pane_mut().process_pty_output(b"MAINFRAME-OUTPUT-42\r\n");
        wm.active_pane_mut().prompt_mut().set_text("echo layout_intact");

        // Execute :detach
        let detach_cmd = parse_colon_command(":detach").unwrap().unwrap();
        assert!(execute_wm_colon_command(&mut wm, detach_cmd));
        assert!(wm.is_client_detached());
        assert!(wm.has_detached_session("work"));

        // Fresh WindowManager re-attaches via :attach work
        let mut wm2 = WindowManager::new_headless(24, 80);
        wm2.config_mut().session_dir = Some(tmp.path().to_string_lossy().into_owned());
        wm2.config_mut().history_session = "work".to_string();
        assert!(wm2.has_detached_session("work"));
        let attach_cmd = parse_colon_command(":attach work").unwrap().unwrap();
        assert!(!execute_wm_colon_command(&mut wm2, attach_cmd));

        assert_eq!(wm2.pane_count(), 2);
        assert_eq!(wm2.layout_mode(), WorkspaceLayoutMode::Stacked);
        assert_eq!(
            wm2.z_stack()
                .geometries()
                .get(&p2)
                .copied()
                .unwrap_or_default(),
            saved_geom
        );
        assert_eq!(wm2.active_pane().prompt().text(), "echo layout_intact");
        assert!(wm2.active_pane().vt().contents().contains("MAINFRAME-OUTPUT-42"));
        assert!(!wm2.has_detached_session("work"));
    }

    #[test]
    fn help_lines_palette_and_tab_completion_cover_mcp_detach_and_all_colon_commands() {
        let joined_help: String = HELP_LINES
            .iter()
            .map(|(k, v)| format!("{k} {v}\n"))
            .collect();
        for required in [
            ":mcp status",
            ":mcp tree",
            ":mcp float",
            ":mcp push",
            ":mcp spawn",
            ":mcp graft",
            ":mcp detach-subtree",
            ":mcp broadcast",
            ":mcp freeze",
            ":mcp derez",
            ":mcp checkpoint",
            ":mcp shutdown",
            ":detach",
            ":attach",
            ":exit-guard",
            ":respawn-pane",
            ":drawer",
            ":session save",
            ":theme",
            ":fontsize",
            ":font",
            ":scroll",
            ":canvas",
            ":tek",
        ] {
            assert!(
                joined_help.contains(required),
                "Help screen missing '{required}'"
            );
        }

        let (_, _, mcp_cands) = compute_completions(":mcp ", 5, std::path::Path::new("."));
        let mcp_texts: Vec<&str> = mcp_cands.iter().map(|c| c.replacement.as_str()).collect();
        assert!(mcp_texts.contains(&"tree"));
        assert!(mcp_texts.contains(&"status"));
        assert!(mcp_texts.contains(&"float"));
        assert!(mcp_texts.contains(&"push"));

        let (_, _, root_cands) = compute_completions(":det", 4, std::path::Path::new("."));
        assert!(root_cands.iter().any(|c| c.replacement == ":detach"));
    }
}
