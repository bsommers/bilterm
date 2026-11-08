//! Opt-in raw PTY output mirror for web clients (`WEB-FE-01`, Phase 60).
//!
//! When enabled, every byte fed to the VT parser is also appended to a bounded buffer that the
//! automation plane drains into the WebSocket output ring. Disabled panes pay nothing.

use super::CompositePane;

/// Most bytes buffered between drains; older bytes are dropped first (the ring reports a gap).
pub const OUTPUT_TAP_CAP: usize = 256 * 1024;

impl CompositePane {
    /// Turns the output tap on or off; turning it off drops buffered bytes.
    pub fn set_output_tap(&mut self, on: bool) {
        match (on, self.output_tap.is_some()) {
            (true, false) => self.output_tap = Some(Vec::new()),
            (false, true) => self.output_tap = None,
            _ => {}
        }
    }

    /// `true` while output bytes are mirrored.
    pub fn output_tap_enabled(&self) -> bool {
        self.output_tap.is_some()
    }

    /// Drains the bytes mirrored since the last call (empty when the tap is off).
    pub fn take_output_tap(&mut self) -> Vec<u8> {
        self.output_tap.as_mut().map(std::mem::take).unwrap_or_default()
    }

    pub(super) fn mirror_output_tap(&mut self, bytes: &[u8]) {
        let Some(tap) = self.output_tap.as_mut() else {
            return;
        };
        if bytes.len() >= OUTPUT_TAP_CAP {
            tap.clear();
            tap.extend_from_slice(&bytes[bytes.len() - OUTPUT_TAP_CAP..]);
            return;
        }
        let overflow = (tap.len() + bytes.len()).saturating_sub(OUTPUT_TAP_CAP);
        if overflow > 0 {
            tap.drain(..overflow);
        }
        tap.extend_from_slice(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_is_opt_in_bounded_and_drains() {
        let mut p = CompositePane::new_headless(1, 5, 20);
        p.process_pty_output(b"before");
        assert!(p.take_output_tap().is_empty());
        p.set_output_tap(true);
        p.process_pty_output(b"hello");
        assert_eq!(p.take_output_tap(), b"hello");
        assert!(p.take_output_tap().is_empty());
        p.process_pty_output(&vec![b'x'; OUTPUT_TAP_CAP + 10]);
        assert_eq!(p.take_output_tap().len(), OUTPUT_TAP_CAP);
        p.set_output_tap(false);
        p.process_pty_output(b"after");
        assert!(p.take_output_tap().is_empty());
    }
}
