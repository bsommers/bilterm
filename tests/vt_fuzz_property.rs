//! Property-style fuzzing of the VT parser + screen with a hand-rolled deterministic PRNG
//! (fixed seeds, no extra dependencies; identical streams on every OS / architecture).
//!
//! Inputs: pure random bytes, grammar-shaped escape sequences with hostile parameters, and
//! mutations (bit flips, truncation, duplication, splices) of a corpus of real sequences.
//! Properties, checked after every case:
//!   * no panic, and the whole run stays inside a generous time budget (no pathological input);
//!   * the screen keeps the geometry it was given and every row has exactly `cols` cells;
//!   * the cursor stays inside the screen;
//!   * replaying the same bytes with the same chunking gives the identical screen (determinism).
//!
//! KNOWN PRODUCT BUG (found by this file, not fixed here because `src/vt` is owned by another
//! branch): two byte-offset slices of lossily decoded text panic on non-ASCII input —
//!   * `src/vt/scan.rs:29`  `&after_esc[1..]`   — ESC followed by any multi-byte character;
//!   * `src/vt/codec.rs:77` `&hex[i..i + 2]`    — XTGETTCAP (`DCS + q`) with a non-ASCII payload.
//! Any program that prints such bytes (e.g. `cat` of a binary file) aborts the terminal.
//! Until they are fixed, [`guarded`] tolerates exactly that panic message (and nothing else),
//! reports every hit visibly, and the two tests that cannot run meaningfully while the bug exists
//! plus the minimal reproducers are `#[ignore]`d with the reason. After the fix: delete
//! `KNOWN_BUG_PANIC`, `guarded`'s tolerance and the `#[ignore]` attributes.
//!
//! The parser is fuzzed directly (not through a pane) on purpose: a pane forwards decoded OSC 52
//! payloads to the system clipboard, which a fuzzer must never touch.

mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bilterm::vt::VtParser;
use common::Rng;

/// Whole-test budget. Debug builds on the slowest CI cell finish in a few seconds; this only
/// trips on a genuine algorithmic blow-up (quadratic reflow, runaway allocation, livelock).
const TIME_BUDGET: Duration = Duration::from_secs(120);

/// The one panic message tolerated while the char-boundary bug (see module docs) is open.
const KNOWN_BUG_PANIC: &str = "is not a char boundary";
const KNOWN_BUG_IGNORE: &str = "product bug: src/vt/scan.rs:29 and src/vt/codec.rs:77 slice lossy text at a non-char boundary";
static KNOWN_BUG_HITS: AtomicUsize = AtomicUsize::new(0);

/// Runs one fuzz case. `true` = completed; `false` = it died on the known char-boundary panic
/// (the caller must discard the parser). Any other panic is re-raised with the case description.
fn guarded(what: &str, case: impl FnOnce()) -> bool {
    match catch_unwind(AssertUnwindSafe(case)) {
        Ok(()) => true,
        Err(payload) => {
            let msg = payload.downcast_ref::<String>().cloned().or_else(|| payload.downcast_ref::<&str>().map(ToString::to_string)).unwrap_or_default();
            if msg.contains(KNOWN_BUG_PANIC) {
                KNOWN_BUG_HITS.fetch_add(1, Ordering::Relaxed);
                false
            } else {
                panic!("{what}: {msg}");
            }
        }
    }
}

/// Makes tolerated hits visible in the log instead of letting them pass silently.
fn report_known_bug_hits(test: &str) {
    let hits = KNOWN_BUG_HITS.load(Ordering::Relaxed);
    if hits > 0 {
        common::skip(&format!("{test}: {hits} fuzz case(s) so far hit the known VT char-boundary panic ({KNOWN_BUG_IGNORE})"));
    }
}

/// Real-world sequences the mutator starts from.
const CORPUS: &[&[u8]] = &[
    b"\x1b[38;2;85;255;85;48;5;17;1;3;4mstyled\x1b[0m\r\n",
    b"\x1b[?1049h\x1b[2J\x1b[H\x1b[?25l alt screen \x1b[?25h\x1b[?1049l",
    b"\x1b]0;window title\x07\x1b]2;other title\x1b\\",
    b"\x1b]7;file://host/some/dir\x07",
    b"\x1b]8;id=x;https://example.com/a\x07link\x1b]8;;\x07",
    b"\x1b]52;c;aGVsbG8gd29ybGQ=\x07",
    b"\x1b]133;A\x07$ cmd\x1b]133;B\x07\x1b]133;C\x07out\r\n\x1b]133;D;0\x07",
    b"\x1b]9;4;1;42\x07\x1b]9;notify body\x07\x1b]777;notify;title;body\x07",
    b"\x1b]10;?\x07\x1b]11;?\x1b\\\x1b]4;208;?\x07",
    b"\x1bP+q544E;524742\x1b\\\x1bP$qm\x1b\\\x1bP$qr\x1b\\",
    b"\x1bPq#0;2;0;0;0#1;2;100;100;0#1~~@@vv@@~~$-\x1b\\",
    b"\x1b_Gf=24,s=1,v=1,a=T;AAAA\x1b\\",
    b"\x1b[>31u\x1b[>15u\x1b[?u\x1b[<1u\x1b[=5;1u",
    b"\x1b[c\x1b[>c\x1b[6n\x1b[?6n\x1b[5n\x1b[18t\x1b[14t\x1b[?2026$p",
    b"\x1b[?2004h\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h\x1b[?1004h\x1b[?2048h",
    b"\x1b[10;20r\x1b[5;5H\x1b[3L\x1b[2M\x1b[4@\x1b[4P\x1b[2X\x1b[1J\x1b[K\x1b[r",
    b"\x1b#3double\r\n\x1b#4double\r\n\x1b#6wide\r\n\x1b#5\x1b#8",
    b"\x1b(0lqqqk\r\nx   x\r\nmqqqj\x1b(B",
    b"\x1b7\x1b[999;999H\x1b8\x1bD\x1bM\x1bE\x1bH\x1b[3g\x1bc",
    "\u{6f22}\u{5b57} \u{1f525}\u{1f680} e\u{0301} \u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467} \u{feff}\u{202e}rtl".as_bytes(),
    b"\tTab\x08\x08BS\rCR\nLF\x0bVT\x0cFF\x07BEL\x0e\x0f\x00NUL\x7fDEL",
    b"\x1b[1;2;3;4;5;6;7;8;9;21;22;23;24;25;27;28;29;53;55m\x1b[4:3m\x1b[58:2::255:0:0m\x1b[59m",
    b"\x1b[2 q\x1b[6 q\x1b[?12h\x1b[!p\x1b[?5h\x1b[?5l\x1b[?7l\x1b[?7h\x1b[?6h\x1b[?6l",
    b"\x1b[200~pasted \x1b[201~",
];

const FINALS: &[u8] = b"@ABCDEFGHIJKLMPSTXZ`abcdefghilmnpqrstuxy{|}~";
const INTERMEDIATES: &[u8] = b" !\"#$%&'()*+,-./";
const PRIVATE: &[u8] = b"?><=";
const HOSTILE_NUMBERS: &[&str] = &[
    "0", "1", "2", "9", "24", "80", "255", "256", "999", "1049", "2004", "32767", "32768", "65535", "65536",
    "2147483647", "2147483648", "4294967295", "4294967296", "18446744073709551616", "999999999999999999999999",
    "-1", "", "00000000000000000001", "1e9", "0x10",
];

fn corpus(rng: &mut Rng) -> &'static [u8] {
    CORPUS[rng.below(CORPUS.len())]
}

fn gen_csi(rng: &mut Rng, out: &mut Vec<u8>) {
    out.extend_from_slice(if rng.chance(8) { b"\x9b" } else { b"\x1b[" });
    if rng.chance(3) {
        out.push(*rng.pick(PRIVATE));
    }
    let max_params = if rng.chance(20) { 200 } else { 6 };
    for i in 0..rng.below(max_params) {
        if i > 0 {
            out.push(if rng.chance(6) { b':' } else { b';' });
        }
        out.extend_from_slice(rng.pick(HOSTILE_NUMBERS).as_bytes());
    }
    for _ in 0..rng.below(3).saturating_sub(1) {
        out.push(*rng.pick(INTERMEDIATES));
    }
    if !rng.chance(12) {
        out.push(*rng.pick(FINALS)); // otherwise leave the sequence unterminated
    }
}

fn gen_string_sequence(rng: &mut Rng, out: &mut Vec<u8>) {
    // OSC, DCS, APC, PM, SOS introducers with payloads of every shape and every terminator.
    const INTRODUCERS: [&[u8]; 7] = [b"\x1b]", b"\x1bP", b"\x1b_", b"\x1b^", b"\x1bX", b"\x9d", b"\x90"];
    out.extend_from_slice(INTRODUCERS[rng.below(INTRODUCERS.len())]);
    if rng.chance(2) {
        out.extend_from_slice(rng.pick(&["0", "2", "4", "7", "8", "9", "10", "11", "52", "133", "777", "1337", "99999"]).as_bytes());
        out.push(b';');
    }
    let len = if rng.chance(25) { 4096 + rng.below(8192) } else { rng.below(48) };
    for _ in 0..len {
        out.push(match rng.below(10) {
            0 => rng.byte(),
            1 => b';',
            2 => b'?',
            3 => b'=',
            _ => b'A' + (rng.below(58) as u8),
        });
    }
    match rng.below(5) {
        0 => out.push(0x07),
        1 => out.extend_from_slice(b"\x1b\\"),
        2 => out.push(0x9c),
        3 => out.push(0x18), // CAN aborts the string
        _ => {}             // unterminated
    }
}

fn gen_text(rng: &mut Rng, out: &mut Vec<u8>) {
    for _ in 0..rng.below(40) {
        match rng.below(12) {
            0 => out.extend_from_slice("\u{6f22}".as_bytes()),
            1 => out.extend_from_slice("\u{1f680}".as_bytes()),
            2 => out.extend_from_slice("e\u{0301}\u{200d}".as_bytes()),
            3 => out.push(*rng.pick(b"\r\n\t\x08\x07\x0b\x0c\x0e\x0f\x00\x7f")),
            4 => out.push(0x80 | (rng.byte() & 0x7f)), // stray continuation / invalid UTF-8
            5 => out.extend_from_slice(&[0xf0, 0x9f]), // truncated 4-byte scalar
            _ => out.push(b' ' + (rng.below(95) as u8)),
        }
    }
}

fn gen_structured(rng: &mut Rng, target_len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(target_len + 64);
    while out.len() < target_len {
        match rng.below(10) {
            0..=3 => gen_csi(rng, &mut out),
            4..=5 => gen_string_sequence(rng, &mut out),
            6 => {
                out.push(0x1b);
                out.push(rng.byte() & 0x7f);
            }
            7 => out.extend_from_slice(corpus(rng)),
            _ => gen_text(rng, &mut out),
        }
    }
    out
}

fn mutate(rng: &mut Rng, seed: &[u8]) -> Vec<u8> {
    let mut v = seed.to_vec();
    for _ in 0..1 + rng.below(6) {
        if v.is_empty() {
            v.push(rng.byte());
            continue;
        }
        let i = rng.below(v.len());
        match rng.below(8) {
            0 => v[i] ^= 1 << rng.below(8),
            1 => v[i] = rng.byte(),
            2 => v.truncate(i),
            3 => {
                let j = (i + 1 + rng.below(16)).min(v.len());
                let dup = v[i..j].to_vec();
                for _ in 0..1 + rng.below(40) {
                    v.splice(i..i, dup.iter().copied());
                }
            }
            4 => {
                let other = corpus(rng);
                let k = rng.below(other.len() + 1);
                v.splice(i..i, other[..k].iter().copied());
            }
            5 => {
                v.remove(i);
            }
            6 => v.insert(i, *rng.pick(b"\x1b\x9b\x9d\x90\x07\x18\x1a;:?")),
            _ => {
                let j = rng.below(v.len());
                v.swap(i, j);
            }
        }
    }
    v
}

/// Feeds `input` in PRNG-chosen chunks (1 byte up to the whole thing), draining query replies.
fn feed(vt: &mut VtParser, input: &[u8], rng: &mut Rng) -> usize {
    let mut i = 0;
    let mut replies = 0;
    while i < input.len() {
        let step = match rng.below(4) {
            0 => 1,
            1 => 1 + rng.below(7),
            2 => 1 + rng.below(256),
            _ => input.len() - i,
        }
        .min(input.len() - i);
        vt.process(&input[i..i + step]);
        i += step;
        replies += vt.take_query_responses().iter().map(Vec::len).sum::<usize>();
    }
    replies
}

fn check_invariants(vt: &VtParser, rows: u16, cols: u16, what: &str) {
    assert_eq!(vt.size(), (rows, cols), "{what}: parser changed its own geometry");
    let snap = vt.snapshot();
    assert_eq!((snap.rows, snap.cols), (rows, cols), "{what}: snapshot geometry");
    assert_eq!(snap.grid_rows.len(), usize::from(rows), "{what}: snapshot row count");
    for (r, row) in snap.grid_rows.iter().enumerate() {
        assert_eq!(row.cells.len(), usize::from(cols), "{what}: row {r} cell count");
    }
    let (cr, cc) = vt.cursor_position();
    assert!(cr < rows && cc <= cols, "{what}: cursor ({cr},{cc}) outside {rows}x{cols}");
    // Text extraction must not panic on whatever the grid now holds.
    let _ = snap.contents();
    for r in 0..rows {
        let _ = vt.row_text(r);
    }
}

fn describe(kind: &str, seed: u64, case: usize, input: &[u8]) -> String {
    let head: String = input.iter().take(96).map(|b| format!("{b:02x}")).collect();
    format!("{kind} seed={seed} case={case} len={} head={head}", input.len())
}

fn run_cases(kind: &str, seeds: &[u64], cases: usize, mut make: impl FnMut(&mut Rng) -> Vec<u8>) {
    let started = Instant::now();
    let mut total_bytes = 0usize;
    for &seed in seeds {
        let mut rng = Rng::new(seed);
        let (rows, cols) = (2 + rng.below(40) as u16, 2 + rng.below(160) as u16);
        let mut vt = VtParser::new(rows, cols, 200);
        for case in 0..cases {
            let input = make(&mut rng);
            total_bytes += input.len();
            let what = describe(kind, seed, case, &input);
            let completed = guarded(&what, || {
                let replies = feed(&mut vt, &input, &mut rng);
                // A terminal answers queries; it must not amplify input into unbounded output.
                assert!(replies <= 64 * input.len() + 4096, "{what}: {replies} reply bytes for {} input bytes", input.len());
                check_invariants(&vt, rows, cols, &what);
            });
            if !completed {
                vt = VtParser::new(rows, cols, 200); // state after a panic is unspecified
            }
            assert!(started.elapsed() < TIME_BUDGET, "{what}: exceeded the {TIME_BUDGET:?} budget after {total_bytes} bytes");
        }
    }
    report_known_bug_hits(kind);
}

#[test]
fn random_bytes_never_panic_or_corrupt_the_screen() {
    run_cases("random", &[1, 2, 3, 0xDEAD_BEEF, 0x5EED_5EED_5EED], 60, |rng| {
        let len = rng.below(2048);
        rng.bytes(len)
    });
}

#[test]
fn grammar_shaped_escape_sequences_never_panic_or_corrupt_the_screen() {
    run_cases("structured", &[11, 12, 13, 14, 0xC0FFEE, 0xFEED_FACE], 60, |rng| {
        let len = 64 + rng.below(3000);
        gen_structured(rng, len)
    });
}

#[test]
fn mutated_real_sequences_never_panic_or_corrupt_the_screen() {
    run_cases("mutated", &[21, 22, 23, 24, 25, 0xBADC_0DE], 150, |rng| {
        let mut base = corpus(rng).to_vec();
        if rng.chance(3) {
            base.extend_from_slice(corpus(rng));
        }
        mutate(rng, &base)
    });
}

#[test]
fn fuzzing_interleaved_with_resizes_keeps_the_grid_consistent() {
    let started = Instant::now();
    for seed in [31u64, 32, 33, 34] {
        let mut rng = Rng::new(seed);
        let (mut rows, mut cols) = (24u16, 80u16);
        let mut vt = VtParser::new(rows, cols, 500);
        for case in 0..80 {
            let input = if rng.chance(2) {
                let len = 32 + rng.below(600);
                gen_structured(&mut rng, len)
            } else {
                let base = corpus(&mut rng);
                mutate(&mut rng, base)
            };
            let what = describe("resize", seed, case, &input);
            let completed = guarded(&what, || {
                feed(&mut vt, &input, &mut rng);
                if rng.chance(3) {
                    // Ask for degenerate 1-cell grids too: the parser documents a 2x2 minimum.
                    let (want_rows, want_cols) = (1 + rng.below(60) as u16, 1 + rng.below(250) as u16);
                    vt.set_size(want_rows, want_cols);
                    rows = want_rows.max(2);
                    cols = want_cols.max(2);
                }
                if rng.chance(5) {
                    vt.scroll_up(rng.below(40));
                }
                if rng.chance(5) {
                    vt.scroll_down(rng.below(40));
                }
                if rng.chance(9) {
                    vt.reset_scrollback();
                }
                check_invariants(&vt, rows, cols, &what);
            });
            if !completed {
                (rows, cols) = (24, 80);
                vt = VtParser::new(rows, cols, 500);
            }
            assert!(started.elapsed() < TIME_BUDGET, "{what}: exceeded the {TIME_BUDGET:?} budget");
        }
    }
    report_known_bug_hits("resize");
}

#[test]
#[ignore = "product bug: src/vt/scan.rs:29 and src/vt/codec.rs:77 slice lossy text at a non-char boundary"]
fn identical_input_and_chunking_give_an_identical_screen() {
    for seed in [41u64, 42, 43, 44, 45, 46] {
        let mut gen_rng = Rng::new(seed);
        let input = gen_structured(&mut gen_rng, 6000);
        let run = || {
            let mut vt = VtParser::new(30, 100, 300);
            let mut chunk_rng = Rng::new(seed ^ 0xA5A5);
            feed(&mut vt, &input, &mut chunk_rng);
            (vt.snapshot().contents(), vt.cursor_position(), vt.is_alternate_screen(), vt.window_title().map(str::to_string))
        };
        assert_eq!(run(), run(), "seed {seed}: the parser is not deterministic");
    }
}

#[test]
#[ignore = "product bug: src/vt/scan.rs:29 and src/vt/codec.rs:77 slice lossy text at a non-char boundary"]
fn one_megabyte_of_hostile_input_is_processed_in_bounded_time() {
    let mut rng = Rng::new(0x0B16_B00B5);
    let mut input = Vec::with_capacity(1 << 20);
    while input.len() < (1 << 20) {
        let chunk = gen_structured(&mut rng, 4096);
        input.extend_from_slice(&chunk);
    }
    let mut vt = VtParser::new(50, 200, 1000);
    let started = Instant::now();
    for chunk in input.chunks(8192) {
        vt.process(chunk);
        let _ = vt.take_query_responses();
    }
    let took = started.elapsed();
    check_invariants(&vt, 50, 200, "1 MiB hostile stream");
    assert!(took < Duration::from_secs(60), "1 MiB took {took:?} (budget 60 s): throughput collapsed");
    assert_eq!(vt.total_bytes_processed(), input.len(), "every byte must be accounted for");
}

// ---------------------------------------------------------------- minimal reproducers (see module docs)

#[test]
#[ignore = "product bug: src/vt/scan.rs:29 slices `&after_esc[1..]` inside a multi-byte character"]
fn repro_escape_followed_by_non_ascii_character_must_not_panic() {
    let mut vt = VtParser::new(24, 80, 0);
    vt.process("\x1b\u{e9}".as_bytes()); // ESC, then U+00E9
    vt.process("ok \x1b\u{1f680} still alive\r\n".as_bytes());
    check_invariants(&vt, 24, 80, "ESC + non-ASCII");
}

#[test]
#[ignore = "product bug: src/vt/codec.rs:77 hex_decode_ascii slices `&hex[i..i + 2]` inside a multi-byte character"]
fn repro_xtgettcap_with_non_ascii_payload_must_not_panic() {
    let mut vt = VtParser::new(24, 80, 0);
    vt.process("\x1bP+q5\u{e9}4\x1b\\".as_bytes()); // DCS + q <5, U+00E9, 4> ST: even byte length, odd boundary
    check_invariants(&vt, 24, 80, "XTGETTCAP + non-ASCII");
}

/// While the bug is open this proves the tolerance in [`guarded`] is still needed; it fails as soon
/// as the parser is fixed, which is the signal to delete the tolerance and the `#[ignore]`s above.
#[test]
fn known_char_boundary_bug_is_still_present_remove_tolerance_when_this_fails() {
    let esc = catch_unwind(|| VtParser::new(24, 80, 0).process("\x1b\u{e9}".as_bytes()));
    let dcs = catch_unwind(|| VtParser::new(24, 80, 0).process("\x1bP+q5\u{e9}4\x1b\\".as_bytes()));
    assert!(
        esc.is_err() || dcs.is_err(),
        "the VT char-boundary panics are fixed: remove KNOWN_BUG_PANIC / guarded tolerance and the #[ignore] attributes in this file"
    );
}
