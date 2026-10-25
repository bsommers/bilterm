//! Classic Tektronix 4010 / 4014 Vector Storage-Tube Terminal State Machine (`TEK-01`).
//!
//! Emulates the classic Tektronix 4010 (10-bit `1024×780`) and Tektronix 4014 (12-bit `4096×3120`)
//! direct-view storage CRT graphics protocol inside any Bilterm [`CompositePane`](crate::pane::CompositePane):
//! - `\x1b[?38h` (`DECSET ?38h`): Enter Tektronix 4010/4014 graphics mode.
//! - `\x1b[?38l` (`DECRST ?38l`) / `\x1b\x03` (`ESC ETX`): Return to VT100 mode.
//! - `GS` (`0x1D`): Enter Vector Graph mode (first coordinate is a dark-vector move; subsequent
//!   coordinates draw bright storage-tube vectors).
//! - `FS` (`0x1C`): Enter Point Plot mode (each coordinate plots a single storage-tube beam dot).
//! - `RS` (`0x1E`): Enter Incremental Plot mode (`' '` pen up, `'P'` pen down, `'A'`/`'E'`/`'D'`/`'F'`/`'B'`/`'J'`/`'H'`/`'I'` 8-way step).
//! - `US` (`0x1F`) / `\r` / `\n`: Return to Alpha text mode at the current beam position.
//! - `\x1b` + `` ` ``..`d` (`0x60..=0x64`): Select Tek 4014 vector line style (`Solid`, `Dotted`, `DotDash`, `ShortDash`, `LongDash`).
//! - `\x1b\x0c` (`ESC FF`): Storage-tube screen erase with green (`3278`) / amber (`3279`) phosphor persistence afterglow decay.

use serde::{Deserialize, Serialize};

/// Logical Tektronix 4014 12-bit horizontal coordinate resolution (`0..4096`).
pub const TEK_MAX_X: u16 = 4096;
/// Logical Tektronix 4014 12-bit vertical coordinate resolution (`0..3120`).
pub const TEK_MAX_Y: u16 = 3120;

/// Active Tektronix 4010/4014 sub-mode (`TEK-01`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TekMode {
    /// Alphanumeric character mode (`US` `0x1F`).
    #[default]
    Alpha,
    /// Vector line-drawing mode (`GS` `0x1D`).
    Graph,
    /// Discrete point-plotting mode (`FS` `0x1C`).
    PointPlot,
    /// Relative step-by-step plotter mode (`RS` `0x1E`).
    IncrementalPlot,
}

/// Tektronix 4014 hardware vector line style (`ESC` + `0x60..=0x64`) (`TEK-01`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TekLineStyle {
    /// `\x1b`` (`0x60`) — Continuous solid storage-tube beam.
    #[default]
    Solid,
    /// `\x1ba` (`0x61`) — Dotted vector pattern.
    Dotted,
    /// `\x1bb` (`0x62`) — Dot-dashed vector pattern.
    DotDash,
    /// `\x1bc` (`0x63`) — Short-dashed vector pattern.
    ShortDash,
    /// `\x1bd` (`0x64`) — Long-dashed vector pattern.
    LongDash,
}

impl TekLineStyle {
    /// Returns `true` when step `idx` along a rasterized vector should illuminate the phosphor beam.
    #[inline]
    pub const fn pattern_bit(self, idx: usize) -> bool {
        match self {
            Self::Solid => true,
            Self::Dotted => idx.is_multiple_of(2),
            Self::DotDash => !matches!(idx % 8, 1 | 2 | 6 | 7),
            Self::ShortDash => (idx % 4) < 2,
            Self::LongDash => (idx % 8) < 5,
        }
    }
}

/// A single vector or point primitive in Tektronix `4096×3120` coordinates (`TEK-01`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TekVectorSegment {
    pub x0: u16,
    pub y0: u16,
    pub x1: u16,
    pub y1: u16,
    pub style: TekLineStyle,
    pub point_only: bool,
    /// `0` = active storage-tube vector; `1..=3` = decaying phosphor afterglow following `\x1b\x0c` erase.
    pub phosphor_age: u8,
}

/// Positioned alphanumeric label drawn in Tek `Alpha` mode (`US` `0x1F`) (`TEK-01`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TekAlphaLabel {
    pub x: u16,
    pub y: u16,
    pub text: String,
}

/// Complete Tektronix 4010/4014 storage-tube state machine (`TEK-01`).
#[derive(Clone, Debug, Default)]
pub struct TektronixState {
    pub active: bool,
    /// `true` only when entered via in-band `DECSET ?38h` (`\x1b[?38h`) so interactive `:tek demo`
    /// never swallows subsequent VT100 shell text output.
    pub stream_capture: bool,
    pub mode: TekMode,
    pub line_style: TekLineStyle,
    pub beam_x: u16,
    pub beam_y: u16,
    pen_down: bool,
    esc_pending: bool,
    csi_pending: bool,
    csi_buf: Vec<u8>,
    hi_y: u16,
    lo_y: u16,
    hi_x: u16,
    extra_byte: u8,
    has_extra: bool,
    saw_lo_y: bool,
    pub segments: Vec<TekVectorSegment>,
    pub decaying_segments: Vec<TekVectorSegment>,
    pub labels: Vec<TekAlphaLabel>,
    pub erase_count: u32,
}

impl TektronixState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` when there are any active or decaying Tektronix vectors or labels to render.
    pub fn has_visible_content(&self) -> bool {
        self.active
            || !self.segments.is_empty()
            || !self.decaying_segments.is_empty()
            || !self.labels.is_empty()
    }

    /// Triggers a storage-tube screen erase (`\x1b\x0c`), moving current bright vectors into
    /// `decaying_segments` (`phosphor_age = 1`) so they emit a green/amber persistence afterglow (`TEK-01`).
    pub fn erase_screen_with_persistence(&mut self) {
        self.erase_count = self.erase_count.saturating_add(1);
        for seg in &mut self.decaying_segments {
            seg.phosphor_age = seg.phosphor_age.saturating_add(1);
        }
        self.decaying_segments.retain(|s| s.phosphor_age <= 2);
        for mut seg in self.segments.drain(..) {
            seg.phosphor_age = 1;
            self.decaying_segments.push(seg);
        }
        self.labels.clear();
        self.mode = TekMode::Alpha;
        self.pen_down = false;
    }

    /// Advances phosphor afterglow decay by one step (`TEK-01`).
    pub fn step_phosphor_decay(&mut self) {
        for seg in &mut self.decaying_segments {
            seg.phosphor_age = seg.phosphor_age.saturating_add(1);
        }
        self.decaying_segments.retain(|s| s.phosphor_age <= 2);
    }

    /// Immediately clears all active and decaying vectors and labels.
    pub fn clear_all(&mut self) {
        self.active = false;
        self.stream_capture = false;
        self.segments.clear();
        self.decaying_segments.clear();
        self.labels.clear();
        self.mode = TekMode::Alpha;
        self.pen_down = false;
    }

    /// Populates a classic Tektronix 4014 wireframe oscilloscope & geometric demo scene (`:canvas tek demo`).
    pub fn load_demo_scene(&mut self) {
        self.clear_all();
        self.active = true;
        // Outer 12-bit frame border
        self.segments.push(TekVectorSegment {
            x0: 200,
            y0: 200,
            x1: 3895,
            y1: 200,
            style: TekLineStyle::Solid,
            point_only: false,
            phosphor_age: 0,
        });
        self.segments.push(TekVectorSegment {
            x0: 3895,
            y0: 200,
            x1: 3895,
            y1: 2919,
            style: TekLineStyle::Solid,
            point_only: false,
            phosphor_age: 0,
        });
        self.segments.push(TekVectorSegment {
            x0: 3895,
            y0: 2919,
            x1: 200,
            y1: 2919,
            style: TekLineStyle::Solid,
            point_only: false,
            phosphor_age: 0,
        });
        self.segments.push(TekVectorSegment {
            x0: 200,
            y0: 2919,
            x1: 200,
            y1: 200,
            style: TekLineStyle::Solid,
            point_only: false,
            phosphor_age: 0,
        });
        // Cross-hair graticule (dotted) & diagonal vector
        self.segments.push(TekVectorSegment {
            x0: 200,
            y0: 1560,
            x1: 3895,
            y1: 1560,
            style: TekLineStyle::Dotted,
            point_only: false,
            phosphor_age: 0,
        });
        self.segments.push(TekVectorSegment {
            x0: 2048,
            y0: 200,
            x1: 2048,
            y1: 2919,
            style: TekLineStyle::Dotted,
            point_only: false,
            phosphor_age: 0,
        });
        self.segments.push(TekVectorSegment {
            x0: 300,
            y0: 300,
            x1: 3800,
            y1: 2800,
            style: TekLineStyle::Solid,
            point_only: false,
            phosphor_age: 0,
        });
        self.labels.push(TekAlphaLabel {
            x: 320,
            y: 2800,
            text: "TEK 4014 STORAGE TUBE".to_string(),
        });
    }

    /// Feeds a raw byte stream into the Tektronix 4010/4014 state machine (`TEK-01`).
    pub fn process_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.process_byte(b);
        }
    }

    /// Processes `bytes` through the Tektronix 4010/4014 state machine and returns only the bytes
    /// that belong to the standard VT100 stream (so Tektronix vector coordinates never print
    /// raw ASCII characters onto the VT100 text screen) (`TEK-01`).
    pub fn process_and_filter_vt100(&mut self, bytes: &[u8]) -> Vec<u8> {
        if !self.stream_capture && !bytes.windows(5).any(|w| w == b"[?38h") {
            return bytes.to_vec();
        }
        let mut vt_out = Vec::with_capacity(bytes.len());
        for &b in bytes {
            let was_capturing = self.stream_capture;
            self.process_byte(b);
            if !was_capturing && !self.stream_capture && !self.esc_pending && !self.csi_pending {
                vt_out.push(b);
            }
        }
        vt_out
    }

    fn process_byte(&mut self, b: u8) {
        if self.csi_pending {
            self.csi_buf.push(b);
            if (0x40..=0x7e).contains(&b) {
                if self.csi_buf == b"?38h" {
                    self.active = true;
                    self.stream_capture = true;
                    self.mode = TekMode::Alpha;
                } else if self.csi_buf == b"?38l" {
                    self.active = false;
                    self.stream_capture = false;
                    self.mode = TekMode::Alpha;
                }
                self.csi_pending = false;
                self.csi_buf.clear();
            }
            return;
        }

        if self.esc_pending {
            self.esc_pending = false;
            match b {
                b'[' => {
                    self.csi_pending = true;
                    self.csi_buf.clear();
                    return;
                }
                0x0c => {
                    if self.active {
                        self.erase_screen_with_persistence();
                    }
                    return;
                }
                0x03 => {
                    self.active = false;
                    self.stream_capture = false;
                    self.mode = TekMode::Alpha;
                    return;
                }
                b'`' => {
                    self.line_style = TekLineStyle::Solid;
                    return;
                }
                b'a' => {
                    self.line_style = TekLineStyle::Dotted;
                    return;
                }
                b'b' => {
                    self.line_style = TekLineStyle::DotDash;
                    return;
                }
                b'c' => {
                    self.line_style = TekLineStyle::ShortDash;
                    return;
                }
                b'd' => {
                    self.line_style = TekLineStyle::LongDash;
                    return;
                }
                _ => return,
            }
        }

        if b == 0x1b {
            self.esc_pending = true;
            return;
        }
        if !self.active {
            return;
        }

        match b {
            0x1d => {
                // GS: Enter Graph vector mode (first coordinate is dark-vector move)
                self.mode = TekMode::Graph;
                self.pen_down = false;
                self.saw_lo_y = false;
                self.has_extra = false;
            }
            0x1c => {
                // FS: Enter Point Plot mode
                self.mode = TekMode::PointPlot;
                self.pen_down = true;
                self.saw_lo_y = false;
                self.has_extra = false;
            }
            0x1e => {
                // RS: Enter Incremental Plot mode
                self.mode = TekMode::IncrementalPlot;
                self.pen_down = false;
            }
            0x1f | b'\r' | b'\n' => {
                // US / CR / LF: Return to Alpha mode
                self.mode = TekMode::Alpha;
                self.pen_down = false;
                self.saw_lo_y = false;
                self.has_extra = false;
            }
            _ => match self.mode {
                TekMode::Graph | TekMode::PointPlot => self.decode_coord_byte(b),
                TekMode::IncrementalPlot => self.decode_incremental_byte(b),
                TekMode::Alpha => {
                    if (0x20..=0x7e).contains(&b) {
                        if let Some(last) = self
                            .labels
                            .last_mut()
                            .filter(|l| l.x == self.beam_x && l.y == self.beam_y)
                        {
                            last.text.push(b as char);
                        } else {
                            self.labels.push(TekAlphaLabel {
                                x: self.beam_x,
                                y: self.beam_y,
                                text: (b as char).to_string(),
                            });
                        }
                    }
                }
            },
        }
    }

    fn decode_coord_byte(&mut self, b: u8) {
        match b {
            0x20..=0x3f => {
                let bits = u16::from(b & 0x1f);
                if !self.saw_lo_y {
                    self.hi_y = bits;
                } else {
                    self.hi_x = bits;
                }
            }
            0x60..=0x7f => {
                let bits = u16::from(b & 0x1f);
                if self.saw_lo_y {
                    // Previous 0x60..=0x7f was the 12-bit extra byte; this one is lo_y
                    self.extra_byte = (self.lo_y as u8) & 0x0f;
                    self.has_extra = true;
                    self.lo_y = bits;
                } else {
                    self.lo_y = bits;
                    self.saw_lo_y = true;
                }
            }
            0x40..=0x5f => {
                let lo_x = u16::from(b & 0x1f);
                let x_10 = (self.hi_x << 5) | lo_x;
                let y_10 = (self.hi_y << 5) | self.lo_y;
                let (x_12, y_12) = if self.has_extra {
                    let x_low2 = u16::from(self.extra_byte & 0x03);
                    let y_low2 = u16::from((self.extra_byte >> 2) & 0x03);
                    ((x_10 << 2) | x_low2, (y_10 << 2) | y_low2)
                } else {
                    (x_10 << 2, y_10 << 2)
                };
                let target_x = x_12.min(TEK_MAX_X - 1);
                let target_y = y_12.min(TEK_MAX_Y - 1);

                if self.mode == TekMode::PointPlot {
                    self.segments.push(TekVectorSegment {
                        x0: target_x,
                        y0: target_y,
                        x1: target_x,
                        y1: target_y,
                        style: self.line_style,
                        point_only: true,
                        phosphor_age: 0,
                    });
                } else if self.pen_down {
                    self.segments.push(TekVectorSegment {
                        x0: self.beam_x,
                        y0: self.beam_y,
                        x1: target_x,
                        y1: target_y,
                        style: self.line_style,
                        point_only: false,
                        phosphor_age: 0,
                    });
                } else {
                    self.pen_down = true;
                }
                self.beam_x = target_x;
                self.beam_y = target_y;
                self.saw_lo_y = false;
                self.has_extra = false;
            }
            _ => {}
        }
    }

    fn decode_incremental_byte(&mut self, b: u8) {
        let step: i32 = 16;
        let (dx, dy) = match b {
            b' ' => {
                self.pen_down = false;
                return;
            }
            b'P' => {
                self.pen_down = true;
                return;
            }
            b'A' => (step, 0),
            b'E' => (step, step),
            b'D' => (0, step),
            b'F' => (-step, step),
            b'B' => (-step, 0),
            b'J' => (-step, -step),
            b'H' => (0, -step),
            b'I' => (step, -step),
            _ => return,
        };
        let nx = (i32::from(self.beam_x) + dx).clamp(0, i32::from(TEK_MAX_X - 1)) as u16;
        let ny = (i32::from(self.beam_y) + dy).clamp(0, i32::from(TEK_MAX_Y - 1)) as u16;
        if self.pen_down {
            self.segments.push(TekVectorSegment {
                x0: self.beam_x,
                y0: self.beam_y,
                x1: nx,
                y1: ny,
                style: self.line_style,
                point_only: false,
                phosphor_age: 0,
            });
        }
        self.beam_x = nx;
        self.beam_y = ny;
    }
}

/// Encodes a 10-bit Tektronix 4010 coordinate `(x_1024, y_780)` into `[Hi-Y, Lo-Y, Hi-X, Lo-X]` bytes (`TEK-01`).
pub const fn encode_tek_10bit_point(x_1024: u16, y_780: u16) -> [u8; 4] {
    let x = if x_1024 > 1023 { 1023 } else { x_1024 };
    let y = if y_780 > 779 { 779 } else { y_780 };
    let hi_y = 0x20 | (((y >> 5) & 0x1f) as u8);
    let lo_y = 0x60 | ((y & 0x1f) as u8);
    let hi_x = 0x20 | (((x >> 5) & 0x1f) as u8);
    let lo_x = 0x40 | ((x & 0x1f) as u8);
    [hi_y, lo_y, hi_x, lo_x]
}

/// Encodes a 12-bit Tektronix 4014 coordinate `(x_4096, y_3120)` into `[Hi-Y, Extra, Lo-Y, Hi-X, Lo-X]` bytes (`TEK-01`).
pub const fn encode_tek_12bit_point(x_4096: u16, y_3120: u16) -> [u8; 5] {
    let x = if x_4096 > 4095 { 4095 } else { x_4096 };
    let y = if y_3120 > 3119 { 3119 } else { y_3120 };
    let hi_y = 0x20 | (((y >> 7) & 0x1f) as u8);
    let extra = 0x60 | ((((y & 0x03) << 2) | (x & 0x03)) as u8);
    let lo_y = 0x60 | (((y >> 2) & 0x1f) as u8);
    let hi_x = 0x20 | (((x >> 7) & 0x1f) as u8);
    let lo_x = 0x40 | (((x >> 2) & 0x1f) as u8);
    [hi_y, extra, lo_y, hi_x, lo_x]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tek_demo_does_not_swallow_subsequent_vt100_terminal_text() {
        let mut tek = TektronixState::default();
        tek.load_demo_scene();
        assert!(tek.active);
        assert!(!tek.stream_capture);
        assert!(tek.has_visible_content());

        let vt_bytes = b"ls -la\r\ntotal 42\r\n";
        let passthrough = tek.process_and_filter_vt100(vt_bytes);
        assert_eq!(passthrough, vt_bytes);
    }

    #[test]
    fn inband_decset_38h_captures_tek_stream_until_38l() {
        let mut tek = TektronixState::default();
        let stream = b"BEFORE\x1b[?38h\x1d\x20\x40\x20\x40\x1b[?38lAFTER";
        let passthrough = tek.process_and_filter_vt100(stream);
        assert_eq!(passthrough, b"BEFOREAFTER");
        assert!(!tek.stream_capture);
    }
}
