# Phase 51 - Research A: colon verbs vs command palette (UIT-02)

**Researched:** 2026-11-05 | **Method:** static read of parser/palette/completion sources + a Python scan (scratchpad `inv.py`); no Rust was compiled, so every "gap" below is a static-analysis result. The new test (section 5) will print the authoritative list on first run.
**Confidence:** HIGH on structure/mechanism (read the code), MEDIUM on exact gap counts (~100-130 probes; regex scan, not the real parser).

## 1. How the palette is built (facts)

- `ColonCommand` is only a type alias: `pub type ColonCommand = crate::action::Action;` (`src/colon_command/mod.rs:15`). `Action` derives `Clone, Debug, PartialEq, Eq` (`src/action.rs:14`), so parsed commands can be compared for equality. This is the key to a semantic coverage test.
- Parser entry: `parse_colon_command` (`src/colon_command/parse.rs:8`) tries, in order, `parse_config` (get/set/config/state/appearance save), `parse_agents` (head `agents`, `parse_agents.rs:182`), `parse_visual` (`parse_visual.rs:106`), then the big `match head` in `parse.rs:24-719`. Drawer sub-verbs: `parse_drawer_nav` (`parse_visual.rs:641-707`) + `parse_drawer_command` (`:710`).
- Palette = a **hand-written static table** `PALETTE_CATALOG: &[(&str cmd, &str shortcut, &str desc)]` at `src/ui/help_catalog.rs:220-491` (268 entries, 90 distinct heads), re-exported by `src/ui/modals.rs:5`. There is no generation: an entry exists only if someone typed it.
- Filtering: `WindowManager::filtered_palette_items` (`src/wm/session.rs:87-114`) runs nucleo over `"{cmd} {shortcut} {desc}"`; empty query returns the whole table. So every new entry needs a description with the words a user would type.
- Enter handling (`src/input/modal.rs:886-915`): selected entry text -> `parse_colon_command` -> `execute_wm_colon_command`. **Entries ending in a space (`:ssh `, `:rename `, `:get `) are NOT "fill in the prompt"; Enter runs the bare verb, and a parse `Err` is silently dropped** (`let Some(Ok(cmd))`, `modal.rs:908-910`). Consequence: a fill-in entry for a free-form verb does nothing useful today (`:rename ` -> usage error swallowed; `:ssh ` -> `RemoteList`). Optional UX fix in scope of "palette entry triggers it": on Enter, if the entry ends with a space or parse is `Err`, close the palette and prefill the prompt (precedent: `Action::SearchPromptPrefill`, `:search prefill`).
- Existing drift guards (all in-tree, all pass today):
  - `src/command_registry.rs:150-198`: every completion name has a summary; aliases point at real commands; every palette head is in `COLON_COMMANDS`; every completion name parses. Direction is table->palette-head only, never palette *coverage*.
  - `src/ui/help_catalog.rs:500-511` `every_palette_entry_with_complete_args_parses_ok` (skips entries ending ' ' or containing `<`/`[`).
  - `src/action.rs` `assert_action_reachable` (exhaustive `match action` giving each variant a colon example, `action.rs:~275-420`; asserts the example's head is in `COLON_COMMANDS`). Adding an `Action` variant is therefore a compile error until a colon head is named - and that head must be in the completion table.
- Single enumerable verb source already available at runtime: `colon_command_table()` (`src/pane/completion.rs:21`, table `:54-~540`, 175 names, many with fixed sub-arg lists) and `command_registry::catalog()`.

## 2. Inventory (92 parser arm-groups; grouped)

Legend: P = palette entries for that head family (alias spellings counted), L = what it lowers to. "eq-covered" = a different spelling in the palette parses to the identical `Action`.

| Verb (aliases) | Lowers to | Palette | Gap / disposition |
|---|---|---|---|
| prompt | PromptTop/Bottom/SplitTop/SplitBottom/SplitToggle/ModeSet/Float/Height/Width/MoveVertical/Resize* | 7 | ADD `:prompt split-bottom`; `fixed/floating/terminal/mode` eq-covered by `:mode`; EXCLUDE `float N`,`height N`,`width N` (numeric) |
| mode | PromptModeSet, Mode(display) | 7 | complete |
| panel / chrome | PanelScale(n), ChromeTitle, ChromeRing | 1 / 2 | ADD `:chrome title all`,`title none`,`title auto`,`ring off`; `panel scale N` has representative 125 |
| help(h,?) / palette(commands,cmd) / quit / quit!(q!,exit!) / detach(d) / attach(a,resume) / respawn-pane / exit-guard | HelpToggle / PaletteToggle / Quit / ForceQuit / DetachClient / SessionRestore / RespawnPaneInPlace / ExitProtectionSet | all present | complete |
| history(hist) | HistoryPopupToggle/Save/Toggle/Runs/MaxFiles | 2 | ADD `:history save`,`:history on`,`:history off`; EXCLUDE `max-files N` |
| block | BlockPrev/Next | 2 | complete |
| split(sp) / vsplit(vs) | SplitHorizontal/Vertical | 2 | complete (`horizontal`/`vertical`/`vsplit` eq-covered) |
| deck | DeckNext/Prev | 1 | ADD `:deck prev` |
| **focus(raise)** | FocusPane(id) / PaneFocusDirection / DeckNext/Prev | **0** | ADD `:focus left|right|up|down|next|prev` (6); EXCLUDE `:focus <id>` (free pane id) |
| zoom / close / quake(guake) / cascade(equalize) / more / paste(p) | ZoomToggle / ClosePane / QuakeToggle / WindowResetGeometry / ScrollPageAdvance / PasteClipboard | present | complete |
| **arrange(auto-arrange)** | LayoutProfileCommand("arrange <pat>") (`parse.rs:180`; patterns `layout_profile.rs:869-890`) | **0** | ADD `:arrange grid`,`columns`,`rows`,`main-side`,`main-top`,`stack-grid`,`float-grid` |
| layout | LayoutCycle / LayoutMode / LayoutProfileCommand | 4 | ADD `:layout autosave` exists; add `:layout save`? EXCLUDE (needs name); `list/next/prev/single/...` eq-covered via `:profile` family except `single` -> ADD `:profile switch single` |
| profile(profiles) / autosave | LayoutProfileCommand(str) | 11 / 1 | ADD `:autosave on`,`:autosave off`; EXCLUDE `profile delete <name>`,`switch <name>`,`save <name>`,`default <name>` (free name) |
| search(find) | SearchOff/Query/Next/Prev/PromptPrefill | 2 | ADD `:search next`,`:search prev`,`:search off`; EXCLUDE `:search <text>` (free text; `:search prefill` exists) |
| copy(vi) / rect(+5 aliases) / select(sel) | CopyModeToggle / SelectCommand(Option<String>) | 1 / 5 / 0 | `:select` bare eq-covered by `:rect`. ADD (`SelectCommand(Some(x))`): `:copy char`,`:copy line`,`:copy block`,`:copy rect`,`:copy yank`,`:copy off`,`:rect select`,`:rect prompt`,`:rect mode on|off` |
| links(url,urls) / hints / inspector(inspect) / lock / broadcast | LinksYank / HintsToggle / InspectorToggle(Option<bool>) / LockToggle / BroadcastToggle | bare present | ADD explicit `on`/`off` for inspector, lock, broadcast (6) |
| save / rename / session(+workspace,sessions) / new-session / rename-session | SessionSave/Restore/List / SessionTreeCommand(str) | 13 | `new-session` eq-covered by `:session new`. ADD `:session rename` (bare, opens prompt), `:session processes on|off`, `:session cmd clear`, `:session status`? (hidden alias, skip). EXCLUDE `switch/shell/cwd/reorder/delete/branch/graft/detach-subtree/cmd <x>/rename <a> <b>` (names/paths) |
| notifications(notify,bell) / trigger(s) | NotificationsClear / TriggerAdd / TriggerClear | 3 / 1 | ADD `:trigger clear`; EXCLUDE `trigger add <pattern>` (fill-in entry `:triggers add ` stays) |
| record(rec,asciinema) | RecordStart/WorkspaceStart/PaneStart/Stop | 3 | ADD `:record pane` |
| ssh / ssh-tmux(sshtmux) / remote(fleet,herdr) | SshConnect / SshTmuxConnect / Remote* | 1/1/2 | `ssh list` eq-covered by `:remote list`. EXCLUDE `ssh <host>`, `ssh-tmux <host>`, `remote add|exec|disconnect ...` (host/command args) |
| yank / clipboard(clip,cb,buffers,yank-history,yank-ring) | **Yank** / SelectCommand("clipboard ...") | **0** / 1 | ADD `:yank`; EXCLUDE `clipboard <sub>` (free sub) |
| **clear** | Clear | **0** | ADD `:clear` |
| **raw** | RawToggle | **0** | ADD `:raw` |
| scrollback(sb) / scroll(scrl,pager) | Scrollback{clear} / ScrollModeSet / TogglePage / PageAdvance / PageBack | 2 / 3 | ADD `:scroll` (bare cycle), `:scroll toggle`,`:scroll next`,`:scroll prev` |
| resize(winresize) / move(winmove) | PaneResize / PaneMove / WindowResetGeometry | 4 / 4 | `+w/-w/+h/-h/reset` eq-covered by arrows + `:cascade`; EXCLUDE numeric `resize right N` |
| mcp(master-control) | McpCommand(str) | 5 | EXCLUDE `attach/spawn/graft/detach-subtree/broadcast/freeze/thaw/derez <node>` (node ids); EXCLUDE `shutdown --confirm` (destructive, requires typed confirm) |
| browse(files,filebrowser,explorer) / itermcolors / theme | ThemeCommand(str) / Theme(preset) / WindowStyle | 1 / 1 / 6 | ADD 12 presets: `:theme turbo-cyan`,`ibm-mono`,`cyberpunk-neon`,`dracula`,`gruvbox-dark`,`nord`,`catppuccin-mocha`,`tokyo-night`,`solarized-dark`,`monokai-remastered`,`tron-grid`,`ibm-3278-green` (3278/3279 forms exist and are eq-covered), `:theme export`; EXCLUDE `theme window|global|import|browse <x>` (names/paths) |
| appearance | Appearance(profile) / AppearanceSave | 6 | complete |
| **glyphs** | Glyphs(pack) | **0** | ADD `:glyphs unicode|cp437|nerd-font|ascii` (4) |
| cursor | Cursor(CaretStyle) / ThemeCommand("cursor ...") | 7 | ADD `:cursor solid`,`:cursor blink`,`:cursor rapid`,`:cursor theme`; EXCLUDE colour names (8, combinatorial - representative combos exist), `custom <char>`, `window|global ...` |
| window(win,pane-style) | WindowStyle(str) / PaneFocusDirection / FocusPane | 14 | ADD `:window show`,`theme prev`,`outline double|single|rounded|thick|dotted|quadruple|ascii|reset`,`bold on|off|reset`; DECIDE: 13 `theme <preset>` entries vs EXCLUDE ("reachable via `:window theme list|next`"); EXCLUDE colour verbs (`fg bg prompt-bg prompt-color inactive-border title-color badge-color crosshair-color shadow-color`) - need colour arg |
| outline(border) | Outline{window,prompt} / WindowStyle("global-bold") | 1 | ADD `:outline solid|double|rounded|thick|dotted|dashed|quadruple|ascii` (8); EXCLUDE `outline window|prompt <s>` (2x8 combinatorial) |
| crosshair / focal | CrosshairToggle / CrosshairCycle / FocalRadiusResize | 2 / 2 | ADD `:crosshair on|off|pane` |
| spotlight(spot,focus-light) / focalzoom(fzoom,lens,magnify) | Spotlight{..} / FocalZoom{..} | 7 / 8 | ADD `:spotlight circle fuzzy`,`:spotlight full`,`:spotlight pane`; `:focalzoom 1.25x`,`3x`,`circle`,`ellipse`; EXCLUDE `size/feather N` (numeric) |
| **shadows(shadow)** | Shadows(Option<ShadowStyle>) | **0** | ADD `:shadows` (cycle) + `on|off|translucent|tinted|solid|none` (note: unknown arg silently becomes `Shadows(None)`, `parse_visual.rs:288-291`) |
| fkeybar / menubar(menu) / version(ver) / crt(scanlines) | FKeyBarToggle / MenuBarToggle / VersionToggle / CrtToggle (Option<bool>) | bare present | ADD explicit `on`/`off` for all four (8) |
| fkeys(fkey,function-keys) | FkeyModeCommand(Option<str>) | 1 | ADD `:fkeys bilterm|app|auto|status`; EXCLUDE `send <F1..F12>` (12 values; help line documents) |
| modern / liquid | VisualStyleCommand(Some("modern-toggle"...)) | 1 / 1 | ADD `on`/`off` each (4) |
| **snazzy(herdr-style)** / **glass(frosted,aero)** | VisualStyleCommand | **0 / 0** | ADD bare + `on`/`off` each (6) |
| **style(visual,visual-style)** | VisualStyleCommand(None or Some(tok)) | **0** | ADD `:style` (cycle),`:style classic|snazzy|glass|both|modern|liquid` (7) |
| fontsize(textsize) / font(panefont,winfont) | FontZoom* / FontSetSize / PaneFontCommand(str) | 3 / 11 | ADD `:font reset`,`:font status`,`:font tek4014`,`:font bold`,`:font italic`,`:font double`; EXCLUDE `fontsize 14pt|125%|150%` (numeric), `font size|scale|pane|all|custom <x>` |
| canvas / tek | CanvasCommand(str) | 9 | ADD `:tek on`,`:canvas status`; EXCLUDE `res backend line circle bezier sprite step plot image pan zoom export encode` (geometry/file args) |
| get / set / config / state | ConfigGet/Set/List/Save / StateQuery | present | `get`,`set` are fill-in entries (see Enter caveat); EXCLUDE as free-form |
| **drawer(sidebar,panels,navigator)** | DrawerToggle/Focus/Dock/Visibility/Layout/Size/Style/Rows/NavOp/PanelScaleOf | 27 | ADD `:drawer style navigator`,`:drawer style classic`,`:drawer rows 1`,`:drawer rows 2`,`:drawer focus`,`:drawer left|right|top|bottom`,`:drawer fixed|autohide|ondemand|unpin`,`:drawer minimize|maximize`,`:drawer menu close`,`:drawer mode`; EXCLUDE `size N`,`scale N` (numeric) |
| **agents** | `Action::Agents(AgentsOp)` | 29 | ADD `:agents demo`,`blur`,`topology`,`fade`,`select right|up|down`,`nudge left|up|down`,`edge prev`,`scroll top`,`follow on|off`,`grab on|off`,`focus graph|snoop|compose`,`pin toggle`; EXCLUDE `rect x y w h`,`node-pin key x y`,`topology export <file>`,`size WxH`,`filter text <kw>` ; Phase 51 adds `demo stop`, `list` window verbs - those plans must add their own entries |

Totals (scan): 14 of 92 parser arm-groups have no palette entry under any spelling; two are pure aliases already eq-covered (`vsplit` -> `:split v`, `new-session` -> `:session new`) and `select` is eq-covered by `:rect`. **Truly uncovered verbs (11):** `focus`, `arrange`, `rename-session`, `yank`, `clear`, `raw`, `glyphs`, `shadows`, `snazzy`, `glass`, `style`. Plus the sub-verb gaps above (the user's example `:drawer style navigator|classic` is one: `style ` is in `COLON_COMMANDS` `completion.rs:~495` but no palette entry has it; ditto `:drawer rows`).

## 3. Verbs reasonably EXCLUDED (reasons - use as the `reason` vocabulary)

| Reason code | Examples |
|---|---|
| `free-text` | `:search <text>`, `:set k v`, `:get k`, `:trigger add <pat>`, `:rename a b`, `:clipboard <sub>` |
| `host-or-path` | `:ssh <host>`, `:ssh-tmux <host>`, `:remote add/exec/disconnect`, `:theme import/browse <path>`, `:font custom <f>`, `:canvas export/encode/image`, `:agents topology export <file>` |
| `entity-id` | `:focus <id>`, `:mcp attach/spawn/graft/freeze/thaw/derez <node>`, `:session switch/delete/branch <name>`, `:profile delete/switch/save <name>` |
| `numeric` | `:fontsize 14pt|125%`, `:prompt float/height/width N`, `:spotlight size/feather N`, `:drawer size/scale N`, `:agents rect/node-pin/size` , `:history max-files N` |
| `combinatorial` | `:window <10 colour verbs> <colour>`, `:cursor <8 colours>`, `:outline window|prompt <style>` (global form is in the palette) |
| `destructive` | `:mcp shutdown --confirm`, `:mcp derez` (typed confirmation is the safety) |
| `reachable-via-picker` (decision) | `:window theme <preset>` x13 (use `:window theme list|next`) |

Also needing a decision: the 12 parser heads absent from `COLON_COMMANDS` (`?`, `a`, `arrange`, `auto-arrange`, `cb`, `d`, `h`, `p`, `sb`, `sp`, `vi`, `vs`). Short aliases (`? a cb d h p sb sp vi vs`) -> exclude as `short-alias`; **`arrange`/`auto-arrange` should be ADDED to `COLON_COMMANDS`** (real verb with fixed pattern set, missing from Tab completion and from the registry). Likewise drawer sub-verbs the parser accepts but the table omits: `minimize`, `maximize`, `unpin`, `pin`, `scale `, `mode `, `edge ` (`parse_visual.rs:716-775`).

## 4. Recommended data-driven mechanism

Use the **completion table as the single enumeration source** (it already has a parser-drift test, `action.rs: every_completion_entry_is_a_command_the_parser_recognises`) and compare by **parsed `Action` equality**, not by text:

1. **Probes** = every `COLON_COMMANDS` row: bare head, plus `"{head} {sub}"` for each sub (`sub` ending in a space = "needs an argument").
2. **Covered** iff
   - complete probe: `parse_colon_command(probe)` is `Some(Ok(a))` and some `PALETTE_CATALOG` entry parses to the same `a` (this automatically treats aliases, `:lens 2x` == `:focalzoom 2x`, `:select` == `:rect`, `:ssh list` == `:remote list` as covered, so the palette need not duplicate spellings), or
   - needs-arg probe (or bare head that parses `Err`): some palette entry text starts with the probe (a representative/fill-in entry), or
   - probe is in `PALETTE_EXCLUSIONS`.
3. **Exclusions** = `pub(crate) const PALETTE_EXCLUSIONS: &[(&str probe, &str reason)]` next to the existing `EXTRA` in `src/command_registry.rs` (that module is already "the single read-only view over every colon command"). Reason must come from the vocabulary in section 3. Expose it on `CommandInfo` (`palette_exclusion: Option<&'static str>`) so `:commands`/MCP resources can show it (optional).
4. **Anti-rot guards**: (a) every exclusion probe must be a real probe (catches typos/renames); (b) an excluded probe must NOT also be covered (forces deleting stale exclusions); (c) reason non-empty.
5. **Alias-noise control**: probe sub-args only for heads whose `EXTRA` `alias_of` is `None`; probe the bare head for all heads. Prerequisite clean-up: several `EXTRA` canonicals do not match the parser's arm groups (e.g. `:pane-style` is its own canonical but shares an arm with `:window`; `:q!`/`:exit!` vs palette `:quit!`; `:url` -> `:links` while the palette uses `:urls`; `:rect-select`, `:select`, `:new-session`, `:rename-session`, `:snazzy`, `:glass`, `:style` have no palette head). Point `:pane-style`->`:window`, `:rect-select`->`:rect`, `:exit!`/`:q!`->`:quit!`. With Action-equality on bare heads this mostly self-heals, but without it `:pane-style theme ` etc. would demand duplicate exclusions.
6. **Make fixed sets enumerable**: where the table says `"style "` / `"rows "` but the set is fixed, replace with the concrete values (`"style navigator"`, `"style classic"`, `"rows 1"`, `"rows 2"`) - also improves Tab completion - so the test sees each value. Add the omitted drawer sub-verbs and `arrange`/`auto-arrange` to the table.
7. **Parser -> table direction (optional tier-2)**: no runtime list of parser heads exists (arms are hand-written; the registry doc says the parser stays hand-written for Legacy parity). A cheap guard is a test that `include_str!`s `parse.rs`, `parse_visual.rs`, `parse_config.rs` and scans lines matching `^        ("x" | "y")+ (if ..)? =>` for head literals, asserting each is in `colon_command_names()` or a small `PARSER_ONLY_ALIASES` list. It is formatting-dependent; mark it as best-effort. Variant-level coverage is already compile-enforced by the exhaustive match in `action.rs` (new `Action` variant -> needs colon example -> head must be in `COLON_COMMANDS` -> mechanism above forces a palette entry/exclusion).

### Test sketch (drop into `src/command_registry.rs` tests; uses only existing pub items)

```rust
fn probes() -> Vec<(String, bool)> {
    let mut v = Vec::new();
    for &(head, subs) in colon_command_table() {
        v.push((head.to_string(), false));
        for s in subs { v.push((format!("{head} {}", s.trim()), s.ends_with(' '))); }
    }
    v
}

#[test]
fn every_completion_probe_has_a_palette_entry_or_a_documented_exclusion() {
    let palette: Vec<_> = PALETTE_CATALOG.iter()
        .filter_map(|e| parse_colon_command(e.0)).filter_map(Result::ok).collect();
    let mut missing = Vec::new();
    for (probe, needs_arg) in probes() {
        if PALETTE_EXCLUSIONS.iter().any(|(p, r)| *p == probe && !r.is_empty()) { continue; }
        let covered = match parse_colon_command(&probe) {
            Some(Ok(a)) if !needs_arg => palette.contains(&a),
            _ => PALETTE_CATALOG.iter().any(|e| e.0.starts_with(&probe)),
        };
        if !covered { missing.push(probe); }
    }
    assert!(missing.is_empty(), "add a PALETTE_CATALOG entry or a PALETTE_EXCLUSIONS reason for: {missing:#?}");
}
// + palette_exclusions_are_real_and_not_redundant(): each exclusion probe is in probes() and NOT covered.
```

Note multi-word subs (`"expand all"`, `"menu pick "`) work because the probe is the literal concatenation. Because it reads `colon_command_table()`, any new verb added to Tab completion (already mandatory via the `action.rs` assertion) fails this test until it is in the palette or excluded - satisfying the ROADMAP success criterion "a test fails when a new verb has neither".

## 5. Implementation notes for the planner

- **File size**: `help_catalog.rs` is 654 lines; ~110-130 new palette lines -> ~780, still under the ARCH-R04 soft goal (800-1000, backstop 1200, `tests/session_lessons_and_regression_suite.rs:372-376`). Preferred: move `PALETTE_CATALOG` into new `src/ui/palette_catalog.rs` and keep `pub use` in `help_catalog.rs`/`modals.rs:5` (imports unchanged); update the file map in `docs/DEVELOPER_AND_AGENT_GUIDE.md:122` and `AGENTS.md`.
- **Parallelism / ownership**: the palette table is one array, so only ONE plan should own it. Other Phase 51 plans (`:agents demo stop`, agents-list window verbs) add new verbs; the new test will fail until their entries exist. Sequence: the palette-parity plan first adds the test + exclusions + all existing-verb entries; later plans append their own entries (or the orchestrator wires them). Alternative to avoid contention: change `PALETTE_CATALOG` to a `LazyLock<Vec<_>>`/fn chaining per-area slices; only 4 non-test call sites (`wm/session.rs:88-98`, `command_registry.rs:123,186`, `help_catalog.rs` tests) - not required.
- **Shortcut column**: use "-" (em dash) for entries without a key, as existing; description should include searchable words (e.g. `:drawer style navigator` -> "Navigator drawer style (tree with agent badges)").
- **Don't** add an "execute every palette entry" test: `:quit`, `:detach`, `:ssh`, `:record`, `:mcp checkpoint` have side effects; the existing parse test (`help_catalog.rs:500`) plus the new coverage test is the right level. Spot-execute only the new entries via `WindowManager::new_headless(24, 80)` + `execute_wm_colon_command` (pattern at `help_catalog.rs:527-538`).

## 6. Assumptions / open questions

| # | Item | Risk |
|---|---|---|
| A1 | Gap counts come from a regex scan, not compiled code; exact list = first failing run of the new test | Planner should expect +/-15% |
| A2 | `:session rename` bare opens the prompt (inferred from palette text for `:drawer rename`, help_catalog) | Verify in `execute_wm.rs` before writing the description |
| A3 | Which explicit `on`/`off` and per-preset entries to add (~30 entries) vs exclude as "bare toggle suffices" is a product call | Default recommendation: add `on`/`off`; exclude `:window theme <preset>` as picker-reachable |
| A4 | Optional Enter-handler change (prefill prompt for fill-in/`Err` entries) touches `src/input/modal.rs` (orchestrator-owned input path, ARCH-R01 `handle_wm_key_event <= 80` is unaffected - this is the modal handler) | Needs user OK; otherwise leave fill-in entries as exclusions-with-entry |
