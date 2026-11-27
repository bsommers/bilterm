//! Left-side attention indicator tracker and 1970s TRON synthesizer chime (`src/wm/attention.rs`).
//!
//! Tracks whether any built-in session (active or parked in `background_sessions`) or window
//! (`CompositePane`) needs user attention (blocked/errored agent state, unread pane-mail or
//! agent-mail, unread VT notifications or bell, password prompt on an unfocused pane, or an
//! exited background shell).
//!
//! Visual pulse is intentionally calm (`ATTENTION_HALF_MS = 900`, ~0.55 Hz: 900 ms lit, 900 ms dim)
//! so it catches the user's eye on the left without flashing frantically. When a session or window
//! newly enters the attention state (rising edge), a 1970s TRON 3-note electronic synthesizer
//! arpeggio chime (`synthesize_tron_beep_wav`) is queued (rate-limited by `BEEP_COOLDOWN`).

use std::collections::BTreeSet;
use std::f64::consts::TAU;
use std::time::{Duration, Instant};

use crate::pane::{CompositePane, PaneId};
use crate::stt::wav::{encode_wav_s16le_mono, STT_SAMPLE_RATE};
use crate::wm::agent_state::AgentState;
use crate::wm::drawer::{DrawerRow, DrawerRowKind};
use crate::wm::WindowManager;

/// Half-period of the attention indicator pulse (900 ms lit, 900 ms dim = ~0.55 Hz).
/// Deliberately slower than a 500 ms cursor blink so it gently breathes rather than flashing wildly.
pub const ATTENTION_HALF_MS: u64 = 900;

/// Minimum spacing between automatic 1970s TRON attention chimes so bursts never spam audio.
pub const BEEP_COOLDOWN: Duration = Duration::from_secs(4);

/// Private per-WM attention pulse and rising-edge sound state.
#[derive(Clone, Debug, Default)]
pub struct AttentionTracker {
    /// Anchor instant when the current attention wave began (`None` when nothing needs attention).
    since: Option<Instant>,
    /// Last rendered pulse phase (`true` = lit, `false` = dim).
    last_phase: bool,
    /// Keys (`"p:<id>"`, `"bg:<session>:<id>"`) currently needing attention; used for rising-edge detection.
    active_keys: BTreeSet<String>,
    /// When the last TRON chime was triggered.
    last_beep_at: Option<Instant>,
    /// Set on rising edge (or explicit `:find-beep`) and consumed by the host loop (`take_pending_tron_beep`).
    pending_tron_beep: bool,
    /// Monotonic counter of triggered TRON chimes (for deterministic tests).
    beep_count: u64,
}

impl AttentionTracker {
    pub fn beep_count(&self) -> u64 {
        self.beep_count
    }
}

fn pulse_phase_on(since: Instant, now: Instant) -> bool {
    (now.saturating_duration_since(since).as_millis() as u64 / ATTENTION_HALF_MS).is_multiple_of(2)
}

impl WindowManager {
    /// True when live pane `id` in the active session needs user attention.
    ///
    /// A pane needs attention when:
    /// - It has unread pane-mail (`:mail`) or unread agent mail, OR
    /// - It has an unread VT bell (`\x07` / `:find-beep`), OR
    /// - It is not the currently focused pane (or the Quake terminal is covering it) AND:
    ///   - Its agent state is `Blocked` or `Errored`, OR
    ///   - It has unread OSC 9 / OSC 777 notifications, OR
    ///   - It is waiting at a password prompt (`is_password_prompt`), OR
    ///   - Its shell has exited (`held_exited_shell` or `has_exited`).
    pub fn pane_needs_attention(&self, id: PaneId) -> bool {
        let Some(pane) = self.panes.get(&id) else {
            return false;
        };
        if pane.empty_placeholder {
            return false;
        }
        if self.pane_mail.has_unread(id)
            || self.agent_mail.unread_count_by_pane(id) > 0
            || pane.vt().unread_bell()
        {
            return true;
        }
        let is_focused = !self.quick_terminal.visible && self.active_pane_id() == id;
        if is_focused {
            return false;
        }
        matches!(self.agent_state(id), Some(AgentState::Blocked | AgentState::Errored))
            || !pane.vt().notifications().is_empty()
            || pane.is_password_prompt()
            || pane.held_exited_shell().is_some()
            || pane.has_exited()
    }

    /// True when a pane inside a parked background session needs user attention.
    pub fn background_pane_needs_attention(pane: &CompositePane) -> bool {
        if pane.empty_placeholder {
            return false;
        }
        pane.vt().unread_bell()
            || !pane.vt().notifications().is_empty()
            || pane.is_password_prompt()
            || pane.held_exited_shell().is_some()
            || pane.has_exited()
    }

    /// True when built-in session `session_name` (active or parked in `background_sessions`) has at
    /// least one window needing attention.
    pub fn session_needs_attention(&self, session_name: &str) -> bool {
        if session_name == self.config.history_session {
            return self
                .ordered_pane_ids()
                .into_iter()
                .any(|id| self.pane_needs_attention(id));
        }
        self.window_manage
            .background_sessions
            .get(session_name)
            .is_some_and(|bg| bg.panes.values().any(Self::background_pane_needs_attention))
    }

    /// True when `row` in the slide-out drawer (Classic or Navigator) represents a session or
    /// window that currently needs attention.
    pub fn drawer_row_needs_attention(&self, row: &DrawerRow) -> bool {
        if !self.config.ui.attention_indicator {
            return false;
        }
        match &row.kind {
            DrawerRowKind::SessionHeader(name) => self.session_needs_attention(name),
            DrawerRowKind::Pane(id) => {
                if let Some(sess) = row.navigator.as_ref().and_then(|m| m.session_name.as_deref())
                    && sess != self.config.history_session
                {
                    return self
                        .window_manage
                        .background_sessions
                        .get(sess)
                        .and_then(|bg| bg.panes.get(id))
                        .is_some_and(Self::background_pane_needs_attention);
                }
                self.pane_needs_attention(*id)
            }
            _ => false,
        }
    }

    /// True when the left-side attention indicator is enabled and any built-in session or window
    /// currently needs attention.
    pub fn any_attention_needed(&self) -> bool {
        if !self.config.ui.attention_indicator {
            return false;
        }
        if self
            .ordered_pane_ids()
            .into_iter()
            .any(|id| self.pane_needs_attention(id))
        {
            return true;
        }
        self.window_manage
            .background_sessions
            .keys()
            .any(|name| self.session_needs_attention(name))
    }

    /// True when the left-side attention pulse is in its "lit" half-period at `now` (always `true`
    /// when `config.ui.attention_flash` is `false`).
    pub fn attention_pulse_lit(&self, now: Instant) -> bool {
        if !self.config.ui.attention_flash {
            return true;
        }
        match self.attention.since {
            Some(since) => pulse_phase_on(since, now),
            None => true,
        }
    }

    /// Collects the set of attention keys currently active across the active and background sessions.
    fn collect_attention_keys(&self) -> BTreeSet<String> {
        let mut keys = BTreeSet::new();
        for id in self.ordered_pane_ids() {
            if self.pane_needs_attention(id) {
                keys.insert(format!("p:{id}"));
            }
        }
        for (name, bg) in &self.window_manage.background_sessions {
            for (&id, pane) in &bg.panes {
                if Self::background_pane_needs_attention(pane) {
                    keys.insert(format!("bg:{name}:{id}"));
                }
            }
        }
        keys
    }

    /// Advances the attention pulse and checks for rising-edge attention events. Returns `true`
    /// when the visual state changed (requesting a frame redraw).
    pub fn attention_tick(&mut self, now: Instant) -> bool {
        if !self.config.ui.attention_indicator && !self.config.ui.attention_sound {
            let had = self.attention.since.take().is_some();
            self.attention.active_keys.clear();
            return had;
        }
        let keys = self.collect_attention_keys();
        let rising_edge = keys.iter().any(|k| !self.attention.active_keys.contains(k));
        let mut changed = keys != self.attention.active_keys;
        self.attention.active_keys = keys;

        if rising_edge && self.config.ui.attention_sound {
            let cooled_down = self
                .attention
                .last_beep_at
                .is_none_or(|last| now.saturating_duration_since(last) >= BEEP_COOLDOWN);
            if cooled_down {
                self.attention.last_beep_at = Some(now);
                self.attention.pending_tron_beep = true;
                self.attention.beep_count = self.attention.beep_count.saturating_add(1);
            }
        }

        if self.attention.active_keys.is_empty() {
            if self.attention.since.take().is_some() {
                changed = true;
            }
            self.attention.last_phase = false;
            return changed;
        }

        let since = *self.attention.since.get_or_insert(now);
        let phase = !self.config.ui.attention_flash || pulse_phase_on(since, now);
        if phase != self.attention.last_phase {
            self.attention.last_phase = phase;
            changed = true;
        }
        changed
    }

    /// Explicitly queues a 1970s TRON synthesizer chime (used by `:find-beep` when `attention_sound` is on).
    pub fn trigger_tron_beep(&mut self) {
        if self.config.ui.attention_sound {
            self.attention.pending_tron_beep = true;
            self.attention.beep_count = self.attention.beep_count.saturating_add(1);
        }
    }

    /// Takes the pending TRON synthesizer chime flag so the host loop can play it asynchronously.
    pub fn take_pending_tron_beep(&mut self) -> bool {
        std::mem::take(&mut self.attention.pending_tron_beep)
    }

    /// Monotonic count of TRON chimes triggered on this `WindowManager`.
    pub fn tron_beep_count(&self) -> u64 {
        self.attention.beep_count()
    }
}

/// Synthesizes a 1970s *TRON* / Wendy Carlos GDS / arcade MCP 3-note electronic synthesizer
/// arpeggio chime as a 16 kHz 16-bit mono RIFF/WAVE byte buffer (~240 ms, 3,840 samples).
///
/// Timbre:
/// - Three ascending synth triad notes (`C5 = 523.25 Hz` -> `G5 = 783.99 Hz` -> `C6 = 1046.50 Hz`)
///   with a 12 ms analog pitch-glide portamento at each note onset.
/// - Odd-harmonic analog pulse/sawtooth stack (`1.00*f0 + 0.34*3*f0 + 0.14*5*f0 + 0.06*7*f0`)
///   with a subtle 32 Hz ring-modulator shimmer and click-free attack/exponential decay envelope.
pub fn synthesize_tron_beep_wav() -> Vec<u8> {
    let sr = f64::from(STT_SAMPLE_RATE);
    // (start_freq_hz, target_freq_hz, duration_ms)
    let notes: [(f64, f64, usize); 3] = [
        (493.88, 523.25, 75),  // B4 -> C5 glide (75 ms)
        (739.99, 783.99, 75),  // F#5 -> G5 glide (75 ms)
        (987.77, 1046.50, 95), // B5 -> C6 glide + resonant sustain tail (95 ms)
    ];
    let total_samples: usize = notes.iter().map(|&(_, _, ms)| (usize::from(STT_SAMPLE_RATE as u16) * ms) / 1000).sum();
    let mut samples = Vec::with_capacity(total_samples);
    let mut phase = 0.0_f64;
    let mut global_idx = 0usize;

    for &(f_start, f_target, dur_ms) in &notes {
        let note_samples = (usize::from(STT_SAMPLE_RATE as u16) * dur_ms) / 1000;
        let glide_samples = ((usize::from(STT_SAMPLE_RATE as u16) * 12) / 1000).max(1);
        let attack_samples = ((usize::from(STT_SAMPLE_RATE as u16) * 4) / 1000).max(1);

        for i in 0..note_samples {
            let glide_t = (i as f64 / glide_samples as f64).min(1.0);
            let freq = f_start + (f_target - f_start) * glide_t;
            phase = (phase + TAU * freq / sr).rem_euclid(TAU);

            // Odd-harmonic 1970s analog synth lead (fundamental + 3rd, 5th, 7th harmonics)
            let h1 = phase.sin();
            let h3 = (3.0 * phase).sin();
            let h5 = (5.0 * phase).sin();
            let h7 = (7.0 * phase).sin();
            let raw_wave = (h1 + 0.34 * h3 + 0.14 * h5 + 0.06 * h7) / 1.54;

            // Subtle 32 Hz ring-modulation shimmer characteristic of 1970s sci-fi computer telemetry
            let t_global = global_idx as f64 / sr;
            let ring_mod = 0.88 + 0.12 * (TAU * 32.0 * t_global).sin();

            // Click-free linear attack + smooth exponential decay
            let attack = (i as f64 / attack_samples as f64).min(1.0);
            let progress = i as f64 / note_samples.max(1) as f64;
            let decay = (-2.6 * progress).exp();
            // Anti-click 2 ms fade-out at end of each note
            let tail_fade = ((note_samples.saturating_sub(1 + i)) as f64 / (attack_samples as f64 * 0.5)).min(1.0);
            let env = attack * decay * tail_fade;

            let amp = 0.42 * f64::from(i16::MAX);
            let sample = (raw_wave * ring_mod * env * amp).round().clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16;
            samples.push(sample);
            global_idx += 1;
        }
    }

    encode_wav_s16le_mono(&samples)
}

/// Spawns a non-blocking background thread to play the 1970s TRON synthesizer chime via the OS
/// audio player (`afplay` on macOS, `pw-play`/`paplay`/`aplay` on Linux, `SoundPlayer` on Windows).
/// Never blocks the UI loop; silently no-ops if no audio player is installed.
pub fn play_tron_beep_async() {
    if std::env::var_os("BILTERM_DISABLE_AUDIO").is_some() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("bilterm-tron-beep".to_string())
        .spawn(|| {
            let Ok(dir) = crate::secure_fs::private_runtime_dir() else {
                return;
            };
            let wav_path = dir.join("tron-attention-beep.wav");
            let wav = synthesize_tron_beep_wav();
            if crate::secure_fs::write_private_file(&wav_path, &wav).is_err() {
                return;
            }
            play_wav_file_blocking(&wav_path);
        });
}

fn play_wav_file_blocking(path: &std::path::Path) {
    use std::process::{Command, Stdio};
    let Some(path_str) = path.to_str() else {
        return;
    };
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("afplay")
            .arg(path_str)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(target_os = "linux")]
    {
        for (bin, args) in [
            ("pw-play", vec![path_str]),
            ("paplay", vec![path_str]),
            ("aplay", vec!["-q", path_str]),
        ] {
            if crate::stt::audio::is_executable_on_path(bin) {
                let _ = Command::new(bin)
                    .args(&args)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
                break;
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let escaped = path_str.replace('\'', "''");
        let script = format!("(New-Object Media.SoundPlayer '{escaped}').PlaySync()");
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stt::wav::{compute_rms_dbfs, parse_wav_s16le_mono, WAV_HEADER_LEN};

    #[test]
    fn tron_beep_wav_is_valid_16khz_triad_and_unclipped() {
        let wav = synthesize_tron_beep_wav();
        assert!(wav.len() > WAV_HEADER_LEN + 6000, "expected ~245ms of 16kHz S16_LE PCM, got {} bytes", wav.len());
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        let samples = parse_wav_s16le_mono(&wav).expect("valid WAV");
        // 75ms + 75ms + 95ms = 245ms @ 16kHz = 3920 samples
        assert_eq!(samples.len(), 3920);
        let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
        assert!(peak > 5_000 && peak < 20_000, "expected warm non-clipping peak, got {peak}");
        let dbfs = compute_rms_dbfs(&samples);
        assert!(dbfs > -30.0 && dbfs < -6.0, "expected audible RMS dBFS, got {dbfs}");
    }

    #[test]
    fn attention_pulse_is_calm_and_rising_edge_triggers_tron_beep_with_cooldown() {
        let mut wm = WindowManager::new_headless(24, 80);
        let p2 = wm.split_focused(crate::wm::SplitDirection::Vertical).unwrap();
        wm.focus_and_raise(1);
        let t0 = Instant::now();

        assert!(!wm.any_attention_needed());
        assert!(!wm.attention_tick(t0));
        assert_eq!(wm.tron_beep_count(), 0);

        // Unread pane-mail on pane 2 triggers attention + rising-edge TRON chime.
        wm.note_pane_message(p2, "agent", t0);
        assert!(wm.pane_needs_attention(p2));
        assert!(wm.session_needs_attention("default"));
        assert!(wm.any_attention_needed());
        assert!(wm.attention_tick(t0));
        assert!(wm.take_pending_tron_beep());
        assert_eq!(wm.tron_beep_count(), 1);

        // Pulse stays lit at t0 + 400ms (< 900ms half-period), then dims at t0 + 950ms.
        assert!(wm.attention_pulse_lit(t0 + Duration::from_millis(400)));
        assert!(!wm.attention_pulse_lit(t0 + Duration::from_millis(950)));
        assert!(wm.attention_tick(t0 + Duration::from_millis(950)), "phase flip requests redraw");
        assert!(wm.attention_pulse_lit(t0 + Duration::from_millis(1850)), "lit again in next cycle");

        // Acknowledge mail -> attention clears; re-triggering within BEEP_COOLDOWN (4s) does not re-beep,
        // re-triggering after 4s beeps again.
        wm.pane_mail_ack(Some(p2), t0 + Duration::from_secs(1));
        assert!(!wm.any_attention_needed());
        let _ = wm.attention_tick(t0 + Duration::from_secs(1));

        wm.note_pane_message(p2, "agent", t0 + Duration::from_secs(2));
        let _ = wm.attention_tick(t0 + Duration::from_secs(2));
        assert!(!wm.take_pending_tron_beep(), "within 4s cooldown: no second beep");
        assert_eq!(wm.tron_beep_count(), 1);

        wm.pane_mail_ack(Some(p2), t0 + Duration::from_secs(3));
        let _ = wm.attention_tick(t0 + Duration::from_secs(3));
        wm.note_pane_message(p2, "agent", t0 + Duration::from_secs(5));
        let _ = wm.attention_tick(t0 + Duration::from_secs(5));
        assert!(wm.take_pending_tron_beep(), "after 4s cooldown: rising edge beeps again");
        assert_eq!(wm.tron_beep_count(), 2);
    }
}
