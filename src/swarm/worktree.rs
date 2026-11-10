//! SWARM-01: ephemeral git-worktree specs, command builders, create/cleanup.

use super::SwarmError;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

pub const WORKTREE_DIR: &str = ".bilterm/worktrees";
pub const BRANCH_PREFIX: &str = "swarm/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeSpec {
    pub slug: String,
    pub repo_root: PathBuf,
    pub path: PathBuf,
    pub branch: String,
}

impl WorktreeSpec {
    /// Validate the slug and derive `<repo>/.bilterm/worktrees/<slug>` + `swarm/<slug>`.
    pub fn new(repo_root: &Path, slug: &str) -> Result<Self, SwarmError> {
        crate::secure_fs::validate_safe_name(slug).map_err(SwarmError::InvalidName)?;
        let path = repo_root.join(WORKTREE_DIR).join(slug);
        let spec = Self {
            slug: slug.to_string(),
            repo_root: repo_root.to_path_buf(),
            path,
            branch: format!("{BRANCH_PREFIX}{slug}"),
        };
        spec.check_contained()?;
        Ok(spec)
    }

    /// T-WT-01: the worktree path must lie strictly under the worktree root.
    fn check_contained(&self) -> Result<(), SwarmError> {
        let root = self.repo_root.join(WORKTREE_DIR);
        let ok = self.path.starts_with(&root)
            && self.path != root
            && !self
                .path
                .components()
                .any(|c| matches!(c, Component::ParentDir));
        if ok {
            Ok(())
        } else {
            Err(SwarmError::InvalidName(format!(
                "path escapes worktree root: {}",
                self.path.display()
            )))
        }
    }

    /// `git worktree add -b swarm/<slug> <path>` (args after `git`).
    pub fn add_args(&self) -> Vec<String> {
        vec![
            "worktree".into(),
            "add".into(),
            "-b".into(),
            self.branch.clone(),
            self.path.to_string_lossy().into_owned(),
        ]
    }

    pub fn remove_args(&self, force: bool) -> Vec<String> {
        let mut v = vec!["worktree".into(), "remove".into()];
        if force {
            v.push("--force".into());
        }
        v.push(self.path.to_string_lossy().into_owned());
        v
    }

    pub fn branch_delete_args(&self, force: bool) -> Vec<String> {
        vec![
            "branch".into(),
            if force { "-D" } else { "-d" }.into(),
            self.branch.clone(),
        ]
    }

    fn git(&self, args: &[String]) -> Result<String, SwarmError> {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.repo_root)
            .args(args)
            .output()
            .map_err(|e| SwarmError::Git(e.to_string()))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(SwarmError::Git(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ))
        }
    }

    /// Create the worktree + branch. On failure nothing is left behind.
    pub fn create(&self) -> Result<(), SwarmError> {
        if let Err(e) = self.git(&self.add_args()) {
            let _ = self.cleanup(true);
            return Err(e);
        }
        Ok(())
    }

    /// Remove worktree and branch (best effort for already-missing pieces).
    pub fn cleanup(&self, force: bool) -> Result<(), SwarmError> {
        self.check_contained()?;
        let mut first_err = None;
        if self.path.exists()
            && let Err(e) = self.git(&self.remove_args(force))
        {
            first_err = Some(e);
        }
        let _ = self.git(&["worktree".into(), "prune".into()]);
        let branch_exists = self
            .git(&[
                "rev-parse".into(),
                "--verify".into(),
                "--quiet".into(),
                format!("refs/heads/{}", self.branch),
            ])
            .is_ok();
        if branch_exists && let Err(e) = self.git(&self.branch_delete_args(force)) {
            first_err.get_or_insert(e);
        }
        first_err.map_or(Ok(()), Err)
    }
}
