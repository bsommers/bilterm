//! Surface uniformity and non-blocking behaviour for the retro text-to-speech feature
//! (`TTS-01`, `UNI-R01`, `ARCH-R01`): `:say`, `speech.*` RPC and the `bilterm_speech` MCP tool all
//! lower to `Action::Speech` and reach the same worker.

use bilterm::action::{Action, SpeechOp};
use bilterm::api::json::JsonValue;
use bilterm::api::rpc::AutomationControlPlane;
use bilterm::colon_command::parse_colon_command;
use bilterm::speech::SpeechService;
use bilterm::speech::player::RecordingPlayer;
use bilterm::wm::WindowManager;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn service(hold_ms: u64, dir: &std::path::Path) -> (SpeechService, Arc<RecordingPlayer>) {
    let p = Arc::new(RecordingPlayer { hold: Duration::from_millis(hold_ms), ..Default::default() });
    (SpeechService::new(p.clone(), Some(dir.to_path_buf())), p)
}

fn wait_idle(mut pending: impl FnMut() -> usize) {
    let t = Instant::now();
    while pending() > 0 {
        assert!(t.elapsed() < Duration::from_secs(30), "worker never went idle");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn say(wm: &mut WindowManager, line: &str) {
    let action = parse_colon_command(line).expect("colon command").expect("parses");
    wm.apply(action).expect("apply");
}

#[test]
fn colon_say_speaks_off_thread_and_stop_cancels() {
    let dir = tempfile::tempdir().unwrap();
    let (svc, player) = service(20_000, dir.path());
    let mut wm = WindowManager::new_headless(24, 80);
    wm.install_speech_service(svc);

    let t = Instant::now();
    say(&mut wm, ":say --voice tron hello from the grid");
    assert!(t.elapsed() < Duration::from_secs(2), "apply must not wait for audio");
    assert!(wm.active_pane().status_message().unwrap_or("").starts_with("say: "));
    assert_eq!(wm.speech_status().pending, 1);

    say(&mut wm, ":say stop");
    wait_idle(|| wm.speech_status().pending);
    assert!(player.played.lock().unwrap().is_empty(), "cancelled before playback finished");

    say(&mut wm, ":say status");
    assert!(wm.active_pane().status_message().unwrap_or("").contains("pending 0"));
}

#[test]
fn colon_rpc_and_mcp_reach_the_same_worker_with_the_same_audio() {
    let dir = tempfile::tempdir().unwrap();
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let (svc, player) = service(0, dir.path());
    plane.wm.install_speech_service(svc);

    // 1. colon
    say(&mut plane.wm, ":say --voice votrax --speed 1.2 testing one two");
    wait_idle(|| plane.wm.speech_status().pending);

    // 2. RPC
    let params = JsonValue::parse(r#"{"text":"testing one two","voice":"votrax","speed":1.2}"#).unwrap();
    let r = plane.dispatch_method("speech.say", &params).expect("speech.say");
    assert_eq!(r.get("voice").and_then(JsonValue::as_str), Some("votrax"));
    wait_idle(|| plane.wm.speech_status().pending);

    // 3. MCP
    let line = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"bilterm_speech","arguments":{"action":"say","text":"testing one two","voice":"votrax","speed":1.2}}}"#;
    let resp = plane.handle_mcp_line(line).expect("mcp response");
    assert!(!resp.contains("\"isError\":true"), "{resp}");
    wait_idle(|| plane.wm.speech_status().pending);

    let played = player.played.lock().unwrap().clone();
    assert_eq!(played.len(), 3, "all three surfaces played");
    assert!(played.iter().all(|n| *n == played[0] && *n > 10_000), "identical, non-trivial audio: {played:?}");
}

#[test]
fn rpc_validates_and_reports_errors() {
    let dir = tempfile::tempdir().unwrap();
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let (svc, _) = service(0, dir.path());
    plane.wm.install_speech_service(svc);
    let call = |plane: &mut AutomationControlPlane, json: &str| plane.dispatch_method("speech.say", &JsonValue::parse(json).unwrap());
    assert!(call(&mut plane, r#"{"text":"hi","voice":"hal"}"#).unwrap_err().contains("unknown voice"));
    assert!(call(&mut plane, r#"{"text":"hi","speed":50}"#).unwrap_err().contains("out of range"));
    assert!(call(&mut plane, r#"{"text":"hi","wav":"../escape"}"#).unwrap_err().contains("wav name"));
    assert!(call(&mut plane, r#"{"text":""}"#).is_err());
    assert!(call(&mut plane, &format!(r#"{{"text":"{}"}}"#, "x".repeat(2001))).unwrap_err().contains("too long"));
    assert!(call(&mut plane, "{}").is_err());
    let voices = plane.dispatch_method("speech.voices", &JsonValue::Null).unwrap();
    assert_eq!(voices.get("voices").and_then(JsonValue::as_array).map(<[JsonValue]>::len), Some(5));
}

#[test]
fn rpc_queue_depth_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let (svc, _) = service(30_000, dir.path());
    plane.wm.install_speech_service(svc);
    let p = JsonValue::parse(r#"{"text":"queue flood"}"#).unwrap();
    let results: Vec<_> = (0..bilterm::speech::QUEUE_DEPTH + 3).map(|_| plane.dispatch_method("speech.say", &p)).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), bilterm::speech::QUEUE_DEPTH);
    assert!(results.last().unwrap().as_ref().unwrap_err().contains("queue full"));
    plane.dispatch_method("speech.stop", &JsonValue::Null).unwrap();
    wait_idle(|| plane.wm.speech_status().pending);
}

#[test]
fn wav_output_lands_in_the_private_dir() {
    let dir = tempfile::tempdir().unwrap();
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let (svc, player) = service(0, dir.path());
    plane.wm.install_speech_service(svc);
    plane.dispatch_method("speech.say", &JsonValue::parse(r#"{"text":"save this","wav":"clip1"}"#).unwrap()).unwrap();
    wait_idle(|| plane.wm.speech_status().pending);
    let wav = std::fs::read(dir.path().join("clip1.wav")).expect("wav written");
    assert_eq!(&wav[..4], b"RIFF");
    assert!(player.played.lock().unwrap().is_empty());
    let st = plane.dispatch_method("speech.status", &JsonValue::Null).unwrap();
    assert_eq!(st.get("status").and_then(|s| s.get("last_wav")).and_then(JsonValue::as_str), Some("clip1.wav"));
}

#[test]
fn every_surface_is_listed() {
    for m in ["speech.say", "speech.stop", "speech.status", "speech.voices"] {
        assert!(bilterm::api::RPC_METHODS.contains(&m), "{m} in RPC_METHODS");
    }
    let tools = AutomationControlPlane::mcp_tools_list().to_json_string();
    assert!(tools.contains("bilterm_speech"));
    assert!(bilterm::command_registry::catalog().iter().any(|c| c.name == ":say"));
    assert_eq!(
        parse_colon_command(":say stop").unwrap().unwrap(),
        Action::Speech(SpeechOp::Stop)
    );
}
