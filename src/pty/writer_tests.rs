//! Deterministic tests for the PTY writer thread: fake sinks, no real PTY, no sleeps.

use std::io::{self, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::*;
use crate::traffic::ChannelCounters;

const LONG: Duration = Duration::from_secs(30);

/// Sink that stores what it is given; each `write` first waits for a permit when gated.
struct Sink {
    out: Arc<Mutex<Vec<u8>>>,
    gate: Option<Receiver<()>>,
    entered: Option<Sender<()>>,
    fail: bool,
}

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(tx) = self.entered.as_ref() {
            let _ = tx.send(());
        }
        if let Some(gate) = self.gate.as_ref() {
            // A closed gate means "the PTY was torn down": the blocked write fails.
            gate.recv().map_err(|_| io::Error::other("torn down"))?;
        }
        if self.fail {
            return Err(io::Error::other("EIO"));
        }
        self.out.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn open_sink() -> (Box<Sink>, Arc<Mutex<Vec<u8>>>) {
    let out = Arc::new(Mutex::new(Vec::new()));
    (Box::new(Sink { out: out.clone(), gate: None, entered: None, fail: false }), out)
}

/// A sink whose every `write` blocks until a permit is sent (like a child that is not reading).
fn gated_sink() -> (Box<Sink>, Arc<Mutex<Vec<u8>>>, Sender<()>, Receiver<()>) {
    let out = Arc::new(Mutex::new(Vec::new()));
    let (permit_tx, permit_rx) = channel();
    let (entered_tx, entered_rx) = channel();
    let sink = Sink { out: out.clone(), gate: Some(permit_rx), entered: Some(entered_tx), fail: false };
    (Box::new(sink), out, permit_tx, entered_rx)
}

fn counters() -> crate::traffic::TrafficTap {
    let c = ChannelCounters::new();
    c.set_flags(crate::traffic::cell::F_ENABLED);
    c
}

#[test]
fn enqueue_returns_while_the_sink_is_blocked() {
    let (sink, out, permit, entered) = gated_sink();
    let w = PtyWriter::spawn(sink, counters()).unwrap();
    // If enqueue wrote on the caller this would deadlock: nobody sends a permit yet.
    w.enqueue(&vec![b'a'; 1024 * 1024]).unwrap();
    w.enqueue(b"tail").unwrap();
    entered.recv_timeout(LONG).expect("writer thread reached the blocking write");
    assert_eq!(w.pending_bytes(), 1024 * 1024 + 4, "in-flight bytes still count as pending");
    assert!(out.lock().unwrap().is_empty());
    // Release the two chunks' writes (one `write` call each: the sink takes the whole buffer).
    permit.send(()).unwrap();
    permit.send(()).unwrap();
    assert!(w.wait_drained(LONG));
    assert_eq!(out.lock().unwrap().len(), 1024 * 1024 + 4);
}

#[test]
fn order_is_preserved_across_many_small_and_one_large_write() {
    let (sink, out) = open_sink();
    let traffic = counters();
    let w = PtyWriter::spawn(sink, traffic.clone()).unwrap();
    let mut expect = Vec::new();
    let mut push = |bytes: Vec<u8>| {
        w.enqueue(&bytes).unwrap();
        expect.extend_from_slice(&bytes);
    };
    for i in 0..2000u32 {
        push(format!("[{i}]").into_bytes());
    }
    push((0..1_500_000u32).map(|i| (i % 251) as u8).collect());
    for i in 2000..4000u32 {
        push(format!("[{i}]").into_bytes());
    }
    assert!(w.wait_drained(LONG));
    assert!(*out.lock().unwrap() == expect, "byte stream differs from the write order");
    let r = traffic.read();
    assert_eq!((r.tx_bytes, r.tx_msgs), (expect.len() as u64, 4001), "tx counted once per write");
}

#[test]
fn full_queue_refuses_the_whole_write_without_blocking_or_dropping_silently() {
    let (sink, out, permit, entered) = gated_sink();
    let w = PtyWriter::spawn_with_limit(sink, counters(), 100).unwrap();
    w.enqueue(&[b'a'; 60]).unwrap();
    entered.recv_timeout(LONG).unwrap();
    w.enqueue(&[b'b'; 40]).unwrap();
    let err = w.enqueue(b"c").expect_err("over the limit");
    assert!(err.to_string().contains("not reading"), "{err}");
    assert_eq!(w.rejected_bytes(), 1);
    assert_eq!(w.pending_bytes(), 100, "a refused write leaves no fragment behind");
    permit.send(()).unwrap();
    permit.send(()).unwrap();
    assert!(w.wait_drained(LONG));
    // Room again: accepted, and the stream is exactly the accepted writes in order.
    w.enqueue(b"d").unwrap();
    permit.send(()).unwrap();
    assert!(w.wait_drained(LONG));
    let mut expect = vec![b'a'; 60];
    expect.extend_from_slice(&[b'b'; 40]);
    expect.push(b'd');
    assert_eq!(*out.lock().unwrap(), expect);
}

#[test]
fn write_error_is_recorded_fails_later_writes_and_ends_the_thread() {
    let out = Arc::new(Mutex::new(Vec::new()));
    let traffic = counters();
    let sink = Sink { out: out.clone(), gate: None, entered: None, fail: true };
    let w = PtyWriter::spawn(Box::new(sink), traffic.clone()).unwrap();
    let probe = w.probe();
    let seq = super::super::wake::output_seq();
    w.enqueue(b"x").unwrap();
    assert!(probe.wait_finished(LONG));
    assert_eq!(w.failure().as_deref(), Some("EIO"));
    assert!(w.enqueue(b"y").is_err(), "writes after a failure are refused, not queued");
    assert_eq!(w.pending_bytes(), 0);
    assert_eq!(traffic.read().tx_bytes, 0, "bytes that never reached the PTY are not counted");
    assert_ne!(super::super::wake::output_seq(), seq, "the UI loop is woken to notice the dead PTY");
}

#[test]
fn drop_ends_an_idle_thread() {
    let (sink, _out) = open_sink();
    let w = PtyWriter::spawn(sink, counters()).unwrap();
    let probe = w.probe();
    assert!(!probe.finished());
    drop(w);
    assert!(probe.wait_finished(LONG));
}

#[test]
fn drop_ends_a_thread_blocked_in_write_once_the_pty_is_torn_down() {
    let (sink, out, permit, entered) = gated_sink();
    let w = PtyWriter::spawn(sink, counters()).unwrap();
    let probe = w.probe();
    w.enqueue(b"stuck").unwrap();
    w.enqueue(b"never written").unwrap();
    entered.recv_timeout(LONG).unwrap();
    drop(w);
    assert!(!probe.finished(), "still inside the blocked write");
    drop(permit); // the PTY goes away: the blocked write fails
    assert!(probe.wait_finished(LONG));
    assert!(out.lock().unwrap().is_empty(), "queued bytes are discarded on close");
}
