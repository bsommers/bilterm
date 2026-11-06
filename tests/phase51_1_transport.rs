//! Phase 51.1 plan 03: control-line splitter and PTY-backed link driver.
//! No ssh and no tmux are started; PTY tests run `/bin/sh` only (unix).

mod splitter {
    use bilterm::tmux::link::{CONTROL_LINE_MAX, DIAG_LINE_CAP};
    use bilterm::tmux::splitter::{diag_lines, ControlLineSplitter, SplitOut};

    fn raw_bytes(outs: &[SplitOut]) -> Vec<u8> {
        outs.iter()
            .filter_map(|o| match o {
                SplitOut::Raw(b) => Some(b.clone()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    fn lines(outs: &[SplitOut]) -> Vec<String> {
        outs.iter()
            .filter_map(|o| match o {
                SplitOut::Line(l) => Some(l.clone()),
                _ => None,
            })
            .collect()
    }

    fn count(outs: &[SplitOut], want: &SplitOut) -> usize {
        outs.iter().filter(|o| *o == want).count()
    }

    #[test]
    fn raw_before_sentinel() {
        let mut sp = ControlLineSplitter::new();
        let out = sp.push(b"user@box's password: ");
        assert_eq!(out, vec![SplitOut::Raw(b"user@box's password: ".to_vec())]);
        assert!(!sp.in_control());
    }

    #[test]
    fn sentinel_split_at_every_boundary() {
        let data: &[u8] = b"hello\x1bP1000p%begin 1 1 0\r\n";
        for cut in 0..=data.len() {
            let mut sp = ControlLineSplitter::new();
            let mut all = sp.push(&data[..cut]);
            all.extend(sp.push(&data[cut..]));
            assert_eq!(raw_bytes(&all), b"hello", "cut {cut}");
            assert_eq!(count(&all, &SplitOut::ControlStart), 1, "cut {cut}");
            assert_eq!(lines(&all), vec!["%begin 1 1 0".to_string()], "cut {cut}");
            assert!(sp.in_control());
        }
    }

    #[test]
    fn sentinel_split_byte_by_byte() {
        let data: &[u8] = b"a\x1bP1000p%x\n";
        let mut sp = ControlLineSplitter::new();
        let mut all = Vec::new();
        for b in data {
            all.extend(sp.push(&[*b]));
        }
        assert_eq!(raw_bytes(&all), b"a");
        assert_eq!(lines(&all), vec!["%x".to_string()]);
    }

    #[test]
    fn failed_escape_match_is_released_as_raw() {
        let mut sp = ControlLineSplitter::new();
        let mut all = sp.push(b"x\x1bP10");
        all.extend(sp.push(b"1 y\x1b[0m"));
        all.extend(sp.flush_partial());
        assert_eq!(raw_bytes(&all), b"x\x1bP101 y\x1b[0m");
        assert!(!sp.in_control());
    }

    #[test]
    fn crlf_and_double_cr_lines() {
        for tail in ["\r\n", "\r\r\n", "\n"] {
            let mut sp = ControlLineSplitter::new();
            sp.push(b"\x1bP1000p");
            let out = sp.push(format!("%end 1 1 0{tail}").as_bytes());
            assert_eq!(lines(&out), vec!["%end 1 1 0".to_string()], "tail {tail:?}");
        }
    }

    #[test]
    fn partial_line_waits() {
        let mut sp = ControlLineSplitter::new();
        sp.push(b"\x1bP1000p");
        assert!(lines(&sp.push(b"%output %1 ")).is_empty());
        assert!(lines(&sp.push(b"abc")).is_empty());
        let out = sp.push(b"def\r\nnext");
        assert_eq!(lines(&out), vec!["%output %1 abcdef".to_string()]);
        let flushed = sp.flush_partial();
        assert_eq!(flushed, vec![SplitOut::Line("next".to_string())]);
        assert!(sp.flush_partial().is_empty());
    }

    #[test]
    fn st_ends_control_mode() {
        let mut sp = ControlLineSplitter::new();
        let out = sp.push(b"\x1bP1000p%exit\n\x1b\\bye$ ");
        assert_eq!(lines(&out), vec!["%exit".to_string()]);
        assert_eq!(count(&out, &SplitOut::ControlEnd), 1);
        assert_eq!(raw_bytes(&out), b"bye$ ");
        assert!(!sp.in_control());
        let again = sp.push(b"\x1bP1000p%begin 2 2 0\n");
        assert_eq!(count(&again, &SplitOut::ControlStart), 1);
        assert!(sp.in_control());
        assert_eq!(lines(&again), vec!["%begin 2 2 0".to_string()]);
    }

    #[test]
    fn st_split_across_chunks() {
        let mut sp = ControlLineSplitter::new();
        sp.push(b"\x1bP1000p%exit\n\x1b");
        assert!(sp.in_control());
        let out = sp.push(b"\\rest");
        assert_eq!(count(&out, &SplitOut::ControlEnd), 1);
        assert_eq!(raw_bytes(&out), b"rest");
    }

    #[test]
    fn fallback_start_on_begin_or_output() {
        for first in ["%output %1 x\r\n", "%begin 1 1 0\r\n"] {
            let mut sp = ControlLineSplitter::new();
            let mut all = sp.push(b"motd line\r\n");
            all.extend(sp.push(first.as_bytes()));
            assert_eq!(count(&all, &SplitOut::ControlStart), 1);
            assert_eq!(lines(&all), vec![first.trim_end().to_string()]);
            assert_eq!(raw_bytes(&all), b"motd line\r\n");
            assert!(sp.in_control());
        }
        // A percent sign mid-line or a different word is plain text.
        let mut sp = ControlLineSplitter::new();
        let out = sp.push(b"50%begin \n%other\n");
        assert!(!sp.in_control());
        assert_eq!(raw_bytes(&out), b"50%begin \n%other\n");
    }

    #[test]
    fn invalid_utf8_is_lossy_and_nonfatal() {
        let mut sp = ControlLineSplitter::new();
        sp.push(b"\x1bP1000p");
        let out = sp.push(b"%output %1 \xff\xfe ok\n%output %1 next\n");
        let ls = lines(&out);
        assert_eq!(ls.len(), 2);
        assert!(ls[0].contains("ok"));
        assert_eq!(ls[1], "%output %1 next");
    }

    #[test]
    fn overlong_line_is_dropped_and_recovers() {
        let mut sp = ControlLineSplitter::new();
        sp.push(b"\x1bP1000p");
        let mut all = Vec::new();
        let chunk = vec![b'a'; 64 * 1024];
        let mut sent = 0usize;
        let target = CONTROL_LINE_MAX + 10;
        let mut max_held = 0usize;
        while sent < target {
            let n = chunk.len().min(target - sent);
            all.extend(sp.push(&chunk[..n]));
            sent += n;
            max_held = max_held.max(sp.buffered_len());
        }
        all.extend(sp.push(b"\r\n%end 1 1 0\r\n"));
        let dropped: Vec<usize> = all
            .iter()
            .filter_map(|o| match o {
                SplitOut::Dropped(n) => Some(*n),
                _ => None,
            })
            .collect();
        assert_eq!(dropped.len(), 1);
        assert!(dropped[0] > CONTROL_LINE_MAX);
        assert_eq!(lines(&all), vec!["%end 1 1 0".to_string()]);
        assert!(max_held <= CONTROL_LINE_MAX + 1, "held {max_held}");
    }

    #[test]
    fn endless_output_without_newline_stays_bounded() {
        let mut sp = ControlLineSplitter::new();
        sp.push(b"\x1bP1000p");
        let chunk = vec![b'z'; 100_000];
        for _ in 0..20 {
            sp.push(&chunk);
            assert!(sp.buffered_len() <= CONTROL_LINE_MAX + 1);
        }
        assert_eq!(sp.flush_partial().len(), 1);
    }

    #[test]
    fn diag_lines_sanitise_and_cap() {
        let raw = b"\x1b[31mssh: connect to host x port 22: Connection refused\x1b[0m\r\n\r\n\
                    \x1b]0;title\x07client_loop: send disconnect: Broken pipe\r\n   \n";
        let ls = diag_lines(raw);
        assert_eq!(ls.len(), 2, "{ls:?}");
        assert_eq!(ls[0], "ssh: connect to host x port 22: Connection refused");
        assert!(ls[1].contains("client_loop: send disconnect: Broken pipe"));
        assert!(ls.iter().all(|l| !l.contains('\u{1b}')));
        let bidi = diag_lines("ab\u{202e}cd\u{200b}\n".as_bytes());
        assert_eq!(bidi, vec!["abcd".to_string()]);
        let long = vec![b'q'; 500];
        let capped = diag_lines(&long);
        assert_eq!(capped.len(), 1);
        assert!(capped[0].chars().count() <= DIAG_LINE_CAP);
        assert!(diag_lines(b"\r\n\n\r").is_empty());
    }

    #[test]
    fn byte_fuzz_never_panics_and_in_control_is_consistent() {
        let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as u32
        };
        let alphabet: &[u8] = b"\x1bP1000p\\%begin output \r\n\xff\x07abc";
        let mut sp = ControlLineSplitter::new();
        let mut expect_control = false;
        for _ in 0..2000 {
            let len = (next() % 40) as usize;
            let chunk: Vec<u8> = (0..len)
                .map(|_| {
                    if next() % 4 == 0 {
                        (next() & 0xff) as u8
                    } else {
                        alphabet[(next() as usize) % alphabet.len()]
                    }
                })
                .collect();
            for o in sp.push(&chunk) {
                match o {
                    SplitOut::ControlStart => expect_control = true,
                    SplitOut::ControlEnd => expect_control = false,
                    _ => {}
                }
            }
            assert_eq!(sp.in_control(), expect_control);
            assert!(sp.buffered_len() <= CONTROL_LINE_MAX + 1);
        }
        let _ = sp.flush_partial();
    }
}
