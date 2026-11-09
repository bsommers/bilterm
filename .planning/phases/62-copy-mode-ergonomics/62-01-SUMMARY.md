# Phase 62-01 Summary: Nested TUI Copy Fix and Mouse Routing Policy

## Delivered Scope
- `src/input/mouse_select.rs`: Pure mouse routing policy (`MouseRoute`: `ToChild`, `Local(Stream|Block)`, `Paste`) implementing `OverridePolicy` (`Shift`, `Alt`, `Any`, `Never`), sticky per-pane selection toggle (`:copy mouse on|off|toggle`), and SGR tracking prioritization.
- Correct SGR modifier bits (`+4 Shift`, `+8 Alt`, `+16 Ctrl`, `+32 Motion`) forwarded to child PTY when child owns mouse tracking.
- `copy_on_select` default set to `true`. Releasing local mouse selection auto-copies into system clipboard, pane yank ring, outbound OSC 52, and `ClipboardRegistry`.
- Status toast with TTL expiration (`set_status_toast`, `tick_toast`). Ordinary status messages remain non-expiring for Classic goldens.
- Transient mouse selection: mouse-originated selections release on non-copy keystrokes and deliver keystrokes to child PTY; keyboard-originated copy mode remains modal.
- `src/vt/scan_osc_dcs.rs` & `src/vt/scan.rs`: Bounded OSC 52 carry across arbitrary read chunk boundaries, protecting against split sequences, split ST (`\x1b\\`), oversized payloads (>1MB + 64), and smuggled CSI sequences.
- OIA `MOUSE->APP` badge and clickable `[COPY]` badge affordance.
- 15/15 tests passing in `tests/phase62_nested_tui_copy.rs`.

## Red-Test Baseline
Initial test execution failed with documented root causes D1, D2, D3, D5:
```
failures:
    test_01_alt_screen_shift_drag_selects_locally
    test_02_alt_screen_plain_drag_forwards_sgr_to_child
    test_03_sgr_modifiers_match_xterm_standard
    test_05_release_copies_by_default_and_toasts
    test_08_keyboard_copy_mode_stays_modal
    test_10_osc52_split_at_every_offset_decodes_once
    test_11_osc52_9000_byte_payload_split_at_8192
    test_12_osc52_carry_is_bounded_and_safe
    test_13_oia_shows_mouse_capture_badge
    test_14_ctrl_shift_c_gui_chord
```

## SGR-Bit Confirmation Source
- Verified against XTerm ctlseqs specification:
  - Button 0..2 (Left, Middle, Right), Button 3 (Release), Wheel Up/Down (64/65).
  - Modifier mask: `+4` for Shift, `+8` for Alt/Meta, `+16` for Control. Motion events add `+32`.

## Verification Results
- `CARGO_TARGET_DIR=/tmp/bilterm-target-62 cargo test -q --test phase62_nested_tui_copy`: 15 passed, 0 failed.
- `CARGO_TARGET_DIR=/tmp/bilterm-target-62 cargo clippy -q --all-targets -- -D warnings`: 0 warnings, 0 errors.
- `scripts/check_classic_goldens.sh`: GOLDENS OK 11.
