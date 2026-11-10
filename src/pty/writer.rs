//! Per-PTY input writer thread (`RESP-01`).
//!
//! `PtySession::write_all` used to write to the PTY master on the caller, which is the UI
//! thread: a child that stops reading (or is itself blocked writing output nobody drains) fills
//! the kernel tty buffer (about 4 KiB on Linux) and the next write stalls the whole multiplexer.
//! Here the caller only appends to a FIFO queue; one thread per PTY owns the master writer and
//! performs the blocking writes.
//!
//! Guarantees:
//! * **Order**: one queue, one consumer, chunks written whole and in arrival order.
//! * **Bounded memory**: at most [`MAX_QUEUED_INPUT_BYTES`] accepted-but-unwritten bytes.
//! * **At the limit**: the *whole* write is refused with an error (never a fragment, so no torn
//!   escape or UTF-8 sequence), the caller is never blocked, and the refusal is returned to the
//!   caller, counted in [`PtyWriter::rejected_bytes`] and logged. Nothing is dropped silently.
//! * **Write error**: recorded once; later writes fail fast, and the session reports the PTY as
//!   exited. The error never leaves this module as a panic.
//! * **Shutdown**: `close()` ends the thread as soon as it is not inside a kernel write; a
//!   thread blocked in a write ends when the PTY is torn down (the write fails with `EIO`).

use std::collections::VecDeque;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{BiltermError, Result};
use crate::traffic::TrafficTap;

/// Upper bound on bytes accepted for one PTY but not yet written to it.
///
/// Sized from the largest legitimate burst, a paste delivered as one event: the remote link
/// already caps that at 1 MiB (`REMOTE_INPUT_CAP`); 4 MiB leaves room for bracketed-paste
/// framing and several pastes in flight, while a pane whose child never reads costs at most
/// 4 MiB (50 such panes: 200 MiB) instead of growing without bound.
pub const MAX_QUEUED_INPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Default)]
struct State {
    queue: VecDeque<Vec<u8>>,
    /// Bytes in `queue` plus the chunk currently being written.
    pending: usize,
    closed: bool,
    failed: Option<String>,
    rejected: u64,
    overflow_logged: bool,
}

struct Shared {
    state: Mutex<State>,
    work: Condvar,
    drained: Condvar,
    finished: AtomicBool,
    limit: usize,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // The state is plain data; a panic elsewhere must not wedge the UI thread.
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Sets `finished` when the writer thread ends, however it ends.
struct FinishGuard(Arc<Shared>);

impl Drop for FinishGuard {
    fn drop(&mut self) {
        self.0.finished.store(true, Ordering::SeqCst);
        self.0.drained.notify_all();
    }
}

/// Observer for "has this PTY's writer thread ended" that outlives the session.
#[derive(Clone)]
pub struct WriterProbe(Arc<Shared>);

impl WriterProbe {
    pub fn finished(&self) -> bool {
        self.0.finished.load(Ordering::SeqCst)
    }

    /// Blocks until the thread has ended or `patience` passed; returns whether it ended.
    pub fn wait_finished(&self, patience: Duration) -> bool {
        let deadline = Instant::now() + patience;
        let mut st = self.0.lock();
        while !self.finished() {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            st = self.0.drained.wait_timeout(st, deadline - now).unwrap_or_else(|p| p.into_inner()).0;
        }
        true
    }
}

/// Queue handle for one PTY's writer thread.
pub struct PtyWriter {
    shared: Arc<Shared>,
}

impl PtyWriter {
    /// Starts the writer thread over `sink` with the default byte limit.
    pub fn spawn(sink: Box<dyn Write + Send>, traffic: TrafficTap) -> std::io::Result<Self> {
        Self::spawn_with_limit(sink, traffic, MAX_QUEUED_INPUT_BYTES)
    }

    /// Starts the writer thread with an explicit pending-byte limit.
    pub fn spawn_with_limit(
        mut sink: Box<dyn Write + Send>,
        traffic: TrafficTap,
        limit: usize,
    ) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            work: Condvar::new(),
            drained: Condvar::new(),
            finished: AtomicBool::new(false),
            limit,
        });
        let th = shared.clone();
        thread::Builder::new().name("bilterm-pty-writer".to_string()).spawn(move || {
            let _guard = FinishGuard(th.clone());
            loop {
                let chunk = {
                    let mut st = th.lock();
                    loop {
                        if st.closed {
                            return;
                        }
                        if let Some(c) = st.queue.pop_front() {
                            break c;
                        }
                        st = th.work.wait(st).unwrap_or_else(|p| p.into_inner());
                    }
                };
                let written = sink.write_all(&chunk).and_then(|()| sink.flush());
                let mut st = th.lock();
                match written {
                    Ok(()) => {
                        // Counted when the bytes are really on the wire (`TMON-01`).
                        traffic.tx(chunk.len());
                        st.pending -= chunk.len();
                        if st.pending == 0 {
                            th.drained.notify_all();
                        }
                    }
                    Err(e) => {
                        st.failed = Some(e.to_string());
                        st.queue.clear();
                        st.pending = 0;
                        drop(st);
                        th.drained.notify_all();
                        // Let the UI loop notice the dead PTY now rather than on its next tick.
                        super::wake::notify_output();
                        return;
                    }
                }
            }
        })?;
        Ok(Self { shared })
    }

    /// Queues `bytes` for the PTY and returns at once. `Err` when the writer has failed or the
    /// write would exceed the pending-byte limit (the write is then refused whole).
    pub fn enqueue(&self, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        let mut st = self.shared.lock();
        if let Some(e) = st.failed.as_ref() {
            return Err(BiltermError::Pty(format!("PTY write failed: {e}")));
        }
        if st.closed {
            return Ok(());
        }
        if st.pending.saturating_add(bytes.len()) > self.shared.limit {
            st.rejected = st.rejected.saturating_add(bytes.len() as u64);
            if !st.overflow_logged {
                st.overflow_logged = true;
                tracing::warn!(pending = st.pending, refused = bytes.len(), "PTY input queue full; write refused");
            }
            return Err(BiltermError::Pty(format!(
                "input not sent: {} bytes are still waiting for a program that is not reading (limit {} bytes)",
                st.pending, self.shared.limit
            )));
        }
        st.overflow_logged = false;
        st.pending += bytes.len();
        st.queue.push_back(bytes.to_vec());
        drop(st);
        self.shared.work.notify_one();
        Ok(())
    }

    /// Bytes accepted and not yet written.
    pub fn pending_bytes(&self) -> usize {
        self.shared.lock().pending
    }

    /// Total bytes refused because the queue was full.
    pub fn rejected_bytes(&self) -> u64 {
        self.shared.lock().rejected
    }

    /// The write error that stopped the thread, if any.
    pub fn failure(&self) -> Option<String> {
        self.shared.lock().failed.clone()
    }

    pub fn probe(&self) -> WriterProbe {
        WriterProbe(self.shared.clone())
    }

    /// Waits (off the UI thread only) until everything accepted has been written, the writer
    /// failed or ended, or `patience` passed. Returns whether the queue is empty.
    pub fn wait_drained(&self, patience: Duration) -> bool {
        let deadline = Instant::now() + patience;
        let mut st = self.shared.lock();
        while st.pending > 0 && st.failed.is_none() && !self.shared.finished.load(Ordering::SeqCst) {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            st = self.shared.drained.wait_timeout(st, deadline - now).unwrap_or_else(|p| p.into_inner()).0;
        }
        st.pending == 0
    }

    /// Stops accepting writes and ends the thread once it is not inside a kernel write.
    /// Unwritten bytes are discarded.
    pub fn close(&self) {
        let mut st = self.shared.lock();
        st.closed = true;
        let discarded: usize = st.queue.drain(..).map(|c| c.len()).sum();
        st.pending -= discarded;
        drop(st);
        self.shared.work.notify_all();
    }
}

impl Drop for PtyWriter {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
