# Phase 62: Copy Mode Ergonomics & Nested-TUI Copy Fix - Research

**Researched:** 2026-11-08
**Domain:** Terminal mouse-reporting vs. local selection, alt-screen copy mode, OSC 52 inbound parsing, copy-mode UX (vi/emacs/mouse), cross-OS selection-override conventions
**Confidence:** HIGH for the verdict and root causes (each one reproduced headlessly this session); MEDIUM for cross-OS convention details (web docs); LOW where tagged `[ASSUMED]`.

<user_constraints>
## User Constraints (from CONTEXT.md)

No `62-CONTEXT.md` exists (discuss-phase was not run). Constraints come from the orchestrator brief and `.planning/REQUIREMENTS.md`:

### Locked Decisions
- Requirements: `COPY-01` ("Copying text from panes running nested TUIs with alt-screen + mouse tracking (e.g. `herdr`, Claude Code) works reliably via mouse (modifier override) and keyboard copy mode.") and `COPY-02` ("Copy mode is easy to discover and use: one-key entry/exit, on-screen key hints, arrow/vi/emacs selection, word/line/block modes, search, copy-on-release, and visual confirmation.") [VERIFIED: .planning/REQUIREMENTS.md:264-267]
- Keybinding harmonization across OSes is owned by **Phase 63** (`KEY-01..03`). Phase 62 must reference it, not redefine global hotkeys. [VERIFIED: .planning/REQUIREMENTS.md:269-273]
- `UNI-R01` surface uniformity, `ARCH-R01` (`handle_wm_key_event <= 80` lines, everything lowers to `Action`/`WindowManager::apply`), `ARCH-R02`, `SEC-R01..04` apply (CLAUDE.md).

### Claude's Discretion
- Which modifier(s) override mouse tracking, default for copy-on-release, copy-mode key profile, hint rendering, plan split.

### Deferred Ideas (OUT OF SCOPE)
- Global/OS-level hotkey remapping, per-OS keymap profiles, conflict detection against nested multiplexers (Phase 63).
- Traffic monitor (Phase 64), window/drawer regressions (Phase 61).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| COPY-01 | Copy from nested alt-screen + mouse-tracking TUIs (herdr, Claude Code) via mouse override and keyboard copy mode | Sections 1-2: three reproduced defects (Shift not an override; OSC 52 lost across PTY reads; keys swallowed after a mouse selection) and one gating gap (alt-screen keyboard entry). Plan 62-01. |
| COPY-02 | Copy mode discoverable and easy: one-key entry/exit, hints, vi/emacs/arrow, word/line/block, search, copy-on-release, visual confirmation | Sections 3-4: gap table against current `handle_copy_mode_key`, reproduced scroll-selection corruption, proposals. Plan 62-02. |
</phase_requirements>

## Summary

**Verdict: COPY from a pane running herdr (or Claude Code) is still broken, in four independent ways.** Phase 57 fixed PASTE only (Ctrl+V smart paste, Shift+Right/Middle-click paste, inbound OSC 52 decode). It did not touch selection. All findings below were reproduced this session with a headless probe linked against the working tree (`WindowManager::new_headless(25,80)`, child state `?1049h ?1000h ?1002h ?1006h ?2004h`, i.e. alt-screen + mouse tracking exactly as herdr/Claude Code set them). Probe source and binary live in the session scratchpad; the repo was not modified.

**Primary root cause (what the user feels):** with mouse tracking on, a plain drag is forwarded to the child as SGR bytes by design, and the only local-selection override is **Alt**; **Shift is not an override for left-drag**, and the SGR encoder additionally *drops the Shift/Alt/Ctrl bits*, so `Shift+drag` is delivered to herdr as a plain drag (probe: `PLAIN drag` and `SHIFT drag` both wrote `ESC[<0;21;5m`). Shift is the de-facto bypass in xterm, WezTerm, Alacritty, kitty-family and Windows Terminal, so the muscle-memory gesture silently does nothing useful. The Alt override that does exist produces a **rectangular block** selection, **does not copy** (default `copy_on_select = false`), and afterward the pane **swallows every keystroke** (copy mode stays active) until `y`/`Esc` is pressed, so the terminal appears frozen.

Secondary causes: (a) herdr's own drag-select copies via the terminal, but a large inbound **OSC 52** that spans two 8192-byte PTY reads is **silently dropped** (probe: 9000-byte payload whole -> clipboard length 9000; split at 8192 -> clipboard empty); (b) keyboard copy-mode entry chords `Alt+C`/`F8`/`Ctrl+Up`/`Alt+R` are deliberately gated off on the alternate screen (probe: they are forwarded to the PTY), leaving only the leader chord `Ctrl+B [`, which collides with herdr's own default prefix `Ctrl+B` + `[`; (c) copy-mode selection is stored in viewport-relative coordinates, so selecting past the viewport edge copies the wrong text (probe: 40x `k` selecting 41 lines yielded 24 lines starting at `LINE 060`, ending in `L`).

**Primary recommendation:** Plan 62-01 makes Shift (and Alt) a real local-selection override over mouse-tracking apps, forwards modifier bits to children, copies on release by default with visible confirmation, makes mouse-originated selections transient (no key swallowing), and fixes OSC 52 chunk splitting. Plan 62-02 rebuilds copy-mode ergonomics on a scrollback-absolute selection model with real word/line/search motions, vi+emacs+arrow keys, a state-driven hint line, and clickable entry/exit.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Decide mouse event goes to child vs local selection | Input routing (`src/input/mouse.rs` + new `mouse_select.rs`) | Pane VT state (`is_mouse_sgr_or_tracking`) | Policy is pure function of modifiers + pane state |
| SGR mouse encoding to child | `src/wm/quick_terminal.rs::encode_sgr_mouse_local` | VT mode flags | Must carry modifier/motion bits faithfully |
| Selection model, motions, search | `src/pane/` (`modes.rs` + new pure modules) | VT snapshot/scrollback | State machine, headless-testable |
| Clipboard write (OS + OSC 52 out + registry) | `CompositePane::record_yank_with_source` | `src/clipboard.rs`, `WindowManager::sync_clipboard_records` | Already the single funnel |
| Inbound OSC 52 | `src/vt/` scanner | `src/pane/mod.rs::drain_osc52_decoded` | Parser must be chunk-boundary safe |
| Hints / badges / toast | `src/ui/oia.rs` (+ new pure `copy_hints`) | GUI via shared ratatui buffer | One renderer feeds TUI and GUI |
| Surface parity | `Action::{CopyModeToggle,SelectCommand}` via `WindowManager::apply` | palette, help catalog, completion, RPC `wm.command` | No new Action variants required |

## Current State (with file:line evidence)

### 1. Mouse path over a mouse-tracking child
- `handle_wm_mouse_event` computes `force_local_select = ALT || shift_mouse_paste || copy_mode.active || rect_mode || mouse_drag.is_some()` where `shift_mouse_paste` is **only** Shift + Down(Right|Middle). [VERIFIED: src/input/mouse.rs:928-937] Quote: `let shift_mouse_paste = pane_mouse.modifiers.contains(KeyModifiers::SHIFT) && matches!(pane_mouse.kind, MouseEventKind::Down(MouseButton::Right | MouseButton::Middle));`
- If routing is `RawPtyPassthrough` and not forced local, and tracking is on, the event is encoded and written to the child and the function returns. [VERIFIED: src/input/mouse.rs:939-953]
- `encode_sgr_mouse_local` builds `\x1b[<{btn};{col};{row}{M|m}` from button/kind only: no `+4` Shift, `+8` Alt, `+16` Ctrl, no motion handling for Right/Middle drag or bare motion, and no ?1000-vs-?1002/?1003 distinction. [VERIFIED: src/wm/quick_terminal.rs:27-48]
- Alt+click/drag starts a **Block** selection with `visual_lock = true` and status `"RECT SELECT [1×1]: Drag or Shift+Arrows | Enter/y copy | x to prompt | Esc clear"`; the canvas-only variant (alt-screen, prompt hidden) does the same. [VERIFIED: src/input/mouse.rs:210-235, 483-504]
- On `Up(Left)` the selection is only *staged* (`stage_mouse_canvas_selection`); it is copied only when `config.copy_on_select` is true, which defaults to `false`. [VERIFIED: src/input/mouse.rs:420-443, 923, 956-968; src/config/mod.rs:260 `copy_on_select: false,`]
- GUI backends deliver Shift/Ctrl/Option(=Alt) mouse modifiers (`NS_EVENT_MODIFIER_SHIFT/CONTROL/OPTION` -> `KeyModifiers`); macOS Command is **not** mapped for mouse. [VERIFIED: src/ui/gui_macos.rs:565-575] X11 uses `modifiers_from_x11_state`. [VERIFIED: src/ui/gui_font/linux_x11.rs:746]
- TUI mode (running inside another terminal): the outer terminal usually consumes Shift+drag for its own native selection, so bilterm never sees it. [CITED: wezterm.org/config/lua/config/bypass_mouse_reporting_modifiers.html - "default value ... is SHIFT"; Alacritty/xterm same convention per ghostty.org/docs/vt/csi/xtshiftescape]. Alt reaches bilterm as SGR bit 8 in most terminals. [ASSUMED]

### 2. Keyboard path into copy mode on the alt screen
- `handle_key_event`: when copy mode is active it routes to `handle_copy_mode_key` first. [VERIFIED: src/input/mod.rs:79-82]
- In `RawPtyPassthrough`, `Alt+R`, `Alt+C`, `F8`, `Ctrl+Up` enter copy mode **only inside `if !pane.vt().is_alternate_screen() && !pane.is_active_canvas_password_prompt()`**. [VERIFIED: src/input/mod.rs:152-167] Probe confirmed on alt-screen: `Alt+C -> ESC c`, `F8 -> ESC[19~`, `Ctrl+Up -> ESC[1;5A`, `Alt+R -> ESC r` all forwarded; copy mode not entered. The gate is correct (F8 is htop/mc, Alt+C is readline/emacs) but leaves no discoverable direct chord.
- Working entry on alt-screen: leader `Ctrl+B`/`Ctrl+X` then `[`/`c`/`C`/`F8`. [VERIFIED: src/input/keymap_tables.rs:127-130; probe `Ctrl+B [: copy_active=true`]. `Ctrl+B Ctrl+B` forwards a literal Ctrl+B. [VERIFIED: src/input/keymap.rs:243-251]
- Collision: herdr's default prefix is `ctrl+b` and its copy mode is `prefix+[`; `v`/Space start, `y`/Enter copy, `q`/Esc leave; mouse drag-select copies without entering copy mode. [CITED: herdr.dev/tui/docs/keyboard/] In bilterm, `Ctrl+B [` is captured by bilterm's leader first. Resolution of that conflict belongs to Phase 63 (KEY-02 "conflict detection against ... nested-multiplexer bindings"); Phase 62 only surfaces the real chord in hints. Not changed here.
- `enter_copy_mode` starts the cursor at `vt.cursor_position()` with no selection and status `"CANVAS SELECT: Arrows/hjkl move | r/b/Ctrl+V=Rect | v/Space=Char | V=Line | Enter/y copy | x to prompt | Esc exit"`. [VERIFIED: src/pane/modes.rs:280-296] The status message has no expiry and is overwritten by any later message (`set_status_message` plain assignment). [VERIFIED: src/pane/scroll.rs:720-722]

### 3. Copy-mode key handler gaps (`handle_copy_mode_key`)
[VERIFIED: src/input/modal.rs:61-272]
- Motions present: `h j k l`/arrows, PageUp/PageDown (8 rows), `Home`/`0`, `End`/`$`, `w` = **+5 columns** (probe: `w` from col 0 -> col 5 on "alpha beta gamma"), `B` = -5, `Ctrl/Alt+Left/Right` = +/-5 cols.
- Missing: real word motions (`w b e`), `gg`/`G`, `H M L`, half-page `Ctrl+u/d`, `^`, any search (probe: `/` is ignored, `canvas_search.active == false`), emacs keys (`Ctrl+f/b/n/p/a/e`, `Ctrl+Space`, `Alt+w`), line/word via mouse (no multi-click handling anywhere; grep `click_count|double_click` finds only the browser canvas).
- Selection modes present: `v`/Space (char), `V` (line), `r`/`b`/`Ctrl+V` (block), `Shift+Arrows`. `y`/`c`/Enter copy; `x` copy + insert to prompt; `Esc q i Tab F8` exit.
- Search infrastructure exists but is not wired to copy mode and is viewport-only: `CompositePane::set_canvas_search` / `next_/prev_canvas_search_match`, `VtParser::search_canvas` iterates rows `0..rows` of the visible grid. [VERIFIED: src/pane/modes.rs:225-269; src/vt/detect.rs:188-210]

### 4. Selection coordinates are viewport-relative (correctness bug)
- `CopyModeState { anchor: Option<(u16,u16)>, cursor_row: u16, cursor_col: u16, ... }`; `extract_selected_text(&GridSnapshot)` indexes the **current viewport snapshot**. [VERIFIED: src/pane/modes.rs:52-71, 163-192] `k` at row 0 calls `vt.scroll_up(1)` without moving the anchor. [VERIFIED: src/input/modal.rs:163-169]
- Probe (primary screen, 100 lines, `v` then 40x `k`): `anchor=(23,0)`, `cursor=(0,0)`, `scrollback_offset=17`; yank returned **24 lines**, first `"LINE 060"`, last `"L"`. Expected 41 lines. Alt-screen has no scrollback, so there it only ever means "the visible frame", which should be said in the hint.
- `scrollback_used_rows()` already exists and gives the absolute mapping `abs_row = used_rows - scrollback_offset + viewport_row`. [VERIFIED: src/vt/mod.rs:340-347]

### 5. Post-selection key capture (UX trap)
- Mouse drag sets `copy_mode.active = true`. [VERIFIED: src/input/mouse.rs:398-410] Because `handle_key_event` routes to copy mode first and `in_raw_passthrough` in `handle_wm_key_event` requires `!copy_mode().active`, every key is eaten. [VERIFIED: src/input/mod.rs:79-82, 699-705] Probe: after an Alt-drag, key `a` -> `copy_active=true pty_write=""`; after `y`, `a` -> `"a"`. The only on-screen cue is the OIA badge `" INPUT: CANVAS [F8/Tab/Esc] "`, and **F8 does not exit on the alt screen path users expect** (it does exit inside copy mode via `modal.rs:152`, but the hint reads as an entry key). [VERIFIED: src/ui/oia.rs:107-120]
- OIA has no indicator that the child owns the mouse, so users do not know why drag "does nothing". [VERIFIED: grep of src/ui/oia.rs for `mouse` returns no badge]

### 6. OSC 52 inbound (herdr copy -> clipboard)
- Per-chunk scanner: `scan_control_sequences(bytes)` is called once per `process(raw_bytes)` with that chunk only; the OSC 52 branch does `osc_rest.find(['\x07','\x1b'])` and, if absent, **silently returns**. No carry buffer exists. [VERIFIED: src/vt/scan.rs:18-34; src/vt/scan_osc_dcs.rs:158-175; src/vt/mod.rs:211-260 (only `utf8_carry` exists)]
- PTY reads are 8192 bytes: `let mut buf = [0u8; 8192];`. [VERIFIED: src/vt/worker.rs:301] Probe: 9000-char payload `encode_osc52_clipboard` fed whole -> `clipboard_len=Some(9000)`; fed as `chunks(8192)` -> `None`.
- Small copies (< a few KB, not straddling a read boundary) work, which is why this is intermittent and easy to mistake for "herdr copy randomly fails". Decode limit 1 MiB and `?` query rejection are correct and must be kept. [VERIFIED: src/vt/scan_osc_dcs.rs:458-470; src/vt/codec.rs:90-110]
- Phase 57 already routes decoded text to `set_system_clipboard`, yank ring, `clipboard.log` history and registry (`drain_osc52_decoded`). [VERIFIED: src/pane/mod.rs:852-862]

### 7. What already works (do not rebuild)
- `record_yank_with_source` is the single funnel: yank ring, `history_store`, OS clipboard, outbound OSC 52 (`last_osc52_sequence`), registry event. [VERIFIED: src/pane/modes.rs:558-579]
- CJK/wide-glyph exact extraction is pinned by `tests/copy_paste_unicode.rs` (reuse its `pane_with`/`extract` helpers). [VERIFIED: tests/copy_paste_unicode.rs:15-38]
- Colon/palette/completion/registry already know `:copy`, `:rect`, `:select [rect|char|line|mode|copy|clear]`, `:copy auto [on|off]`. [VERIFIED: src/colon_command/execute.rs:238-249, 355-375; src/pane/completion.rs:448-449, 509-525]

## Verdict Matrix (herdr / Claude Code in a bilterm pane)

| Gesture | Today | Verdict |
|---------|-------|---------|
| Plain drag | Forwarded to child (herdr selects+copies itself via OSC 52) | OK for small copies; **fails when OSC 52 straddles an 8192-byte read** (D3) |
| Shift+drag | Forwarded as plain drag, modifier bit dropped | **Broken** (D1) |
| Alt+drag | Local **rect** selection, not copied, then keys swallowed | **Poor** (D1, D2, D4) |
| `Ctrl+B [` | Enters bilterm copy mode (not herdr's) on frame-only snapshot | Works but conflicts with herdr prefix (Phase 63 handoff) |
| `Alt+C` / `F8` / `Ctrl+Up` | Forwarded to herdr | By design; no discoverable direct chord (D5) |
| Select beyond viewport | Wrong text | **Broken** (D6, primary screen) |

Defects: D1 Shift not an override + modifier bits dropped; D2 selection not auto-copied; D3 OSC 52 chunk split; D4 key capture after mouse selection; D5 no discoverable alt-screen entry; D6 viewport-relative selection.

## Cross-OS Selection-Override Conventions

| Terminal | Bypass mouse reporting | Source |
|----------|-----------------------|--------|
| xterm and most VTE-family terminals | Shift | [CITED: ghostty.org/docs/vt/csi/xtshiftescape] |
| WezTerm | Shift by default, configurable | [CITED: wezterm.org/config/lua/config/bypass_mouse_reporting_modifiers.html] |
| Alacritty | Shift (Shift added to mouse bindings when captured) | [CITED: search result summary, alacritty commit page via web search] |
| iTerm2 (macOS) | Option | [ASSUMED] (training knowledge; not confirmed this session) |
| Windows Terminal | Shift | [ASSUMED] |

**Policy for bilterm (prescriptive):** `Shift+drag` and `Alt/Option+drag` both force local selection when the child has mouse tracking. Shift = character-stream selection; Alt/Option = rectangular block (keeps the existing Phase 41/VAL-04 meaning). In GUI mode both modifiers reach bilterm on macOS, X11 and (assumed) Windows. In TUI mode the outer terminal normally keeps Shift for its own native selection (which is also an acceptable user outcome), so Alt is the in-band override there. For terminals that deliver neither, a **sticky per-pane "mouse select" toggle** (`:copy mouse on|off|toggle`, clickable OIA badge) switches mouse routing to local selection regardless of modifiers. macOS `Cmd+C`/Windows-Linux `Ctrl+Shift+C` are GUI-mode copy chords; in TUI mode `Ctrl+Shift+C` is indistinguishable from `Ctrl+C` because no keyboard-enhancement flags are pushed [VERIFIED: grep `KeyboardEnhancementFlags` in src/ returns no push site] and the outer terminal captures it, so it cannot be the TUI entry chord. Final per-OS chord assignment is Phase 63's; Phase 62 declares the entry chords in one table so Phase 63 can re-point them.

## Standard Stack

No new crates. Existing: `crossterm 0.29`, `ratatui 0.30`, `vt100-ctt 0.17.1` (default VT engine), `insta` for snapshots. [VERIFIED: Cargo.toml:20-23; CLAUDE.md]

### Package Legitimacy Audit
No external packages are added by this phase. Audit not applicable.

## Architecture Patterns

### System Architecture Diagram
```
 host input (TUI crossterm / GUI X11 / macOS)
        |  MouseEvent{kind, col,row, modifiers}
        v
 handle_wm_mouse_event --(hit-test, frames, OIA badges)--> pane_mouse
        |
        v
 mouse_select::route(pane, pane_mouse, cfg)  <-- NEW pure policy
        |  decision: ToChild | Local{stream|block} | Paste
        +--> ToChild: encode_sgr_mouse(modifiers, motion, mode) --> PTY --> herdr
        |
        +--> Local: handle_mouse_event (selection) --Up(Left)--> copy_on_release?
                          |                                 yes: record_yank_with_source
                          v                                      -> OS clipboard + OSC52 out + registry
                    CopyModeState{origin: Mouse (transient) | Keyboard (sticky)}
                          |                                      -> toast + selection flash
 key events -> handle_key_event -> copy_mode keys (vi | emacs | arrows) -> motions (pure)
 child bytes -> PTY(8192 reads) -> VtParser::process -> scan_control_sequences
                                    + OSC52 carry (NEW) -> drain_osc52_decoded -> clipboard
```

### Recommended Project Structure
```
src/input/mouse_select.rs     # NEW  routing policy (override modifiers, sticky toggle) - keeps mouse.rs < 1000 lines
src/input/copy_mode_keys.rs   # NEW  (62-02) extracted copy-mode key tables: vi + emacs + arrows
src/pane/copy_motion.rs       # NEW  (62-02) pure word/line/paragraph motions over row text
src/pane/copy_search.rs       # NEW  (62-02) scrollback-wide search + n/N
src/ui/copy_hints.rs          # NEW  (62-02) pure state -> hint-line string
tests/phase62_nested_tui_copy.rs        # 62-01
tests/phase62_copy_mode_ergonomics.rs   # 62-02
```
`src/input/mouse.rs` is 972 lines and `src/pane/modes.rs` 784 against the ~800-1000 soft goal (hard backstop 1200), so new logic goes in new files. [VERIFIED: wc -l]

### Pattern 1: Pure routing policy
`enum MouseRoute { ToChild, Local(SelectKind), Paste }` computed from `(modifiers, kind, pane.copy_mode, sticky_mouse_select, tracking)`, unit-testable without a PTY; `handle_wm_mouse_event` calls it in place of the inline `force_local_select` expression.

### Pattern 2: Transient vs sticky copy mode
`CopyModeState.origin: CopyOrigin { Keyboard, Mouse }`. A `Mouse`-origin selection treats `y/Enter/Ctrl+C/Cmd+C/x/Esc/Shift+Arrow` as copy-mode keys and **any other key clears the selection and is forwarded as if copy mode were inactive**. `Keyboard` origin stays modal (vi semantics).

### Pattern 3: Carry buffer for split OSC 52 (bounded)
On an unterminated `ESC ] 52 ;` at the end of a chunk, store that tail (cap `MAX_OSC52_PAYLOAD_BYTES + 64`), drop it if it exceeds the cap or if a non-ST `ESC`/new `ESC ]` arrives, and prepend it to the next `scan_control_sequences` input only (never to the vt100 parser input). Mirrors the existing `utf8_carry`. [VERIFIED pattern: src/vt/mod.rs:211-224]

### Anti-Patterns to Avoid
- Putting copy-mode entry on `F8`/`Alt+C` on the alt screen: breaks htop/mc/readline. Use the leader chord, GUI chords, and clickable badge instead.
- Making the override a global config only: terminals that swallow both Shift and Alt need the sticky per-pane toggle.
- Adding new `Action` variants for each sub-feature: extend `:copy`/`:select` sub-arguments (they already lower to `Action::CopyModeToggle`/`Action::SelectCommand(Some(sub))`), so the `every_action_is_reachable_from_keymap_colon_or_rpc` gate is unaffected. [VERIFIED: src/action.rs:136-138, 547-548]
- Hard-coding OIA column widths again: `handle_oia_status_bar_mouse` and `oia_mic_badge_rect` each re-derive `title_w/state_w/routing_w` by hand (src/input/mouse.rs:66-84, src/ui/oia.rs:503-520). Add one shared layout helper before adding clickable badges.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Clipboard + OSC 52 + registry on copy | New write path | `CompositePane::record_yank_with_source` | Already writes OS clipboard, OSC 52, yank ring, registry; secret classification in registry |
| Wide-glyph-correct extraction | New extractor | `CopyModeState::extract_selected_text` / `push_selected_cell` | Pinned by `tests/copy_paste_unicode.rs` |
| Scrollback row count | Own counter | `VtParser::scrollback_used_rows()` | Clamped probe via `set_scrollback` |
| Word/regex search | New matcher | Reuse `CanvasSearchState` + extend `search_canvas` to scrollback | Highlight rendering already consumes it |
| Colon/palette/RPC parity | New RPC methods | `:copy`/`:select` sub-args through `wm.command` | RPC/MCP already lower colon commands |

## Common Pitfalls

### Pitfall 1: Shift never reaches bilterm in TUI mode
**What goes wrong:** the outer terminal handles Shift+drag natively; tests pass (headless injects Shift) but TUI users still see no bilterm selection.
**How to avoid:** document in hint text; make Alt the in-band override; provide the sticky toggle; keep tests for both.
**Warning signs:** user reports "Shift-drag highlights across panes".

### Pitfall 2: Modifier bits and Drag-with-mouse-tracking modes
`?1000` apps want press/release only, `?1002` wants drag, `?1003` wants all motion. Forwarding drags to a `?1000` app can confuse it. Track which of the three is active (currently collapsed into one `mouse_tracking_mode` bool: src/vt/scan.rs:66-70) or at minimum add the modifier bits. Keep scope bounded: modifier bits mandatory, mode distinction best-effort.

### Pitfall 3: Flipping `copy_on_select` default breaks Classic parity goldens
No test asserts the default (grep of `tests/` finds no `copy_on_select`), but status text goldens may include copy messages. Run the full `scripts/verify.sh`, not only `--quick`.

### Pitfall 4: Absolute-row selection vs history eviction/live output
Abs rows shift when the scrollback ring evicts. Clamp on read and clear the selection if the anchor falls out of retained history; document that alt-screen selection is frame-only.

### Pitfall 5: ARCH-R01
Any new key handling in `handle_wm_key_event` must be table-driven (`keymap_tables.rs`); copy-mode key logic lives in `handle_key_event` / the new `copy_mode_keys.rs`, not in `handle_wm_key_event` (currently ~50 lines, limit 80). [VERIFIED: src/input/mod.rs:662-719]

### Pitfall 6: Auto-copy must not leak secrets
Selected text can contain secrets; registry classification (`pane::redact::classify`) and `clipboard.log` redaction already apply via `record_clipboard_event`; auto-copy must go through the same funnel, never a side channel (SEC-R01..04).

## Proposals (answering the brief)

1. **One-key entry/exit, discoverable.** Entry: leader `[` (exists, tmux/herdr muscle memory) surfaced in an alt-screen OIA hint; GUI-only direct `Ctrl+Shift+C` (Linux/Windows) / `Cmd+C` (macOS) which copies if a selection exists else enters; clickable `[COPY]` OIA badge (mouse path, TUI+GUI); `:copy`. Exit: `Esc`/`q`/the same toggle. Declared in a single `COPY_ENTRY_CHORDS` table for Phase 63 to re-point.
2. **On-screen key hints.** `copy_hint_line(&CopyModeState, alt_screen, search) -> String`, state-driven (idle / char / line / block / search), shown in the OIA message slot with priority over transient messages while copy mode is active; wide-terminal full form, narrow form `v sel  y copy  / find  Esc exit`.
3. **Mouse select that works over tracking apps.** Section "Cross-OS" policy above plus mouse-capture OIA badge `MOUSE->APP  Shift/Alt+drag=select`.
4. **Vi + emacs + arrows.** Vi: `h j k l w b e 0 ^ $ gg G H M L Ctrl+u/d/f/b v V Ctrl+v y / ? n N`. Emacs: `Ctrl+f/b/n/p`, `Alt+f/b`, `Ctrl+a/e`, `Ctrl+Space` mark, `Alt+w` copy. Arrows/Shift+Arrows/Ctrl+Arrows (word)/Home/End/PageUp/PageDown. `copy_mode_keys = "both"` default. Known conflict: `Ctrl+G` currently toggles raw override inside copy mode (src/input/modal.rs:66-71); keep it, do not bind emacs cancel to it (Esc/`q` cancel).
5. **Word/line/block.** Double-click = word, triple-click = line, `v`/`V`/`Ctrl+v`; `:select word|line|block`.
6. **Search.** `/` `?` `n` `N` inside copy mode over scrollback (primary) or frame (alt-screen), highlight reuse.
7. **Auto-copy on release.** Flip `copy_on_select` default to `true` for mouse selections (matches herdr and X11 convention; `:copy auto off` restores explicit mode). Mouse selection stays visible ~300 ms then clears (flash).
8. **Visual feedback/toast.** `status_toast(msg, ttl)` with expiry; text `Copied 42 chars -> clipboard (OSC52 sent)`; the copied region is rendered in a distinct flash style until expiry.

## Code Examples

### Reproduction harness pattern (from Phase 57 test, reuse)
```rust
// Source: tests/phase57_clipboard_paste_and_viewer.rs:121-139 [VERIFIED]
let mut wm = WindowManager::new_headless(25, 80);
wm.active_pane_mut()
    .process_pty_output(b"\x1b[?1049h\x1b[?1000h\x1b[?1006h\x1b[?2004h");
wm.active_pane_mut().write_to_pty(b"").unwrap();
handle_wm_mouse_event(&mut wm, MouseEvent { kind: MouseEventKind::Down(MouseButton::Right),
    column: 10, row: 5, modifiers: KeyModifiers::SHIFT }, Rect::new(0, 0, 80, 25)).unwrap();
assert_eq!(String::from_utf8_lossy(wm.active_pane().last_pty_write()), "...");
```

### SGR modifier bits (xterm ctlseqs; add to `encode_sgr_mouse_local`)
```rust
// btn_code |= 4 (Shift) | 8 (Alt/Meta) | 16 (Ctrl); motion adds 32. [ASSUMED: xterm ctlseqs, training knowledge]
```

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| Select only with explicit Enter/y after drag | Copy on release + toast (herdr, X11, most modern terminals) | Fewer steps |
| Viewport-only copy cursor | History-absolute selection | Correct multi-screen copy |

## Runtime State Inventory
Not a rename/refactor phase: omitted.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| cargo/rustc | build and tests | yes | cargo 1.90.0 | - |
| X11/Wayland clipboard tools | live clipboard only | not needed | - | tests use process clipboard (`clear_process_clipboard`, test builds skip OS calls) |

Headless tests need no display. [VERIFIED: src/clipboard.rs:135-136 `#[cfg(not(test))]`; note integration tests are separate binaries, so use `is_automated_test_process()` path - already exercised by Phase 57 tests.]

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` (unit + `tests/*.rs` integration), `insta` snapshots, ratatui `TestBackend` |
| Config file | none (Cargo.toml) |
| Quick run command | `scripts/verify.sh --quick` (clippy + unit) |
| Full suite command | `scripts/verify.sh` |
| Targeted | `scripts/verify.sh --only <stage> --match phase62` [VERIFIED: scripts/verify.sh:3-10] |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| COPY-01 | Shift/Alt drag over `?1000/1002/1006` child selects locally, writes nothing to PTY | integration | `scripts/verify.sh --only integration --match phase62_nested` | Wave 0 (62-01) |
| COPY-01 | Modifier bits encoded (`Ctrl+click` -> btn 16) | unit | `cargo test -q encode_sgr_mouse_modifiers` | Wave 0 |
| COPY-01 | OSC 52 split at every byte offset decodes exactly once; overflow dropped | unit+integration | `cargo test -q osc52_split` | Wave 0 |
| COPY-01 | After mouse selection a non-copy key reaches PTY | integration | phase62_nested | Wave 0 |
| COPY-01 | Auto-copy on release writes clipboard + registry | integration | phase62_nested | Wave 0 |
| COPY-02 | Scroll-spanning selection returns exact lines | integration | `--match phase62_copy_mode` | Wave 0 (62-02) |
| COPY-02 | `w b e gg G H M L / n N` motions | unit (pure) | `cargo test -q copy_motion` | Wave 0 |
| COPY-02 | vi, emacs, arrow parity -> same selection | integration | phase62_copy_mode | Wave 0 |
| COPY-02 | Hint line per state (snapshot) | snapshot | phase62_copy_mode | Wave 0 |
| COPY-02 | UNI-R01: key/mouse/colon/palette/RPC reach same state | integration | phase62_copy_mode | Wave 0 |

### Sampling Rate
- Per task commit: `scripts/verify.sh --quick`; per wave: `scripts/verify.sh`; phase gate: full suite green. PASS/FAIL reading rule per CLAUDE.md (read only `target/verify/<stage>/fail.txt` on FAIL).

### Wave 0 Gaps
- [ ] `tests/phase62_nested_tui_copy.rs`, `tests/phase62_copy_mode_ergonomics.rs` (do not exist)
- [ ] Shared helper to build a "herdr-like" pane (`?1049h?1000h?1002h?1006h?2004h` + painted frame); put in the test files (support dir has only `phase60.rs`).

## Security Domain

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | yes | OSC 52 body charset + 1 MiB cap kept; carry buffer capped; `?` read queries stay rejected (`is_forwardable_osc52_write`) |
| V8 Data Protection | yes | Auto-copy routes through `record_clipboard_event` -> registry `pane::redact::classify`; secrets never written to `clipboard.log` |
| V2/V3/V4/V6 | no | - |

### Known Threat Patterns
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Hostile child floods unterminated `ESC]52;` to grow memory | DoS | Carry cap `MAX_OSC52_PAYLOAD_BYTES + 64`, reset on overflow/foreign ESC (bounding-untrusted-resource-use) |
| Child reads clipboard via OSC 52 `?` | Info disclosure | Already rejected; add regression test through the carry path |
| Auto-copy silently overwrites clipboard with secret text | Info disclosure | Toast always shows; registry redaction; `:copy auto off` |
| Escape smuggling riding on OSC 52 carry | Tampering | Keep existing "ESC that is not ST aborts" rule (src/vt/scan_osc_dcs.rs:160-166); test it across the chunk boundary |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | iTerm2 uses Option and Windows Terminal uses Shift to bypass mouse reporting | Cross-OS | Only affects documented rationale; the implemented policy accepts both modifiers |
| A2 | Outer terminals pass the Alt bit (SGR +8) to bilterm in TUI mode | Section 1 | If not, TUI users depend on the sticky toggle; covered |
| A3 | SGR modifier bit values (+4 Shift, +8 Alt, +16 Ctrl, +32 motion) from xterm ctlseqs | Code Examples | Wrong bits mis-signal children; executor should confirm against xterm ctlseqs before coding |
| A4 | herdr copies pane drag-selections to the system clipboard via OSC 52 (docs do not say) | Verdict | If herdr shells out to xclip/pbcopy instead, D3 is not the herdr path; D1/D2/D4/D5/D6 stand regardless |
| A5 | `copy_on_select` default flip to `true` is acceptable to the user | Proposals | Config knob + `:copy auto off` remain; surface as a confirm point |
| A6 | Real herdr issues `?1002`/`?1003` (probe used both) | Verdict | Behavior of routing is identical for any tracking mode |

## Open Questions

1. **Default for `copy_on_select`.** Recommend `true`. Needs user confirmation (A5).
2. **Herdr prefix collision (`Ctrl+B`).** Out of scope; handed to Phase 63 KEY-02. Phase 62 only prints the true chord in hints. Does the user want `leader_key = "Ctrl+X"` documented as the herdr-friendly setting? (`is_leader_key_with_tmux` accepts Ctrl+B and Ctrl+X together unless inside tmux: src/input/keymap.rs:182-199.)
3. **TUI-mode direct chord.** No modifier-free chord is safe on the alt screen; recommendation is leader `[` + clickable badge + sticky toggle. Phase 63 may assign a final chord.

## Sources

### Primary (HIGH confidence)
- Repository files read this session (all citations above) and headless probe runs against the working tree.
- [herdr keyboard docs](https://herdr.dev/tui/docs/keyboard/) - default prefix `ctrl+b`, `prefix+[` copy mode, `v/Space/y/Enter/q/Esc`, "Mouse drag-select copies without entering copy mode".

### Secondary (MEDIUM confidence)
- [WezTerm bypass_mouse_reporting_modifiers](https://wezterm.org/config/lua/config/bypass_mouse_reporting_modifiers.html) - default Shift.
- [Ghostty XTSHIFTESCAPE](https://ghostty.org/docs/vt/csi/xtshiftescape) - Shift is the common mouse-reporting override.

### Tertiary (LOW confidence)
- iTerm2/Windows Terminal conventions (training knowledge, A1).

## Metadata

**Confidence breakdown:**
- Verdict and root causes: HIGH - each reproduced headlessly, plus code read.
- Standard stack: HIGH - no new dependencies.
- Architecture: MEDIUM-HIGH - file layout follows existing conventions; abs-row mapping needs executor validation against vt100 `set_scrollback`.
- Pitfalls: MEDIUM.

**Research date:** 2026-11-08
**Valid until:** 2026-12-08 (stable codebase; re-verify line numbers if Phase 61/63 land first, both touch `src/input/`).

## Project Constraints (from CLAUDE.md)
- Run `scripts/verify.sh`; read its single output line; on FAIL read only `target/verify/<stage>/fail.txt`, then `log`.
- `ARCH-R04`: files target ~800-1000 lines (hard test backstop 1200); `ARCH-R01`: `handle_wm_key_event <= 80` lines; all commands/keys/RPC lower to `Action` via `WindowManager::apply`.
- `ARCH-R02`: `WindowManager`/`CompositePane` internal fields stay private; no new `pub` topology fields.
- `SEC-R01..04`: runtime files via `src/secure_fs.rs`; no secrets on argv.
- `UNI-R01`: mouse + key + colon + palette + help + RPC/MCP parity with a parity test; exceptions listed in plan `uniformity_exceptions`.
- Quiet cargo flags (`-q`); never push to main directly (PR only) [user memory]; planning artifacts only in this task (no commits).
