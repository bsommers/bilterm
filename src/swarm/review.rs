//! SWARM-03: diff-stat parsing, merge-tree conflict pre-check, merge plan.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStat {
    pub path: String,
    pub changes: u32,
    pub binary: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffStat {
    pub files: Vec<FileStat>,
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
}

/// Parse `git diff --stat` output.
pub fn parse_diff_stat(out: &str) -> DiffStat {
    let mut st = DiffStat::default();
    for line in out.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        if let Some((path, rest)) = l.split_once(" | ") {
            let rest = rest.trim();
            let binary = rest.starts_with("Bin");
            let changes = rest
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            st.files.push(FileStat {
                path: path.trim().to_string(),
                changes,
                binary,
            });
        } else if l.contains("changed") {
            for part in l.split(',') {
                let n: u32 = part
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0);
                if part.contains("changed") {
                    st.files_changed = n;
                } else if part.contains("insertion") {
                    st.insertions = n;
                } else if part.contains("deletion") {
                    st.deletions = n;
                }
            }
        }
    }
    if st.files_changed == 0 {
        st.files_changed = st.files.len() as u32;
    }
    st
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeCheck {
    Clean,
    Conflicts(Vec<String>),
}

/// Interpret `git merge-tree --write-tree --name-only <a> <b>` (exit code + stdout).
/// Exit 0 = clean; exit 1 = conflicts, with conflicted paths after the tree oid line
/// and before the first blank line.
pub fn parse_merge_tree(exit_code: i32, stdout: &str) -> MergeCheck {
    if exit_code == 0 {
        return MergeCheck::Clean;
    }
    let mut files: Vec<String> = stdout
        .lines()
        .skip(1)
        .take_while(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .collect();
    files.sort();
    files.dedup();
    MergeCheck::Conflicts(files)
}

pub fn merge_tree_args(base: &str, branch: &str) -> Vec<String> {
    ["merge-tree", "--write-tree", "--name-only", base, branch]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    Merge,
    Squash,
    CherryPick,
}

/// Build the ordered git command lines (args after `git`) to integrate branches.
/// `commits` supplies commit ids per branch for `CherryPick`.
pub fn merge_plan(
    branches: &[String],
    strategy: MergeStrategy,
    commits: &[Vec<String>],
) -> Vec<Vec<String>> {
    let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let mut cmds = Vec::new();
    for (i, b) in branches.iter().enumerate() {
        match strategy {
            MergeStrategy::Merge => {
                let mut c = v(&["merge", "--no-ff", "-m"]);
                c.push(format!("swarm: merge {b}"));
                c.push(b.clone());
                cmds.push(c);
            }
            MergeStrategy::Squash => {
                cmds.push(vec!["merge".into(), "--squash".into(), b.clone()]);
                cmds.push(vec![
                    "commit".into(),
                    "-m".into(),
                    format!("swarm: squash {b}"),
                ]);
            }
            MergeStrategy::CherryPick => {
                if let Some(cs) = commits.get(i) {
                    for c in cs {
                        cmds.push(vec!["cherry-pick".into(), c.clone()]);
                    }
                }
            }
        }
    }
    cmds
}
