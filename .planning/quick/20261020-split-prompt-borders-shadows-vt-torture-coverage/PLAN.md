---
phase: quick
plan: 20261020-split-prompt-borders-shadows-vt-torture-coverage
type: execute
wave: 1
depends_on: []
files_modified:
  - src/pane/mod.rs
  - src/pane/colon_command.rs
  - src/pane/completion.rs
  - src/ui/chrome.rs
  - src/ui/theme.rs
  - src/ui/mod.rs
  - src/config/mod.rs
  - src/input/mod.rs
  - src/vt/mod.rs
  - tests/terminal_torture_and_stress.rs
  - docs/TEST_COVERAGE.md
autonomous: true
---

# Quick Plan: Non-Overlapping Split Prompt Mode, Themeable DOS Drop Shadows, Selectable Border Outline Styles, Full VT Control Sequence Fidelity & Comprehensive Stress/Torture Test Suite

## Objectives
1. **Non-Overlapping Command Pane + Output Pane Split Mode (`PromptAnchor::SplitTop` / `PromptAnchor::SplitBottom` or `PromptLayoutMode::Floating` vs `PromptLayoutMode::Split`)**:
   - Allow each `CompositePane` to either float the command prompt over the full-height `OutputCanvas` (`Floating` mode) OR split the composite pane into two dedicated, non-overlapping regions (`SplitTop`: command pane on top, output canvas below; `SplitBottom`: output canvas on top, command pane below) so the prompt never overlaps output text when desired!
   - When in `SplitTop` or `SplitBottom` mode (and not in `ALT_SCREEN` / `RawPtyPassthrough`), `canvas_rect` occupies the exact remaining non-overlapping height (`pane_height - 1 - prompt_height`), and `resize_viewport` resizes the `VtParser` / `PtySession` cleanly to the non-overlapping canvas size (`TIOCSWINSZ` deduplicated).
   - Expose via `:prompt split-top`, `:prompt split-bottom`, `:prompt split` (toggles floating vs split), `Alt+S` hotkey, and `default_prompt_anchor = "split-top" | "split-bottom"` in `config.toml`.
2. **Themeable & Selectable DOS-Style Drop Shadows (`ShadowStyle` / `shadow_fg` / `shadow_bg` / `drop_shadows`)**:
   - Enable Borland DOS-style drop shadows by default (`drop_shadows: true` in `BiltermConfig::default()`, selectable/toggleable via `:shadows on|off|translucent|solid|tinted` and `config.toml`).
   - Add `ShadowStyle` (`None`, `Translucent` VGA attribute `0x08` dimming that preserves underlying glyphs, `SolidHalfBlock` CP437 `▄`/`█` shadow, `ThemeTinted` using `palette.shadow_fg` / `palette.shadow_bg`) and add `shadow_fg` / `shadow_bg` to `ThemePalette` + `.ini` theme parser (`shadow_fg`, `shadow_bg`).
   - Render drop shadows on both multi-pane windows (in Stacked AND Tiled/Overlay modes when enabled) and floating prompts.
3. **Selectable Window & Pane Outline Border Styles (`OutlineStyle`)**:
   - Add `OutlineStyle` enum (`Double`, `Solid` / `Plain`, `Rounded`, `Thick`, `Dashed`, `Dotted`, `Quadruple`, `Ascii`) selectable independently or together for windows (`window_outline`) and command prompt panes (`prompt_outline`) via `config.toml` and `:outline <style>` / `:border <style>` (`solid`, `double`, `dotted`, `dashed`, `rounded`, `thick`, `quadruple`, `ascii`).
   - Implement custom border glyph sets (`ratatui::symbols::border::Set`) so `Dotted` (`┈`/`┊`), `Dashed` (`┄`/`┆`), `Quadruple`, `Double` (`═`/`║`), `Solid` (`─`/`│`), `Rounded` (`╭`/`╮`), `Thick` (`━`/`┃`), and `Ascii` (`+`/`-`/`|`) all render accurately on both `render_borland_frame` and `render_floating_prompt`.
4. **Full VT100/VT220/Xterm Control Sequence Compliance (`VtParser`)**:
   - Respect and track the full set of terminal control sequences:
     - **SGR (`CSI ... m`)**: Bold (`1`), Dim/Faint (`2`), Italic (`3`), Underline (`4`), Slow/Rapid Blink (`5`/`6`), Inverse (`7`), Hidden/Conceal (`8`), Strikethrough (`9`), 16-color, 256-color (`38;5;n` / `48;5;n`), 24-bit TrueColor (`38;2;r;g;b` / `48;2;r;g;b`), Reset (`0`).
     - **Cursor & Screen (`CSI`)**: `CUP` (`H`/`f`), `CUU`/`CUD`/`CUF`/`CUB` (`A`/`B`/`C`/`D`), `CNL`/`CPL` (`E`/`F`), `CHA` (`G`), `VPA` (`d`), `ED` (`J` 0/1/2/3), `EL` (`K` 0/1/2), `IL`/`DL` (`L`/`M`), `DCH`/`ICH` (`P`/`@`), `ECH` (`X`), `SU`/`SD` (`S`/`T`), `DECSTBM` (`r` scroll region), `DECSC`/`DECRC` (`ESC 7` / `ESC 8` & `CSI s` / `CSI u`).
     - **DEC Private Modes (`DECSET` / `DECRST` `CSI ? ... h/l`)**:
       - `DECCKM` (`?1` Application Cursor Keys — so `Up`/`Down`/`Left`/`Right`/`Home`/`End` emit `\x1bOA`..`\x1bOD` when application cursor mode is active!)
       - `DECAWM` (`?7` Auto-Wrap Mode)
       - `DECTCEM` (`?25` Show/Hide Cursor)
       - `DECSET 47 / 1047 / 1049` (Alternate Screen Buffer)
       - `DECSET 1000 / 1002 / 1003 / 1006` (Mouse Tracking modes)
       - `DECSET 1004` (Focus In/Out reporting)
       - `DECSET 2004` (Bracketed Paste)
       - `DECSET 2026` (Synchronized Output / BSU-ESU)
     - **OSC Sequences**:
       - `OSC 0` / `OSC 2` (`\x1b]0;title\x07`, `\x1b]2;title\x07`) dynamic window/pane title updates!
       - `OSC 7` (`\x1b]7;file://host/path\x07`) working directory reporting!
       - `OSC 52` (`\x1b]52;c;<base64>\x07`) clipboard passthrough!
       - `OSC 133` (`\x1b]133;A/B/C/D\x07`) semantic prompt/command zones!
     - **Device Status / Queries (`DSR` / `DA`)**:
       - `CSI 5 n` (Device Status Report -> responds `\x1b[0n` OK)
       - `CSI 6 n` (Cursor Position Report `CPR` -> responds `\x1b[<row>;<col>R`!)
       - `CSI c` / `CSI 0 c` (Primary Device Attributes `DA1` -> responds `\x1b[?62;22c` VT220 with color)
5. **Comprehensive Test Suite, Stress Tests, Torture Tests & `docs/TEST_COVERAGE.md` (`/swe-skills:test-coverage`)**:
   - Create `tests/terminal_torture_and_stress.rs` covering VT100/VT220/Xterm torture sequences, split-vs-floating prompt non-overlap invariants, themeable drop shadows, dotted/dashed/solid/double/thick/rounded outline styles, 100KB high-frequency resize + split + z-order shuffle + random binary stream stress tests, DSR/CPR/DA query response verification, `DECCKM` application cursor key encoding, and `OSC 0/2/7` tracking.
   - Re-run `cargo llvm-cov` and write `docs/TEST_COVERAGE.md` following the `/swe-skills:test-coverage` template.
