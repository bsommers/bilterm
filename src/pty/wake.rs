//! Process-wide "a PTY has something for the UI loop" wake (`RESP-01`).
//!
//! PTY reader threads (and writer threads, on failure) call [`notify_output`]; an event loop
//! awaits [`output_ready`] in its `select!` and drains PTYs at once instead of waiting for its
//! next timer tick. The signal is level-triggered through a sequence number, so a notification
//! that lands while the loop is busy (drawing, handling input) is never lost, and any number of
//! loops in one process (tests) can wait without stealing each other's wakeups.

use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::Notify;

static OUTPUT_WAKE: Notify = Notify::const_new();
static OUTPUT_SEQ: AtomicU64 = AtomicU64::new(0);
static INPUT_SEQ: AtomicU64 = AtomicU64::new(0);

/// Signals that some PTY queued output (or changed state) for the UI loop. Callable from any
/// thread; never blocks.
pub fn notify_output() {
    OUTPUT_SEQ.fetch_add(1, Ordering::SeqCst);
    OUTPUT_WAKE.notify_waiters();
}

/// Current output sequence number; pass a copy of it to [`output_ready`].
pub fn output_seq() -> u64 {
    OUTPUT_SEQ.load(Ordering::SeqCst)
}

/// Resolves once [`notify_output`] has been called since `*seen` was taken, then advances
/// `*seen`. Cancel-safe: dropping the future loses nothing because `*seen` only moves on return.
pub async fn output_ready(seen: &mut u64) {
    loop {
        let notified = OUTPUT_WAKE.notified();
        tokio::pin!(notified);
        // Register before reading the sequence so a notify between the two is not missed.
        notified.as_mut().enable();
        let now = OUTPUT_SEQ.load(Ordering::SeqCst);
        if now != *seen {
            *seen = now;
            return;
        }
        notified.await;
    }
}

/// Records that bytes were queued for some PTY (keyboard input, paste, query reply).
pub(super) fn note_input() {
    INPUT_SEQ.fetch_add(1, Ordering::Relaxed);
}

/// Count of PTY writes accepted so far in this process. An event loop compares it around an
/// input event to learn whether that event sent bytes to a PTY (and so may be echoed).
pub fn input_seq() -> u64 {
    INPUT_SEQ.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn notify_before_wait_is_not_lost_and_seen_advances() {
        let mut seen = output_seq();
        notify_output();
        output_ready(&mut seen).await;
        assert!(seen >= 1);
        // Nothing new since `seen` (other tests may notify concurrently, so only check that a
        // fresh notify resolves a fresh wait).
        let mut again = output_seq();
        let waiter = tokio::spawn(async move {
            output_ready(&mut again).await;
        });
        tokio::task::yield_now().await;
        notify_output();
        waiter.await.unwrap();
    }
}
