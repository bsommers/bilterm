//! Stateful, single-pass control-sequence framer (`ARCH-R04`).
//!
//! [`SeqFramer`] sits in front of both the VT engine and the side handlers in `scan.rs` /
//! `scan_osc_dcs.rs`. It is a byte-level state machine that carries its state across
//! [`SeqFramer::feed`] calls, so what it produces is a function of the byte stream alone, never of
//! how the stream was chunked by PTY reads:
//!
//! * Ground bytes go straight to the engine buffer.
//! * `ESC` / `CSI` / `OSC` sequences are withheld until complete and then released whole, so the
//!   per-chunk rewriting stages behind the framer never see half a sequence.
//! * `DCS` / `APC` / `SOS` / `PM` strings never reach the engine (it would print their payload).
//! * Completed sequences the side handlers care about are reported as [`FramedSeq`] events.
//!
//! Sequence boundaries follow the engine's own parser (`vte` 0.13 state table): `ESC` restarts a
//! sequence from any state, `CAN` / `SUB` cancel it, C0 controls inside `ESC` / `CSI` are executed
//! at once (they are handed to the engine ahead of the withheld sequence, which is equivalent),
//! bytes `>= 0x7f` inside `ESC` / `CSI` are ignored, and 8-bit C1 bytes are *not* introducers
//! (the stream is UTF-8, where `0x80..=0x9f` are continuation bytes). Strings end at `BEL`, `ST`
//! (`ESC \`), `CAN`, `SUB` or a bare `ESC`; only `BEL` and `ST` complete them, anything else
//! cancels without an event.
//!
//! Linear time: every input byte is examined once (`steps` counts them), copied at most once into
//! the carry and once into the output, and a completed body is handed to exactly one handler.
//! Nothing is ever re-scanned on the next `feed`. The carry is bounded: a sequence longer than its
//! limit is dropped whole (no event, nothing forwarded) and the rest of it is skipped unstored.

use super::codec::MAX_OSC52_PAYLOAD_BYTES;
use super::graphics::MAX_KITTY_PAYLOAD_B64_BYTES;
use super::scan::split_incomplete_utf8_tail;

/// Longest `CSI` held (bytes after `ESC [`); same bound as the count clamp and the Tektronix buffer.
pub(super) const MAX_CSI_BODY_BYTES: usize = 256;
/// Longest `OSC` held (introducer included): the largest forwardable `OSC 52` write plus its header.
pub(super) const MAX_OSC_BODY_BYTES: usize = MAX_OSC52_PAYLOAD_BYTES + 64;
/// Longest `APC` body held: the largest accepted Kitty payload plus its control keys.
pub(super) const MAX_APC_BODY_BYTES: usize = MAX_KITTY_PAYLOAD_B64_BYTES + 4096;
/// Longest `DCS` body held. Sixel has no payload clamp of its own (the decoder clamps the image
/// to `MAX_GRAPHIC_DIM_PX` instead), so this is the bound on a Sixel stream.
pub(super) const MAX_DCS_BODY_BYTES: usize = 4 * 1024 * 1024;
/// Most intermediates kept for a plain `ESC` sequence (the engine honours two).
const MAX_ESC_BODY_BYTES: usize = 8;
/// Largest buffer capacity kept between feeds.
pub(super) const RETAINED_BUFFER_BYTES: usize = 64 * 1024;
/// CSI final bytes the side handlers act on; other finals are forwarded without an event.
const CSI_EVENT_FINALS: &[u8] = b"hlnucqpmJ";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SeqKind {
    /// `ESC # 3..6` (DEC line size) or `ESC c` (RIS); body is the bytes after `ESC`.
    Esc,
    /// Body is the parameter, intermediate and final bytes after `ESC [`.
    Csi,
    Osc,
    Dcs,
    Apc,
    /// `CSI ? 38 h`: the stream switches to the Tektronix machine. Not forwarded to the engine.
    TekEnter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StringTerminator {
    Bel,
    St,
}

impl StringTerminator {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Bel => "\x07",
            Self::St => "\x1b\\",
        }
    }
}

/// One completed control sequence.
#[derive(Clone, Copy, Debug)]
pub(super) struct FramedSeq {
    pub(super) kind: SeqKind,
    /// How a string sequence ended (`Bel` for non-string kinds).
    pub(super) terminator: StringTerminator,
    /// Length of [`FrameOutput::engine`] once this sequence (and everything before it) is in it.
    pub(super) engine_end: usize,
    in_engine: bool,
    start: usize,
    end: usize,
}

/// Result of one [`SeqFramer::feed`]; reused across calls to keep its allocations.
#[derive(Debug, Default)]
pub(super) struct FrameOutput {
    /// Bytes for the VT engine, in stream order.
    pub(super) engine: Vec<u8>,
    /// Completed sequences, in stream order.
    pub(super) seqs: Vec<FramedSeq>,
    strings: Vec<u8>,
}

impl FrameOutput {
    pub(super) fn body(&self, seq: &FramedSeq) -> &[u8] {
        let buf = if seq.in_engine { &self.engine } else { &self.strings };
        &buf[seq.start..seq.end]
    }

    fn clear(&mut self) {
        for buf in [&mut self.engine, &mut self.strings] {
            if buf.capacity() > RETAINED_BUFFER_BYTES {
                *buf = Vec::new();
            } else {
                buf.clear();
            }
        }
        self.seqs.clear();
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum State {
    #[default]
    Ground,
    Escape,
    EscapeIntermediate,
    Csi,
    /// Over-long `CSI`: skipped up to its final byte.
    CsiDiscard,
    Osc,
    OscEsc,
    /// `DCS` (`true`) or `APC` (`false`) body being collected.
    Str(bool),
    StrEsc(bool),
    /// `SOS` / `PM`, or an over-long `OSC` / `DCS` / `APC`: skipped up to its terminator.
    StrDiscard,
}

/// `CSI` grammar position, mirroring the engine's entry / param / intermediate / ignore states.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum CsiPhase {
    #[default]
    Entry,
    Param,
    Intermediate,
    Ignore,
}

#[derive(Debug, Default)]
pub(super) struct SeqFramer {
    state: State,
    /// Withheld `ESC` / `CSI` / `OSC` bytes (from the `ESC`), or the `DCS` / `APC` body.
    held: Vec<u8>,
    csi_phase: CsiPhase,
    /// Incomplete trailing UTF-8 char of the previous feed's ground text.
    utf8_tail: Vec<u8>,
    steps: u64,
    dropped: u64,
}

fn is_string_end(b: u8) -> bool {
    matches!(b, 0x07 | 0x18 | 0x1a | 0x1b)
}

impl SeqFramer {
    /// Bytes currently withheld (bounded by the limits above).
    pub(super) fn carry_len(&self) -> usize {
        self.held.len() + self.utf8_tail.len()
    }

    /// Input bytes consumed so far; each byte is counted exactly once.
    pub(super) fn steps(&self) -> u64 {
        self.steps
    }

    /// Sequences dropped for exceeding their length limit.
    pub(super) fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Consumes `input` into `out` (cleared first) and returns how many bytes were consumed:
    /// all of them, unless a [`SeqKind::TekEnter`] stopped the feed early.
    pub(super) fn feed(&mut self, input: &[u8], out: &mut FrameOutput) -> usize {
        out.clear();
        out.engine.append(&mut self.utf8_tail);
        let mut i = 0;
        while i < input.len() {
            let rest = &input[i..];
            let run = match self.state {
                State::Ground => {
                    let run = rest.iter().position(|&b| b == 0x1b).unwrap_or(rest.len());
                    out.engine.extend_from_slice(&rest[..run]);
                    run
                }
                State::Osc | State::Str(_) | State::StrDiscard => {
                    let run = rest.iter().position(|&b| is_string_end(b)).unwrap_or(rest.len());
                    self.collect_string(&rest[..run]);
                    run
                }
                _ => 0,
            };
            i += run;
            self.steps += run as u64;
            let Some(&b) = input.get(i) else { break };
            i += 1;
            self.steps += 1;
            if self.step(b, out) {
                return i;
            }
        }
        if self.state == State::Ground {
            let (complete, tail) = split_incomplete_utf8_tail(&out.engine);
            if !tail.is_empty() {
                let keep = complete.len();
                self.utf8_tail.extend_from_slice(tail);
                out.engine.truncate(keep);
            }
        }
        input.len()
    }

    fn collect_string(&mut self, run: &[u8]) {
        let limit = match self.state {
            State::Osc => MAX_OSC_BODY_BYTES,
            State::Str(true) => MAX_DCS_BODY_BYTES,
            State::Str(false) => MAX_APC_BODY_BYTES,
            _ => return,
        };
        if self.held.len() + run.len() > limit {
            self.drop_overlong(State::StrDiscard);
        } else {
            self.held.extend_from_slice(run);
        }
    }

    fn drop_overlong(&mut self, discard: State) {
        self.held = Vec::new();
        self.dropped += 1;
        self.state = discard;
    }

    /// Gives back the allocation of a large released sequence (an image, a clipboard write).
    fn shrink_held(&mut self) {
        if self.held.capacity() > RETAINED_BUFFER_BYTES {
            self.held = Vec::new();
        }
    }

    fn begin_escape(&mut self) {
        self.held.clear();
        self.held.push(0x1b);
        self.state = State::Escape;
    }

    /// `CAN` / `SUB`: the engine drops the sequence and executes the control byte.
    fn cancel(&mut self, b: u8, out: &mut FrameOutput) {
        self.held.clear();
        out.engine.push(b);
        self.state = State::Ground;
    }

    /// Advances by one byte; `true` when the feed must stop (Tektronix capture begins).
    fn step(&mut self, b: u8, out: &mut FrameOutput) -> bool {
        match self.state {
            State::Ground => self.begin_escape(),
            State::Escape | State::EscapeIntermediate | State::Csi | State::CsiDiscard => {
                return self.step_escape(b, out);
            }
            State::Osc => match b {
                0x07 => self.finish_osc(StringTerminator::Bel, out),
                0x1b => self.state = State::OscEsc,
                _ => self.cancel(b, out),
            },
            State::OscEsc | State::StrEsc(_) => {
                if b == b'\\' {
                    match self.state {
                        State::OscEsc => self.finish_osc(StringTerminator::St, out),
                        State::StrEsc(dcs) => self.finish_string(dcs, StringTerminator::St, out),
                        _ => {}
                    }
                } else {
                    // A bare ESC cancels the string and begins a new sequence with `b`.
                    self.begin_escape();
                    return self.step_escape(b, out);
                }
            }
            State::Str(dcs) => match b {
                0x07 => self.finish_string(dcs, StringTerminator::Bel, out),
                0x1b => self.state = State::StrEsc(dcs),
                _ => self.cancel(b, out),
            },
            State::StrDiscard => match b {
                0x07 => self.state = State::Ground,
                0x1b => self.begin_escape(),
                _ => self.cancel(b, out),
            },
        }
        false
    }

    fn step_escape(&mut self, b: u8, out: &mut FrameOutput) -> bool {
        match b {
            0x1b => self.begin_escape(),
            0x18 | 0x1a => self.cancel(b, out),
            // Executed by the engine immediately; the pending sequence is unaffected.
            0x00..=0x1f => out.engine.push(b),
            // Ignored by the engine inside escape sequences (DEL and non-ASCII).
            0x7f..=0xff => {}
            _ => return self.step_escape_printable(b, out),
        }
        false
    }

    fn step_escape_printable(&mut self, b: u8, out: &mut FrameOutput) -> bool {
        match self.state {
            State::Escape => match b {
                0x20..=0x2f => {
                    self.held.push(b);
                    self.state = State::EscapeIntermediate;
                }
                b'[' => {
                    self.held.push(b);
                    self.csi_phase = CsiPhase::Entry;
                    self.state = State::Csi;
                }
                b']' => {
                    self.held.push(b);
                    self.state = State::Osc;
                }
                b'P' | b'_' => {
                    self.held.clear();
                    self.state = State::Str(b == b'P');
                }
                b'X' | b'^' => {
                    self.held.clear();
                    self.state = State::StrDiscard;
                }
                _ => self.finish_esc(b, out),
            },
            State::EscapeIntermediate => match b {
                0x20..=0x2f => {
                    if self.held.len() <= MAX_ESC_BODY_BYTES {
                        self.held.push(b);
                    }
                }
                _ => self.finish_esc(b, out),
            },
            State::Csi => match b {
                0x20..=0x3f => {
                    self.csi_phase = match (self.csi_phase, b) {
                        (CsiPhase::Ignore, _) => CsiPhase::Ignore,
                        (_, 0x20..=0x2f) => CsiPhase::Intermediate,
                        (CsiPhase::Entry, _) | (CsiPhase::Param, 0x30..=0x3b) => CsiPhase::Param,
                        _ => CsiPhase::Ignore,
                    };
                    if self.held.len() >= MAX_CSI_BODY_BYTES + 2 {
                        self.drop_overlong(State::CsiDiscard);
                    } else {
                        self.held.push(b);
                    }
                }
                _ => return self.finish_csi(b, out),
            },
            State::CsiDiscard => {
                if (0x40..=0x7e).contains(&b) {
                    self.state = State::Ground;
                }
            }
            _ => {}
        }
        false
    }

    /// Releases the withheld sequence to the engine; returns where it starts in the buffer.
    fn release(&mut self, out: &mut FrameOutput) -> usize {
        let start = out.engine.len();
        out.engine.append(&mut self.held);
        self.shrink_held();
        self.state = State::Ground;
        start
    }

    fn finish_esc(&mut self, b: u8, out: &mut FrameOutput) {
        self.held.push(b);
        let event = matches!(self.held[1..], [b'#', b'3'..=b'6'] | [b'c']);
        let start = self.release(out);
        if event {
            let end = out.engine.len();
            out.seqs.push(FramedSeq {
                kind: SeqKind::Esc,
                terminator: StringTerminator::Bel,
                engine_end: end,
                in_engine: true,
                start: start + 1,
                end,
            });
        }
    }

    fn finish_csi(&mut self, b: u8, out: &mut FrameOutput) -> bool {
        self.held.push(b);
        let well_formed = self.csi_phase != CsiPhase::Ignore;
        if well_formed && self.held[2..] == *b"?38h" {
            self.held.clear();
            self.state = State::Ground;
            let end = out.engine.len();
            out.seqs.push(FramedSeq {
                kind: SeqKind::TekEnter,
                terminator: StringTerminator::Bel,
                engine_end: end,
                in_engine: true,
                start: end,
                end,
            });
            return true;
        }
        let start = self.release(out);
        if well_formed && CSI_EVENT_FINALS.contains(&b) {
            let end = out.engine.len();
            out.seqs.push(FramedSeq {
                kind: SeqKind::Csi,
                terminator: StringTerminator::Bel,
                engine_end: end,
                in_engine: true,
                start: start + 2,
                end,
            });
        }
        false
    }

    fn finish_osc(&mut self, terminator: StringTerminator, out: &mut FrameOutput) {
        let start = self.release(out);
        let end = out.engine.len();
        out.engine.extend_from_slice(terminator.as_str().as_bytes());
        out.seqs.push(FramedSeq {
            kind: SeqKind::Osc,
            terminator,
            engine_end: out.engine.len(),
            in_engine: true,
            start: start + 2,
            end,
        });
    }

    fn finish_string(&mut self, dcs: bool, terminator: StringTerminator, out: &mut FrameOutput) {
        let start = out.strings.len();
        out.strings.append(&mut self.held);
        self.shrink_held();
        self.state = State::Ground;
        out.seqs.push(FramedSeq {
            kind: if dcs { SeqKind::Dcs } else { SeqKind::Apc },
            terminator,
            engine_end: out.engine.len(),
            in_engine: false,
            start,
            end: out.strings.len(),
        });
    }
}
