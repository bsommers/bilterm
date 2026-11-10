//! Durable session state, checkpoints, crash recovery, reparent persistence and the shared
//! RPC/colon session save path.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use bilterm::action::Action;
use bilterm::api::json::JsonValue;
use bilterm::api::AutomationControlPlane;
use bilterm::config::BiltermConfig;
use bilterm::secure_fs::{backup_path, read_with_backup, write_private_file_durable, DurableSource};
use bilterm::wm::resilience::DaemonJob;
use bilterm::wm::WindowManager;

fn wm_in(dir: &Path) -> WindowManager {
    let mut wm = WindowManager::new_headless(24, 80);
    let cfg = BiltermConfig { session_dir: Some(dir.to_string_lossy().into_owned()), ..BiltermConfig::default() };
    wm.apply_config(cfg);
    wm
}

fn wait_for(mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn durable_write_keeps_last_good_backup_and_read_falls_back() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("state.toml");
    write_private_file_durable(&path, b"v = 1\n").unwrap();
    write_private_file_durable(&path, b"v = 2\n").unwrap();
    assert_eq!(fs::read_to_string(backup_path(&path)).unwrap(), "v = 1\n");
    let valid = |t: &str| t.starts_with("v = ");
    assert_eq!(read_with_backup(&path, &valid).unwrap(), ("v = 2\n".to_string(), DurableSource::Primary));

    fs::write(&path, "v").unwrap(); // truncated
    assert_eq!(read_with_backup(&path, &valid).unwrap(), ("v = 1\n".to_string(), DurableSource::Backup));
    assert_eq!(fs::read_to_string(&path).unwrap(), "v = 1\n", "good copy is put back");
    assert!(tmp.path().join("state.toml.corrupt").exists(), "damaged copy is kept for inspection");

    fs::write(&path, "x").unwrap();
    fs::write(backup_path(&path), "y").unwrap();
    assert!(read_with_backup(&path, &valid).is_err(), "two bad copies are an error, not empty state");
}

#[test]
fn truncated_session_file_restores_from_backup_and_says_so() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(tmp.path());
    wm.apply(Action::SplitVertical).unwrap();
    wm.save_session("work").unwrap();
    let path = wm.save_session("work").unwrap(); // second save rotates the first into .bak
    let full = fs::read_to_string(&path).unwrap();
    assert!(full.contains("format_version = 1"));
    fs::write(&path, &full[..full.len() / 2]).unwrap();

    let mut fresh = wm_in(tmp.path());
    assert_eq!(fresh.restore_session("work").expect("falls back to the backup"), 2);
    let note = fresh.take_restore_note().expect("the user is told about the fallback");
    assert!(note.contains("corrupt") && note.contains("backup"), "{note}");

    // Both copies damaged: an error (never a panic, never a silent empty session).
    fs::write(&path, "not toml [").unwrap();
    fs::write(backup_path(&path), "also bad [").unwrap();
    assert!(wm_in(tmp.path()).restore_session("work").is_err());
}

#[test]
fn newer_session_format_is_refused_with_a_reason() {
    let tmp = tempfile::tempdir().unwrap();
    let wm = wm_in(tmp.path());
    let path = wm.save_session("future").unwrap();
    let text = fs::read_to_string(&path).unwrap().replace("format_version = 1", "format_version = 99");
    fs::write(&path, text).unwrap();
    let err = wm_in(tmp.path()).restore_session("future").unwrap_err().to_string();
    assert!(err.contains("v99"), "{err}");
}

#[test]
fn detach_does_not_claim_success_when_the_save_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let blocker = tmp.path().join("not-a-dir");
    fs::write(&blocker, "x").unwrap();
    let mut wm = wm_in(&blocker.join("sessions"));
    assert!(!wm.detach_client(), "detach must not proceed without a saved session");
    assert!(!wm.is_client_detached());
    let msg = wm.active_pane().status_message().unwrap_or_default().to_string();
    assert!(msg.contains("could not be saved"), "{msg}");
}

#[test]
fn unclean_shutdown_is_detected_and_last_checkpoint_restored() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(tmp.path());
    assert_eq!(wm.begin_run(true), None, "first run has nothing to recover");
    wm.apply(Action::SplitVertical).unwrap();
    wm.apply(Action::SplitVertical).unwrap();
    assert!(wm.checkpoint_now());
    let ui_cost = wm.resilience().last_ui_cost();
    assert!(ui_cost < Duration::from_millis(5), "checkpoint cost {ui_cost:?} on the UI thread");
    assert!(wait_for(|| wm.resilience().checkpoints_written() >= 1), "worker writes the checkpoint");
    std::mem::forget(wm); // crash: no clean-exit path runs, the running marker stays

    let mut next = wm_in(tmp.path());
    let note = next.begin_run(true).expect("unclean shutdown is reported");
    assert!(note.contains("Recovered session 'default'") && note.contains("3 pane(s)"), "{note}");
    assert_eq!(next.pane_count(), 3);
    drop(next); // clean exit clears the marker

    assert_eq!(wm_in(tmp.path()).begin_run(true), None, "a clean exit is not a crash");
}

#[test]
fn layout_change_is_checkpointed_after_the_debounce() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(tmp.path());
    wm.begin_run(false);
    let t0 = Instant::now();
    wm.resilience_tick(t0); // periodic baseline checkpoint
    assert!(wait_for(|| wm.resilience().checkpoints_written() >= 1));
    wm.apply(Action::SplitVertical).unwrap();
    wm.resilience_tick(t0 + Duration::from_millis(600)); // change seen, debounce starts
    assert_eq!(wm.resilience().checkpoints_written(), 1, "no write inside the debounce window");
    wm.resilience_tick(t0 + Duration::from_millis(1500));
    assert!(wait_for(|| wm.resilience().checkpoints_written() >= 2), "debounced checkpoint written");
    let saved = fs::read_to_string(wm.checkpoint_path().unwrap()).unwrap();
    assert_eq!(saved.matches("[[panes]]").count(), 2);
}

#[test]
fn reparent_rejects_cycles_and_persists_across_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_in(tmp.path());
    wm.begin_run(false);
    wm.session_tree_mut().branch("main", "a").unwrap();
    wm.session_tree_mut().branch("a", "b").unwrap();
    assert!(wm.session_tree_mut().graft("a", "b").unwrap_err().contains("cycle"));
    assert!(wm.session_tree_mut().graft("a", "a").is_err());
    assert!(wm.session_tree_mut().graft("a", "ghost").unwrap_err().contains("does not exist"));
    wm.session_tree_mut().graft("b", "main").unwrap();
    wm.resilience_tick(Instant::now());
    assert!(wait_for(|| tmp.path().join("session_tree.state").exists()));

    let mut next = wm_in(tmp.path());
    assert!(next.load_persisted_session_tree());
    assert_eq!(next.session_tree().node("b").unwrap().parent.as_deref(), Some("main"));
    assert_eq!(next.session_tree().node("a").unwrap().parent.as_deref(), Some("main"));
}

#[test]
fn rpc_session_save_uses_the_same_default_and_activation_as_the_colon_command() {
    let tmp = tempfile::tempdir().unwrap();
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let cfg = BiltermConfig {
        session_dir: Some(tmp.path().to_string_lossy().into_owned()),
        history_session: "alpha".to_string(),
        ..BiltermConfig::default()
    };
    plane.wm.apply_config(cfg);
    let none = JsonValue::obj([] as [(&str, JsonValue); 0]);
    let saved = plane.handle_rpc_session_save(&none).unwrap();
    assert_eq!(saved.get("session").and_then(JsonValue::as_str), Some("alpha"), "defaults to the active session");
    let named = JsonValue::obj([("name", JsonValue::String("beta".to_string()))]);
    plane.handle_rpc_session_save(&named).unwrap();
    assert_eq!(plane.wm.config().history_session, "beta", "naming a session activates it, as `:session save beta` does");
    let restored = plane.handle_rpc_session_restore(&none).unwrap();
    assert_eq!(restored.get("session").and_then(JsonValue::as_str), Some("beta"));
}

#[test]
fn daemon_stop_job_reports_through_the_tick_instead_of_blocking() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = WindowManager::new_headless(24, 80);
    let started = Instant::now();
    wm.spawn_daemon_job(tmp.path(), DaemonJob::Stop { name: "nobody".to_string(), force: false });
    assert!(started.elapsed() < Duration::from_millis(50), "the caller returns at once");
    assert!(wait_for(|| {
        wm.resilience_tick(Instant::now());
        !wm.daemon_job_pending()
    }));
    assert_eq!(wm.active_pane().status_message(), Some("Daemon 'nobody' was not running"));
}
