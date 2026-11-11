//! Frame pacing for the wake-driven event loop (`RESP-01`).
//!
//! PTY output now wakes the loop at once, so something must stop a flooding pane from causing
//! one redraw per 8 KiB chunk. [`FramePacer`] decides *when* a pending redraw may be drawn:
//!
//! * **Frame floor** ([`FRAME_FLOOR`]): two draws are never started closer together than this.
//!   An idle terminal is not affected (the first draw after a pause is immediate); a flood is
//!   capped at `1 / FRAME_FLOOR` frames per second.
//! * **Echo hold** ([`ECHO_GRACE`]): a redraw caused by input that sent bytes to a PTY is held
//!   back briefly so the echo lands in the *same* frame. Without it the keypress draws a frame
//!   in which nothing visible changed, and that draw starts the frame floor, delaying the echo
//!   that arrives a fraction of a millisecond later. The redraw is only deferred, never skipped,
//!   so input that also changed window-manager state is still drawn, at most `ECHO_GRACE` late.

use std::cell::Cell;
use std::time::{Duration, Instant};

/// Minimum time between the starts of two draws.
///
/// 8 ms caps a flooding pane at 125 frames/s: no slower than a 120 Hz display can show and
/// twice the old fixed 16 ms cadence, while bounding redraw work however fast output arrives.
pub const FRAME_FLOOR: Duration = Duration::from_millis(8);

/// How long a redraw caused by PTY-bound input waits for the echo before drawing anyway.
///
/// A local echo takes well under a millisecond (measured median 0.06 ms for `cat`, `tests/echo_latency.rs`); 2 ms covers
/// it with margin and is far below what is perceptible when no echo comes (password prompts).
pub const ECHO_GRACE: Duration = Duration::from_millis(2);

/// Decides when a pending redraw may be drawn. Pure: every method takes the current instant.
#[derive(Debug, Default, Clone, Copy)]
pub struct FramePacer {
    last_draw: Option<Instant>,
    hold_until: Option<Instant>,
}

impl FramePacer {
    /// Earliest instant at which a pending redraw may be drawn.
    pub fn due(&self, now: Instant) -> Instant {
        let floor = self.last_draw.map_or(now, |t| t + FRAME_FLOOR);
        let hold = self.hold_until.unwrap_or(now);
        floor.max(hold).max(now)
    }

    pub fn can_draw(&self, now: Instant) -> bool {
        self.due(now) <= now
    }

    /// A draw started at `now`.
    pub fn drew(&mut self, now: Instant) {
        self.last_draw = Some(now);
        self.hold_until = None;
    }

    /// Input just sent bytes to a PTY. Starts the echo hold unless a redraw was already pending
    /// (`redraw_was_pending`), so repeated input can never push a frame back indefinitely.
    pub fn input_sent_to_pty(&mut self, now: Instant, redraw_was_pending: bool) {
        if !redraw_was_pending && self.hold_until.is_none() {
            self.hold_until = Some(now + ECHO_GRACE);
        }
    }

    /// PTY output was ingested: whatever the hold was waiting for is here.
    pub fn output_arrived(&mut self) {
        self.hold_until = None;
    }
}

/// Counters of one event loop run, readable while it runs (tests, diagnostics).
#[derive(Debug, Default)]
pub struct LoopStats {
    /// Passes through the loop (one `select!` each).
    pub turns: Cell<u64>,
    /// Frames drawn.
    pub draws: Cell<u64>,
    /// Host input stream items consumed.
    pub inputs: Cell<u64>,
    /// `turns` value at which the latest input item was consumed.
    pub last_input_turn: Cell<u64>,
    /// Wake-driven PTY drains (not counting the 16 ms tick).
    pub pty_wakes: Cell<u64>,
    /// 16 ms timer ticks serviced.
    pub ticks: Cell<u64>,
}

impl LoopStats {
    pub(super) fn bump(c: &Cell<u64>) {
        c.set(c.get() + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn first_draw_is_immediate_and_floor_spaces_the_next() {
        let t0 = Instant::now();
        let mut p = FramePacer::default();
        assert!(p.can_draw(t0));
        p.drew(t0);
        assert!(!p.can_draw(t0 + ms(1)));
        assert_eq!(p.due(t0 + ms(1)), t0 + FRAME_FLOOR);
        assert!(p.can_draw(t0 + FRAME_FLOOR));
        // After a pause the floor is long past: no added latency for an idle terminal.
        assert!(p.can_draw(t0 + ms(500)));
    }

    #[test]
    fn echo_hold_defers_but_never_skips_and_is_cleared_by_output() {
        let t0 = Instant::now();
        let mut p = FramePacer::default();
        p.input_sent_to_pty(t0, false);
        assert!(!p.can_draw(t0), "the keypress frame waits for the echo");
        assert_eq!(p.due(t0), t0 + ECHO_GRACE);
        assert!(p.can_draw(t0 + ECHO_GRACE), "no echo: drawn anyway after the grace");
        p.output_arrived();
        assert!(p.can_draw(t0 + Duration::from_micros(100)), "echo arrived: draw now");
    }

    #[test]
    fn repeated_input_cannot_postpone_a_pending_frame() {
        let t0 = Instant::now();
        let mut p = FramePacer::default();
        p.input_sent_to_pty(t0, false);
        for i in 1..50 {
            p.input_sent_to_pty(t0 + Duration::from_micros(i * 30), true);
            p.input_sent_to_pty(t0 + Duration::from_micros(i * 30), false);
        }
        assert_eq!(p.due(t0), t0 + ECHO_GRACE, "the deadline is fixed by the first input");
    }

    #[test]
    fn hold_never_shortens_the_frame_floor() {
        let t0 = Instant::now();
        let mut p = FramePacer::default();
        p.drew(t0);
        p.input_sent_to_pty(t0 + ms(1), false);
        assert_eq!(p.due(t0 + ms(1)), t0 + FRAME_FLOOR);
        p.output_arrived();
        assert_eq!(p.due(t0 + ms(1)), t0 + FRAME_FLOOR);
    }
}
