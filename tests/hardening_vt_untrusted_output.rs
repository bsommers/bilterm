//! Regression tests: bytes printed by a program in a pane are untrusted and must not be able to
//! inject input, crash, hang, or exhaust memory through the VT scanner.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::mpsc;
use std::time::Duration;

use bilterm::vt::{
    GraphicProtocol, MAX_GRAPHIC_DIM_PX, TektronixState, VtParser, encode_tek_10bit_point,
};

/// Counts bytes allocated by the current thread so memory blow-ups are observable.
struct CountingAlloc;

thread_local! {
    static THREAD_ALLOCATED: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = THREAD_ALLOCATED.try_with(|c| c.set(c.get().saturating_add(layout.size())));
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = THREAD_ALLOCATED.try_with(|c| c.set(c.get().saturating_add(new_size)));
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

/// Bytes allocated (cumulative, not net) on this thread while running `f`.
fn allocated_during(f: impl FnOnce()) -> usize {
    let before = THREAD_ALLOCATED.with(Cell::get);
    f();
    THREAD_ALLOCATED.with(Cell::get) - before
}

/// Runs `f` on a helper thread and fails (instead of hanging the suite) if it does not finish.
fn must_finish_within<T: Send + 'static>(
    secs: u64,
    what: &str,
    f: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs))
        .unwrap_or_else(|_| panic!("{what} did not finish within {secs}s"))
}

fn vt() -> VtParser {
    VtParser::new(24, 80, 100)
}

// ── 1. DECRQSS / XTGETTCAP replies must never reflect attacker text ──────────────────────────

#[test]
fn decrqss_invalid_request_reply_is_constant_and_reflects_nothing() {
    let mut vt = vt();
    vt.process(b"\x1bP$qx\nrm -rf ~\n\x1b\\");
    assert_eq!(vt.take_query_responses(), vec![b"\x1bP0$r\x1b\\".to_vec()]);

    // Valid requests still answer.
    vt.process(b"\x1bP$qm\x1b\\");
    assert_eq!(vt.take_query_responses(), vec![b"\x1bP1$r0m\x1b\\".to_vec()]);
}

#[test]
fn xtgettcap_unknown_name_reply_contains_only_validated_hex() {
    let mut vt = vt();
    // "4E4F5045" = "NOPE" (unknown), "0A6C73" = "\nls" (unknown, decodes to a newline).
    vt.process(b"\x1bP+q4E4F5045;0A6C73\x1b\\");
    let replies = vt.take_query_responses();
    assert_eq!(replies.len(), 2);
    for reply in replies {
        let body = reply
            .strip_prefix(b"\x1bP0+r")
            .and_then(|r| r.strip_suffix(b"\x1b\\"))
            .expect("unknown-name reply framing");
        assert!(body.iter().all(u8::is_ascii_hexdigit), "{body:?}");
    }

    // Not hex at all (`from_str_radix` used to accept a sign): no reply.
    vt.process(b"\x1bP+q+a;-1;zz;\n\n\x1b\\");
    assert!(vt.take_query_responses().is_empty());
}

// ── 2. Non-ASCII XTGETTCAP payload must not panic on a char boundary ────────────────────────

#[test]
fn xtgettcap_non_ascii_payload_does_not_panic() {
    let mut vt = vt();
    vt.process(b"\x1bP+qa\xc3\xa9b\x1b\\");
    vt.process("\x1bP+q\u{1F525}\u{1F525};é;aé;éa;4Eé4\x1b\\".as_bytes());
    assert!(vt.take_query_responses().is_empty());
    // The parser is still alive and answering.
    vt.process(b"\x1bP+q544E\x1b\\");
    assert_eq!(vt.take_query_responses().len(), 1);
}

// ── 3. Kitty keyboard pop count is attacker-supplied ────────────────────────────────────────

#[test]
fn kitty_keyboard_pop_with_huge_count_returns_promptly() {
    let flags = must_finish_within(10, "CSI < N u", || {
        let mut vt = vt();
        vt.process(b"\x1b[>5u\x1b[>3u");
        assert_eq!(vt.kitty_keyboard_flags(), 3);
        vt.process(b"\x1b[<18446744073709551615u");
        let after_huge_pop = vt.kitty_keyboard_flags();
        // A normal pop still pops exactly one entry.
        vt.process(b"\x1b[>5u\x1b[>3u\x1b[<1u");
        (after_huge_pop, vt.kitty_keyboard_flags())
    });
    assert_eq!(flags, (0, 5));
}

// ── 4. vt100-ctt does not clamp ICH / IL / SD counts ────────────────────────────────────────

fn feed_chunked(vt: &mut VtParser, bytes: &[u8], step: usize) {
    for chunk in bytes.chunks(step) {
        vt.process(chunk);
    }
}

#[test]
fn oversized_csi_counts_are_clamped_to_the_grid() {
    let rows = must_finish_within(20, "CSI 65535 @ / L / T flood", || {
        let mut vt = vt();
        let mut flood = Vec::new();
        for _ in 0..40 {
            flood.extend_from_slice(b"\x1b[65535@\x1b[65535L\x1b[65535T\x1b[65535S\x1b[65535M");
            flood.extend_from_slice(b"\x1b[65535P\x1b[65535X\x1b[99999999999999999999@");
            // Extra parameters, leading zeros, an embedded C0 control, ignored bytes.
            flood.extend_from_slice(b"\x1b[65535;1@\x1b[000000000065535L\x1b[655\x0035T");
            flood.extend_from_slice(b"\x1b\x7f[65535@\x1b[65\x7f535L");
            flood.extend_from_slice("\x1b[65é535T\x1b[é65535@".as_bytes());
            // A parameter run too long to hold must not become a way around the clamp.
            flood.extend_from_slice(b"\x1b[65535");
            flood.extend_from_slice(&b";".repeat(600));
            flood.push(b'@');
        }
        vt.process(&flood);
        // Split at every byte boundary so the count and final byte arrive in separate chunks.
        feed_chunked(&mut vt, &flood[..flood.len() / 40], 1);
        feed_chunked(&mut vt, &flood[..flood.len() / 40], 3);

        // Semantics are preserved: small counts are exact, huge counts behave as "whole line".
        vt.process(b"\x1b[2J\x1b[1;1Habc\x1b[1;1H\x1b[2@\x1b[2;1Hxyz\x1b[2;1H\x1b[65535@");
        vt.process(b"\x1b[3;1Hkeep\x1b[38;2;1;2;3mZ\x1b[0m\x1b[?25l\x1b[?25h");
        (vt.row_text(0), vt.row_text(1), vt.row_text(2))
    });
    assert_eq!(rows.0.trim_end(), "  abc");
    assert_eq!(rows.1.trim_end(), "");
    assert_eq!(rows.2.trim_end(), "keepZ");
}

#[test]
fn csi_count_clamp_does_not_disturb_split_ordinary_sequences() {
    let stream = "one\x1b[1;31mred\x1b[0m\r\n\x1b[2;5Hat-col-5\x1b[3;1Hé漢\x1b[1Ptail\x1b[4;1H\x1b[?2004hok";
    let mut whole = vt();
    whole.process(stream.as_bytes());
    for step in [1usize, 2, 3, 5] {
        let mut split = vt();
        feed_chunked(&mut split, stream.as_bytes(), step);
        for row in 0..5 {
            assert_eq!(split.row_text(row), whole.row_text(row), "step {step} row {row}");
        }
    }
    assert_eq!(whole.row_text(0).trim_end(), "onered");
    assert_eq!(whole.row_text(1).trim_end(), "    at-col-5");
    assert_eq!(whole.row_text(3).trim_end(), "ok");
}

// ── 5. OSC payloads are capped in bytes, not only in entry count ────────────────────────────

#[test]
fn window_title_is_truncated_on_a_char_boundary() {
    let mut vt = vt();
    let long = "é".repeat(10_000);
    vt.process(format!("\x1b]2;{long}\x07").as_bytes());
    let title = vt.window_title().expect("title");
    assert_eq!(title.len(), 256);
    assert!(title.chars().all(|c| c == 'é'));

    // 255 bytes of ASCII + a 2-byte char: the cut lands inside the char and must back off.
    vt.process(format!("\x1b]0;{}é\x1b\\", "a".repeat(255)).as_bytes());
    assert_eq!(vt.window_title(), Some("a".repeat(255).as_str()));

    vt.process(b"\x1b]2;short\x07");
    assert_eq!(vt.window_title(), Some("short"));
}

#[test]
fn osc8_oversized_uri_is_not_retained() {
    let mut vt = vt();
    let huge = format!("https://example.invalid/{}", "a".repeat(100_000));
    vt.process(format!("\x1b]8;id={};{huge}\x07x\x1b]8;;\x07", "i".repeat(5000)).as_bytes());
    assert!(vt.osc8_links().is_empty());
    assert!(vt.recent_escape_sequences().iter().all(|s| s.len() <= 4096));

    let ok = format!("https://example.invalid/{}", "a".repeat(2048 - 24));
    assert_eq!(ok.len(), 2048);
    vt.process(format!("\x1b]8;id={};{ok}\x07x\x1b]8;;\x07", "i".repeat(5000)).as_bytes());
    let links = vt.osc8_links();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].uri, ok);
    assert_eq!(links[0].id.as_deref().map(str::len), Some(256));
}

#[test]
fn notification_title_and_body_are_capped() {
    let mut vt = vt();
    vt.process(format!("\x1b]9;{}\x07", "é".repeat(50_000)).as_bytes());
    vt.process(
        format!("\x1b]777;notify;{};{}\x07", "t".repeat(50_000), "漢".repeat(50_000)).as_bytes(),
    );
    let notes = vt.notifications();
    assert_eq!(notes.len(), 2);
    for note in notes {
        assert!(note.title.len() <= 256, "title {}", note.title.len());
        assert!(note.body.len() <= 1024, "body {}", note.body.len());
        assert!(!note.body.is_empty());
    }
    assert_eq!(notes[0].body.len(), 1024);
    assert_eq!(notes[1].title.len(), 256);
    assert_eq!(notes[1].body.len(), 1023); // 341 three-byte chars
    assert!(vt.recent_escape_sequences().iter().all(|s| s.len() <= 4096));
}

// ── 6. Sixel decode must not accumulate beyond the maximum image dimension ──────────────────

#[test]
fn sixel_decode_memory_is_bounded_by_max_dimension() {
    let mut vt = vt();
    // 20 000 run-length tokens of 512 columns each: ~10 M columns in a single band.
    let wide = format!("\x1bPq{}\x1b\\", "!512~".repeat(20_000));
    // 200 000 band separators with a pixel each.
    let tall = format!("\x1bPq{}\x1b\\", "~-".repeat(200_000));
    // Plain (non run-length) columns.
    let plain = format!("\x1bPq{}\x1b\\", "~".repeat(2_000_000));

    for (name, payload) in [("wide", &wide), ("tall", &tall), ("plain", &plain)] {
        let allocated = allocated_during(|| vt.process(payload.as_bytes()));
        // Input copies are proportional to the payload; the decoded image is at most 1 MiB.
        let budget = payload.len() * 8 + 4 * 1024 * 1024;
        assert!(allocated < budget, "{name}: allocated {allocated} bytes for {} input", payload.len());
    }

    let placements = vt.graphic_placements();
    assert_eq!(placements.len(), 3);
    for p in placements {
        assert_eq!(p.protocol, GraphicProtocol::Sixel);
        assert!(p.pixel_width <= MAX_GRAPHIC_DIM_PX && p.pixel_height <= MAX_GRAPHIC_DIM_PX);
        assert_eq!(p.rgba_bytes.len(), usize::from(p.pixel_width) * usize::from(p.pixel_height) * 4);
    }
    assert_eq!(placements[0].pixel_width, MAX_GRAPHIC_DIM_PX);
    assert_eq!(placements[1].pixel_height, MAX_GRAPHIC_DIM_PX);
    assert_eq!(placements[2].pixel_width, MAX_GRAPHIC_DIM_PX);

    // An ordinary small image is unchanged: 3 columns, 2 bands.
    vt.clear_graphics();
    vt.process(b"\x1bPq~~~-~~~\x1b\\");
    let small = &vt.graphic_placements()[0];
    assert_eq!((small.pixel_width, small.pixel_height), (3, 12));
}

// ── 7. Tektronix mode state must not grow without limit ─────────────────────────────────────

#[test]
fn tektronix_segments_are_capped() {
    let mut vt = vt();
    let mut stream = b"\x1b[?38h\x1c".to_vec(); // enter Tek mode, point-plot
    stream.extend_from_slice(&b"@".repeat(70_000));
    stream.push(0x1e); // incremental plot, pen down, 70 000 steps
    stream.push(b'P');
    stream.extend_from_slice(&b"A".repeat(70_000));
    vt.process(&stream);
    assert_eq!(vt.tektronix().segments.len(), 65_536);
}

#[test]
fn tektronix_label_text_and_label_count_are_capped() {
    let mut vt = vt();
    let mut stream = b"\x1b[?38h\x1f".to_vec(); // enter Tek mode, alpha
    stream.extend_from_slice(&b"A".repeat(10_000));
    vt.process(&stream);
    let labels = &vt.tektronix().labels;
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].text.len(), 4096);

    // A new label at a new beam position for every character.
    let mut many = Vec::new();
    for i in 0..10_000u16 {
        many.push(0x1d);
        many.extend_from_slice(&encode_tek_10bit_point(i % 1024, i / 1024));
        many.push(0x1f);
        many.push(b'B');
    }
    vt.process(&many);
    assert_eq!(vt.tektronix().labels.len(), 4096);
}

#[test]
fn tektronix_csi_buffer_is_capped() {
    let mut tek = TektronixState::new();
    tek.process_bytes(b"\x1b[?38h\x1b[");
    let junk = vec![b'0'; 4 * 1024 * 1024];
    let allocated = allocated_during(|| tek.process_bytes(&junk));
    assert!(allocated <= 4096, "csi buffer allocated {allocated} bytes");

    // The sequence still terminates and Tek mode can still be left.
    tek.process_bytes(b"h\x1b[?38l");
    assert!(!tek.active && !tek.stream_capture);
}

#[test]
fn esc_followed_by_multibyte_char_does_not_panic() {
    // `ESC é` used to slice the lossy text inside the two-byte char (scan.rs `&after_esc[1..]`).
    for input in [
        "\x1bé".as_bytes(),
        "\x1b€[31m".as_bytes(),
        "a\x1b\u{1F600}b\x1b[0m".as_bytes(),
        b"\x1b\xff\xfe[1m".as_slice(),
    ] {
        let mut p = vt();
        p.process(input);
    }
}
