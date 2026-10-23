# Phase 20 Plan 01: VT Module Decomposition & UI Style Migration Summary

**Decomposed `src/vt/mod.rs` (2,716 lines) and `src/vt/scrollback.rs` (834 lines) into focused `<= 800`-line sub-modules, relocated `cell_to_ratatui_style` to `src/ui/`, and populated `ARCH-R01..ARCH-R05` in `.planning/REQUIREMENTS.md`.**

## Accomplishments

- Populated `ARCH-R01..ARCH-R05` requirements and traceability table entries in `.planning/REQUIREMENTS.md`.
- Relocated `cell_to_ratatui_style` and `vt_color_to_ratatui` from `src/vt/mod.rs` into `src/ui/mod.rs`, removing the `ratatui` dependency from `src/vt/` (`grep -rn "use ratatui" src/vt/` returns zero matches).
- Split `src/vt/mod.rs` (2,716 lines) into:
  - `src/vt/mod.rs` (800 lines): `Osc133Phase`, `CommandBlockRecord`, `RawChunkRecord`, `VtParser` core state, grid building, and scrollback navigation.
  - `src/vt/scan.rs` (776 lines): `scan_control_sequences`, `scan_csi_and_sgr_sequences`, and `normalize_extended_sgr_for_vt100`.
  - `src/vt/detect.rs` (278 lines): `CanvasMatch`, `DetectedUrl`, `DetectedPathMatch`, and regex-free grid scanners.
  - `src/vt/graphics.rs` (262 lines): `InlineGraphicProtocol`, `InlineGraphicEntry`, Kitty APC `G`, Sixel DCS `q`, and iTerm2 OSC `1337` parsers.
  - `src/vt/codec.rs` (178 lines): `sanitize_paste_bytes`, `is_unsafe_paste`, `encode_osc52_clipboard`, `base64_encode`, `base64_decode`, hex/JSON helpers, and binary noise chunk sanitizers.
  - `src/vt/recording.rs` (60 lines): `VtParser` asciicast v2 recording implementation (`start_recording`, `start_recording_private`, `stop_recording`, `append_recording_event`).
  - `src/vt/tests.rs` (335 lines): `vt` unit test suite.
- Extracted `src/vt/scrollback.rs` unit tests into `src/vt/scrollback_tests.rs` (203 lines), reducing `src/vt/scrollback.rs` from 834 to 623 lines.

## Files Created/Modified

- `.planning/REQUIREMENTS.md`
- `src/vt/mod.rs`
- `src/vt/scan.rs`
- `src/vt/detect.rs`
- `src/vt/graphics.rs`
- `src/vt/codec.rs`
- `src/vt/recording.rs`
- `src/vt/tests.rs`
- `src/vt/scrollback.rs`
- `src/vt/scrollback_tests.rs`
- `src/ui/mod.rs`
- `tests/terminal_torture_and_stress.rs`

## Verification

- `cargo test --lib vt::` (21 passed)
- `cargo test --test terminal_torture_and_stress` (6 passed)
- `cargo clippy --all-targets -- -D warnings` (0 warnings)
- `awk 'ENDFILE{if(FNR>800){print FILENAME,FNR; e=1}} END{exit e}' src/vt/*.rs` (exited 0)
