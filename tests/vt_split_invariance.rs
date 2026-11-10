//! The control-sequence scanner is stateful: however PTY output is chunked, the terminal ends up
//! in the same state and emits the same events as when the bytes arrive in one piece.
//!
//! Covers split invariance over a corpus of real and malformed sequences (deterministic seed),
//! one regression test per bug the stateless scanner had, and linear-time evidence counted in
//! scanner steps (no wall-clock assertions).

use bilterm::vt::snapshot::UnderlineKind;
use bilterm::vt::{LineSizeAttr, VtParser};
use bilterm::wm::WindowManager;

const ROWS: u16 = 6;
const COLS: u16 = 24;

fn vt() -> VtParser {
    VtParser::new(ROWS, COLS, 50)
}

/// Everything observable about a parser except wall-clock values and the per-call generation.
fn fingerprint(vt: &mut VtParser) -> String {
    let mut snap = vt.snapshot();
    snap.generation = 0;
    let history: Vec<Option<String>> =
        (0..vt.total_available_rows()).map(|r| vt.abs_row_text(r)).collect();
    let modes = (
        vt.is_alternate_screen(),
        vt.is_bracketed_paste(),
        vt.is_application_cursor(),
        vt.mouse_report_level(),
        vt.is_mouse_sgr_or_tracking(),
        vt.is_focus_reporting(),
        vt.is_synchronized_output(),
        vt.is_mouse_alternate_scroll(),
        vt.is_grapheme_cluster_2027(),
        vt.is_report_color_scheme_2031(),
        vt.is_in_band_size_2048(),
        vt.is_backarrow_key_67(),
    );
    let style = (
        vt.kitty_keyboard_flags(),
        vt.active_underline_kind(),
        vt.active_underline_color(),
        vt.is_overline(),
    );
    let events = (
        vt.window_title().map(str::to_owned),
        vt.reported_cwd().map(std::path::Path::to_path_buf),
        vt.osc133_phase(),
        vt.last_exit_code(),
        vt.osc9_progress(),
        vt.notifications().to_vec(),
        vt.notifications_received(),
        vt.unread_bell(),
        vt.osc8_links().to_vec(),
        vt.graphic_placements().to_vec(),
        vt.recent_escape_sequences().to_vec(),
    );
    let outbound = (
        vt.take_query_responses(),
        vt.take_outbound_osc52(),
        vt.take_osc52_decoded(),
        vt.tektronix().stream_capture,
        vt.total_bytes_processed(),
        vt.scanner_dropped_sequences(),
    );
    format!("{snap:?}\n{history:?}\n{modes:?}\n{style:?}\n{events:?}\n{outbound:?}")
}

fn whole(input: &[u8]) -> String {
    let mut vt = vt();
    vt.process(input);
    fingerprint(&mut vt)
}

fn chunked(input: &[u8], sizes: impl Iterator<Item = usize>) -> String {
    let mut vt = vt();
    let mut rest = input;
    for size in sizes {
        if rest.is_empty() {
            break;
        }
        let (head, tail) = rest.split_at(size.clamp(1, rest.len()));
        vt.process(head);
        rest = tail;
    }
    vt.process(rest);
    fingerprint(&mut vt)
}

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Well-formed sequences as real programs emit them, plus malformed ones. All valid UTF-8.
fn corpus() -> Vec<Vec<u8>> {
    let items: &[&[u8]] = &[
        b"plain text\r\n",
        "h\u{e9}llo \u{4e16}\u{754c} \u{276f} \u{1f389} caf\u{e9}\r\n".as_bytes(),
        "c1 as text \u{9b}31m \u{9d}0;x\u{9c} \u{90}q\r\n".as_bytes(),
        b"\x1b[31mred\x1b[0m \x1b[1;4mbold-ul\x1b[m\r\n",
        b"\x1b[4:3mcurly\x1b[58:2::255:0:0mcol\x1b[4:0m\x1b[59m ",
        b"\x1b[38:2::10:20:30mfg\x1b[48;2;1;2;3mbg\x1b[38;5;42m256\x1b[0m\x1b[53mover\x1b[55m",
        b"\x1b[2;3Hmoved\x1b[K\x1b[1;1H\x1b[2Kline\x1b[3B\x1b[5C*",
        b"\x1b[?1049h\x1b[Halt screen\x1b[>7u\x1b[?u\x1b[<u\x1b[?1049l",
        b"\x1b[?2004h\x1b[?1h\x1b[?1000;1006h\x1b[?1002h\x1b[?1003h\x1b[?1004h",
        b"\x1b[?2026h\x1b[?2027h\x1b[?2031h\x1b[?2048h\x1b[?67h\x1b[?1007l\x1b[?25l",
        b"\x1b[?1003l\x1b[?1004l\x1b[?2026l\x1b[?25h\x1b[?2004l",
        b"\x1b[5n\x1b[6nabc\x1b[6n\x1b[c\x1b[>c\x1b[>q\x1b[?996n",
        b"\x1b[?2004$p\x1b[?25$p\x1b[?1049$p\x1b[4$p\x1b[?9999$p",
        b"\x1b[>5u\x1b[=3;2u\x1b[?u\x1b[>1u\x1b[<2u\x1b[?u",
        "\x1b]0;t\u{ef}tle \u{4e16}\x07after-title ".as_bytes(),
        b"\x1b]2;st title\x1b\\ following text\x07",
        b"\x1b]4;1;?\x07\x1b]10;?\x1b\\\x1b]11;?\x07\x1b]12;?\x07",
        b"\x1b]7;file://host/home/u/proj\x07\x1b]8;id=a;https://example.com/x\x1b\\link\x1b]8;;\x1b\\",
        b"\x1b]9;4;1;50\x07\x1b]9;build done\x07\x1b]777;notify;Title;Body text\x1b\\",
        b"\x1b]52;c;aGVsbG8gd29ybGQ=\x07\x1b]52;p;Zm9v\x1b\\\x1b]52;c;?\x07",
        b"\x1b]133;A\x07$ \x1b]133;B\x07ls\r\n\x1b]133;C\x07out\r\n\x1b]133;D;3\x07",
        b"\x1b]1337;Canvas=line 1 1 30 15\x07\x1b]1337;SetMark\x07",
        b"\x1bP+q544e;436f;7a7a\x1b\\\x1bP$qm\x1b\\\x1bP$qr\x1b\\\x1bP$q q\x1b\\\x1bP$qzz\x1b\\",
        b"ab\x1bP0;1;q\"1;1;4;6#1;2;100;0;0#1~~~~-~~~~\x1b\\cd",
        b"x\x1b_Ga=T,f=24,s=2,v=2,i=7;AAAAAAAAAAAAAAAA\x1b\\y\x1b_Ga=q,i=9;AAAA\x1b\\z",
        b"\x1b_Ga=d,i=7\x1b\\\x1b^privacy message\x1b\\\x1bXsos\x1b\\ok",
        b"\x1b#3TOP\r\n\x1b#4BOTTOM\r\n\x1b#6WIDE\r\n\x1b#5\x1b7\x1b8\x1bM\x1b(B",
        b"\x1bPtmux;\x1b\x1b]52;c;dG11eA==\x07\x1b\\after",
        // Malformed / hostile shapes.
        b"lone \x1b",
        b"\x1b[31\x18cancelled \x1b[4\x1asubbed \x1b[1\x1b[32mrestart",
        b"\x1b[3\n1mC0 inside CSI\x1b\r[0m",
        b"\x1b]0;aborted\x1b[7mby csi\x1b[0m \x1b]0;can\x18celled \x1b]52;c;QQ==\x1b[2J\x07",
        b"\x1bP$qm\x07bel-dcs \x1bPq#1~~\x1bXnested\x1b\\ \x1b_Gbroken\x18x",
        "\x1b\u{e9}[1mESC then non-ascii\x1b[\u{4e16}0m".as_bytes(),
        b"\x1b[?1;?2h\x1b[1 2m\x1b[ q\x1b[?2004\x1b\\h",
        b"\x1b]\x07\x1b[\x1b]\x1b\\\x1bP\x1b\\\x1b_\x07\x1b[m",
    ];
    let mut out: Vec<Vec<u8>> = items.iter().map(|i| i.to_vec()).collect();
    // An over-long CSI (dropped) and a long, valid SGR.
    out.push([b"\x1b[".as_slice(), "1;".repeat(200).as_bytes(), b"mlong"].concat());
    out.push([b"\x1b[".as_slice(), "38;5;1;".repeat(20).as_bytes(), b"mok"].concat());
    out
}

#[test]
fn every_corpus_item_is_split_invariant_at_every_boundary() {
    for (n, item) in corpus().iter().enumerate() {
        let expected = whole(item);
        assert_eq!(chunked(item, std::iter::repeat(1)), expected, "item {n}: 1 byte at a time");
        for cut in 1..item.len() {
            assert_eq!(chunked(item, std::iter::once(cut)), expected, "item {n}: split at {cut}");
        }
    }
}

#[test]
fn generated_streams_are_split_invariant_under_random_chunking() {
    let corpus = corpus();
    let mut rng = XorShift(0x9e37_79b9_7f4a_7c15);
    for stream_no in 0..40 {
        let mut stream = Vec::new();
        for _ in 0..30 {
            stream.extend_from_slice(&corpus[rng.below(corpus.len())]);
            if rng.below(3) == 0 {
                stream.extend_from_slice(format!("t{}\u{276f}\r\n", rng.below(1000)).as_bytes());
            }
        }
        let expected = whole(&stream);
        assert_eq!(chunked(&stream, std::iter::repeat(1)), expected, "stream {stream_no}: bytes");
        for max in [2usize, 7, 64, 1024] {
            let mut sizes = XorShift(rng.next() | 1);
            let got = chunked(&stream, std::iter::repeat_with(|| 1 + sizes.below(max)));
            assert_eq!(got, expected, "stream {stream_no}: random chunks up to {max}");
        }
    }
}

/// Raw 8-bit C1 bytes and other invalid UTF-8: never introducers, never a panic, same result.
/// (Fewer than two binary C0 bytes each, so the per-chunk binary-noise heuristic stays out.)
#[test]
fn invalid_utf8_and_raw_c1_bytes_are_split_invariant() {
    let items: &[&[u8]] = &[
        b"a\x9b31mb\x9d0;title\x9c\x90q\x9cz",
        b"\xe2\x1b[1mbold\xe2\x9d\x1b[0m\xf0\x9f\x1b]0;t\xe2\x07x\xff",
        b"\x1b]0;caf\xc3\x07 \x1b]9;\xe2\x9d\xaf\xe2\x1b\\ \x1bP$q\xffm\x1b\\",
        b"\x1b[\xff3\x801m\xc3 red \x1b\xc3\xa9[0m",
    ];
    for (n, item) in items.iter().enumerate() {
        let expected = whole(item);
        assert_eq!(chunked(item, std::iter::repeat(1)), expected, "item {n}: 1 byte at a time");
        for cut in 1..item.len() {
            assert_eq!(chunked(item, std::iter::once(cut)), expected, "item {n}: split at {cut}");
        }
    }
    let mut vt = vt();
    vt.process(b"a\x9b31mb");
    assert_eq!(vt.snapshot().row_text(0).trim_end().chars().last(), Some('b'));
    assert!(vt.snapshot().row_text(0).contains("31m"), "0x9b must not act as CSI");
}

// ── Regression tests: each failed with the stateless per-chunk scanner ────────────────────────

fn feed(parts: &[&[u8]]) -> VtParser {
    let mut vt = vt();
    for part in parts {
        vt.process(part);
    }
    vt
}

#[test]
fn split_csi_mode_changes_are_not_lost() {
    let vt = feed(&[b"\x1b[?10", b"04h\x1b[>", b"5u"]);
    assert!(vt.is_focus_reporting(), "CSI ?1004h split across reads");
    assert_eq!(vt.kitty_keyboard_flags(), 5, "CSI >5u split across reads");
}

#[test]
fn split_osc_title_cwd_and_exit_code_are_not_lost() {
    let vt = feed(&[b"\x1b]0;hel", b"lo\x07\x1b]7;file://h/tm", b"p\x1b", b"\\\x1b]133;D;", b"7\x07"]);
    assert_eq!(vt.window_title(), Some("hello"));
    assert_eq!(vt.reported_cwd(), Some(std::path::Path::new("/tmp")));
    assert_eq!(vt.last_exit_code(), Some(7));
}

#[test]
fn osc_ends_at_its_own_terminator_not_a_later_bel() {
    let vt = feed(&[b"\x1b]0;one\x1b\\ text \x1b]9;note\x07"]);
    assert_eq!(vt.window_title(), Some("one"));
    assert_eq!(vt.notifications().len(), 1);
    assert_eq!(vt.notifications()[0].body, "note");
}

#[test]
fn split_extended_sgr_is_tracked() {
    let vt = feed(&[b"\x1b[4:", b"3mcurly"]);
    assert_eq!(vt.active_underline_kind(), UnderlineKind::Curly);
}

#[test]
fn split_graphics_payloads_place_an_image_and_print_nothing() {
    let vt = feed(&[b"\x1bP0;1;q\"1;1;4;6#1;2;100;0;0#1~~", b"~~-~~~~\x1b", b"\\"]);
    assert_eq!(vt.graphic_placements().len(), 1, "sixel split across reads");
    assert_eq!(vt.snapshot().contents().trim(), "", "sixel payload must not reach the grid");

    let mut vt = feed(&[b"\x1b_Ga=T,f=24,s=2,v=2,i=7;AAAAAAAA", b"AAAAAAAA\x1b\\"]);
    assert_eq!(vt.graphic_placements().len(), 1, "kitty image split across reads");
    assert_eq!(vt.snapshot().contents().trim(), "", "kitty payload must not reach the grid");
    assert_eq!(vt.take_query_responses(), vec![b"\x1b_Gi=7;OK\x1b\\".to_vec()]);
}

#[test]
fn cursor_report_uses_the_position_at_the_query() {
    let mut vt = feed(&[b"\x1b[6nabc"]);
    assert_eq!(vt.take_query_responses(), vec![b"\x1b[1;1R".to_vec()]);
}

#[test]
fn split_dec_line_size_escape_is_applied_to_the_cursor_row() {
    let vt = feed(&[b"one\r\n\x1b#", b"3TOP"]);
    assert_eq!(vt.line_size_attr(1), LineSizeAttr::DoubleHeightTop);
    assert_eq!(vt.line_size_attr(0), LineSizeAttr::default());
}

#[test]
fn tektronix_entry_neither_eats_earlier_sequences_nor_needs_one_read() {
    let vt = feed(&[b"\x1b[2Jx\x1b[?38h"]);
    assert!(vt.tektronix().stream_capture);
    assert_eq!(vt.snapshot().contents().trim(), "x", "CSI 2J must not print its final byte");

    let vt = feed(&[b"\x1b[?3", b"8h\x1dAB", b"\x1b[?38", b"lback"]);
    assert!(!vt.tektronix().stream_capture, "capture ended by a split CSI ?38l");
    assert_eq!(vt.snapshot().contents().trim(), "back");
}

#[test]
fn split_canvas_osc_reaches_the_pane() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.active_pane_mut().process_pty_output(b"\x1b]1337;Canvas=line 1 1");
    assert_eq!(wm.active_pane().canvas_scene().primitives.len(), 0);
    wm.active_pane_mut().process_pty_output(b" 30 15\x07");
    assert_eq!(wm.active_pane().canvas_scene().primitives.len(), 1);
}

#[test]
fn osc52_is_forwarded_once_for_every_split_and_aborted_by_a_bare_esc() {
    let seq = b"\x1b]52;c;aGVsbG8=\x1b\\";
    for cut in 1..seq.len() {
        let mut vt = feed(&[&seq[..cut], &seq[cut..]]);
        assert_eq!(vt.take_outbound_osc52(), vec!["\x1b]52;c;aGVsbG8=\x1b\\".to_string()], "cut {cut}");
        assert_eq!(vt.take_osc52_decoded(), vec!["hello".to_string()]);
    }
    let mut vt = feed(&[b"\x1b]52;c;QQ==", b"\x1b", b"[2J\x07"]);
    assert!(vt.take_outbound_osc52().is_empty(), "ESC that is not ST aborts the OSC");
}

// ── Bounded carry and linear time ────────────────────────────────────────────────────────────

#[test]
fn unterminated_sequences_have_a_bounded_carry_and_are_dropped_whole() {
    const OSC_LIMIT: usize = 1024 * 1024 + 64;
    let mut vt = vt();
    vt.process(b"\x1b]0;");
    let block = [b'A'; 4096];
    let mut peak = 0;
    for _ in 0..600 {
        vt.process(&block);
        peak = peak.max(vt.osc52_carry_len());
    }
    assert!(peak <= OSC_LIMIT, "carry peaked at {peak}");
    assert_eq!(vt.osc52_carry_len(), 0, "over-long OSC is dropped, not kept");
    assert_eq!(vt.scanner_dropped_sequences(), 1);
    vt.process(b"tail\x07shown");
    assert_eq!(vt.window_title(), None, "no title from an over-long OSC");
    assert_eq!(vt.snapshot().contents().trim(), "shown");

    // DCS / APC: same rule with the graphics limit; nothing is printed.
    let mut vt = self::vt();
    vt.process(b"\x1bPq");
    for _ in 0..1100 {
        vt.process(&block);
    }
    assert_eq!(vt.osc52_carry_len(), 0);
    vt.process(b"\x1b\\after");
    assert_eq!(vt.graphic_placements().len(), 0);
    assert_eq!(vt.snapshot().contents().trim(), "after");
}

/// The stateless scanner re-read the whole carried OSC 52 on every chunk (quadratic). The framer
/// consumes each byte exactly once, whatever the chunking.
#[test]
fn scanner_steps_equal_input_bytes_for_long_unterminated_sequences() {
    let mut data = b"\x1b]52;c;".to_vec();
    data.resize(512 * 1024, b'A');
    for chunk in [1usize, 64, 4096, data.len()] {
        let mut vt = vt();
        for part in data.chunks(chunk) {
            vt.process(part);
        }
        assert_eq!(vt.scanner_steps(), data.len() as u64, "chunk size {chunk}");
    }

    // Many unterminated introducers in one read: each one used to scan to the end of the chunk.
    let data = b"\x1b]0;xxxxxxxxxxxx".repeat(16_000);
    let mut vt = vt();
    vt.process(&data);
    assert_eq!(vt.scanner_steps(), data.len() as u64);
    assert_eq!(vt.window_title(), None);
}
