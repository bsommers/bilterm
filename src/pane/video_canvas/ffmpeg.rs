//! FFmpeg rawvideo RGBA pipe decoder and companion audio backend (`VID-01..02`).

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::error::{BiltermError, Result};
use crate::pane::browser_canvas::MediaRgbaFrame;

use super::{SyntheticVideoBackend, VideoPlaybackSnapshot, VideoPlayerEngine};

/// Checks if `ffmpeg` is available on the host system PATH.
pub fn is_ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Out-of-process FFmpeg rawvideo RGBA pipe player.
pub struct FfmpegPipeBackend {
    source: String,
    width_px: u16,
    height_px: u16,
    synthetic: SyntheticVideoBackend,
    child_ffmpeg: Option<Child>,
    child_ffplay: Option<Child>,
    running: Arc<AtomicBool>,
    /// Latest frame decoded by the background reader thread from the child's stdout.
    latest: Arc<Mutex<Option<MediaRgbaFrame>>>,
    reader: Option<std::thread::JoinHandle<()>>,
    /// Sequence of the last real frame handed out by `tick`.
    last_seq: Arc<AtomicU64>,
    live: bool,
}

impl FfmpegPipeBackend {
    pub fn new(source: &str, width_px: u16, height_px: u16) -> Self {
        let synthetic = SyntheticVideoBackend::new(source, width_px, height_px);
        let mut backend = Self {
            source: source.to_string(),
            width_px,
            height_px,
            synthetic,
            child_ffmpeg: None,
            child_ffplay: None,
            running: Arc::new(AtomicBool::new(true)),
            latest: Arc::new(Mutex::new(None)),
            reader: None,
            last_seq: Arc::new(AtomicU64::new(0)),
            live: false,
        };
        let _ = backend.spawn_pipeline();
        backend
    }

    /// Builds a backend around an arbitrary child command that writes raw RGBA frames
    /// (`width_px * height_px * 4` bytes each) to stdout. Used for tests and custom decoders.
    pub fn from_command(mut cmd: Command, width_px: u16, height_px: u16) -> Self {
        let mut backend = Self::new("pipe://custom", width_px, height_px);
        backend.kill_children();
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        if let Ok(child) = cmd.spawn() {
            backend.attach_child(child);
        }
        backend
    }

    /// True when a real child process pipe is feeding frames (vs. synthetic fallback).
    pub fn is_live(&self) -> bool {
        self.live
    }

    fn attach_child(&mut self, mut child: Child) {
        self.stop_reader();
        self.running = Arc::new(AtomicBool::new(true));
        self.latest = Arc::new(Mutex::new(None));
        if let Some(mut stdout) = child.stdout.take() {
            let (w, h) = (self.width_px, self.height_px);
            let running = Arc::clone(&self.running);
            let latest = Arc::clone(&self.latest);
            let frame_len = w as usize * h as usize * 4;
            self.reader = Some(std::thread::spawn(move || {
                let mut seq = 0u64;
                let mut buf = vec![0u8; frame_len];
                while running.load(Ordering::SeqCst) && frame_len > 0 {
                    if stdout.read_exact(&mut buf).is_err() {
                        break;
                    }
                    seq += 1;
                    let frame = MediaRgbaFrame::new(seq, w, h, 0, buf.clone());
                    if let Ok(mut slot) = latest.lock() {
                        *slot = Some(frame);
                    }
                }
            }));
            self.live = true;
        }
        self.child_ffmpeg = Some(child);
    }

    fn stop_reader(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        // Reader unblocks once the child is killed (EOF); detach if still pending.
        self.reader = None;
        self.live = false;
    }

    fn spawn_pipeline(&mut self) -> Result<()> {
        if self.source.starts_with("demo://") || !is_ffmpeg_available() {
            return Ok(());
        }

        // Spawn ffmpeg decoding rawvideo into pipe
        let child = Command::new("ffmpeg")
            .arg("-nostdin")
            .arg("-loglevel")
            .arg("error")
            .arg("-re")
            .arg("-i")
            .arg(&self.source)
            .arg("-vf")
            .arg(format!("scale={}:{}:flags=fast_bilinear", self.width_px, self.height_px))
            .arg("-pix_fmt")
            .arg("rgba")
            .arg("-f")
            .arg("rawvideo")
            .arg("-")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(BiltermError::Io)?;

        self.attach_child(child);
        Ok(())
    }

    fn kill_children(&mut self) {
        self.stop_reader();
        if let Some(mut child) = self.child_ffmpeg.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(mut child) = self.child_ffplay.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for FfmpegPipeBackend {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        self.kill_children();
    }
}

impl VideoPlayerEngine for FfmpegPipeBackend {
    fn open(&mut self, source: &str, width_px: u16, height_px: u16) -> Result<()> {
        self.kill_children();
        self.source = source.to_string();
        self.width_px = width_px;
        self.height_px = height_px;
        self.synthetic.open(source, width_px, height_px)?;
        self.spawn_pipeline()
    }

    fn play(&mut self) -> Result<()> {
        self.synthetic.play()
    }

    fn pause(&mut self) -> Result<()> {
        self.synthetic.pause()
    }

    fn toggle_play_pause(&mut self) -> Result<()> {
        self.synthetic.toggle_play_pause()
    }

    fn seek_relative(&mut self, delta_ms: i64) -> Result<()> {
        self.synthetic.seek_relative(delta_ms)
    }

    fn seek_absolute(&mut self, position_ms: u64) -> Result<()> {
        self.synthetic.seek_absolute(position_ms)
    }

    fn set_volume(&mut self, volume_pct: u8) -> Result<()> {
        self.synthetic.set_volume(volume_pct)
    }

    fn set_muted(&mut self, muted: bool) -> Result<()> {
        self.synthetic.set_muted(muted)
    }

    fn set_looping(&mut self, looping: bool) -> Result<()> {
        self.synthetic.set_looping(looping)
    }

    fn set_speed(&mut self, speed_x100: u16) -> Result<()> {
        self.synthetic.set_speed(speed_x100)
    }

    fn resize_target(&mut self, width_px: u16, height_px: u16) -> Result<()> {
        self.width_px = width_px;
        self.height_px = height_px;
        self.synthetic.resize_target(width_px, height_px)
    }

    fn tick(&mut self, now_ms: u64) -> Option<MediaRgbaFrame> {
        if self.live {
            let frame = self.latest.lock().ok().and_then(|g| g.clone());
            if let Some(f) = frame {
                self.last_seq.store(f.seq, Ordering::SeqCst);
                return Some(f);
            }
            return None;
        }
        self.synthetic.tick(now_ms)
    }

    fn state_snapshot(&self) -> VideoPlaybackSnapshot {
        let mut snap = self.synthetic.state_snapshot();
        snap.backend_name = if self.live { "ffmpeg-pipe" } else { "synthetic" };
        snap
    }
}
