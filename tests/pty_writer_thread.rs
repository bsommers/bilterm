//! RESP-01: PTY writes never block the caller; real-PTY evidence for the writer thread.
//!
//! The pure queue semantics (order, limit, error, shutdown) are unit-tested with fake sinks in
//! `src/pty/writer_tests.rs`; these tests prove the same properties against a real kernel PTY.

#![cfg(unix)]

use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use bilterm::pane::CompositePane;
use bilterm::pty::writer::MAX_QUEUED_INPUT_BYTES;
use bilterm::pty::{PtyConfig, PtyEvent, PtySession};

/// Generous bound for "this must eventually happen"; passing never depends on it being short.
const LONG: Duration = Duration::from_secs(30);

fn sh(script: &str) -> PtyConfig {
    PtyConfig { program: "sh".into(), args: vec!["-c".into(), script.into()], rows: 24, cols: 80, ..PtyConfig::default() }
}

fn wait_for(mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + LONG;
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

/// A child that never reads its tty. The kernel tty buffer holds a few KiB, so a synchronous
/// 1 MiB write could never complete: before the writer thread this call hung the caller forever.
#[test]
fn write_to_a_child_that_does_not_read_returns_to_the_caller() {
    let session = PtySession::spawn(sh("exec sleep 600")).unwrap();
    let (done_tx, done_rx) = channel();
    let worker = std::thread::spawn(move || {
        let big = vec![b'x'; 1024 * 1024];
        let r = session.write_all(&big);
        let pending = session.pending_input_bytes();
        let _ = done_tx.send((r.is_ok(), pending));
        session
    });
    let (ok, pending) = done_rx.recv_timeout(LONG).expect("write_all blocked on a non-reading child");
    assert!(ok);
    assert!(pending > 512 * 1024, "the bytes are queued, not written: pending={pending}");
    drop(worker.join().unwrap());
}

/// The backlog for a non-reading child is bounded, and overflow is reported to the caller.
#[test]
fn backlog_for_a_non_reading_child_is_bounded_and_overflow_is_an_error() {
    let session = PtySession::spawn(sh("exec sleep 600")).unwrap();
    let chunk = vec![b'y'; 1024 * 1024];
    let mut accepted = 0usize;
    let mut refused = None;
    for _ in 0..16 {
        match session.write_all(&chunk) {
            Ok(()) => accepted += chunk.len(),
            Err(e) => {
                refused = Some(e.to_string());
                break;
            }
        }
    }
    let msg = refused.expect("a 16 MiB backlog must be refused");
    assert!(msg.contains("not reading"), "{msg}");
    assert!(accepted <= MAX_QUEUED_INPUT_BYTES + 64 * 1024, "accepted {accepted}");
    assert!(session.pending_input_bytes() <= MAX_QUEUED_INPUT_BYTES);
    assert_eq!(session.rejected_input_bytes(), chunk.len() as u64);
}

/// Dropping a session whose writer thread is stuck in a kernel write ends that thread.
#[test]
fn writer_thread_ends_when_the_pty_is_dropped_even_while_blocked() {
    let session = PtySession::spawn(sh("exec sleep 600")).unwrap();
    session.write_all(&vec![b'z'; 1024 * 1024]).unwrap();
    let probe = session.writer_probe();
    assert!(!probe.finished());
    drop(session);
    assert!(probe.wait_finished(LONG), "writer thread leaked after the PTY was dropped");
}

#[test]
fn writer_thread_ends_when_an_idle_pty_is_dropped() {
    let session = PtySession::spawn(sh("exec sleep 600")).unwrap();
    let probe = session.writer_probe();
    drop(session);
    assert!(probe.wait_finished(LONG));
}

/// Many small writes, one large write, many small writes: the child sees exactly that stream.
#[test]
fn byte_order_through_a_real_pty_is_exact() {
    // Raw, no echo: `cat` copies its input verbatim. The marker says the tty is configured.
    let mut session = PtySession::spawn(sh("stty raw -echo; printf READY; exec cat")).unwrap();
    let mut seen = Vec::new();
    assert!(wait_for(|| {
        for ev in session.drain_events() {
            if let PtyEvent::Output(b) = ev {
                seen.extend_from_slice(&b);
            }
        }
        seen.ends_with(b"READY")
    }));
    let mut expect = Vec::new();
    let mut put = |bytes: Vec<u8>| {
        session.write_all(&bytes).unwrap();
        expect.extend_from_slice(&bytes);
    };
    for i in 0..500u32 {
        put(format!("<{i}>").into_bytes());
    }
    // Printable, so no tty special characters are involved even in raw mode.
    put((0..300_000u32).map(|i| b'a' + (i % 26) as u8).collect());
    for i in 500..1000u32 {
        put(format!("<{i}>").into_bytes());
    }
    let mut got = Vec::new();
    assert!(
        wait_for(|| {
            for ev in session.drain_events() {
                if let PtyEvent::Output(b) = ev {
                    got.extend_from_slice(&b);
                }
            }
            got.len() >= expect.len()
        }),
        "got {} of {} bytes",
        got.len(),
        expect.len()
    );
    assert!(got == expect, "the child saw a different byte stream than was written");
    assert_eq!(session.pending_input_bytes(), 0);
}

/// A write that fails (the child is gone) marks the pane exited; it is not an error that can
/// escape and take the application down.
#[test]
fn write_after_the_child_died_marks_the_pane_exited() {
    let mut pane = CompositePane::spawn(7, sh("exit 0")).unwrap();
    assert!(wait_for(|| {
        // Keep writing: once the slave side is closed a write fails, or the reader sees EOF.
        let _ = pane.write_to_pty(b"x");
        pane.poll_pty_events();
        pane.has_exited()
    }));
    assert!(pane.has_exited());
}
