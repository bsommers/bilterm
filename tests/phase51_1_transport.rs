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

mod transport {
    use bilterm::pty::PtyConfig;
    use bilterm::remote::RemoteHostProfile;
    use bilterm::tmux::link::{DriverExit, LinkConnector, LinkDriver, LinkTarget};
    use bilterm::tmux::transport::{PtyLinkConnector, PtyLinkDriver};

    fn target(dest: &str) -> LinkTarget {
        LinkTarget {
            profile: RemoteHostProfile::new("t", dest),
            session: "main".into(),
            tags: vec![],
        }
    }

    #[cfg(unix)]
    mod pty {
        use super::*;
        use bilterm::pty::PtySession;
        use std::time::{Duration, Instant};

        fn sh(script: &str) -> PtyConfig {
            PtyConfig {
                program: "/bin/sh".into(),
                args: vec!["-c".into(), script.into()],
                rows: 24,
                cols: 80,
                ..PtyConfig::default()
            }
        }

        #[test]
        fn driver_reports_raw_then_control_lines_then_exit_code() {
            let (mut control, mut driver) = PtyLinkDriver::spawn_named(
                "main",
                sh("printf 'login: '; printf '\\033P1000p'; printf '%%begin 1 1 0\\n%%end 1 1 0\\n'; exit 3"),
            )
            .expect("spawn sh");
            assert_eq!(control.session_name, "main");
            let mut raw = Vec::new();
            let mut starts = 0;
            let mut lines = Vec::new();
            let mut exit = None;
            let mut lines_at_exit = 0;
            let deadline = Instant::now() + Duration::from_secs(5);
            while exit.is_none() && Instant::now() < deadline {
                let p = driver.pump();
                raw.extend(p.raw_output);
                if p.control_started {
                    starts += 1;
                }
                while let Ok(l) = control.line_rx.try_recv() {
                    lines.push(l);
                }
                if p.exited.is_some() {
                    lines_at_exit = lines.len();
                    exit = p.exited;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(String::from_utf8_lossy(&raw).contains("login: "), "{raw:?}");
            assert_eq!(starts, 1);
            assert_eq!(lines, vec!["%begin 1 1 0".to_string(), "%end 1 1 0".to_string()]);
            assert_eq!(lines_at_exit, 2, "lines delivered before the exit");
            assert_eq!(exit, Some(DriverExit { code: Some(3) }));
            // exactly once
            for _ in 0..3 {
                assert!(driver.pump().exited.is_none());
            }
        }

        #[test]
        fn driver_writes_commands_and_raw_input() {
            let (control, mut driver) = PtyLinkDriver::spawn_named(
                "main",
                sh("read x; echo got1:$x; read y; echo got2:$y; sleep 1"),
            )
            .expect("spawn sh");
            control.cmd_tx.send("abc".to_string()).unwrap();
            let mut raw = Vec::new();
            let mut wrote_raw = false;
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                raw.extend(driver.pump().raw_output);
                let text = String::from_utf8_lossy(&raw).into_owned();
                if text.contains("got1:abc") && !wrote_raw {
                    driver.write_raw(b"def\n").expect("write_raw");
                    wrote_raw = true;
                }
                if text.contains("got2:def") {
                    break;
                }
                assert!(Instant::now() < deadline, "no echo: {text:?}");
                std::thread::sleep(Duration::from_millis(5));
            }
            driver.kill();
        }

        #[test]
        fn driver_kill_is_safe() {
            let (_control, mut driver) =
                PtyLinkDriver::spawn_named("main", sh("sleep 30")).expect("spawn sh");
            driver.kill();
            let started = Instant::now();
            for _ in 0..3 {
                let p = driver.pump();
                assert!(p.exited.is_none());
                assert!(p.raw_output.is_empty());
            }
            driver.write_raw(b"x").expect("write after kill is a no-op");
            driver.resize(30, 100);
            driver.kill();
            assert!(started.elapsed() < Duration::from_secs(1));
        }

        #[test]
        fn exit_code_respects_patience() {
            let running = PtySession::spawn(sh("sleep 5")).expect("spawn");
            let t0 = Instant::now();
            assert_eq!(running.exit_code(Duration::from_millis(30)), None);
            assert!(t0.elapsed() < Duration::from_millis(300), "{:?}", t0.elapsed());
            drop(running);

            let done = PtySession::spawn(sh("exit 7")).expect("spawn");
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut code = None;
            while code.is_none() && Instant::now() < deadline {
                code = done.exit_code(Duration::from_millis(100));
            }
            assert_eq!(code, Some(7));
            // a reaped child keeps reporting its code
            assert_eq!(done.exit_code(Duration::from_millis(10)), Some(7));
        }

        #[test]
        fn spawn_failure_is_an_error_with_a_reason() {
            let cfg = PtyConfig {
                program: "/nonexistent/bilterm-no-such-program".into(),
                ..PtyConfig::default()
            };
            let err = PtyLinkDriver::spawn_named("main", cfg).err().expect("must fail");
            assert!(!err.to_string().is_empty());
        }

        #[test]
        fn connector_builds_ssh_argv_and_reports_spawn_errors() {
            let mut c = PtyLinkConnector::default();
            // An unsafe destination is rejected before any process exists.
            let err = c
                .connect(&target("-oProxyCommand=x"), (24, 80))
                .err()
                .expect("unsafe destination");
            assert!(!err.to_string().is_empty());
            // A valid destination either fails with a reason (no ssh) or yields a driver to kill.
            match c.connect(&target("bilterm-test@invalid.invalid"), (24, 80)) {
                Err(e) => assert!(!e.to_string().is_empty()),
                Ok(mut t) => {
                    assert_eq!(t.control.session_name, "main");
                    t.driver.as_mut().expect("driver").kill();
                }
            }
        }
    }
}
