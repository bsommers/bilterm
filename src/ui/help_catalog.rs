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
    ("Ctrl+X z / F5 / Alt+Z", "Toggle maximize/zoom on focused pane"),
    ("Ctrl+X o / F6 / Alt+Tab", "Shuffle pane deck top-to-bottom (cycle focus)"),
    ("Ctrl+X 1..9 / Alt+1..9", "Selectively raise & focus pane #1..#9"),
    ("Ctrl+X M / Alt+M", "Toggle Tiled Splits vs Z-Order Stacked Windows"),
    ("Ctrl+X d / F7 / :drawer", "Toggle Slide-Out Sessions / Windows / Panes Drawer"),
    ("Ctrl+X q / Ctrl+\\ / F10", "Toggle persistent Guake Quick Terminal (macOS-safe)"),
    ("Ctrl+X e / F4 / Alt+E", "Cycle Floating Prompt -> Fixed Prompt -> Regular Terminal"),
    ("Ctrl+X g / F12 / Ctrl+G", "Toggle Command Box vs Regular Terminal (Raw PTY)"),
    ("Ctrl+X x / Alt+X", "Cycle IBM 3270 Cross-Hair (Pane -> Full -> Off)"),
    ("Alt+O / :spotlight", "Cycle Focal Spotlight (Circle/Ellipse × Fuzzy/Pixelated)"),
    ("Alt+Shift+O / :focalzoom", "Cycle Focal Zoom Lens (1.25x -> 1.50x -> 2.00x -> 3.00x -> Off)"),
    ("Alt+Shift+] / Alt+Shift+[", "Grow / shrink Focal Spotlight or Focal Zoom radius"),
    ("Ctrl+Q / :quit / :quit!", "Exit Bilterm (:quit honors Exit Guard; :quit! forces exit)"),
    ("═══ MASTER CONTROL PROGRAM (:mcp) & EXIT GUARD ═══", ""),
    (":mcp / :mcp status", "Open MCP Root Console (↑/↓/Enter/Click, r=Refresh, f=Freeze, n=Branch, m/d, Esc)"),
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
    (":resize <left|right|up|down|reset> [n]", "Resize split ratio or Z-Stack floating window dimensions"),
    (":move <left|right|up|down|reset> [n]", "Move Z-Stack floating window or swap Tiled panes"),
    (":cascade / :equalize", "Reset custom Z-Stack window positions & equalize split ratios"),
    (":quake / :guake", "Toggle drop-down Quake Quick Terminal"),
    (":drawer [toggle|focus|float|push]", "Control Slide-Out Drawer & Float vs Push-Out layout"),
    (":drawer [left|right|top|bottom|size <n>]", "Dock Slide-Out Drawer on any screen edge or set size"),
    (":drawer [fixed|autohide|ondemand]", "Set Slide-Out Drawer visibility behavior"),
    (":session save [name]", "Save multi-pane window layout & pane state to <name>.toml"),
    (":session restore [name]", "Restore multi-pane window layout & pane state from <name>.toml"),
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
    (":mode [float|fixed|terminal|cycle]", "Switch between Floating Prompt, Fixed Prompt, & Raw Terminal"),
    (":raw", "Toggle manual Raw PTY Passthrough override"),
    (":history [popup|runs|on|off]", "Open History Pop-up, inspect run segments, or toggle disk log"),
    (":history save [path]", "Export session command history to a shell script"),
    (":history max-files <N>", "Set maximum retained session history files"),
    (":block prev | :block next", "Jump between prior commands in scrollback (OSC 133)"),
    (":yank / :clear", "Yank last command/output via OSC 52 or clear OutputCanvas"),
    ("Shift+Arrows / Ctrl+C/V / Alt+B/F", "Select text, Copy/Paste, or move/delete by word in Prompt"),
    ("Tab / Shift+Tab / Ctrl+R", "Tab-complete paths/commands/:colon or Ctrl+R fuzzy history"),
    ("═══ COLON (:) CANVAS, SCROLLER, COPY, SEARCH & RECORDING ═══", ""),
    (":scroll [smooth|page|instant|toggle]", "Configure Smooth Kinetic, Mainframe Page-Hold (MORE...), or Instant"),
    (":scroll next | :scroll prev | :more", "Advance or step back 1 full page in Page-Hold (MORE...) mode"),
    (":scrollback [clear|status]", "Report adaptive scrollback memory budget or clear scrollback"),
    (":copy / :vi (F8 / Alt+C)", "Enter OutputCanvas Vi/Cursor Copy & Rectangular Block mode"),
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
    (":theme <preset> | :theme list", "Switch theme preset or open Universal Theme browser window"),
    (":theme import <path> | export [file]", "Import iTerm2/Alacritty/Kitty/Base16 theme or export TOML"),
    (":fontsize [+|-|reset|<pt>|<pct>%]", "Zoom terminal/GUI font size in/out or set exact size/scale"),
    (":font <preset|custom <ttf>|list|reset>", "Configure per-pane/global font family, .ttf file, bold/italic"),
    (":outline [window|prompt] <style>", "double | solid | rounded | thick | dotted | dashed | quadruple | ascii"),
    (":shadows [on|off|translucent|tinted|solid]", "Configure Borland Turbo Vision window & prompt drop shadows"),
    (":glyphs <unicode|cp437|nerd-font|ascii>", "Switch box-drawing & status indicator glyph pack"),
    (":cursor <block|hollow|underscore|bar>", "Set 3270 / DOS caret glyph shape"),
    (":crosshair [on|off|full|pane]", "Configure IBM 3270 full-screen or pane-local cross-hair ruler"),
    (":spotlight <circle|ellipse|pixel|fuzzy|off>", "Configure focal spotlight shape, feathering, or radius (+/-)"),
    (":focalzoom <1.25x|1.5x|2x|3x|pixel|fuzzy|off>", "Configure focal zoom magnification lens"),
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
    (":drawer toggle", "F7 / Ctrl+X d", "Toggle herdr-style Slide-Out Sessions/Windows/Panes Drawer"),
    (":drawer float", "—", "Set Slide-Out Drawer to Floating overlay mode"),
    (":drawer push", "—", "Set Slide-Out Drawer to Push-Out tiled mode"),
    (":inspector", "Ctrl+X i / F9", "Open Live 3270 Terminal Inspector modal"),
    (":hints", "Ctrl+X f / F3", "Activate QuickSelect 3270 Field Hints ([a]..[z])"),
    (":copy", "F8 / Alt+C / Ctrl+Up", "Focus OutputCanvas to move cursor & select text"),
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
    (":theme list", "—", "Open Universal Theme & Palette Catalog in floating/push window"),
    (":theme turbo-blue", "—", "Switch palette to Borland Turbo Vision Blue"),
    (":theme 3278", "—", "Switch palette to IBM 3278 Phosphor Green"),
    (":theme 3279", "—", "Switch palette to IBM 3279 Amber CRT"),
    (":session save default", "—", "Save multi-pane workspace session & window layout to TOML"),
    (":session restore default", "—", "Restore saved multi-pane workspace session & window layout"),
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
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colon_command::{execute_wm_colon_command, parse_colon_command};
    use crate::pane::completion::compute_completions;
    use crate::wm::split_tree::SplitDirection;
    use crate::wm::{WindowManager, WorkspaceLayoutMode};

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
