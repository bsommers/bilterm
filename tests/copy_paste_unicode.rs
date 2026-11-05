//! Regression suite: copying a canvas selection (stream or 2D block) must reproduce the
//! on-screen text exactly for non-ASCII content (accents, CJK, emoji, box drawing, combining
//! marks). A wide glyph occupies two cells (lead + empty continuation tail); the tail must never
//! be turned into a stray space in the copied text (garbled copy / paste). The paste direction
//! (`Event::Paste` -> prompt, sanitiser, OSC 52 payload) is pinned to be UTF-8 lossless too.

use bilterm::input::handle_paste_event;
use bilterm::pane::{CompositePane, CopySelectionKind};
use bilterm::vt::{base64_decode, encode_osc52_clipboard, sanitize_paste_bytes};

const SAMPLE: &str = "café 世界 😀 ┌─┐ nai\u{308}ve e\u{301} Ünïcödé ✓";

fn pane_with(rows: &[&str]) -> CompositePane {
    let mut p = CompositePane::new_headless(1, 8, 40);
    let mut bytes = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            bytes.extend_from_slice(b"\r\n");
        }
        bytes.extend_from_slice(row.as_bytes());
    }
    p.vt_mut().process(&bytes);
    p
}

/// Selects `(r0, c0)..=(r1, c1)` with `kind` and returns the text `extract_selected_text` yields.
fn extract(p: &mut CompositePane, kind: CopySelectionKind, a: (u16, u16), b: (u16, u16)) -> String {
    let cm = p.copy_mode_mut();
    cm.active = true;
    cm.anchor = Some(a);
    cm.cursor_row = b.0;
    cm.cursor_col = b.1;
    cm.selection_kind = kind;
    cm.visual_lock = true;
    let snap = p.vt().snapshot();
    p.copy_mode().extract_selected_text(&snap)
}

#[test]
fn stream_select_cjk_has_no_spaces_between_wide_glyphs() {
    let mut p = pane_with(&["世界你好"]);
    // 4 wide glyphs = 8 cells (cols 0..=7).
    let got = extract(&mut p, CopySelectionKind::Char, (0, 0), (0, 7));
    assert_eq!(got, "世界你好");
}

#[test]
fn stream_select_mixed_ascii_cjk_emoji_roundtrips() {
    let line = "ab世界cd😀ef";
    let mut p = pane_with(&[line]);
    let got = extract(&mut p, CopySelectionKind::Char, (0, 0), (0, 20));
    assert_eq!(got, line);
}

#[test]
fn stream_select_accents_box_drawing_and_combining_marks_roundtrip() {
    let line = "café ┌─┐ nai\u{308}ve e\u{301}";
    let mut p = pane_with(&[line]);
    let got = extract(&mut p, CopySelectionKind::Char, (0, 0), (0, 30));
    assert_eq!(got, line);
}

#[test]
fn block_select_over_cjk_keeps_glyphs_contiguous() {
    // Rows: wide glyph pairs; select cols 0..=3 (two glyphs) on both rows.
    let mut p = pane_with(&["世界你好", "日本語です"]);
    let got = extract(&mut p, CopySelectionKind::Block, (0, 0), (1, 3));
    assert_eq!(got, "世界\n日本");
}

#[test]
fn block_select_column_slice_of_mixed_width_rows() {
    let mut p = pane_with(&["a世b", "xyzw"]);
    // cols 1..=2 hold the wide glyph on row 0 and "yz" on row 1.
    let got = extract(&mut p, CopySelectionKind::Block, (0, 1), (1, 2));
    assert_eq!(got, "世\nyz");
}

#[test]
fn block_select_edge_landing_on_wide_tail_keeps_whole_glyph() {
    let mut p = pane_with(&["a世b"]);
    // Block starts on the continuation tail (col 2) of the wide glyph at cols 1..=2.
    let got = extract(&mut p, CopySelectionKind::Block, (0, 2), (0, 3));
    assert_eq!(got, "世b");
}

#[test]
fn yank_copy_selection_returns_exact_unicode_text() {
    let line = "naïve 世界 ✓ ┌┐";
    let mut p = pane_with(&[line]);
    let cm = p.copy_mode_mut();
    cm.active = true;
    cm.anchor = Some((0, 0));
    cm.cursor_row = 0;
    cm.cursor_col = 30;
    cm.selection_kind = CopySelectionKind::Char;
    assert_eq!(p.yank_copy_selection().as_deref(), Some(line));
}

#[test]
fn paste_event_into_command_prompt_is_utf8_lossless() {
    let mut p = CompositePane::new_headless(1, 8, 40);
    handle_paste_event(&mut p, SAMPLE).unwrap();
    assert_eq!(p.prompt().text(), SAMPLE);
}

#[test]
fn paste_event_multiline_unicode_into_command_prompt_keeps_every_line() {
    let mut p = CompositePane::new_headless(1, 8, 40);
    handle_paste_event(&mut p, "世界\r\n😀 ok\n┌─┐").unwrap();
    assert_eq!(p.prompt().text(), "世界\n😀 ok\n┌─┐");
}

#[test]
fn sanitize_paste_bytes_passes_non_ascii_through_untouched() {
    assert_eq!(sanitize_paste_bytes(SAMPLE), SAMPLE);
    assert_eq!(sanitize_paste_bytes("a\x1bb\u{7f}c世"), "a b c世");
}

#[test]
fn osc52_payload_roundtrips_utf8_for_every_length_residue() {
    for n in 0..SAMPLE.chars().count() {
        let text: String = SAMPLE.chars().take(n).collect();
        let seq = encode_osc52_clipboard(&text);
        let b64 = seq
            .strip_prefix("\x1b]52;c;")
            .and_then(|r| r.strip_suffix('\x07'))
            .expect("OSC 52 envelope");
        let bytes = base64_decode(b64);
        assert_eq!(String::from_utf8(bytes).unwrap(), text, "prefix of {n} chars");
    }
}
