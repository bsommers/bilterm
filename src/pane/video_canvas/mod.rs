//! Video Player Canvas state, triple-buffer ring, and playback engine (`VID-01..02`).

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::pane::browser_canvas::MediaRgbaFrame;
use crate::pane::PaneId;

pub mod ffmpeg;
pub mod osd;

pub use ffmpeg::FfmpegPipeBackend;
pub use osd::{VideoOsdHit, hit_test_video_osd_bar, render_video_osd_bar};

/// Video playback status snapshot (`VID-01..02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoPlaybackSnapshot {
    pub source: String,
    pub playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub volume_pct: u8,
    pub muted: bool,
    pub looping: bool,
    pub speed_x100: u16,
    pub width_px: u16,
    pub height_px: u16,
    pub frame_seq: u64,
    pub backend_name: &'static str,
}

/// Bounded 3-slot triple buffer ring for lock-free video frame delivery.
pub struct VideoFrameRing {
    slots: [Option<MediaRgbaFrame>; 3],
    head: usize,
}

impl Default for VideoFrameRing {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoFrameRing {
    pub fn new() -> Self {
        Self {
            slots: [None, None, None],
            head: 0,
        }
    }

    pub fn push(&mut self, frame: MediaRgbaFrame) {
        self.head = (self.head + 1) % 3;
        self.slots[self.head] = Some(frame);
    }

    pub fn latest(&self) -> Option<MediaRgbaFrame> {
        self.slots[self.head].clone()
    }
}

/// Canonical video player operations (`ARCH-R01`, `VID-01..02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoOp {
    Toggle,
    Open {
        source: String,
        pane_id: Option<PaneId>,
    },
    Play {
        pane_id: Option<PaneId>,
    },
    Pause {
        pane_id: Option<PaneId>,
    },
    TogglePlayPause {
        pane_id: Option<PaneId>,
    },
    SeekRelative {
        delta_ms: i64,
        pane_id: Option<PaneId>,
    },
    SeekPercent {
        pct: u8,
        pane_id: Option<PaneId>,
    },
    SeekAbsolute {
        position_ms: u64,
        pane_id: Option<PaneId>,
    },
    VolumeDelta {
        delta_pct: i8,
        pane_id: Option<PaneId>,
    },
    ToggleMute {
        pane_id: Option<PaneId>,
    },
    ToggleLoop {
        pane_id: Option<PaneId>,
    },
    SetSpeed {
        speed_x100: u16,
        pane_id: Option<PaneId>,
    },
    Close {
        pane_id: Option<PaneId>,
    },
}

/// Pluggable Video Player engine trait (`FfmpegPipeBackend`, `SyntheticVideoBackend`).
pub trait VideoPlayerEngine: Send {
    fn open(&mut self, source: &str, width_px: u16, height_px: u16) -> Result<()>;
    fn play(&mut self) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn toggle_play_pause(&mut self) -> Result<()>;
    fn seek_relative(&mut self, delta_ms: i64) -> Result<()>;
    fn seek_absolute(&mut self, position_ms: u64) -> Result<()>;
    fn set_volume(&mut self, volume_pct: u8) -> Result<()>;
    fn set_muted(&mut self, muted: bool) -> Result<()>;
    fn set_looping(&mut self, looping: bool) -> Result<()>;
    fn set_speed(&mut self, speed_x100: u16) -> Result<()>;
    fn resize_target(&mut self, width_px: u16, height_px: u16) -> Result<()>;
    fn tick(&mut self, now_ms: u64) -> Option<MediaRgbaFrame>;
    fn state_snapshot(&self) -> VideoPlaybackSnapshot;
}

/// Stateful video canvas manager owned by [`crate::pane::CompositePane`].
pub struct VideoCanvasState {
    engine: Box<dyn VideoPlayerEngine>,
    cached_frame: Option<MediaRgbaFrame>,
    last_tick_ms: u64,
}

impl VideoCanvasState {
    pub fn new(source: &str, width_px: u16, height_px: u16) -> Self {
        let mut engine: Box<dyn VideoPlayerEngine> =
            if source.starts_with("demo://") || !ffmpeg::is_ffmpeg_available() {
                Box::new(SyntheticVideoBackend::new(source, width_px, height_px))
            } else {
                Box::new(FfmpegPipeBackend::new(source, width_px, height_px))
            };
        let cached_frame = engine.tick(0);
        Self {
            engine,
            cached_frame,
            last_tick_ms: 0,
        }
    }

    pub fn new_synthetic(source: &str, width_px: u16, height_px: u16) -> Self {
        let mut engine = Box::new(SyntheticVideoBackend::new(source, width_px, height_px));
        let cached_frame = engine.tick(0);
        Self {
            engine,
            cached_frame,
            last_tick_ms: 0,
        }
    }

    pub fn open(&mut self, source: &str, width_px: u16, height_px: u16) -> Result<()> {
        self.engine.open(source, width_px, height_px)?;
        self.cached_frame = None;
        Ok(())
    }

    pub fn play(&mut self) -> Result<()> {
        self.engine.play()
    }

    pub fn pause(&mut self) -> Result<()> {
        self.engine.pause()
    }

    pub fn toggle_play_pause(&mut self) -> Result<()> {
        self.engine.toggle_play_pause()
    }

    pub fn seek_relative(&mut self, delta_ms: i64) -> Result<()> {
        self.engine.seek_relative(delta_ms)
    }

    pub fn seek_percent(&mut self, pct: u8) -> Result<()> {
        let snap = self.engine.state_snapshot();
        let target_ms = (snap.duration_ms.max(1) * u64::from(pct.min(100))) / 100;
        self.engine.seek_absolute(target_ms)
    }

    pub fn seek_absolute(&mut self, position_ms: u64) -> Result<()> {
        self.engine.seek_absolute(position_ms)
    }

    pub fn volume_delta(&mut self, delta_pct: i8) -> Result<()> {
        let snap = self.engine.state_snapshot();
        let cur = snap.volume_pct as i16;
        let next = (cur + delta_pct as i16).clamp(0, 100) as u8;
        self.engine.set_volume(next)
    }

    pub fn toggle_mute(&mut self) -> Result<()> {
        let snap = self.engine.state_snapshot();
        self.engine.set_muted(!snap.muted)
    }

    pub fn toggle_loop(&mut self) -> Result<()> {
        let snap = self.engine.state_snapshot();
        self.engine.set_looping(!snap.looping)
    }

    pub fn set_speed(&mut self, speed_x100: u16) -> Result<()> {
        self.engine.set_speed(speed_x100)
    }

    pub fn resize_target(&mut self, width_px: u16, height_px: u16) -> Result<()> {
        self.engine.resize_target(width_px, height_px)
    }

    pub fn tick(&mut self, now_ms: u64) -> bool {
        self.last_tick_ms = now_ms;
        if let Some(frame) = self.engine.tick(now_ms) {
            let changed = self.cached_frame.as_ref().map(|c| c.seq) != Some(frame.seq);
            self.cached_frame = Some(frame);
            changed
        } else {
            false
        }
    }

    pub fn cached_frame(&self) -> Option<&MediaRgbaFrame> {
        self.cached_frame.as_ref()
    }

    pub fn snapshot(&self) -> VideoPlaybackSnapshot {
        self.engine.state_snapshot()
    }
}

/// Built-in procedural/synthetic video generator (`demo://smpte`, `demo://plasma`).
pub struct SyntheticVideoBackend {
    source: String,
    width_px: u16,
    height_px: u16,
    playing: bool,
    position_ms: u64,
    duration_ms: u64,
    volume_pct: u8,
    muted: bool,
    looping: bool,
    speed_x100: u16,
    frame_seq: u64,
    ring: VideoFrameRing,
    last_tick_ms: u64,
}

impl SyntheticVideoBackend {
    pub fn new(source: &str, width_px: u16, height_px: u16) -> Self {
        let mut backend = Self {
            source: source.to_string(),
            width_px: width_px.clamp(64, 1920),
            height_px: height_px.clamp(64, 1080),
            playing: true,
            position_ms: 0,
            duration_ms: 60_000, // 60s demo loop
            volume_pct: 100,
            muted: false,
            looping: true,
            speed_x100: 100,
            frame_seq: 0,
            ring: VideoFrameRing::new(),
            last_tick_ms: 0,
        };
        backend.render_frame();
        backend
    }

    fn render_frame(&mut self) {
        self.frame_seq += 1;
        let w = self.width_px as usize;
        let h = self.height_px as usize;
        let mut rgba = vec![0u8; w * h * 4];

        if self.source.contains("plasma") {
            // Sine plasma generator
            let t = (self.position_ms as f64) / 500.0;
            for y in 0..h {
                let fy = (y as f64) / (h as f64);
                for x in 0..w {
                    let fx = (x as f64) / (w as f64);
                    let v = ((fx * 8.0 + t).sin() + (fy * 8.0 - t).cos() + ((fx + fy) * 4.0 + t).sin()) / 3.0;
                    let norm = ((v + 1.0) * 0.5 * 255.0) as u8;
                    let idx = (y * w + x) * 4;
                    rgba[idx] = norm;
                    rgba[idx + 1] = norm.wrapping_mul(2);
                    rgba[idx + 2] = 255u8.saturating_sub(norm);
                    rgba[idx + 3] = 255;
                }
            }
        } else {
            // SMPTE color bars (White, Yellow, Cyan, Green, Magenta, Red, Blue)
            const BARS: &[[u8; 3]] = &[
                [200, 200, 200], // White
                [200, 200, 0],   // Yellow
                [0, 200, 200],   // Cyan
                [0, 200, 0],     // Green
                [200, 0, 200],   // Magenta
                [200, 0, 0],     // Red
                [0, 0, 200],     // Blue
            ];
            let top_h = (h * 3) / 4;
            let bar_w = (w / BARS.len()).max(1);

            for y in 0..top_h {
                for x in 0..w {
                    let b_idx = (x / bar_w).min(BARS.len() - 1);
                    let color = BARS[b_idx];
                    let idx = (y * w + x) * 4;
                    rgba[idx] = color[0];
                    rgba[idx + 1] = color[1];
                    rgba[idx + 2] = color[2];
                    rgba[idx + 3] = 255;
                }
            }

            // Bottom section: black / dark grey
            for y in top_h..h {
                for x in 0..w {
                    let idx = (y * w + x) * 4;
                    rgba[idx] = 16;
                    rgba[idx + 1] = 16;
                    rgba[idx + 2] = 16;
                    rgba[idx + 3] = 255;
                }
            }

            // Moving ball / time indicator
            let progress = (self.position_ms as f64) / (self.duration_ms as f64);
            let ball_x = ((progress * (w as f64)) as usize).min(w.saturating_sub(1));
            let ball_y = top_h + (h.saturating_sub(top_h)) / 2;
            let rad = 6;
            for dy in ball_y.saturating_sub(rad)..=(ball_y + rad).min(h.saturating_sub(1)) {
                for dx in ball_x.saturating_sub(rad)..=(ball_x + rad).min(w.saturating_sub(1)) {
                    let dist = (dx as i32 - ball_x as i32).pow(2) + (dy as i32 - ball_y as i32).pow(2);
                    if dist <= (rad as i32).pow(2) {
                        let idx = (dy * w + dx) * 4;
                        rgba[idx] = 255;
                        rgba[idx + 1] = 255;
                        rgba[idx + 2] = 255;
                        rgba[idx + 3] = 255;
                    }
                }
            }
        }

        let frame = MediaRgbaFrame::new(
            self.frame_seq,
            self.width_px,
            self.height_px,
            self.position_ms,
            rgba,
        );
        self.ring.push(frame);
    }
}

impl VideoPlayerEngine for SyntheticVideoBackend {
    fn open(&mut self, source: &str, width_px: u16, height_px: u16) -> Result<()> {
        self.source = source.to_string();
        self.width_px = width_px.clamp(64, 1920);
        self.height_px = height_px.clamp(64, 1080);
        self.position_ms = 0;
        self.render_frame();
        Ok(())
    }

    fn play(&mut self) -> Result<()> {
        self.playing = true;
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.playing = false;
        Ok(())
    }

    fn toggle_play_pause(&mut self) -> Result<()> {
        self.playing = !self.playing;
        Ok(())
    }

    fn seek_relative(&mut self, delta_ms: i64) -> Result<()> {
        let cur = self.position_ms as i64;
        let dur = self.duration_ms as i64;
        let next = (cur + delta_ms).clamp(0, dur) as u64;
        self.position_ms = next;
        self.render_frame();
        Ok(())
    }

    fn seek_absolute(&mut self, position_ms: u64) -> Result<()> {
        self.position_ms = position_ms.min(self.duration_ms);
        self.render_frame();
        Ok(())
    }

    fn set_volume(&mut self, volume_pct: u8) -> Result<()> {
        self.volume_pct = volume_pct.min(100);
        Ok(())
    }

    fn set_muted(&mut self, muted: bool) -> Result<()> {
        self.muted = muted;
        Ok(())
    }

    fn set_looping(&mut self, looping: bool) -> Result<()> {
        self.looping = looping;
        Ok(())
    }

    fn set_speed(&mut self, speed_x100: u16) -> Result<()> {
        self.speed_x100 = speed_x100.clamp(25, 400);
        Ok(())
    }

    fn resize_target(&mut self, width_px: u16, height_px: u16) -> Result<()> {
        self.width_px = width_px.clamp(64, 1920);
        self.height_px = height_px.clamp(64, 1080);
        self.render_frame();
        Ok(())
    }

    fn tick(&mut self, now_ms: u64) -> Option<MediaRgbaFrame> {
        let delta = if self.last_tick_ms == 0 {
            16
        } else {
            now_ms.saturating_sub(self.last_tick_ms).min(200)
        };
        self.last_tick_ms = now_ms;

        if self.playing {
            let advance = (delta * u64::from(self.speed_x100)) / 100;
            self.position_ms += advance;
            if self.position_ms >= self.duration_ms {
                if self.looping {
                    self.position_ms %= self.duration_ms.max(1);
                } else {
                    self.position_ms = self.duration_ms;
                    self.playing = false;
                }
            }
            self.render_frame();
        }
        self.ring.latest()
    }

    fn state_snapshot(&self) -> VideoPlaybackSnapshot {
        VideoPlaybackSnapshot {
            source: self.source.clone(),
            playing: self.playing,
            position_ms: self.position_ms,
            duration_ms: self.duration_ms,
            volume_pct: self.volume_pct,
            muted: self.muted,
            looping: self.looping,
            speed_x100: self.speed_x100,
            width_px: self.width_px,
            height_px: self.height_px,
            frame_seq: self.frame_seq,
            backend_name: "synthetic",
        }
    }
}
