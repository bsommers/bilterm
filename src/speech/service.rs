//! Background speech worker: bounded queue, off-thread synthesis and playback, cancellable.
//!
//! The UI/event loop only ever calls [`SpeechService::submit`] / [`SpeechService::stop`] /
//! [`SpeechService::status`], all of which are O(1) and never block on audio.

use super::player::Player;
use super::render::{check_text, render_text};
use super::voice::SayOptions;
use crate::secure_fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Maximum number of utterances waiting (including the one being spoken).
pub const QUEUE_DEPTH: usize = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpeechState {
    #[default]
    Idle,
    Rendering,
    Speaking,
}

impl SpeechState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Rendering => "rendering",
            Self::Speaking => "speaking",
        }
    }
}

/// Snapshot of the speech subsystem.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpeechStatus {
    pub state: SpeechState,
    /// Utterances queued or in progress.
    pub pending: usize,
    /// Utterances completed since start.
    pub spoken: u64,
    pub last_error: Option<String>,
    /// File name of the last WAV written by `--wav`.
    pub last_wav: Option<String>,
    pub last_duration_ms: u64,
    pub last_truncated: bool,
    pub player: String,
}

struct Job {
    text: String,
    opts: SayOptions,
    wav_name: Option<String>,
    generation: u64,
}

enum Cmd {
    Say(Job),
    Shutdown,
}

struct Shared {
    generation: AtomicU64,
    pending: AtomicUsize,
    status: Mutex<SpeechStatus>,
}

/// Owner of the worker thread.
pub struct SpeechService {
    tx: SyncSender<Cmd>,
    shared: Arc<Shared>,
    player: Arc<dyn Player>,
    thread: Option<JoinHandle<()>>,
}

/// Resolves the private directory for generated WAVs (`<runtime>/speech`, 0700).
pub fn default_out_dir() -> std::io::Result<PathBuf> {
    secure_fs::private_dir_in(&secure_fs::private_runtime_dir()?, "speech")
}

impl SpeechService {
    /// Starts the worker. `out_dir` overrides where WAVs go (default: [`default_out_dir`]).
    pub fn new(player: Arc<dyn Player>, out_dir: Option<PathBuf>) -> Self {
        let (tx, rx) = sync_channel::<Cmd>(QUEUE_DEPTH);
        let shared = Arc::new(Shared {
            generation: AtomicU64::new(0),
            pending: AtomicUsize::new(0),
            status: Mutex::new(SpeechStatus { player: player.name(), ..Default::default() }),
        });
        let (sh, pl) = (shared.clone(), player.clone());
        let thread = std::thread::Builder::new()
            .name("bilterm-speech".to_string())
            .spawn(move || worker(rx, sh, pl, out_dir))
            .ok();
        Self { tx, shared, player, thread }
    }

    /// Queues an utterance. With `wav_name` the audio is written to `<out_dir>/<wav_name>.wav`
    /// instead of played. Returns the queue position (1 = next up) or why it was refused.
    pub fn submit(&self, text: &str, opts: SayOptions, wav_name: Option<String>) -> Result<usize, String> {
        check_text(text)?;
        if let Some(n) = &wav_name {
            secure_fs::validate_safe_name(n).map_err(|e| format!("invalid wav name: {e}"))?;
        } else {
            self.player.available()?;
        }
        if self.shared.pending.fetch_add(1, Ordering::SeqCst) >= QUEUE_DEPTH {
            self.shared.pending.fetch_sub(1, Ordering::SeqCst);
            return Err(format!("speech queue full (max {QUEUE_DEPTH}); use :say stop"));
        }
        let job = Job {
            text: text.to_string(),
            opts,
            wav_name,
            generation: self.shared.generation.load(Ordering::SeqCst),
        };
        match self.tx.try_send(Cmd::Say(job)) {
            Ok(()) => Ok(self.shared.pending.load(Ordering::SeqCst)),
            Err(e) => {
                self.shared.pending.fetch_sub(1, Ordering::SeqCst);
                Err(match e {
                    TrySendError::Full(_) => format!("speech queue full (max {QUEUE_DEPTH}); use :say stop"),
                    TrySendError::Disconnected(_) => "speech worker is not running".to_string(),
                })
            }
        }
    }

    /// Cancels the current utterance and everything queued. Returns how many were pending.
    pub fn stop(&self) -> usize {
        let n = self.shared.pending.load(Ordering::SeqCst);
        self.shared.generation.fetch_add(1, Ordering::SeqCst);
        n
    }

    pub fn status(&self) -> SpeechStatus {
        let mut s = self.shared.status.lock().expect("speech status").clone();
        s.pending = self.shared.pending.load(Ordering::SeqCst);
        s
    }
}

impl Drop for SpeechService {
    fn drop(&mut self) {
        self.shared.generation.fetch_add(1, Ordering::SeqCst);
        let _ = self.tx.try_send(Cmd::Shutdown);
        if let Some(t) = self.thread.take() {
            // Dropping `tx` (below, with `self`) also ends the loop if the queue was full.
            let tx = std::mem::replace(&mut self.tx, sync_channel(1).0);
            drop(tx);
            let _ = t.join();
        }
    }
}

fn set_state(sh: &Shared, st: SpeechState) {
    sh.status.lock().expect("speech status").state = st;
}

fn worker(rx: Receiver<Cmd>, sh: Arc<Shared>, player: Arc<dyn Player>, out_dir: Option<PathBuf>) {
    while let Ok(cmd) = rx.recv() {
        let Cmd::Say(job) = cmd else { break };
        let cancelled = || sh.generation.load(Ordering::SeqCst) != job.generation;
        if !cancelled() {
            set_state(&sh, SpeechState::Rendering);
            let result = speak(&job, &*player, out_dir.as_deref(), &cancelled, &|| set_state(&sh, SpeechState::Speaking));
            let mut st = sh.status.lock().expect("speech status");
            match result {
                Ok(done) => {
                    st.last_error = None;
                    st.last_duration_ms = done.duration_ms;
                    st.last_truncated = done.truncated;
                    if done.wav.is_some() {
                        st.last_wav = done.wav;
                    }
                    if !cancelled() {
                        st.spoken += 1;
                    }
                }
                Err(e) => st.last_error = Some(e),
            }
            st.state = SpeechState::Idle;
        }
        sh.pending.fetch_sub(1, Ordering::SeqCst);
    }
}

struct Done {
    duration_ms: u64,
    truncated: bool,
    wav: Option<String>,
}

fn speak(
    job: &Job,
    player: &dyn Player,
    out_dir: Option<&std::path::Path>,
    cancelled: &dyn Fn() -> bool,
    on_play: &dyn Fn(),
) -> Result<Done, String> {
    let audio = render_text(&job.text, &job.opts)?;
    let wav = audio.to_wav();
    let dir = match out_dir {
        Some(d) => {
            secure_fs::create_private_dir_all(d).map_err(|e| format!("speech dir: {e}"))?;
            d.to_path_buf()
        }
        None => default_out_dir().map_err(|e| format!("speech dir: {e}"))?,
    };
    let mut done = Done { duration_ms: audio.duration_ms(), truncated: audio.truncated, wav: None };
    if let Some(name) = &job.wav_name {
        let file = format!("{name}.wav");
        secure_fs::write_private_file(&dir.join(&file), &wav).map_err(|e| format!("cannot write {file}: {e}"))?;
        done.wav = Some(file);
        return Ok(done);
    }
    if cancelled() {
        return Ok(done);
    }
    let path = dir.join(format!("play-{}.wav", secure_fs::unique_suffix()));
    secure_fs::write_private_file(&path, &wav).map_err(|e| format!("cannot write temp wav: {e}"))?;
    on_play();
    let res = player.play(&path, cancelled);
    let _ = std::fs::remove_file(&path);
    res.map(|()| done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::player::RecordingPlayer;
    use std::time::{Duration, Instant};

    fn wait_idle(s: &SpeechService) {
        let t = Instant::now();
        while s.status().pending > 0 {
            assert!(t.elapsed() < Duration::from_secs(20), "timeout waiting for idle");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn svc(p: RecordingPlayer, dir: &std::path::Path) -> (SpeechService, Arc<RecordingPlayer>) {
        let p = Arc::new(p);
        (SpeechService::new(p.clone(), Some(dir.to_path_buf())), p)
    }

    #[test]
    fn speaks_through_the_player_off_thread() {
        let d = tempfile::tempdir().unwrap();
        let (s, p) = svc(RecordingPlayer::default(), d.path());
        assert_eq!(s.submit("hello there", SayOptions::default(), None), Ok(1));
        wait_idle(&s);
        assert_eq!(p.played.lock().unwrap().len(), 1);
        assert_eq!(s.status().spoken, 1);
        assert_eq!(s.status().last_error, None);
        assert!(std::fs::read_dir(d.path()).unwrap().next().is_none(), "temp wav removed");
    }

    #[test]
    fn writes_wav_instead_of_playing() {
        let d = tempfile::tempdir().unwrap();
        let (s, p) = svc(RecordingPlayer::default(), d.path());
        s.submit("write me", SayOptions::default(), Some("out1".into())).unwrap();
        wait_idle(&s);
        assert!(p.played.lock().unwrap().is_empty());
        let bytes = std::fs::read(d.path().join("out1.wav")).unwrap();
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(s.status().last_wav.as_deref(), Some("out1.wav"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(d.path().join("out1.wav")).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn rejects_bad_wav_names_and_text() {
        let d = tempfile::tempdir().unwrap();
        let (s, _) = svc(RecordingPlayer::default(), d.path());
        for bad in ["../x", "a/b", "a.b", "", "CON"] {
            assert!(s.submit("hi", SayOptions::default(), Some(bad.into())).is_err(), "{bad}");
        }
        assert!(s.submit("   ", SayOptions::default(), None).is_err());
        assert!(s.submit(&"a".repeat(5000), SayOptions::default(), None).is_err());
    }

    #[test]
    fn reports_missing_player() {
        let d = tempfile::tempdir().unwrap();
        let (s, _) = svc(RecordingPlayer { unavailable: Some("no player".into()), ..Default::default() }, d.path());
        assert_eq!(s.submit("hi", SayOptions::default(), None), Err("no player".to_string()));
        assert_eq!(s.status().pending, 0);
        s.submit("hi", SayOptions::default(), Some("ok".into())).unwrap();
    }

    #[test]
    fn queue_is_bounded_and_stop_cancels_everything() {
        let d = tempfile::tempdir().unwrap();
        let (s, p) = svc(RecordingPlayer { hold: Duration::from_secs(30), ..Default::default() }, d.path());
        let mut accepted = 0;
        for _ in 0..QUEUE_DEPTH + 3 {
            if s.submit("queue test", SayOptions::default(), None).is_ok() {
                accepted += 1;
            }
        }
        assert_eq!(accepted, QUEUE_DEPTH);
        let t = Instant::now();
        assert!(s.stop() >= 1);
        wait_idle(&s);
        assert!(t.elapsed() < Duration::from_secs(5), "stop is prompt");
        assert!(p.played.lock().unwrap().is_empty());
        assert!(s.submit("again", SayOptions::default(), None).is_ok(), "usable after stop");
        s.stop();
    }

    #[test]
    fn drop_does_not_hang_with_a_busy_worker() {
        let d = tempfile::tempdir().unwrap();
        let (s, _) = svc(RecordingPlayer { hold: Duration::from_secs(30), ..Default::default() }, d.path());
        s.submit("long", SayOptions::default(), None).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let t = Instant::now();
        drop(s);
        assert!(t.elapsed() < Duration::from_secs(5));
    }
}
