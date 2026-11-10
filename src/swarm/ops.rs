//! Git-backed swarm operations on manifest-tracked worktrees (SWARM-01/03).
//!
//! "Tracked" = a worktree git itself lists under `<repo>/.bilterm/worktrees/<slug>` on branch
//! `swarm/<slug>`; nothing else is ever diffed, merged or removed (T-WT-02).

use super::review::{self, DiffStat, MergeCheck, MergeStrategy};
use super::worktree::{BRANCH_PREFIX, WORKTREE_DIR, WorktreeSpec};
use super::SwarmError;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct GitOut {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn git(repo: &Path, args: &[String]) -> Result<GitOut, SwarmError> {
    let o = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| SwarmError::Git(e.to_string()))?;
    Ok(GitOut {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).trim().to_string(),
    })
}

fn git_ok(repo: &Path, args: &[&str]) -> Result<String, SwarmError> {
    let a: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let o = git(repo, &a)?;
    if o.code == 0 { Ok(o.stdout) } else { Err(SwarmError::Git(o.stderr)) }
}

/// Primary checkout root for any directory inside the repo or one of its worktrees.
pub fn repo_root(cwd: &Path) -> Result<PathBuf, SwarmError> {
    let common = git_ok(cwd, &["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
    PathBuf::from(common.trim())
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| SwarmError::Git("cannot resolve repository root".into()))
}

/// Parse `git worktree list --porcelain` into tracked swarm worktrees.
pub fn tracked(repo: &Path) -> Result<Vec<WorktreeSpec>, SwarmError> {
    let out = git_ok(repo, &["worktree", "list", "--porcelain"])?;
    let root = repo.join(WORKTREE_DIR);
    let mut found = Vec::new();
    let (mut path, mut branch): (Option<PathBuf>, Option<String>) = (None, None);
    let mut flush = |p: Option<PathBuf>, b: Option<String>| {
        if let (Some(p), Some(b)) = (p, b)
            && p.parent() == Some(root.as_path())
            && let Some(slug) = b.strip_prefix(&format!("refs/heads/{BRANCH_PREFIX}"))
            && p.file_name().and_then(|n| n.to_str()) == Some(slug)
            && let Ok(spec) = WorktreeSpec::new(repo, slug)
        {
            found.push(spec);
        }
    };
    for line in out.lines().chain(std::iter::once("")) {
        if let Some(p) = line.strip_prefix("worktree ") {
            path = Some(PathBuf::from(p));
        } else if let Some(b) = line.strip_prefix("branch ") {
            branch = Some(b.to_string());
        } else if line.is_empty() {
            flush(path.take(), branch.take());
        }
    }
    Ok(found)
}

pub fn find_tracked(repo: &Path, slug: &str) -> Result<WorktreeSpec, SwarmError> {
    crate::secure_fs::validate_safe_name(slug).map_err(SwarmError::InvalidName)?;
    tracked(repo)?
        .into_iter()
        .find(|s| s.slug == slug)
        .ok_or_else(|| SwarmError::Git(format!("'{slug}' is not a Bilterm-tracked swarm worktree")))
}

pub fn diff_stat(repo: &Path, slug: &str) -> Result<DiffStat, SwarmError> {
    let spec = find_tracked(repo, slug)?;
    let range = format!("HEAD...{}", spec.branch);
    Ok(review::parse_diff_stat(&git_ok(repo, &["diff", "--stat", &range])?))
}

pub fn primary_is_clean(repo: &Path) -> Result<bool, SwarmError> {
    Ok(git_ok(repo, &["status", "--porcelain", "--untracked-files=no"])?.trim().is_empty())
}

pub fn check_merge(repo: &Path, slug: &str) -> Result<MergeCheck, SwarmError> {
    let spec = find_tracked(repo, slug)?;
    let o = git(repo, &review::merge_tree_args("HEAD", &spec.branch))?;
    if o.code > 1 {
        return Err(SwarmError::Git(o.stderr));
    }
    Ok(review::parse_merge_tree(o.code, &o.stdout))
}

pub fn is_merged(repo: &Path, slug: &str) -> Result<bool, SwarmError> {
    let spec = find_tracked(repo, slug)?;
    let o = git(repo, &["merge-base".into(), "--is-ancestor".into(), spec.branch, "HEAD".into()])?;
    Ok(o.code == 0)
}

/// Merge a tracked branch into the primary checkout. Refuses on a dirty tree or conflicts.
pub fn merge(repo: &Path, slug: &str, squash: bool) -> Result<(), SwarmError> {
    let spec = find_tracked(repo, slug)?;
    if !primary_is_clean(repo)? {
        return Err(SwarmError::Git("primary working tree has uncommitted changes".into()));
    }
    if let MergeCheck::Conflicts(files) = check_merge(repo, slug)? {
        return Err(SwarmError::Git(format!("merge would conflict in: {}", files.join(", "))));
    }
    let strategy = if squash { MergeStrategy::Squash } else { MergeStrategy::Merge };
    for cmd in review::merge_plan(&[spec.branch], strategy, &[]) {
        let o = git(repo, &cmd)?;
        if o.code != 0 {
            let _ = git(repo, &["merge".into(), "--abort".into()]);
            return Err(SwarmError::Git(o.stderr));
        }
    }
    Ok(())
}

/// Remove a tracked worktree + branch; unmerged work needs `force`.
pub fn clean(repo: &Path, slug: &str, force: bool) -> Result<(), SwarmError> {
    let spec = find_tracked(repo, slug)?;
    // A squash merge is not an ancestor merge, so `force` is the escape hatch.
    if !force && !is_merged(repo, slug)? {
        return Err(SwarmError::Git(format!("'{slug}' has unmerged commits; use force to discard")));
    }
    spec.cleanup(force)
}
