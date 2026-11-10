//! Phase 69 (SWARM-01..04) tests.
use bilterm::swarm::dag::NodeState;
use bilterm::swarm::heal::*;
use bilterm::swarm::review::*;
use bilterm::swarm::{SwarmError, SwarmOrchestrator, SwarmPlan, WorktreeSpec};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn temp_repo(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("bilterm-p69-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    git(&d, &["init", "-q", "-b", "main"]);
    git(&d, &["config", "user.email", "t@example.com"]);
    git(&d, &["config", "user.name", "t"]);
    std::fs::write(d.join("a.txt"), "base\n").unwrap();
    git(&d, &["add", "."]);
    git(&d, &["commit", "-q", "-m", "init"]);
    d
}

// ---- SWARM-01 ----
#[test]
fn spec_rejects_traversal_slugs() {
    let r = Path::new("/repo");
    for bad in ["../x", "a/b", "..", ".", "", "a b", "x;y"] {
        assert!(
            matches!(WorktreeSpec::new(r, bad), Err(SwarmError::InvalidName(_))),
            "{bad}"
        );
    }
}

#[test]
fn spec_paths_and_commands() {
    let s = WorktreeSpec::new(Path::new("/repo"), "api-1").unwrap();
    assert_eq!(s.branch, "swarm/api-1");
    assert_eq!(s.path, Path::new("/repo/.bilterm/worktrees/api-1"));
    assert_eq!(
        s.add_args(),
        [
            "worktree",
            "add",
            "-b",
            "swarm/api-1",
            "/repo/.bilterm/worktrees/api-1"
        ]
    );
    assert_eq!(
        s.remove_args(true),
        [
            "worktree",
            "remove",
            "--force",
            "/repo/.bilterm/worktrees/api-1"
        ]
    );
    assert_eq!(s.branch_delete_args(false), ["branch", "-d", "swarm/api-1"]);
}

#[test]
fn worktree_create_and_cleanup_real_repo() {
    let repo = temp_repo("wt");
    let s = WorktreeSpec::new(&repo, "one").unwrap();
    s.create().unwrap();
    assert!(s.path.join("a.txt").exists());
    assert!(git(&repo, &["branch", "--list", "swarm/one"]).contains("swarm/one"));
    s.cleanup(true).unwrap();
    assert!(!s.path.exists());
    assert!(
        git(&repo, &["branch", "--list", "swarm/one"])
            .trim()
            .is_empty()
    );
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn failed_create_leaves_nothing() {
    let repo = temp_repo("fail");
    let s = WorktreeSpec::new(&repo, "dup").unwrap();
    s.create().unwrap();
    // Second create collides on branch; must error without removing the first? It cleans its own
    // spec, which is the same slug, so everything is gone — assert no partial state remains.
    assert!(s.create().is_err());
    assert!(git(&repo, &["worktree", "list"]).lines().count() <= 2);
    let _ = std::fs::remove_dir_all(&repo);
}

// ---- SWARM-02 ----
const PLAN: &str = r#"
[swarm]
max_parallel = 2
[[node]]
id = "api"
prompt = "do api"
verify_cmd = "true"
[[node]]
id = "db"
prompt = "do db"
[[node]]
id = "ui"
prompt = "do ui"
depends_on = ["api", "db"]
[[node]]
id = "e2e"
prompt = "e2e"
depends_on = ["ui"]
"#;

#[test]
fn plan_parses_and_waves() {
    let p = SwarmPlan::parse(PLAN).unwrap();
    assert_eq!(p.max_parallel, 2);
    assert_eq!(p.waves(), vec![vec!["api", "db"], vec!["ui"], vec!["e2e"]]);
    assert_eq!(p.nodes[0].agent, "claude");
    assert_eq!(p.nodes[0].worktree_slug(), "api");
}

#[test]
fn plan_rejections() {
    let cyc = "[[node]]\nid='a'\nprompt='x'\ndepends_on=['b']\n[[node]]\nid='b'\nprompt='x'\ndepends_on=['a']\n";
    assert!(matches!(SwarmPlan::parse(cyc), Err(SwarmError::Cycle(_))));
    let selfc = "[[node]]\nid='a'\nprompt='x'\ndepends_on=['a']\n";
    assert!(matches!(SwarmPlan::parse(selfc), Err(SwarmError::Cycle(_))));
    let unk = "[[node]]\nid='a'\nprompt='x'\ndepends_on=['zz']\n";
    assert!(matches!(
        SwarmPlan::parse(unk),
        Err(SwarmError::UnknownDep { .. })
    ));
    let dup = "[[node]]\nid='a'\nprompt='x'\n[[node]]\nid='a'\nprompt='y'\n";
    assert!(matches!(
        SwarmPlan::parse(dup),
        Err(SwarmError::DuplicateId(_))
    ));
    let bad = "[[node]]\nid='../a'\nprompt='x'\n";
    assert!(matches!(
        SwarmPlan::parse(bad),
        Err(SwarmError::InvalidName(_))
    ));
    assert!(matches!(SwarmPlan::parse(""), Err(SwarmError::Empty)));
    assert!(matches!(
        SwarmPlan::parse("not toml ["),
        Err(SwarmError::Parse(_))
    ));
    let par = "[swarm]\nmax_parallel=99\n[[node]]\nid='a'\nprompt='x'\n";
    assert!(matches!(SwarmPlan::parse(par), Err(SwarmError::Limit(_))));
    let ret = "[swarm]\nmax_retries=99\n[[node]]\nid='a'\nprompt='x'\n";
    assert!(matches!(SwarmPlan::parse(ret), Err(SwarmError::Limit(_))));
}

#[test]
fn node_cap_enforced() {
    let mut s = String::new();
    for i in 0..65 {
        s.push_str(&format!("[[node]]\nid='n{i}'\nprompt='x'\n"));
    }
    assert!(matches!(
        SwarmPlan::parse(&s),
        Err(SwarmError::TooManyNodes(65, 64))
    ));
}

#[test]
fn multi_wave_execution_order_and_parallel_cap() {
    let mut o = SwarmOrchestrator::new(SwarmPlan::parse(PLAN).unwrap());
    let w1: Vec<_> = o.start_ready().unwrap().into_iter().map(|t| t.id).collect();
    assert_eq!(w1, ["api", "db"]);
    assert!(o.start_ready().unwrap().is_empty(), "ui blocked on deps");
    o.begin_verify("api").unwrap();
    o.pass("api").unwrap();
    assert!(o.start_ready().unwrap().is_empty(), "db still running");
    o.pass("db").unwrap();
    assert_eq!(o.start_ready().unwrap()[0].id, "ui");
    o.pass("ui").unwrap();
    assert_eq!(o.start_ready().unwrap()[0].id, "e2e");
    o.pass("e2e").unwrap();
    assert!(o.is_finished() && o.scheduler().all_passed());
}

#[test]
fn parallel_cap_limits_fanout() {
    let mut src = String::from("[swarm]\nmax_parallel=2\n");
    for i in 0..5 {
        src.push_str(&format!("[[node]]\nid='n{i}'\nprompt='x'\n"));
    }
    let mut o = SwarmOrchestrator::new(SwarmPlan::parse(&src).unwrap());
    assert_eq!(o.start_ready().unwrap().len(), 2);
    o.pass("n0").unwrap();
    assert_eq!(o.start_ready().unwrap().len(), 1);
}

#[test]
fn state_machine_rejects_illegal_and_blocks_descendants() {
    let mut o = SwarmOrchestrator::new(SwarmPlan::parse(PLAN).unwrap());
    assert!(matches!(
        o.pass("api"),
        Err(SwarmError::BadTransition { .. })
    ));
    assert!(matches!(o.pass("nope"), Err(SwarmError::UnknownNode(_))));
    o.start_ready().unwrap();
    o.fail("api").unwrap();
    assert!(o.pass("api").is_err(), "Failed is terminal");
    assert_eq!(o.scheduler().blocked(), ["ui", "e2e"]);
    o.pass("db").unwrap();
    assert!(o.start_ready().unwrap().is_empty());
    assert!(o.is_finished() && !o.scheduler().all_passed());
    assert_eq!(o.scheduler().state("api"), Some(NodeState::Failed));
}

// ---- SWARM-03 ----
#[test]
fn diff_stat_parser() {
    let out = " src/a.rs | 12 ++++++------\n img.png  | Bin 0 -> 120 bytes\n 2 files changed, 6 insertions(+), 6 deletions(-)\n";
    let d = parse_diff_stat(out);
    assert_eq!(d.files.len(), 2);
    assert_eq!(
        d.files[0],
        FileStat {
            path: "src/a.rs".into(),
            changes: 12,
            binary: false
        }
    );
    assert!(d.files[1].binary);
    assert_eq!((d.files_changed, d.insertions, d.deletions), (2, 6, 6));
    let one = parse_diff_stat(" 1 file changed, 1 insertion(+)\n");
    assert_eq!(
        (one.files_changed, one.insertions, one.deletions),
        (1, 1, 0)
    );
}

#[test]
fn merge_tree_parser() {
    assert_eq!(parse_merge_tree(0, "abc123\n"), MergeCheck::Clean);
    let out = "abc123\nb.txt\na.txt\na.txt\n\nAuto-merging a.txt\nCONFLICT (content)\n";
    assert_eq!(
        parse_merge_tree(1, out),
        MergeCheck::Conflicts(vec!["a.txt".into(), "b.txt".into()])
    );
}

#[test]
fn merge_plans() {
    let b = vec!["swarm/a".to_string(), "swarm/b".to_string()];
    let m = merge_plan(&b, MergeStrategy::Merge, &[]);
    assert_eq!(m.len(), 2);
    assert_eq!(m[0][..2], ["merge", "--no-ff"]);
    let s = merge_plan(&b, MergeStrategy::Squash, &[]);
    assert_eq!(s.len(), 4);
    assert_eq!(s[0], ["merge", "--squash", "swarm/a"]);
    let c = merge_plan(
        &b,
        MergeStrategy::CherryPick,
        &[vec!["c1".into(), "c2".into()], vec![]],
    );
    assert_eq!(
        c,
        vec![vec!["cherry-pick", "c1"], vec!["cherry-pick", "c2"]]
    );
}

#[test]
fn real_repo_diff_and_conflict_precheck() {
    let repo = temp_repo("rev");
    let a = WorktreeSpec::new(&repo, "a").unwrap();
    let b = WorktreeSpec::new(&repo, "b").unwrap();
    a.create().unwrap();
    b.create().unwrap();
    for (s, txt) in [(&a, "from a\n"), (&b, "from b\n")] {
        std::fs::write(s.path.join("a.txt"), txt).unwrap();
        git(&s.path, &["commit", "-q", "-am", "edit"]);
    }
    let stat = parse_diff_stat(&git(&repo, &["diff", "--stat", "main", "swarm/a"]));
    assert_eq!(stat.files[0].path, "a.txt");
    let args = merge_tree_args("swarm/a", "swarm/b");
    let o = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(&args)
        .output()
        .unwrap();
    let chk = parse_merge_tree(
        o.status.code().unwrap(),
        &String::from_utf8_lossy(&o.stdout),
    );
    assert_eq!(chk, MergeCheck::Conflicts(vec!["a.txt".into()]));
    let o = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(merge_tree_args("main", "swarm/a"))
        .output()
        .unwrap();
    assert_eq!(
        parse_merge_tree(
            o.status.code().unwrap(),
            &String::from_utf8_lossy(&o.stdout)
        ),
        MergeCheck::Clean
    );
    a.cleanup(true).unwrap();
    b.cleanup(true).unwrap();
    let _ = std::fs::remove_dir_all(&repo);
}

// ---- SWARM-04 ----
#[test]
fn heal_exhausts_at_max_retries() {
    let mut h = HealLoop::new(HealPolicy::new(2));
    assert!(matches!(
        h.on_failure("cargo test", 101, "boom"),
        HealDecision::Retry { attempt: 1, .. }
    ));
    assert!(matches!(
        h.on_failure("cargo test", 101, "boom"),
        HealDecision::Retry { attempt: 2, .. }
    ));
    assert_eq!(
        h.on_failure("cargo test", 101, "boom"),
        HealDecision::Exhausted { attempts: 3 }
    );
    assert_eq!(
        h.on_failure("cargo test", 101, "boom"),
        HealDecision::Exhausted { attempts: 3 }
    );
    let mut zero = HealLoop::new(HealPolicy::new(0));
    assert!(matches!(
        zero.on_failure("x", 1, ""),
        HealDecision::Exhausted { .. }
    ));
}

#[test]
fn heal_prompt_is_bounded_clean_and_informative() {
    let noisy = format!(
        "\x1b[31mred\x1b[0m\x1b]0;title\x07{}END",
        "é".repeat(10_000)
    );
    let p = HealPolicy {
        max_retries: 2,
        max_feedback_bytes: 100,
    };
    let s = synthesize_prompt("cargo test", 101, &noisy, 1, &p);
    assert!(
        s.contains("101")
            && s.contains("cargo test")
            && s.contains("END")
            && s.contains("truncated")
    );
    assert!(!s.contains('\x1b') && !s.contains("title"));
    assert!(s.len() < 400);
    assert_eq!(strip_ansi("\x1b[1;32mok\x1b[0m"), "ok");
    assert_eq!(tail_bytes("aéb", 2), "b");
}

#[test]
fn argv_splitting_has_no_shell() {
    assert_eq!(
        split_argv("cargo test -p 'my pkg'").unwrap(),
        ["cargo", "test", "-p", "my pkg"]
    );
    assert!(split_argv("").is_err() && split_argv("a 'b").is_err());
    let d = std::env::temp_dir();
    let r = run_verify(
        "echo hi; touch /tmp/bilterm-p69-pwned",
        &d,
        Duration::from_secs(5),
    )
    .unwrap();
    assert!(r.output.contains("hi;"), "metachars are literal args");
    assert!(!Path::new("/tmp/bilterm-p69-pwned").exists());
}

#[test]
fn run_verify_exit_code_and_timeout() {
    let d = std::env::temp_dir();
    let ok = run_verify("true", &d, Duration::from_secs(5)).unwrap();
    assert_eq!(ok.exit_code, 0);
    let bad = run_verify("false", &d, Duration::from_secs(5)).unwrap();
    assert_ne!(bad.exit_code, 0);
    let t = run_verify("sleep 5", &d, Duration::from_millis(100)).unwrap();
    assert!(t.timed_out);
    assert!(run_verify("definitely-not-a-binary-p69", &d, Duration::from_secs(1)).is_err());
}

#[test]
fn heal_pass_after_n_failures_end_to_end() {
    let dir = std::env::temp_dir().join(format!("bilterm-p69-heal-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let marker = dir.join("fixed");
    let mut h = HealLoop::new(HealPolicy::new(2));
    let cmd = format!("test -f {}", marker.display());
    let mut passed = false;
    for round in 0..4 {
        let r = run_verify(&cmd, &dir, Duration::from_secs(5)).unwrap();
        if r.exit_code == 0 {
            passed = true;
            break;
        }
        match h.on_failure(&cmd, r.exit_code, &r.output) {
            HealDecision::Retry { .. } if round == 1 => std::fs::write(&marker, "x").unwrap(),
            HealDecision::Retry { .. } => {}
            HealDecision::Exhausted { .. } => break,
        }
    }
    assert!(passed);
    assert_eq!(h.retries_used(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}
