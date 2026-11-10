//! Swarm wiring: colon parse -> Action -> WindowManager::apply and RPC share one implementation.
use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::wm::WindowManager;
use bilterm::wm::swarm_cmd::SwarmOp;
use std::path::Path;
use std::process::Command;

fn git(d: &Path, a: &[&str]) -> String {
    let o = Command::new("git").arg("-C").arg(d).args(a).output().unwrap();
    assert!(o.status.success(), "{a:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn parse_roundtrip_and_errors() {
    for s in [
        ":swarm status",
        ":swarm run",
        ":swarm load plans/a.toml",
        ":swarm verify api",
        ":swarm diff api",
        ":swarm merge api --squash",
        ":swarm clean api --force",
        ":swarm spawn api claude do the thing",
    ] {
        let a = parse_colon_command(s).unwrap().unwrap_or_else(|e| panic!("{s}: {e}"));
        assert!(matches!(a, Action::Swarm(_)), "{s}");
        let Action::Swarm(op) = a else { unreachable!() };
        assert_eq!(op.colon_form(), s);
    }
    for bad in [":swarm bogus", ":swarm merge x --force", ":swarm clean x --squash", ":swarm diff a b", ":swarm merge a b"] {
        assert!(matches!(parse_colon_command(bad), Some(Err(_))), "{bad}");
    }
}

// Single test: swarm ops resolve the repo from the process cwd, which is process-global.
#[test]
fn end_to_end_in_temp_repo() {
    let repo = std::env::temp_dir().join(format!("bilterm-swarm-wiring-{}", std::process::id()));
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

    let mut wm = WindowManager::new_headless(80, 24);
    for bad in [SwarmOp::Load(String::new()), SwarmOp::Diff(String::new()), SwarmOp::Verify(String::new())] {
        assert!(wm.swarm_apply(bad).is_err());
    }
    assert!(wm.swarm_apply(SwarmOp::Spawn { slug: "../x".into(), agent: "claude".into(), prompt: String::new() }).is_err());
    assert!(wm.swarm_apply(SwarmOp::Run).is_err(), "run before load");
    let loaded = wm.swarm_apply(SwarmOp::Load("plan.toml".into())).unwrap().to_json_string();
    assert!(loaded.contains("true") && loaded.contains("api: true"), "{loaded}");
    assert!(wm.swarm_apply(SwarmOp::Load("../etc/nope.toml".into())).is_err());

    let panes_before = wm.pane_count();
    let run = wm.swarm_apply(SwarmOp::Run).unwrap().to_json_string();
    assert!(run.contains("\"started\":[\"api\"]"), "{run}");
    assert_eq!(wm.pane_count(), panes_before + 1);
    assert!(repo.join(".bilterm/worktrees/api").exists());

    // Dependent node stays blocked until api passes.
    assert!(wm.swarm_apply(SwarmOp::Run).unwrap().to_json_string().contains("\"started\":[]"));
    wm.swarm_apply(SwarmOp::Verify("api".into())).unwrap();
    for _ in 0..100 {
        let st = wm.swarm_apply(SwarmOp::Status).unwrap().to_json_string();
        if st.contains("\"state\":\"passed\"") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(wm.swarm_apply(SwarmOp::Run).unwrap().to_json_string().contains("\"started\":[\"ui\"]"));

    // Diff / merge / clean on tracked worktree only.
    let wt = repo.join(".bilterm/worktrees/api");
    std::fs::write(wt.join("b.txt"), "new\n").unwrap();
    git(&wt, &["add", "."]);
    git(&wt, &["commit", "-q", "-m", "work"]);
    let d = wm.swarm_apply(SwarmOp::Diff("api".into())).unwrap().to_json_string();
    assert!(d.contains("b.txt"), "{d}");
    assert!(wm.swarm_apply(SwarmOp::Diff("main".into())).is_err(), "untracked slug refused");
    assert!(wm.swarm_apply(SwarmOp::Clean { slug: "api".into(), force: false }).is_err(), "unmerged refused");
    wm.swarm_apply(SwarmOp::Merge { slug: "api".into(), squash: false }).unwrap();
    assert!(repo.join("b.txt").exists());
    wm.swarm_apply(SwarmOp::Clean { slug: "api".into(), force: false }).unwrap();
    assert!(!wt.exists());

    // Dirty primary tree blocks merge.
    let ui = repo.join(".bilterm/worktrees/ui");
    std::fs::write(ui.join("c.txt"), "x\n").unwrap();
    git(&ui, &["add", "."]);
    git(&ui, &["commit", "-q", "-m", "ui"]);
    std::fs::write(repo.join("a.txt"), "dirty\n").unwrap();
    let e = wm.swarm_apply(SwarmOp::Merge { slug: "ui".into(), squash: false }).unwrap_err();
    assert!(e.contains("uncommitted"), "{e}");
    let _ = std::fs::remove_dir_all(&repo);
}
