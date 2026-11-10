//! Phase 70 HUD x Phase 69 swarm: node/state/wave/retries on tiles and in JSON, real worktree diff.
use bilterm::action::Action;
use bilterm::api::agents_hud_json::hud_snapshot;
use bilterm::wm::WindowManager;
use bilterm::wm::agents_hud_cmd::HudOp;
use bilterm::wm::mission_control_hud::HudButtonKind;
use bilterm::wm::swarm_surface::SwarmOp;
use std::path::Path;
use std::process::Command;

fn git(d: &Path, a: &[&str]) {
    let o = Command::new("git").arg("-C").arg(d).args(a).output().unwrap();
    assert!(o.status.success(), "{a:?}: {}", String::from_utf8_lossy(&o.stderr));
}

fn diff_status(wm: &mut WindowManager, pane: usize) -> Result<Option<String>, String> {
    wm.agents_hud_apply(&HudOp::Press { pane: Some(pane), kind: HudButtonKind::Diff, generation: None })
}

// Single sequential test: swarm ops resolve the repo from the process cwd (process-global).
#[test]
fn hud_shows_swarm_fields_and_real_diff() {
    let repo = std::env::temp_dir().join(format!("bilterm-hud-swarm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&repo);
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "t@example.com"]);
    git(&repo, &["config", "user.name", "t"]);
    std::fs::write(repo.join("a.txt"), "base\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "init"]);
    std::fs::write(
        repo.join("plan.toml"),
        "[[node]]\nid='api'\nprompt='p'\nverify_cmd='true'\n[[node]]\nid='ui'\nprompt='q'\ndepends_on=['api']\n",
    )
    .unwrap();
    std::env::set_current_dir(&repo).unwrap();

    let mut wm = WindowManager::new_headless(100, 30);
    let main_pane = wm.active_pane_id();
    wm.apply(Action::Swarm(SwarmOp::Load("plan.toml".into()))).unwrap();
    wm.apply(Action::Swarm(SwarmOp::Run)).unwrap();
    let api_pane = wm.swarm_surface().runtime.pane_for("api").expect("api pane");

    let info = wm.swarm_node_info(api_pane).unwrap();
    assert_eq!((info.node.as_str(), info.state, info.wave, info.retries), ("api", "running", Some(0), 0));
    assert_eq!(wm.swarm_surface().runtime.node_for_pane(api_pane), Some("api"));
    assert!(wm.swarm_node_info(main_pane).is_none());

    wm.apply(Action::AgentsHud(HudOp::Open)).unwrap();
    let snap = hud_snapshot(&wm).to_json_string();
    let v = bilterm::api::json::JsonValue::parse(&snap).unwrap();
    let bilterm::api::json::JsonValue::Array(cards) = v.get("cards").unwrap().clone() else { panic!() };
    let card = |id: usize| cards.iter().find(|c| c.get("pane_id").and_then(|x| x.as_i64()) == Some(id as i64)).unwrap().clone();
    let sw = card(api_pane).get("swarm").unwrap().clone();
    assert_eq!(sw.get("node").and_then(|x| x.as_str()), Some("api"));
    assert_eq!(sw.get("state").and_then(|x| x.as_str()), Some("running"));
    assert_eq!(sw.get("wave").and_then(|x| x.as_i64()), Some(0));
    assert_eq!(sw.get("retries").and_then(|x| x.as_i64()), Some(0));
    assert!(matches!(card(main_pane).get("swarm"), Some(bilterm::api::json::JsonValue::Null)));

    // Non-swarm pane: clear status, no error.
    let msg = diff_status(&mut wm, main_pane).unwrap().unwrap();
    assert!(msg.contains("not part of a swarm"), "{msg}");

    // Swarm pane: real diff of its worktree branch.
    let wt = repo.join(".bilterm/worktrees/api");
    std::fs::write(wt.join("b.txt"), "new\n").unwrap();
    git(&wt, &["add", "."]);
    git(&wt, &["commit", "-q", "-m", "work"]);
    let msg = diff_status(&mut wm, api_pane).unwrap().unwrap();
    assert!(msg.contains("1 file(s) +1 -0") && msg.contains("b.txt"), "{msg}");

    // State advances with the swarm (verify -> passed) and wave of the dependent node is 1.
    wm.apply(Action::Swarm(SwarmOp::Verify("api".into()))).unwrap();
    for _ in 0..100 {
        wm.swarm_poll();
        if wm.swarm_node_info(api_pane).unwrap().state == "passed" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(wm.swarm_node_info(api_pane).unwrap().state, "passed");
    wm.apply(Action::Swarm(SwarmOp::Run)).unwrap();
    let ui_pane = wm.swarm_surface().runtime.pane_for("ui").unwrap();
    assert_eq!(wm.swarm_node_info(ui_pane).unwrap().wave, Some(1));
    let _ = std::fs::remove_dir_all(&repo);
}
