# Phase 63: Keyboard-First Navigation & Cross-OS Hotkey Harmonization - Research

**Researched:** 2026-11-08
**Domain:** TUI/GUI key routing, per-OS keymap profiles, modal navigation, conflict detection (Rust, crossterm 0.29, native Cocoa/Win32/X11 GUIs)
**Confidence:** HIGH on the in-repo inventory and bugs (code read this session); MEDIUM on third-party consumer chords (Windows Terminal, Claude Code, herdr cited; OS/desktop/vim/tmux/emacs rows are `[ASSUMED]`)

<user_constraints>
## User Constraints (from CONTEXT.md)

No `63-CONTEXT.md` exists (discuss-phase was not run). Binding inputs are the requirements in `.planning/REQUIREMENTS.md:271-273` and the orchestrator brief:

### Locked Decisions
- KEY-01: "Pure-keyboard navigation of windows, panes, drawers, and menus in both TUI and GUI without typing long colon commands (modal window mode + leader key with which-key hints)."
- KEY-02: "Per-OS default keymap profiles (macOS, Windows, Linux) with a platform-aware modifier abstraction, platform-correct menu accelerator labels, and load-time conflict detection against OS/desktop, shell, editor, and nested-multiplexer bindings."
- KEY-03: "`Window -> Stack windows` (currently `Alt+M`, which conflicts with macOS Option-compose) and other conflicting defaults are rebound to harmonized, conflict-free bindings that work uniformly on all target OSes."
- Every binding must lower to `crate::action::Action` and run through `WindowManager::apply`. `handle_wm_key_event` stays `<= 80` lines (ARCH-R01). `UNI-R01` surface parity applies (key, colon, palette, menu, RPC, MCP).
- Phase 62 (copy mode) consumes the copy-mode entry key defined here. Phase 61 fixes zoom-toggle input fall-through; Window mode must not reintroduce it.

### Claude's Discretion
Everything else: chord choices, table layout, file split, config schema, overlay look.

### Deferred Ideas (OUT OF SCOPE)
None recorded. Treated as out of scope by this research: rewriting copy-mode internals (Phase 62), zoom routing fix (Phase 61), a full user-rebindable window-mode editor UI.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| KEY-01 | Pure-keyboard navigation of windows/panes/drawers/menus, TUI + GUI | Window mode (sticky modal), keyboard-openable menu bar with mnemonics, which-key overlay (sections "Recommendation", "Gaps found") |
| KEY-02 | Per-OS profiles, modifier abstraction, platform labels, load-time conflict detection | `KeyEnv` + `Primary` resolution, `KeyProfile` tables, glyph formatter, reserved-chord table + `:keys check` |
| KEY-03 | Rebind Alt+M and other conflicting defaults uniformly | Alt+M replacement table; i18n/compose bugs found in normalizer, mac GUI, Win32 AltGr |
</phase_requirements>

## Summary

Bilterm's keyboard model today is three declarative tables plus a leader (`src/input/keymap_tables.rs`) and is heavily Alt-based: `DEFAULT_GLOBAL_BINDINGS` has 102 rows, most of them `Alt+<printable>` or `Ctrl+Alt+<letter>`. Those are exactly the chords that are unusable or hostile on macOS (Option is a compose key), on Windows with AltGr layouts (AltGr is Ctrl+Alt), on GNOME/KDE (Alt+F7/F8/Space belong to the window manager) and inside nested apps (readline `M-t/c/l/r/n/p`, Claude Code `Alt+P/T/O`, and `Alt+M` on Windows). The only truly portable mechanism already in the product is the leader key (`Ctrl+X`), but it needs two keystrokes plus memory, has no discoverability beyond a one-line status hint, and the menu bar cannot be opened from the keyboard at all.

Research also surfaced concrete defects that must be fixed in this phase because they are the same root cause (Alt/Option/AltGr treated as a hotkey modifier regardless of OS and layout): (1) `normalize_macos_option_key` is applied unconditionally on every OS, including raw PTY passthrough (`src/input/mod.rs:67`), so typing `ø`, `ß`, `å`, `ç`, `µ`, `π` on Linux/Windows is rewritten to `Alt+<letter>` and sent to the child as `ESC <letter>`; (2) the macOS GUI always turns Option+key into an Alt chord (`gui_macos.rs:271-285`), so `Option+5/6/7/8/9/L/N` (`[ ] | { } @ ~` on German/Nordic Mac layouts) can never be typed; (3) the Win32 GUI treats AltGr (Ctrl+Alt) as chords and suppresses `WM_CHAR` (`gui_window.rs:326-345`); (4) the menu bar category navigation is hard-coded to 7 categories while `MENUBAR_CATEGORIES` has 9 (`modal_palette.rs:505-510`); (5) four Alt chords collide after Shift-folding (`Alt+Shift+W` focus-up vs `Alt+W` ClosePane, and `H`, `D`, `O`).

**Primary recommendation:** Keep `Ctrl+X`/`Ctrl+B` as the universal leader, add a sticky **Window mode** (leader `w`, plus a direct `Primary+O`) with hjkl/arrows/digits/z/s/t/x/`[`, add a keyboard-openable menu (leader `m`) with mnemonics, introduce a `KeyEnv`/`Primary` abstraction (Cmd on macOS GUI, Ctrl+Shift on Windows/Linux GUI and on kitty-protocol TUIs, none on legacy TUIs) with per-OS profiles that bind **no `Alt+<printable>` WM chord on macOS**, and ship a data-driven reserved-chord table behind `:keys check`. Alt+M is replaced by **`Primary+L` (Cmd+L / Ctrl+Shift+L)**, **Window-mode `s`/`t`**, and the existing **leader `M`**; Alt+M survives only as a hidden legacy alias on Windows/Linux.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| OS key capture (Cmd/Option/AltGr, compose) | Native GUI runner (`gui_macos.rs`, `gui_window.rs`, `linux_x11.rs`) | TUI host terminal | Only the runner knows physical key vs composed text |
| Modifier abstraction (`Primary`) and profile selection | `src/input` (`keyenv`, `keyprofile`) | `src/config` | Pure functions, testable on every OS |
| Chord to `Action` lowering | `src/input/keymap*.rs` tables | `Action`/`apply` | Existing ARCH-R01 rule |
| Window mode / key-mode state | `src/wm/key_mode.rs` (state), `src/input/window_mode*.rs` (planner) | `src/ui` renderers | Mirrors `AgentsListState` + `agents_list_keys.rs` split |
| Which-key overlay, menu accelerator labels | `src/ui` (render from tables) | `src/input` (reverse lookup) | Render must derive from tables to prevent drift |
| Conflict detection | `src/input/keyconflict*.rs` (pure) | `src/config` load hook, RPC/MCP | Same data for load-time, `:keys check`, RPC |
| Surface parity (colon, palette, RPC, MCP) | `src/colon_command`, `src/command_registry.rs`, `src/api` | - | UNI-R01 |

## Standard Stack

No new crates. All needed capability exists in the pinned dependencies.

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| crossterm | 0.29.0 `[VERIFIED: Cargo.toml:21]` | Key events; `KeyModifiers::SUPER`, `PushKeyboardEnhancementFlags`, `supports_keyboard_enhancement()` | Already the input layer; kitty protocol support is in-crate `[CITED: docs.rs crossterm via kitty keyboard-protocol docs]` |
| ratatui | 0.30.2 | Which-key overlay and menu rendering | Existing renderer |
| serde + toml | existing | `[keys]` config table | Existing config stack |
| insta | existing | Snapshot of which-key overlay | Existing snapshot tests |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Own `KeyChord` type | `crokey` / `keymap-parser` crates | Extra dependency and slopsquat surface; our grammar needs `Primary`, leader sequences and Action lowering anyway. Hand-roll a ~200-line parser. |
| Static reserved-chord table | Runtime probing of OS shortcuts (`defaults read com.apple.symbolichotkeys`, `gsettings`) | Not portable, not testable, privacy-sensitive; offer as a future opt-in only |

**Installation:** none. **Package Legitimacy Audit:** no external packages added; section not applicable. Packages removed (SLOP): none. Flagged (SUS): none.

## Inventory of Current Bindings

All rows `[VERIFIED: src/input/keymap_tables.rs, src/input/keymap.rs, src/input/mod.rs, read this session]`.

### Routing order today (`handle_wm_key_event`, `src/input/mod.rs:662-719`, 58 lines of the 80 allowed)
1. `normalize_macos_option_key(raw_key, config.macos_option_as_alt)` (always on, any OS).
2. `Ctrl+Q` quit. 3. Repeat-suppression for toggle chords. 4. WM modals (`handle_wm_modal_key`: rename, utility windows, drawer focus, help/palette/menu/find). 5. Leader pending. 6. `handle_wm_user_binding_or_leader_entry` (user `[keybindings]`, leader entry, Alt+F-keys, font zoom/paste/palette/pane-resize always forwarded even in raw, Alt+Arrow focus). 7. Reserved Ctrl+Alt chords (Agents, AgentsList, SessionMap, Clipboard, Find, RemoteLink) win over raw passthrough. 8. Raw passthrough: everything else goes to `dispatch_pane_key(wm, raw_key)`. 9. `handle_wm_shortcut_key` (global table). 10. Enter submission, pane key.

### Chord families (counts from the tables)
| Family | Examples (verbatim from tables) | Notes |
|--------|--------------------------------|-------|
| Leader `Ctrl+X` (or `Ctrl+B` outside tmux) `[VERIFIED: keymap.rs:150-178]` | `"`/`-`/`h`/`F2` SplitHorizontal; `%`/`\|`/`\\`/`V` SplitVertical; `z`/`Z`/`F5` ZoomToggle; `o`/`n`/`Tab`/`F6` DeckNext; `1`..`9` FocusPane; `w`/`W` ClosePane; `d` DrawerToggle; `M` LayoutCycle; `m` PromptModeSet; `c`/`[`/`F8` CopyModeToggle; `Space`/`:` PaletteToggle; arrows/`j`/`k` PaneFocusDirection; `J`/`K` PaneMove; `<`/`>` PaneResize | Every lowercase letter a-z is already bound; only `* _ & $ ^ ( )` and Enter/Backspace/Home/End are free |
| Global `Alt+<printable>` `[VERIFIED: keymap_tables.rs DEFAULT_GLOBAL_BINDINGS]` | `Alt+h` SplitHorizontal, `Alt+v` SplitVertical, `Alt+w` ClosePane, `Alt+z` ZoomToggle, `Alt+m` LayoutCycle, `Alt+x` CrosshairCycle, `Alt+o` spotlight, `Alt+1`..`9` FocusPane, `Alt+j`/`Alt+k` focus, `Alt+Space` Palette, ``Alt+` `` Quake | 40+ rows |
| Pane `Alt+<printable>` `[VERIFIED: DEFAULT_PANE_BINDINGS]` | `Alt+t` PromptTop, `Alt+s` PromptSplitToggle, `Alt+c` CopyModeToggle, `Alt+r` SelectCommand, `Alt+l` LockToggle, `Alt+n`/`Alt+N` Search, `Alt+p` HistoryPopup, `Alt+/` SearchPromptPrefill | Shadow readline `M-t/c/l/r/n/p` in the Bilterm prompt |
| `Ctrl+Alt+<letter>` `[VERIFIED]` | `a` Agents, `n` AgentsList, `m` SessionMap, `v` Clipboard, `s` Voice, `b` Browser, `y` Video, `f` Find, `r` Reconnect, `w` Web, `p` DrawerPreview, `h/j/k/l` PaneMove, Arrows PaneResize | Reserved over raw passthrough |
| Ctrl | `Ctrl+P` Palette (forwarded even in raw), `Ctrl+Space`/`Ctrl+\\` Quake, `Ctrl+=`/`+`/`-`/`_`/`0`/`\x1f` font zoom (forwarded even in raw), `Ctrl+G` raw toggle (handled before any routing, `mod.rs:93-100`), `Ctrl+Q` quit | `Ctrl+_`/`\x1f` is readline/emacs undo |
| Function | F1 Help, F2 SplitH, F3 Hints, F4 PromptMode, F5 Zoom, F6 DeckNext, F7 Drawer, F8 CopyMode, F9 Inspector, F10 Quake, F11 VisualStyle, F12 RawToggle (`keymap.rs:bilterm_action_for_fkey`); `Alt+F1..F12` mirror them | `raw_fkeys` Auto/App/Bilterm modes gate interception in alt-screen |
| User overrides | `config.keybindings: BTreeMap<String,String>` chord string to `:command` `[VERIFIED: config/mod.rs:121]`; matched by `format_key_chord` (`keymap.rs:200`) | Grammar: `Ctrl`,`Alt`,`Shift` + Char/F/Tab/BackTab/Enter/Esc. **Arrows, Home/End, PageUp/Down, Insert/Delete cannot be bound** (returns `None`). No `Cmd`/`Super`. No validation or conflict check |
| Navigator drawer keys | `NAVIGATOR_DRAWER_BINDINGS`: `j/k/Enter/Space/e/E/C/N/a/A/n/c/+/w/r/R/x/Delete/PageUp/PageDown/{/}` | Already a scoped declarative table: the model to copy |
| Menu bar `[VERIFIED: ui/modals.rs:376-452]` | `MENUBAR_CATEGORIES: &[(&str, &[(&str,&str)])]`, label strings embed the key text, e.g. `("Stack Windows (Alt+M)", ":layout stack")` | 9 categories; keys handled in `modal_palette.rs:500-522` with hard-coded `% 7`; **no keyboard way to open** (`set_menubar_open_category` is only called from `mouse_wm.rs:192,197`) |
| macOS GUI `[VERIFIED: gui_macos.rs:209-239]` | Cmd+Q Quit, Cmd+C/X/A/V clipboard, Cmd+=/+/-/_/0 font zoom, all other Cmd chords dropped (`_ => None`); Option mapped to ALT | No SUPER in any `KeyModifiers` use: `grep KeyModifiers::SUPER src` returns nothing |
| Win32 GUI `[VERIFIED: gui_window.rs:58-117, 280-345]` | `GetKeyState` for Ctrl/Alt/Shift only; Win key not tracked; `translate_vk_to_key_event` returns chords whenever `ctrl \|\| alt` | AltGr arrives as Ctrl+Alt |
| X11 GUI `[VERIFIED: linux_x11.rs:585-660]` | Mod1 = Alt, Control, Shift only; Mod4/Super ignored | Same shape |
| TUI protocol | No kitty keyboard enhancement is ever pushed: `grep KeyboardEnhancement src` is empty `[VERIFIED]` | So in a TUI Ctrl+Shift+letter == Ctrl+letter and Cmd/Super never arrive |

### Duplicated key text (drift hazard)
The same chord strings are hand-written in `ui/modals.rs` (menu), `ui/help_catalog.rs:44-45,292` (`Alt+M`), `ui/help_catalog_data.rs:23,232`, `README.md:108`, `docs/README.md:79`, `docs/user-guide/{03,06,08,11}`, `docs/launch-presentation-and-pitch-script.md`, `docs/architecture/adr/ADR-003`. Every Alt+M mention must change together; the plan derives labels from the tables and adds a scan test.

### Alt+M specifics
`Alt+m`/`Alt+M` -> `Action::LayoutCycle` `[VERIFIED: keymap_tables.rs, global table]`; leader `M` -> `LayoutCycle`; `Ctrl+Alt+m` -> `SessionMap`. The menu item "Stack Windows" runs `:layout stack` -> `Action::LayoutMode(WorkspaceLayoutMode::Stacked)` `[VERIFIED: colon_command/parse.rs:215-217, action.rs:61]`, which is **not** what Alt+M does (cycle). Existing tests that press Alt+M: `tests/linux_fkeys_and_key_routing.rs:243,249`, `tests/terminal_torture_and_stress.rs:238`; they keep passing if Alt+M stays a legacy alias.

## Conflict Matrix

Legend: **B** = Block (chord is consumed by the OS/desktop/host and never reaches Bilterm; do not bind); **W** = Warn (reaches Bilterm but shadows a common consumer or breaks typing on common layouts); **ok**. Sources: `[CITED]` where noted, `[VERIFIED]` for repo facts, otherwise `[ASSUMED]` (training knowledge, needs user/maintainer confirmation; this is also the seed data for `keyconflict_data.rs`).

### A. Bilterm default chords vs hosts and consumers

| Bilterm chord (Action) | macOS | Windows | Linux (GNOME/KDE) | Nested apps / muxes / shells | Verdict |
|---|---|---|---|---|---|
| `Alt+M` (LayoutCycle) | W: Option+M types U+00B5 (code already special-cases `µ`, `keymap.rs` normalizer) `[VERIFIED]` | W: Claude Code uses `Alt+M` to cycle permission mode on Windows when VT input is off `[CITED: code.claude.com/docs/en/interactive-mode]` | ok (Alt+letter is menu mnemonic in GTK/Qt apps only) | emacs `M-m` back-to-indentation `[ASSUMED]` | **Replace** |
| `Alt+Space` (Palette) | W: Option+Space is NBSP (normalizer maps `\u{00a0}`) `[VERIFIED]` | B: opens system menu; Windows Terminal binds it to `openSystemMenu` `[CITED: learn.microsoft.com/windows/terminal actions]` | W: GNOME window menu, KDE KRunner alt `[ASSUMED]` | - | Demote to legacy |
| `Ctrl+P` (Palette, intercepted even in raw) | ok | ok | ok | W: readline previous-history, vim `<C-p>`, emacs `C-p`, fzf `[ASSUMED]` | Do not intercept in raw passthrough; use `Primary+P`/leader `Space` |
| `Ctrl+Space` (Quake) | W: commonly the input-source switch `[ASSUMED]` (a web search could not confirm the default) | W: Chinese IME toggle `[ASSUMED]` | W: IBus/fcitx IME toggle `[ASSUMED]` | W: emacs `C-SPC` set-mark; sends NUL | Demote to legacy |
| `Ctrl+\\` (Quake) | ok | ok | ok | W: SIGQUIT in cooked tty, vim `<C-\\><C-n>` `[ASSUMED]` | Keep as alias, warn |
| ``Alt+` `` (Quake) | W: Option+` is dead grave | ok | W: same-app window switch on GNOME/KDE `[ASSUMED]` | - | Demote |
| `F10` (Quake) | B/W: media/Mission Control key; docs already record F9-F12 interception `[VERIFIED: docs/LESSONS_LEARNED.md:42]` | W: activates window menu bar (WM_SYSKEYDOWN) | W: GNOME Terminal menu | mc/htop quit key | Keep, but never the only path |
| `F9`/`F11`/`F12` | W: Mission Control / show desktop / volume without Fn `[VERIFIED: LESSONS_LEARNED.md:42]` | F11 fullscreen in hosts | F11 fullscreen | - | Keep, never sole path |
| `Alt+F7`, `Alt+F12` (Drawer / FkeyMode) | ok | ok | B/W: Alt+F7 move-window, Alt+F8 resize, Alt+F2 run dialog on GNOME/KDE/XFCE `[ASSUMED]` | - | Add leader/Primary path; keep |
| `Alt+1..9` (FocusPane) | W: Option+digit types symbols (the normalizer maps `¡ ™ £ ¢ ∞ § ¶ • ª` back to Alt+1..9 `[VERIFIED: keymap.rs]`); on German/Nordic Mac layouts Option+5/6/7/8/9 type `[ ] \| { }` `[ASSUMED]` | W: AltGr digits | W: GNOME Terminal tab switch Alt+1..9 `[ASSUMED]` | tmux `M-1..M-5` select-layout `[ASSUMED]` | **Unbind on macOS**; Primary+1..9, Window mode digits |
| `Alt+t/c/l/r/n/p/y` (pane table) | W: compose chars | ok | ok | W: readline `M-t` transpose, `M-c` capitalize, `M-l` lowercase, `M-r` revert, `M-n/M-p` history search `[ASSUMED]`; `Alt+y` yank-pop is consistent | Window mode + leader own these; Alt kept on legacy-alt only |
| `Alt+h/v/w/z/x/o` + `Alt+Shift+W/H/D/O` | W: compose | ok | ok | `Alt+Shift+W` focus-up shadows `Alt+W` ClosePane when a terminal reports Alt+Shift+W without SHIFT (script: 4 chord collisions after Shift-folding: `Alt+o`, `Alt+h`, `Alt+w`, `Alt+d`) `[VERIFIED: ad-hoc parse of keymap_tables.rs + rows read]` | Remove focus aliases from non-legacy profiles |
| `Alt+Left/Right/Up/Down` (focus) | W: Option+Arrow is word navigation | ok; matches Windows Terminal `moveFocus` Alt+Arrows `[CITED]` | ok | Claude Code uses Alt+Up/Down to scroll a panel `[CITED]` | macOS: Cmd+Option+Arrows; elsewhere keep |
| `Alt+Shift+Arrows` (PaneResize) | W | parity with Windows Terminal `resizePane` Alt+Shift+Arrows `[CITED]` | ok | - | Keep non-mac |
| `Ctrl+Alt+<letter>` (Agents, Find, Web, Clipboard, ...) | W: Ctrl+Option is the VoiceOver modifier; Rectangle uses Ctrl+Option+Arrows `[ASSUMED]` | W: **AltGr = Ctrl+Alt** on many EU layouts (e.g. German `@` is AltGr+Q) `[ASSUMED]`; code treats it as a chord `[VERIFIED: gui_window.rs:326-345]` | W: GNOME `Ctrl+Alt+T/L/D/Arrows`, `Ctrl+Alt+F1-F7` VT switch `[ASSUMED]` | - | Keep as secondary only; leader and Primary are the documented paths |
| `Alt+Tab`/`Alt+BackTab` (DeckNext/Prev) | B (Cmd+Tab is the switcher) | B: OS task switcher | B: window switcher | - | Remove from labels; F6/Tab in modes |
| `Ctrl+X` leader | ok | W: cut in GUI apps | ok | W: readline `C-x` prefix (`C-x C-e` edit-in-editor), emacs `C-x`, nano exit, vim insert-completion, Claude Code `Ctrl+X Ctrl+K` `[CITED]`; double-tap already sends 0x18 to raw child `[VERIFIED: keymap.rs:122-133]` | Keep default; documented override |
| `Ctrl+B` leader (outside tmux) | ok | ok | ok | tmux and herdr default prefix is `Ctrl+B` `[CITED: herdr.dev agent guide]`; vim page-back | Existing `$TMUX` fallback is correct |
| `Ctrl+G` (raw toggle) | ok | ok | ok | **W: handled before raw routing for every pane (`mod.rs:93-100`), so emacs `C-g`, readline abort, nano help never reach the child** `[VERIFIED]` | Restrict to CommandPrompt mode; F12/leader `g` leave raw |
| `Ctrl+_` / `Ctrl+-` / `\x1f` (FontZoomOut, forwarded in raw) | ok | ok | ok | **W: readline/emacs undo is `C-_`** `[VERIFIED table rows + ASSUMED consumer]` | Drop `_` and `\x1f` from FontZoomOut |
| `Ctrl+Q` (Quit, global) | Cmd+Q is the OS quit | ok | GNOME Terminal Ctrl+Shift+Q | XON flow-control resume; vim `<C-q>` | Keep (Exit Guard protects) |
| `Ctrl+Shift+V`/`Shift+Insert` paste | ok | ok | ok | In a TUI, Ctrl+Shift+V is normally consumed by the host terminal as paste | Keep |
| `Ctrl+Shift+Arrows` (PaneMove) | W: n/a | W: text-selection by word in shells | W: GNOME Terminal tab/scroll chords | - | Keep non-mac |
| `Cmd+...` (mac GUI) | B for `Cmd+Space/Tab/H/M/Q`(OS), others free | - | - | - | Only Cmd+Q/C/X/V/A/+/-/0 bound today; add the Primary set |
| Win/Super key | - | B: never delivered (Win+Arrows, Win+1-9) | B: Super is compositor-owned | - | Never bind |

### B. Reserved chords of foreign consumers (seed for `keyconflict_data.rs`)

| Consumer | Chords that must not be silently stolen (severity Warn unless noted) | Source |
|---|---|---|
| readline / bash / zsh emacs mode | `Ctrl+A E B F P N K U W Y R S L D T H`, `Ctrl+_`, `Ctrl+X Ctrl+E/U/R`, `Alt+B F D T U L C R . Y < > Backspace` | `[ASSUMED]` |
| emacs -nw | `C-x` prefix, `M-x`, `C-g`, `C-SPC`, `C-_`, `M-m`, `C-s` | `[ASSUMED]` |
| vim / neovim | `Ctrl+W` window prefix, `Ctrl+B/F/D/U/E/Y` scroll, `Ctrl+P/N` complete, `Ctrl+V` block, `Ctrl+O`, `Ctrl+]`; Alt chords arrive as ESC-prefix and race with Esc (`ttimeout`) | `[ASSUMED]` |
| tmux | `Ctrl+B` prefix, `M-1..M-5` layouts, `M-n/M-p` alert windows, `M-o` rotate, `M-Arrow` resize | `[ASSUMED]` |
| GNU screen | `Ctrl+A` prefix | `[ASSUMED]` |
| zellij | `Ctrl+P/T/N/H/S/O/G` mode keys, `Alt+h/j/k/l/n/f/[/]/=/-` | `[ASSUMED]` |
| herdr | prefix `Ctrl+B` by default; configurable under `[keys]` in `~/.config/herdr/config.toml` (older docs show `ctrl+s`) | `[CITED: herdr.dev/agent-guide.md via web search]` |
| Claude Code | `Ctrl+C D L O R T B G J V X`, `Ctrl+X Ctrl+K`, `Shift+Tab`, `Alt+P` (model), `Alt+T` (thinking), `Alt+O` (fast mode), `Alt+B/F/D/Y` (word ops, need Option-as-Meta on macOS), `Alt+V` (paste image on Windows/WSL), `Alt+M` (Windows fallback) | `[CITED: code.claude.com/docs/en/interactive-mode]` |
| Windows Terminal (host of a TUI) | `Alt+Arrows` focus, `Alt+Shift+Arrows` resize, `Alt+Shift+-`/`+` split, `Ctrl+Shift+W` close pane, `Ctrl+Shift+P` palette, `Ctrl+Shift+F` find, `Ctrl+Shift+T/N/D/1-9`, `Alt+Enter` fullscreen, `Alt+Space` system menu, `Alt+F4` | `[CITED: learn.microsoft.com/windows/terminal/customize-settings/actions]` |
| macOS | `Cmd+Space/Tab/H/M/Q/W/N/T`, `Ctrl+Up` Mission Control, `Ctrl+Left/Right` Spaces (search-confirmed from third-party guides, not Apple's page) | `[CITED: support.apple.com keyboard shortcuts via web search; defaults not individually confirmed]` |
| GNOME/KDE/XFCE | `Alt+F1/F2/F4/F7/F8`, `Alt+Space`, ``Alt+` ``, `Ctrl+Alt+T/L/D/Arrows/F1-F7`, `Super+*` | `[ASSUMED]` |

### C. Key insight from the matrix
The conflict-free zone across all three OSes and all hosts is small: **plain keys inside a mode** (leader and Window mode need no modifier after entry), `Cmd` on macOS GUI, `Ctrl+Shift` on Windows/Linux GUI, and a few F-keys with Fn caveats. `Alt+<printable>` and `Ctrl+Alt+<letter>` are the least portable families and are exactly what Bilterm leans on today.

## Recommendation

### R1. Environment and the `Primary` modifier (`src/input/keyenv.rs`)

```rust
pub enum HostOs { MacOs, Windows, Linux }       // detect(): cfg!(target_os) at startup, injectable in tests
pub enum Surface { Tui, Gui }
pub struct KeyEnv { pub os: HostOs, pub surface: Surface, pub kitty_kb: bool, pub inside_tmux: bool }

/// None => Primary chords are unavailable on this env (legacy TUI): leader / Window mode / F-keys only.
pub fn primary_mods(env: &KeyEnv) -> Option<KeyModifiers> {
    match (env.surface, env.os, env.kitty_kb) {
        (Surface::Gui, HostOs::MacOs, _)        => Some(KeyModifiers::SUPER),
        (Surface::Gui, _, _)                    => Some(KeyModifiers::CONTROL | KeyModifiers::SHIFT),
        (Surface::Tui, _, true)                 => Some(KeyModifiers::CONTROL | KeyModifiers::SHIFT), // never SUPER: host terminal owns Cmd
        (Surface::Tui, _, false)                => None,
    }
}
```
Rule baked into table tests: **Shift is part of Primary on Windows/Linux, so no table row may be `Primary+Shift+x`**; letters must differ.

In the TUI, push `KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES` only when `supports_keyboard_enhancement()` is `Ok(true)` and config `keys.kitty_protocol != off`; pop in `TerminalRestoreGuard`. This is the only way a TUI can see Ctrl+Shift+letter or SUPER `[CITED: crossterm PushKeyboardEnhancementFlags docs; SUPER visibility under DISAMBIGUATE per search, crossterm-specific note unverified]`. Ship `auto` with an `off` escape hatch; risk is regressions in how modified keys are re-encoded for children (see Pitfall 6).

### R2. Default `Primary` table (every row lowers to an existing or newly frozen `Action`)

| Primary + | Action | Mnemonic / parity |
|---|---|---|
| `Z` | `ZoomToggle` | leader `z`, tmux `z` |
| `W` | `ClosePane` | universal close; Exit Guard still applies |
| `D` | `SplitVertical` (side by side) | iTerm `Cmd+D` |
| `B` | `SplitHorizontal` (stacked) | "below" |
| `L` | `LayoutCycle` | "Layout"; **replaces Alt+M** |
| `O` | `WindowMode(Enter)` | "Operate on windows" |
| `P` | `PaletteToggle` | VS Code / Windows Terminal palette |
| `]` / `[` | `DeckNext` / `DeckPrev` | iTerm pane next/prev |
| `1`..`9` | `FocusPane(n)` | iTerm tab select, Windows Terminal profile digits |
| `;` | `MenuOpen` | "menu" without colliding with F10 |

Deliberately **not** Primary: `Cmd+H/M/Space/Tab/Q` (OS reserved), `Ctrl+Shift+U/E` (IBus unicode/emoji `[ASSUMED]`), `C/V/X/A` (clipboard; the copy-mode entry therefore cannot be a Primary chord, see R6), `+ - 0` (font zoom).
X11/Win32 deliver Shift+digit/bracket as `! @ # ... { }`; the runners must map back to the base key via keycode/VK so `Ctrl+Shift+1` is `Primary+1` (Pitfall 3).

### R3. Window mode (sticky modal), `src/wm/key_mode.rs` + `src/input/window_mode*.rs`

Enter: leader `w` (all surfaces), `Primary+O` (GUI / kitty TUI), `:wmode`, palette, menu `Window -> Window Mode`. `Esc`, `Enter` or `q` exit. Bound over raw passthrough (it is a WM modal like leader; nothing reaches the child while active). Table model copies `NAVIGATOR_DRAWER_BINDINGS`: `(WinScope, KeyModifiers, KeyCode, Action)`.

| Scope | Keys | Action |
|---|---|---|
| Focus (default) | `h j k l` / arrows | `PaneFocusDirection` |
| | `1`..`9` | `FocusPane(n)` |
| | `Tab` / `n`, `BackTab` / `p` | `DeckNext` / `DeckPrev` |
| | `z` | `ZoomToggle` |
| | `s` / `t` | `LayoutMode(Stacked)` / `LayoutMode(Tiled)` (explicit "Stack windows" key) |
| | `Space` | `LayoutCycle` |
| | `-` or `"` / `\|` or `%` | `SplitHorizontal` / `SplitVertical` |
| | `x` | `ClosePane` |
| | `d` / `D` | `DrawerToggle` / `DrawerFocus` |
| | `[` or `c` | `CopyModeToggle` (exits Window mode first) |
| | `a`, `/`, `:` | `AgentsList(Toggle)`, `Find(Open{..})`, `PaletteToggle` |
| | `r`, `m` | switch sub-mode to Resize / Move |
| | `?` | pin full which-key panel |
| Resize sub-mode | `h j k l` / arrows | `PaneResize{..}` (step from existing leader `<`/`>`) |
| Move sub-mode | `h j k l` / arrows | `PaneMove{..}` |

Unknown keys are swallowed with a status hint (never forwarded), so a stray key cannot reach an agent. While a pane is zoomed, focus keys follow whatever Phase 61 defines for `apply(PaneFocusDirection)` (no separate Window-mode logic).

### R4. Which-key overlay
One generator `which_key_rows(&KeyMode, &KeyEnv) -> Vec<(String chord, String label)>` reads the live tables; the leader status line, the Window mode footer and a popup panel (shown after `keys.which_key_delay_ms`, default 400, or on `?`) all render from it, in TUI (ratatui) and GUI (existing card texture path). No hand-written hint strings remain (`keymap.rs:"⌨ LEADER ..."` string goes away).

### R5. Keyboard menu bar
`MENUBAR_CATEGORIES` becomes `MenuCategory { title, mnemonic, items: &[MenuItem { label, command, mnemonic }] }` (new `src/ui/menu_model.rs`; `modals.rs` is 830 lines). Open with leader `m` or `Primary+;`; category by mnemonic letter or Left/Right (count taken from the table, fixing `% 7`); items by mnemonic, Up/Down, Home/End, Enter. Accelerator text is **computed**: `hint_for_action(&Action, &KeyEnv) -> Option<String>` reverse-looks up tables preferring Primary chord > leader sequence > Window-mode sequence, formatted per env: macOS GUI `⌘L`, `⇧`, `⌥`, `⌃` glyphs; Windows/Linux and TUI `Ctrl+Shift+L`, `Ctrl+X M` (shown as `^X M` in compact bars). "Stack Windows" shows `Ctrl+X w s`; a new "Toggle Stack/Tile" item shows `⌘L` / `Ctrl+Shift+L`.

### R6. Alt+M replacement and demotions

| Capability | New primary path | Also | Alt+M fate |
|---|---|---|---|
| Stack windows | Window mode `s` | leader `M` (cycle), menu | - |
| Toggle tile/stack | `Primary+L` (Cmd+L / Ctrl+Shift+L) | leader `M`, `:layout cycle` | Hidden legacy alias on Windows/Linux (`legacy_alt=on`), **unbound on macOS** |

Profiles (`keys.profile = auto|mac|windows|linux|portable|legacy`):
- `mac`: Primary=Cmd (GUI). **No `Alt+<printable>` WM chords.** Only prompt-editing Alt keys remain as chords (b f d y Backspace Delete Left Right) so Option-as-Meta word navigation keeps working; every other Option+key is delivered as composed text. Focus aliases: `Cmd+Option+Arrows`.
- `windows` / `linux`: Primary=Ctrl+Shift; `Alt+<printable>` chords retained only when `legacy_alt=on` (default `on` for one release with a `:keys check` deprecation notice, then `off`); AltGr never forms a chord on Windows.
- `portable`: leader + Window mode + F-keys only (works identically in every terminal, no modifiers).
- `legacy`: today's tables bit-for-bit (migration safety, also used by existing tests).
Headless tests default to `legacy` so the 600+ existing key tests are unaffected; new tests set the profile explicitly.

Copy-mode entry contract for **Phase 62**: the only entry action is `Action::CopyModeToggle`; its default keys are leader `[`, `F8`, Window-mode `[`/`c` (all profiles) and `Alt+C` on `legacy`/non-mac `legacy_alt`. Phase 62 must read keys through the profile (`keys_for_action(CopyModeToggle)`), not hard-code chords, and must not claim `Primary+C/X/V` (clipboard). `Alt+C` inside copy mode currently exits it (`modal.rs:75-79`); Phase 62 should add `Esc`/`q` and may drop that.

### R7. Config and overrides
```toml
[keys]
profile = "auto"            # auto|mac|windows|linux|portable|legacy
kitty_protocol = "auto"     # auto|off
legacy_alt = "auto"         # auto|on|off
which_key = true
which_key_delay_ms = 400
macos_option_as_alt = true  # existing field, now honoured only for macOS TUI
[keys.bind]                 # chord = ":command" (same semantics as today's [keybindings])
"Primary+L" = ":layout cycle"
[keys.window_mode]          # key = ":command" inside Window mode
"g" = ":agents list toggle"
[keys.unbind]
chords = ["Alt+V"]
```
The existing `[keybindings]` table and `leader_key` stay valid (aliases for `keys.bind` and `keys.leader`). The chord grammar gains `Primary`, `Cmd`/`Super`, `Option`=`Alt`, named keys (`Left Right Up Down Home End PageUp PageDown Insert Delete Space Enter Tab Esc F1..F24`). `format_key_chord` delegates to the new `KeyChord` so `is_wm_toggle_chord` and user bindings share one implementation.

### R8. Conflict detection (`src/input/keyconflict*.rs`)
`check(effective: &EffectiveKeymap, env: &KeyEnv, cfg: &KeysConfig) -> Vec<Finding { chord, action, severity, class, consumer, note }>` with three classes: **internal** (two actions on one chord after Shift-folding, a chord unreachable because an earlier router branch shadows it), **host** (Block list per `HostOs`: `Cmd+Space/Tab/H/M/Q`, `Alt+F4`, `Alt+Tab`, `Ctrl+Alt+Del`, any Super/Win chord), **consumer** (Warn list from matrix B, filtered by env). Runs (1) at config load in `main.rs` after `BiltermConfig::load` (status message "N key conflicts: :keys check"; Block-class user bindings are rejected with an error), (2) on `:keys check`, `keys.check` RPC, `bilterm_keys` MCP, and (3) as a unit test over every shipped profile (shipped defaults must have zero Block findings and only allow-listed Warns with a reason).

### Alternatives considered (and why not)
| Alternative | Why rejected |
|---|---|
| Rebind Alt+M to another Alt letter | Every Alt+letter has a compose or consumer conflict on at least one OS |
| Single global `Ctrl+Shift` scheme incl. TUI | Indistinguishable from Ctrl+letter on legacy terminals; host terminals consume most of them |
| `Ctrl+Space` / `Alt+Space` as window-mode entry | IME/input-source switching, system menu, KRunner |
| `Ctrl+W` window prefix (vim/Helix style) | Readline kill-word; browsers/WT close-tab; shell users would lose it |
| Make `Esc` enter Window mode when prompt is empty | Esc is forwarded to children in raw mode; high accident risk |
| Zellij-style multiple always-on mode keys (`Ctrl+P/T/N/S/O`) | Each steals a readline key |
| Remove the leader entirely in favour of Primary | Primary does not exist in a legacy TUI |

## Architecture Patterns

### System Architecture Diagram
```
 OS key event --> [GUI runner: gui_macos / gui_window / linux_x11]      [TUI: crossterm Event::Key]
        |  (compose-aware: bound Alt chord => KeyEvent, else text;      |  (kitty flags pushed when supported;
        |   AltGr => text; Shift-digit => base key; Cmd => SUPER)       |   option normalizer ONLY on macOS TUI)
        v                                                               v
                       KeyChord::from_event(KeyEvent, &KeyEnv)
                                   |
                     handle_wm_key_event (<=80 lines)
                                   |
   Ctrl+Q -> modals -> KeyMode::{Leader, Window(sub)} --table--> Action   <-- which_key_rows(table) --> overlay/status
                                   |-- user [keys.bind] (chord -> :command -> Action)
                                   |-- profile tables: PRIMARY / GLOBAL / PANE (+legacy_alt layer)
                                   |-- raw passthrough => child (except reserved)
                                   v
                          WindowManager::apply(Action)
 Menu bar (mnemonics) --lowers--> Action ;  hint_for_action(Action, env) <-- reverse lookup of the same tables
 config load / :keys check / keys.check RPC / bilterm_keys MCP --> keyconflict::check(tables, env)
```

### Recommended Project Structure
```
src/input/
  chord.rs               # KeyChord parse/format/from_event, Primary token  (new)
  keyenv.rs              # HostOs, Surface, KeyEnv, primary_mods             (new)
  keyprofile.rs          # KeyProfile selection, effective table assembly    (new)
  keyprofile_tables.rs   # PRIMARY_BINDINGS, per-profile deltas              (new)
  keymap.rs / keymap_tables.rs   # existing; legacy tables retained
  window_mode.rs         # planner: key -> Action                            (new)
  window_mode_tables.rs  # WINDOW_MODE_BINDINGS                              (new)
  menu_keys.rs           # menu open / mnemonic navigation                   (new)
  keyconflict.rs, keyconflict_data.rs                                         (new)
src/wm/key_mode.rs       # KeyMode state (replaces leader_pending bool; keep accessors)
src/ui/menu_model.rs, which_key_ui.rs                                         (new)
src/config/keys.rs       # KeysConfig                                         (new)
```

### Anti-Patterns to Avoid
- **Hand-written shortcut text** in menus/help (the cause of the Alt+M drift): derive from tables.
- **Growing `handle_wm_key_event`**: add one 3-line call into `keymap::handle_wm_key_mode`; all logic in the new modules.
- **Branching on `cfg!(target_os)` deep in routing**: resolve `KeyEnv` once and inject it (tests need all three OS paths on one machine).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| Key hint strings | Per-surface literal strings | `hint_for_action` + `which_key_rows` | Single source; drift test |
| Which-key content | Static text | Generated from `WINDOW_MODE_BINDINGS` / leader table | Cannot drift |
| Modifier detection of Cmd in TUI | Heuristics on escape bytes | crossterm kitty flags + `KeyModifiers::SUPER` | Protocol-defined |
| Layout-independent keys | Per-layout lookup tables | X11 keycode / Win32 VK / Cocoa `charactersIgnoringModifiers` already used by runners | Existing runner pattern |
| Pane focus geometry | New nearest-neighbour code | `Action::PaneFocusDirection` / `FocusPane` | Already implemented |

## Runtime State Inventory
Not a rename/refactor phase, but one migration item: users' `config.toml` `[keybindings]`/`leader_key` stay valid; nothing stored on disk embeds old chord text except docs. Stored data: none. Live service config: none. OS-registered state: none. Secrets: none. Build artifacts: none.

## Common Pitfalls

### Pitfall 1: Composed text mistaken for chords (Linux/Windows i18n)
**What goes wrong:** `normalize_macos_option_key` rewrites `ø ß å ç µ π Ω ƒ` to `Alt+<letter>` on every OS and `handle_key_event` calls it with `true` unconditionally (`mod.rs:67`); in raw passthrough `dispatch_pane_key(wm, raw_key)` therefore sends `ESC o` to vim for a Danish `ø`. **Avoid:** thread `KeyEnv`; run only for `os==MacOs && surface==Tui && config`. **Detect:** unit test `normalize` is identity for Linux/Windows and a raw-passthrough test asserting `last_pty_write == "ø".as_bytes()` (field `last_pty_write` exists, `pane/mod.rs:94`).

### Pitfall 2: Option eats symbols on macOS GUI
`translate_macos_key_event` emits `Alt+<chars_ignoring_mods>` for any Option key (`gui_macos.rs:271-285`). Fix with an option policy: chord only for the prompt-editing set or if the Alt chord is bound in the active profile; otherwise deliver `chars`.

### Pitfall 3: Shifted symbols break `Primary+digit/bracket` on Windows/Linux
Ctrl+Shift+1 arrives as `!` (X11 keysym / `WM_CHAR`). Normalize by keycode/VK in the runners; table test with `('!', CTRL|SHIFT) -> Primary+1`.

### Pitfall 4: AltGr as Ctrl+Alt (Win32)
`GetKeyState(VK_CONTROL)` and `VK_MENU` are both down for AltGr. Treat right-Alt + left-Ctrl pairs (via `VK_RMENU`) as text; let `WM_CHAR` through. Pure helper `is_altgr(ctrl, lalt, ralt)` for tests.

### Pitfall 5: Window mode and modal ordering
`handle_wm_modal_key` runs before leader. Window mode must sit at the leader's position (after modals) but must not swallow `Ctrl+Q`; utility-window `captures_input()` branches must treat Window mode as pending (same as `!wm.is_leader_pending()` guards, `modal.rs:340-380`). Migrate with `is_leader_pending()` kept as an accessor so those guards keep compiling.

### Pitfall 6: Kitty protocol side effects
Once the outer terminal reports CSI-u, `Tab`/`Ctrl+I`, `Enter`/`Ctrl+M`, `Esc` and Alt+key arrive differently. `encode_key_to_pty_bytes_full` must still produce legacy bytes for children that did not push kitty flags. Add table-driven encode tests for Alt+letter, Ctrl+letter, Tab, Enter, Esc, Backspace with SUPER/ALT/CONTROL combos before enabling `auto`.

### Pitfall 7: Docs and help drift
Nine docs mention Alt+M. A scan test fails if `HELP_LINES`, `PALETTE_CATALOG`, menu hints or user docs mention a chord that is not in the effective tables (legacy section excepted by explicit marker).

### Pitfall 8: Menu bar `% 7`
Right/Left wrap uses literal 7 with 9 categories (`modal_palette.rs:505-510`), so Daemon and Web are mouse-only. Use the table length; regression test visits all categories by key.

## Code Examples

```rust
// Chord lookup with env (sketch; Source: design for this phase, based on src/input/keymap.rs:default_global_action)
pub fn primary_action(key: KeyEvent, env: &KeyEnv) -> Option<Action> {
    let want = primary_mods(env)?;
    let base = base_key(key)?;                    // undo Shift-digit/bracket symbols
    if key.modifiers.difference(KeyModifiers::SHIFT) != want.difference(KeyModifiers::SHIFT)
        || !key.modifiers.contains(want) { return None; }
    PRIMARY_BINDINGS.iter().find(|(c, _)| *c == base).map(|(_, a)| a.clone())
}
```
```rust
// Window mode planner follows agents_list_keys.rs / drawer_navigator.rs: table + scope fallthrough
pub fn window_mode_action(scope: WinScope, key: KeyEvent) -> Option<Action> {
    WINDOW_MODE_BINDINGS.iter()
        .find(|(s, m, c, _)| *s == scope && *m == fold_shift(key) && *c == key.code)
        .map(|(_, _, _, a)| a.clone())
}
```

## State of the Art

| Old approach | Current approach | Impact |
|---|---|---|
| Legacy xterm modifier ambiguity | Kitty keyboard protocol (`CSI u`), supported by kitty, WezTerm (opt-in), foot, Ghostty, iTerm2 recent, Alacritty, crossterm | Lets a TUI see Ctrl+Shift and Super; must be probed, not assumed `[CITED: sw search results; per-terminal support ASSUMED]` |
| Option-as-Meta everywhere | Per-terminal Option-as-Meta setting; Claude Code documents it as a prerequisite for Alt shortcuts on macOS `[CITED]` | Bilterm cannot rely on Alt chords being delivered |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Changing leader `w` from ClosePane to Window mode is acceptable (ClosePane stays on `W`, Window-mode `x`, `Primary+W`, legacy profile) | R3 | Existing muscle memory breaks; alternative entry would be `Leader Space` |
| A2 | `legacy_alt` defaults to on for Windows/Linux for one release then flips to off | R6 | Users lose Alt chords sooner/later than wanted |
| A3 | Primary letter set (Z W D B L O P [ ] 1-9 ;) has no hard conflict inside Bilterm's own GUIs; third-party notes (IBus Ctrl+Shift+E/U, GNOME Terminal) are `[ASSUMED]` | R2 | Some Ctrl+Shift chord intercepted by IME/desktop |
| A4 | macOS defaults (Ctrl+Space input source, Ctrl+Arrows, Ctrl+Option VoiceOver) | Matrix | Only affect warnings, not behaviour |
| A5 | GNOME/KDE/XFCE Alt+F7/F8/Space/` and Ctrl+Alt chords | Matrix | Same |
| A6 | readline/emacs/vim/tmux/zellij reserved chords from memory | Matrix B | `keyconflict_data.rs` seeds are wrong or incomplete |
| A7 | German/Nordic Mac layouts type `[ ] \| { } @ ~` with Option+5/6/7/8/9/L/N | Pitfall 2 | Weaker justification (the compose-key argument stands regardless) |
| A8 | `Ctrl+G` raw toggle should be limited to CommandPrompt mode; `Ctrl+_` removed from FontZoomOut | Matrix | Existing tests/users rely on them (tests to be updated) |
| A9 | Kitty `auto` default is acceptable for the TUI | R1 | Regression risk in key re-encoding; mitigated by `off` and encode tests |
| A10 | Menu/mnemonic letters (S W P V T X H D E) | R5 | Cosmetic; uniqueness enforced by test |

## Open Questions

1. **Is the `Leader w` re-purpose (A1) acceptable?** Recommendation: yes, keep `W` for close and ship a one-line migration hint in the status bar the first time `Leader w` is used.
2. **Should Window mode have a direct modifier-free entry for legacy TUIs?** Recommendation: no (leader `w` is two keystrokes); revisit only if user testing shows friction.
3. **What does Phase 61 define for focus-while-zoomed?** Window mode delegates to `apply(PaneFocusDirection)`; confirm when 61 lands.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|---|---|---|---|---|
| Rust toolchain / cargo | build, tests | assumed (repo builds) | - | - |
| macOS / Windows hosts | GUI runner verification | no (Linux dev host) | - | Pure-function tests via injected `KeyEnv`; GUI translators are tested through their pure functions (`translate_macos_key_event`, `translate_vk_to_key_event`, `translate_x11_keysym_and_bytes`) already `#[cfg]`-free |
| X11/Xvfb | Linux GUI tests | use `scripts/verify.sh --xvfb` | - | skip GUI-only tests |

Missing with no fallback: none. Real-device validation on macOS and Windows remains a manual UAT item.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (unit + `tests/*.rs`), insta snapshots |
| Config file | `Cargo.toml`; `scripts/verify.sh` |
| Quick run command | `scripts/verify.sh --quick` |
| Full suite command | `scripts/verify.sh` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| KEY-02 | Primary resolves per env; profiles have no mac Alt+printable; normalizer gated; AltGr/Option compose | unit | `scripts/verify.sh --only phase63_key_profiles` | Wave 0 (63-01) |
| KEY-03 | Alt+M absent from mac profile and labels; layout reachable in every profile | integration | same | Wave 0 (63-01) |
| KEY-01 | Window mode matrix, sticky behavior, raw passthrough isolation, menu keyboard access, which-key snapshot | integration | `scripts/verify.sh --only phase63_window_mode_and_menu` | Wave 0 (63-02) |
| KEY-02 | Conflict engine, load-time check, `:keys` / RPC / MCP / palette / menu parity | integration | `scripts/verify.sh --only phase63_keys_surface_parity` | Wave 0 (63-03) |

### Sampling Rate
- **Per task commit:** `scripts/verify.sh --quick`
- **Per wave merge:** `scripts/verify.sh`
- **Phase gate:** full suite green; `ARCH-R01` (<=80 lines) and `ARCH-R04` (<=1200 hard) gates in `tests/session_lessons_and_regression_suite.rs` pass.

### Wave 0 Gaps
- [ ] `tests/phase63_key_profiles.rs`, `tests/phase63_window_mode_and_menu.rs`, `tests/phase63_keys_surface_parity.rs`
- [ ] Headless helper to set `KeyEnv` on a `WindowManager` (`wm.set_key_env`) so tests cover all three OS paths.

## Security Domain

`security_enforcement` is not disabled in `.planning/config.json`.

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2/V3 Authentication, Session | no | - |
| V4 Access Control | yes | `keys.*` RPC/MCP mutating calls (`keys.profile`) go through the existing write gate; read-only `keys.check/list` ungated like `agents.list` |
| V5 Input Validation | yes | Chord grammar parser rejects unknown tokens, caps length (64) and table size (`[keys.bind]` <= 512 entries); `[keys.bind]` values must parse via `parse_colon_command` or are stored as prompt prefill exactly as today |
| V6 Cryptography | no | - |
| V12 Files | yes | Config writes via `src/secure_fs.rs` (existing path); no new files |

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Binding that silently disables Exit Guard or quit | Tampering | Block-class check refuses user binds that shadow `Ctrl+Q`; `ClosePane` keeps Exit Guard |
| Key injection via RPC `keys.window_mode` into locked/read-only panes | Elevation | Window mode actions are `Action`s through `apply`, inheriting lock/read-only checks |
| Oversized `[keys]` config causing memory growth | DoS | Entry-count and chord-length caps (bounding-untrusted-resource-use) |
| Which-key overlay leaking secrets | Info disclosure | Overlay shows only static labels from tables, never pane content |

## Sources

### Primary (HIGH)
- Repo files read this session: `src/input/{keymap,keymap_tables,mod,modal,modal_palette,gui_route,encode}.rs`, `src/ui/{gui_macos,gui_window,modals,help_catalog,help_catalog_data}.rs`, `src/ui/gui_font/linux_x11.rs`, `src/config/mod.rs`, `src/action.rs`, `src/colon_command/parse.rs`, `src/wm/mod.rs`, `AGENTS.md`, `docs/DEVELOPER_AND_AGENT_GUIDE.md`, `docs/LESSONS_LEARNED.md`, `.planning/REQUIREMENTS.md`, `.planning/phases/57-*/57-01-PLAN.md`.

### Secondary (MEDIUM, cited)
- https://code.claude.com/docs/en/interactive-mode (Claude Code shortcuts)
- https://learn.microsoft.com/en-us/windows/terminal/customize-settings/actions (Windows Terminal defaults)
- https://herdr.dev/agent-guide.md and search summaries (herdr prefix `ctrl+b`)
- crossterm kitty keyboard docs via web search (`PushKeyboardEnhancementFlags`, `supports_keyboard_enhancement`)
- Apple macOS keyboard shortcuts page (search summary; defaults not individually confirmed)

### Tertiary (LOW)
- Training-knowledge rows tagged `[ASSUMED]` (GNOME/KDE, readline, vim, emacs, tmux, zellij, IME).

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH (no new dependencies)
- Architecture: HIGH (mirrors existing agents-list/navigator table + planner pattern)
- Pitfalls: HIGH for bugs read in code; MEDIUM for platform behavior
- Conflict matrix third-party columns: MEDIUM/LOW (see Assumptions Log)

**Research date:** 2026-11-08
**Valid until:** 30 days (OS/terminal defaults drift slowly; Claude Code/herdr shortcuts drift fast: re-check at execution)
