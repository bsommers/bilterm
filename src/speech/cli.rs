//! `bilterm say` command line, and the option grammar shared with the `:say` colon command.
//!
//! `bilterm say` is self-contained (it synthesizes and plays in-process and needs no running
//! bilterm), so it also works over SSH, in scripts and in CI. `speech.stop` / `speech.status`
//! act on a running instance's queue and are therefore RPC / colon / MCP only.

use super::player::{Player, ShellPlayer};
use super::render::render_text;
use super::voice::{MAX_AUDIO_SECONDS, MAX_PITCH_HZ, MAX_SPEED, MAX_TEXT_CHARS, MIN_PITCH_HZ, MIN_SPEED, SayOptions, VOICES, voice_names};
use crate::secure_fs;
use std::io::{Read, Write};

/// Parsed `say` arguments (shared by the colon command and the CLI).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SayArgs {
    pub voice: Option<String>,
    pub speed: Option<f64>,
    pub pitch: Option<f64>,
    pub wav: Option<String>,
    pub text: String,
}

/// Parses `[--voice V] [--speed X] [--pitch HZ] [--wav W] [--] TEXT...`. Short forms: -v -s -p -w.
pub fn parse_say_args(args: &[&str]) -> Result<SayArgs, String> {
    let mut out = SayArgs::default();
    let mut i = 0;
    while i < args.len() {
        let a = args[i];
        if a == "--" {
            i += 1;
            break;
        }
        if !a.starts_with('-') || a == "-" || a.parse::<f64>().is_ok() {
            break;
        }
        let (key, inline) = match a.split_once('=') {
            Some((k, v)) => (k, Some(v)),
            None => (a, None),
        };
        let value = |i: &mut usize| -> Result<String, String> {
            if let Some(v) = inline {
                return Ok(v.to_string());
            }
            *i += 1;
            args.get(*i).map(|s| (*s).to_string()).ok_or_else(|| format!("{key} needs a value"))
        };
        match key {
            "--voice" | "-v" => out.voice = Some(value(&mut i)?),
            "--speed" | "-s" => {
                let v = value(&mut i)?;
                out.speed = Some(v.parse::<f64>().map_err(|_| format!("invalid speed '{v}'"))?);
            }
            "--pitch" | "-p" => {
                let v = value(&mut i)?;
                out.pitch = Some(v.parse::<f64>().map_err(|_| format!("invalid pitch '{v}'"))?);
            }
            "--wav" | "-w" => out.wav = Some(value(&mut i)?),
            other => return Err(format!("unknown option '{other}' (use -- before text that starts with '-')")),
        }
        i += 1;
    }
    out.text = args[i..].join(" ");
    Ok(out)
}

/// Usage text for `bilterm say` and `:say`.
pub fn usage() -> String {
    format!(
        "Usage: bilterm say [--voice NAME] [--speed X] [--pitch HZ] [--wav FILE] [--] TEXT...\n\
         \x20      bilterm say [options] -        read the text from stdin\n\
         \x20      bilterm say --voices           list voices\n\
         Voices: {}\n\
         --speed {MIN_SPEED}..{MAX_SPEED} (default 1.0)   --pitch {MIN_PITCH_HZ}..{MAX_PITCH_HZ} Hz (default per voice)\n\
         Limits: {MAX_TEXT_CHARS} characters, {MAX_AUDIO_SECONDS} seconds of audio.\n\
         Old-school formant speech: rule-based, offline, no models or network.",
        voice_names()
    )
}

/// Voice table text (`bilterm say --voices`, `:say voices`).
pub fn voices_text() -> String {
    VOICES
        .iter()
        .map(|v| format!("{:<8} {} ({:.0} Hz)", v.name, v.about, v.base_f0))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Process entry point: returns the exit code.
pub fn run(args: &[String]) -> i32 {
    let (stdin, stdout, stderr) = (std::io::stdin(), std::io::stdout(), std::io::stderr());
    run_with(args, &ShellPlayer::new(), &mut stdin.lock(), &mut stdout.lock(), &mut stderr.lock())
}

/// Testable core of [`run`].
pub fn run_with(args: &[String], player: &dyn Player, stdin: &mut dyn Read, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    if refs.iter().any(|a| matches!(*a, "--help" | "-h")) {
        let _ = writeln!(out, "{}", usage());
        return 0;
    }
    if refs.contains(&"--voices") {
        let _ = writeln!(out, "{}", voices_text());
        return 0;
    }
    let parsed = match parse_say_args(&refs) {
        Ok(p) => p,
        Err(e) => {
            let _ = writeln!(err, "bilterm say: {e}\n{}", usage());
            return 2;
        }
    };
    let mut text = parsed.text.clone();
    if text == "-" {
        let mut buf = Vec::new();
        let limit = (MAX_TEXT_CHARS * 4 + 1) as u64;
        if stdin.take(limit).read_to_end(&mut buf).is_err() {
            let _ = writeln!(err, "bilterm say: cannot read stdin");
            return 1;
        }
        text = String::from_utf8_lossy(&buf).into_owned();
    }
    let opts = match SayOptions::build(parsed.voice.as_deref(), parsed.speed, parsed.pitch) {
        Ok(o) => o,
        Err(e) => {
            let _ = writeln!(err, "bilterm say: {e}");
            return 2;
        }
    };
    if text.trim().is_empty() {
        let _ = writeln!(err, "bilterm say: no text given\n{}", usage());
        return 2;
    }
    let audio = match render_text(&text, &opts) {
        Ok(a) => a,
        Err(e) => {
            let _ = writeln!(err, "bilterm say: {e}");
            return 2;
        }
    };
    let wav = audio.to_wav();
    if audio.truncated {
        let _ = writeln!(err, "bilterm say: text truncated at {MAX_AUDIO_SECONDS} seconds of audio");
    }
    if let Some(path) = parsed.wav {
        return match secure_fs::write_private_file(std::path::Path::new(&path), &wav) {
            Ok(()) => {
                let _ = writeln!(out, "wrote {path} ({} ms, voice {})", audio.duration_ms(), opts.voice.name);
                0
            }
            Err(e) => {
                let _ = writeln!(err, "bilterm say: cannot write {path}: {e}");
                1
            }
        };
    }
    if let Err(e) = player.available() {
        let _ = writeln!(err, "bilterm say: {e}");
        return 1;
    }
    let dir = super::service::default_out_dir().unwrap_or_else(|_| std::env::temp_dir());
    let tmp = dir.join(format!("say-{}.wav", secure_fs::unique_suffix()));
    if let Err(e) = secure_fs::write_private_file(&tmp, &wav) {
        let _ = writeln!(err, "bilterm say: cannot write temp file: {e}");
        return 1;
    }
    let res = player.play(&tmp, &|| false);
    let _ = std::fs::remove_file(&tmp);
    match res {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(err, "bilterm say: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::player::RecordingPlayer;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn parses_options_and_text() {
        let p = parse_say_args(&["--voice", "tron", "-s", "1.5", "--pitch=90", "hello", "world"]).unwrap();
        assert_eq!(p.voice.as_deref(), Some("tron"));
        assert_eq!((p.speed, p.pitch), (Some(1.5), Some(90.0)));
        assert_eq!(p.text, "hello world");
        assert_eq!(parse_say_args(&["--", "-v", "x"]).unwrap().text, "-v x");
        assert_eq!(parse_say_args(&["-5", "dollars"]).unwrap().text, "-5 dollars");
        assert!(parse_say_args(&["--bogus", "x"]).is_err());
        assert!(parse_say_args(&["--speed"]).is_err());
        assert!(parse_say_args(&["--speed", "fast"]).is_err());
    }

    #[test]
    fn writes_a_wav_file() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("hi.wav");
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let code = run_with(&v(&["--wav", f.to_str().unwrap(), "--voice", "tron", "hello"]), &RecordingPlayer::default(), &mut std::io::empty(), &mut o, &mut e);
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&e));
        assert_eq!(&std::fs::read(&f).unwrap()[..4], b"RIFF");
        assert!(String::from_utf8_lossy(&o).contains("tron"));
    }

    #[test]
    fn plays_through_the_player_and_reads_stdin() {
        let p = RecordingPlayer::default();
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let code = run_with(&v(&["-"]), &p, &mut "from stdin".as_bytes(), &mut o, &mut e);
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&e));
        assert_eq!(p.played.lock().unwrap().len(), 1);
    }

    #[test]
    fn usage_errors_exit_2_and_missing_player_exits_1() {
        let p = RecordingPlayer::default();
        let run = |a: &[&str], p: &RecordingPlayer| run_with(&v(a), p, &mut std::io::empty(), &mut Vec::new(), &mut Vec::new());
        assert_eq!(run(&[], &p), 2);
        assert_eq!(run(&["--speed", "99", "x"], &p), 2);
        assert_eq!(run(&["--voice", "hal", "x"], &p), 2);
        let none = RecordingPlayer { unavailable: Some("none".into()), ..Default::default() };
        assert_eq!(run(&["hi"], &none), 1);
        assert_eq!(run(&["--voices"], &p), 0);
        assert_eq!(run(&["--help"], &p), 0);
    }
}
