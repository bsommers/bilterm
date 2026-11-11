//! "Exited" is reported at most once per PTY (`RESP-01`).
//!
//! Two things can end a PTY for the pane: the reader thread seeing EOF/EIO, and the writer
//! thread failing a write. Whichever is drained first is the exit; the other is not repeated,
//! in the same poll or any later one.

use super::PtyEvent;

/// Finishes one `drain_events` batch. `write_failure` is the writer's error, passed only once
/// the output queue was drained empty (so the exit never overtakes output queued before it).
/// Returns `true` when an `Exited` was synthesised for the write failure.
pub(super) fn settle(events: &mut Vec<PtyEvent>, exit_reported: &mut bool, write_failure: Option<&str>) -> bool {
    let mut kept_exit = *exit_reported;
    events.retain(|e| {
        if !matches!(e, PtyEvent::Exited(_)) {
            return true;
        }
        let first = !kept_exit;
        kept_exit = true;
        first
    });
    let synthesise = !kept_exit && write_failure.is_some();
    if synthesise {
        events.push(PtyEvent::Exited(None));
        kept_exit = true;
    }
    *exit_reported = kept_exit;
    synthesise
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(b: &[u8]) -> PtyEvent {
        PtyEvent::Output(b.to_vec())
    }

    #[test]
    fn write_failure_is_reported_once_after_queued_output() {
        let mut reported = false;
        let mut ev = vec![out(b"a")];
        assert!(settle(&mut ev, &mut reported, Some("EIO")));
        assert_eq!(ev, vec![out(b"a"), PtyEvent::Exited(None)]);
        let mut later = Vec::new();
        assert!(!settle(&mut later, &mut reported, Some("EIO")));
        assert!(later.is_empty(), "no second exit on later polls");
    }

    #[test]
    fn reader_exit_drained_in_an_earlier_poll_suppresses_the_synthesised_one() {
        let mut reported = false;
        let mut first = vec![out(b"a"), PtyEvent::Exited(Some(3))];
        assert!(!settle(&mut first, &mut reported, None));
        assert_eq!(first.len(), 2);
        assert!(reported);
        // The writer fails afterwards (the review's case): nothing more is reported.
        let mut second = Vec::new();
        assert!(!settle(&mut second, &mut reported, Some("EIO")));
        assert!(second.is_empty());
    }

    #[test]
    fn reader_exit_in_the_same_poll_wins_and_a_late_reader_exit_is_dropped() {
        let mut reported = false;
        let mut ev = vec![PtyEvent::Exited(Some(0))];
        assert!(!settle(&mut ev, &mut reported, Some("EIO")));
        assert_eq!(ev, vec![PtyEvent::Exited(Some(0))]);
        // Synthesised first, reader's EOF arrives in a later poll: dropped, output kept.
        let mut reported = false;
        let mut a = Vec::new();
        assert!(settle(&mut a, &mut reported, Some("EIO")));
        let mut b = vec![out(b"tail"), PtyEvent::Exited(None)];
        assert!(!settle(&mut b, &mut reported, Some("EIO")));
        assert_eq!(b, vec![out(b"tail")]);
    }

    #[test]
    fn no_failure_and_no_exit_reports_nothing() {
        let mut reported = false;
        let mut ev = vec![out(b"x")];
        assert!(!settle(&mut ev, &mut reported, None));
        assert_eq!(ev, vec![out(b"x")]);
        assert!(!reported);
    }
}
