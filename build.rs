//! Build metadata: full version (keeps pre-release suffixes such as `-rc7`), commit, target, rustc.
//!
//! Version precedence: `BILTERM_BUILD_VERSION` env (release scripts) > `git describe --tags` > Cargo.toml.
//! Everything is derived from the repository state (no wall-clock), so rebuilding a commit is reproducible.
use std::process::Command;

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn main() {
    let pkg = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    println!("cargo:rerun-if-env-changed=BILTERM_BUILD_VERSION");
    for p in ["HEAD", "refs/tags", "packed-refs", "index"] {
        if let Some(path) = run("git", &["rev-parse", "--git-path", p]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }

    let described = run("git", &["describe", "--tags", "--match", "v[0-9]*", "--dirty", "--always"]);
    let version = std::env::var("BILTERM_BUILD_VERSION")
        .ok()
        .map(|v| v.trim().trim_start_matches('v').to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| {
            let d = described.clone()?;
            let d = d.strip_prefix('v')?; // `--always` fallback (bare sha) has no tag: use Cargo version
            d.starts_with(|c: char| c.is_ascii_digit()).then(|| d.to_string())
        })
        .unwrap_or_else(|| pkg.clone());

    let commit = run("git", &["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = run("git", &["status", "--porcelain", "--untracked-files=no"]).is_some();
    let commit_date = run("git", &["show", "-s", "--format=%cI", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let rustc_v = run(&rustc, &["--version"]).unwrap_or_else(|| "unknown".into());
    let mut features: Vec<String> = std::env::vars()
        .filter_map(|(k, _)| k.strip_prefix("CARGO_FEATURE_").map(|f| f.to_lowercase().replace('_', "-")))
        .collect();
    features.sort();

    let env = |k: &str| std::env::var(k).unwrap_or_default();
    println!("cargo:rustc-env=BILTERM_VERSION={version}");
    println!("cargo:rustc-env=BILTERM_GIT_COMMIT={commit}");
    println!("cargo:rustc-env=BILTERM_GIT_DIRTY={dirty}");
    println!("cargo:rustc-env=BILTERM_COMMIT_DATE={commit_date}");
    println!("cargo:rustc-env=BILTERM_RUSTC={rustc_v}");
    println!("cargo:rustc-env=BILTERM_TARGET={}", env("TARGET"));
    println!("cargo:rustc-env=BILTERM_PROFILE={}", env("PROFILE"));
    println!("cargo:rustc-env=BILTERM_FEATURES={}", features.join(","));
}
