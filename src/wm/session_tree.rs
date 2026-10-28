//! Multi-Root & Hierarchical Session Trees (`TREE-01`, `TREE-02`).
//!
//! Supports independent root sessions (`SessionNodeKind::Independent`), MCP daemon-rooted
//! sessions (`SessionNodeKind::Rooted`), and arbitrarily nested child subtrees
//! (`SessionNodeKind::Subtree`) with cycle-free `branch`, `graft`, and `detach_subtree`
//! operations, subtree freeze/thaw, fold/unfold, and `herdr` drawer tree rendering.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Topology classification of a session node within [`SessionTree`] (`TREE-01`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SessionNodeKind {
    /// Standalone root workspace session not attached under a parent session.
    #[default]
    Independent,
    /// Rooted session attached directly under the `MCP` Master Control Program root.
    Rooted,
    /// Hierarchical child session branched under a parent session (`parent/child`).
    Subtree,
}

/// Single session node inside [`SessionTree`] (`TREE-01`, `TREE-02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTreeNode {
    pub name: String,
    pub kind: SessionNodeKind,
    pub parent: Option<String>,
    pub children: Vec<String>,
    pub collapsed: bool,
    pub frozen: bool,
    pub pane_count: usize,
    pub active_commands: usize,
}

impl SessionTreeNode {
    /// Creates a new [`SessionTreeNode`] with 1 pane and default telemetry.
    pub fn new(name: impl Into<String>, kind: SessionNodeKind, parent: Option<String>) -> Self {
        Self {
            name: name.into(),
            kind,
            parent,
            children: Vec::new(),
            collapsed: false,
            frozen: false,
            pane_count: 1,
            active_commands: 0,
        }
    }
}

/// Multi-root and hierarchical session forest (`TREE-01`, `TREE-02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTree {
    pub nodes: Vec<SessionTreeNode>,
}

impl Default for SessionTree {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionTree {
    /// Creates a [`SessionTree`] initialized with the default `"main"` (`Rooted`) session.
    pub fn new() -> Self {
        Self {
            nodes: vec![SessionTreeNode::new("main", SessionNodeKind::Rooted, None)],
        }
    }

    /// Looks up a session node by `name`.
    pub fn node(&self, name: &str) -> Option<&SessionTreeNode> {
        self.nodes.iter().find(|n| n.name == name)
    }

    /// Looks up a mutable session node by `name`.
    pub fn node_mut(&mut self, name: &str) -> Option<&mut SessionTreeNode> {
        self.nodes.iter_mut().find(|n| n.name == name)
    }

    /// Inserts or updates a session node in the forest while maintaining parent/child links.
    pub fn upsert_session(&mut self, name: &str, kind: SessionNodeKind, parent: Option<&str>) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Some(p) = parent
            && self.node(p).is_none()
        {
            self.nodes
                .push(SessionTreeNode::new(p, SessionNodeKind::Rooted, None));
        }

        let old_parent = self.node(trimmed).and_then(|n| n.parent.clone());
        if let Some(old_p) = old_parent
            && Some(old_p.as_str()) != parent
            && let Some(p_node) = self.node_mut(&old_p)
        {
            p_node.children.retain(|c| c != trimmed);
        }

        let effective_kind = if parent.is_some() {
            SessionNodeKind::Subtree
        } else {
            kind
        };

        if let Some(existing) = self.node_mut(trimmed) {
            existing.kind = effective_kind;
            existing.parent = parent.map(ToOwned::to_owned);
        } else {
            self.nodes.push(SessionTreeNode::new(
                trimmed,
                effective_kind,
                parent.map(ToOwned::to_owned),
            ));
        }

        if let Some(p) = parent
            && let Some(p_node) = self.node_mut(p)
            && !p_node.children.iter().any(|c| c == trimmed)
        {
            p_node.children.push(trimmed.to_string());
        }
    }

    /// Branches `child` under `parent`, creating either node if needed and rejecting cycles (`TREE-01`).
    pub fn branch(&mut self, parent: &str, child: &str) -> Result<(), String> {
        let parent = parent.trim();
        let child = child.trim();
        if parent.is_empty() || child.is_empty() {
            return Err("session names must not be empty".to_string());
        }
        if parent == child {
            return Err(format!("cannot branch session '{child}' under itself"));
        }
        if self.node(child).is_some() {
            let descendants = self.subtree_session_names(child);
            if descendants.iter().any(|d| d == parent) {
                return Err(format!(
                    "cycle detected: '{parent}' is already a descendant of '{child}'"
                ));
            }
        }
        if self.node(parent).is_none() {
            self.upsert_session(parent, SessionNodeKind::Rooted, None);
        }
        self.upsert_session(child, SessionNodeKind::Subtree, Some(parent));
        Ok(())
    }

    /// Grafts an existing subtree rooted at `src` under `dst_parent` with cycle detection (`TREE-01`).
    pub fn graft(&mut self, src: &str, dst_parent: &str) -> Result<(), String> {
        let src = src.trim();
        let dst_parent = dst_parent.trim();
        if src.is_empty() || dst_parent.is_empty() {
            return Err("session names must not be empty".to_string());
        }
        if src == dst_parent {
            return Err(format!("cannot graft session '{src}' onto itself"));
        }
        if self.node(src).is_none() {
            return Err(format!("source session '{src}' does not exist"));
        }
        if self.node(dst_parent).is_none() {
            return Err(format!(
                "destination parent session '{dst_parent}' does not exist"
            ));
        }
        let descendants = self.subtree_session_names(src);
        if descendants.iter().any(|d| d == dst_parent) {
            return Err(format!(
                "cycle detected: cannot graft '{src}' under its own descendant '{dst_parent}'"
            ));
        }

        let old_parent = self.node(src).and_then(|n| n.parent.clone());
        if let Some(old_p) = old_parent
            && let Some(old_p_node) = self.node_mut(&old_p)
        {
            old_p_node.children.retain(|c| c != src);
        }

        if let Some(src_node) = self.node_mut(src) {
            src_node.parent = Some(dst_parent.to_string());
            src_node.kind = SessionNodeKind::Subtree;
        }
        if let Some(dst_node) = self.node_mut(dst_parent)
            && !dst_node.children.iter().any(|c| c == src)
        {
            dst_node.children.push(src.to_string());
        }
        Ok(())
    }

    /// Detaches `node` from its parent and promotes it to an `Independent` root while keeping its descendants (`TREE-01`).
    pub fn detach_subtree(&mut self, node: &str) -> Result<(), String> {
        let node = node.trim();
        let Some(old_parent) = self.node(node).map(|n| n.parent.clone()) else {
            return Err(format!("session '{node}' does not exist"));
        };
        if let Some(old_p) = old_parent
            && let Some(p_node) = self.node_mut(&old_p)
        {
            p_node.children.retain(|c| c != node);
        }
        if let Some(target) = self.node_mut(node) {
            target.parent = None;
            target.kind = SessionNodeKind::Independent;
        }
        Ok(())
    }

    /// Removes `name` from the tree and re-parents its direct children to `name`'s parent (or `Independent`).
    pub fn remove_session(&mut self, name: &str) -> bool {
        let Some(idx) = self.nodes.iter().position(|n| n.name == name) else {
            return false;
        };
        let removed = self.nodes.remove(idx);

        if let Some(ref p) = removed.parent
            && let Some(p_node) = self.node_mut(p)
        {
            p_node.children.retain(|c| c != name);
            for child in &removed.children {
                if !p_node.children.iter().any(|c| c == child) {
                    p_node.children.push(child.clone());
                }
            }
        }

        for child in &removed.children {
            if let Some(c_node) = self.node_mut(child) {
                c_node.parent = removed.parent.clone();
                if c_node.parent.is_none() {
                    c_node.kind = SessionNodeKind::Independent;
                }
            }
        }
        true
    }

    /// Returns `root` and all its descendant session names in pre-order DFS traversal (`TREE-02`).
    pub fn subtree_session_names(&self, root: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut visited = BTreeSet::new();
        self.collect_subtree_dfs(root, &mut visited, &mut out);
        out
    }

    fn collect_subtree_dfs(
        &self,
        current: &str,
        visited: &mut BTreeSet<String>,
        out: &mut Vec<String>,
    ) {
        if !visited.insert(current.to_string()) {
            return;
        }
        let Some(node) = self.node(current) else {
            return;
        };
        out.push(node.name.clone());
        for child in &node.children {
            self.collect_subtree_dfs(child, visited, out);
        }
    }

    /// Toggles the drawer fold state (`collapsed`) for session `name` (`TREE-02`).
    pub fn toggle_fold(&mut self, name: &str) -> bool {
        if let Some(node) = self.node_mut(name) {
            node.collapsed = !node.collapsed;
            node.collapsed
        } else {
            false
        }
    }

    /// Freezes or thaws `root` and all sessions in its subtree, returning affected session names (`TREE-02`).
    pub fn set_subtree_frozen(&mut self, root: &str, frozen: bool) -> Vec<String> {
        let targets = self.subtree_session_names(root);
        for name in &targets {
            if let Some(node) = self.node_mut(name) {
                node.frozen = frozen;
            }
        }
        targets
    }

    /// Computes `(total_panes, total_active_commands)` rolled up across `root` and its descendants.
    pub fn subtree_rollup(&self, root: &str) -> (usize, usize) {
        let names = self.subtree_session_names(root);
        let mut panes = 0usize;
        let mut active = 0usize;
        for name in names {
            if let Some(n) = self.node(&name) {
                panes += n.pane_count;
                active += n.active_commands;
            }
        }
        (panes, active)
    }

    /// Renders `herdr`-style hierarchical session tree lines with connectors, fold badges, and rollup counts (`TREE-02`).
    pub fn render_herdr_tree_lines(&self, active_session: &str) -> Vec<String> {
        let mut lines = vec!["◈ ROOT [MCP — MASTER CONTROL PROGRAM]".to_string()];
        let roots: Vec<&SessionTreeNode> = self
            .nodes
            .iter()
            .filter(|n| n.parent.is_none())
            .collect();

        let mut visited = BTreeSet::new();
        for (idx, root) in roots.iter().enumerate() {
            let is_last = idx + 1 == roots.len();
            self.render_node_recursive(root, "", is_last, active_session, &mut visited, &mut lines);
        }
        lines
    }

    fn render_node_recursive(
        &self,
        node: &SessionTreeNode,
        prefix: &str,
        is_last: bool,
        active_session: &str,
        visited: &mut BTreeSet<String>,
        lines: &mut Vec<String>,
    ) {
        if !visited.insert(node.name.clone()) {
            return;
        }
        let connector = if is_last { "╰─" } else { "├─" };
        let fold_glyph = if node.children.is_empty() {
            "•"
        } else if node.collapsed {
            "▸"
        } else {
            "▾"
        };
        let kind_tag = match node.kind {
            SessionNodeKind::Independent => "[IND]",
            SessionNodeKind::Rooted => "[MCP]",
            SessionNodeKind::Subtree => "[SUB]",
        };
        let active_marker = if node.name == active_session { "*" } else { "" };
        let freeze_badge = if node.frozen { " [❄ FROZEN]" } else { "" };
        let (rollup_panes, rollup_running) = self.subtree_rollup(&node.name);
        let rollup_suffix = if !node.children.is_empty() {
            format!(" (subtree: {rollup_panes}p/{rollup_running}r)")
        } else {
            format!(" ({}p/{}r)", node.pane_count, node.active_commands)
        };

        lines.push(format!(
            "{prefix}{connector} {fold_glyph} {}{active_marker} {kind_tag}{freeze_badge}{rollup_suffix}",
            node.name
        ));

        if node.collapsed {
            return;
        }
        let child_prefix = format!("{prefix}{}", if is_last { "   " } else { "│  " });
        let valid_children: Vec<&SessionTreeNode> = node
            .children
            .iter()
            .filter_map(|c| self.node(c))
            .collect();
        for (i, child) in valid_children.iter().enumerate() {
            let child_last = i + 1 == valid_children.len();
            self.render_node_recursive(
                child,
                &child_prefix,
                child_last,
                active_session,
                visited,
                lines,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_tree_branch_graft_and_cycle_rejection() {
        let mut tree = SessionTree::new();
        tree.branch("main", "build").unwrap();
        tree.branch("build", "test-runner").unwrap();

        assert_eq!(
            tree.subtree_session_names("main"),
            vec!["main", "build", "test-runner"]
        );

        // Reject grafting a parent under its own descendant
        let err = tree.graft("main", "test-runner").unwrap_err();
        assert!(err.contains("cycle detected"));

        // Reject branching a parent under its own descendant
        let err2 = tree.branch("test-runner", "main").unwrap_err();
        assert!(err2.contains("cycle detected"));

        // Valid graft onto another root
        tree.upsert_session("ops", SessionNodeKind::Independent, None);
        tree.graft("build", "ops").unwrap();
        assert_eq!(tree.subtree_session_names("main"), vec!["main"]);
        assert_eq!(
            tree.subtree_session_names("ops"),
            vec!["ops", "build", "test-runner"]
        );
    }

    #[test]
    fn session_tree_detach_subtree_promotes_to_independent_root() {
        let mut tree = SessionTree::new();
        tree.branch("main", "agents").unwrap();
        tree.branch("agents", "worker-1").unwrap();

        tree.detach_subtree("agents").unwrap();
        let agents = tree.node("agents").unwrap();
        assert_eq!(agents.kind, SessionNodeKind::Independent);
        assert_eq!(agents.parent, None);
        assert_eq!(
            tree.subtree_session_names("agents"),
            vec!["agents", "worker-1"]
        );
        assert_eq!(tree.subtree_session_names("main"), vec!["main"]);
    }

    #[test]
    fn session_tree_freeze_fold_and_render_herdr_lines() {
        let mut tree = SessionTree::new();
        tree.branch("main", "sub-a").unwrap();
        tree.branch("sub-a", "leaf-1").unwrap();

        let frozen = tree.set_subtree_frozen("sub-a", true);
        assert_eq!(frozen, vec!["sub-a", "leaf-1"]);
        assert!(tree.node("leaf-1").unwrap().frozen);

        let lines = tree.render_herdr_tree_lines("main");
        assert!(lines[0].contains("MASTER CONTROL PROGRAM"));
        assert!(lines.iter().any(|l| l.contains("sub-a") && l.contains("❄ FROZEN")));
        assert!(lines.iter().any(|l| l.contains("leaf-1")));

        // Fold sub-a so leaf-1 is hidden from rendered output
        assert!(tree.toggle_fold("sub-a"));
        let folded_lines = tree.render_herdr_tree_lines("main");
        assert!(!folded_lines.iter().any(|l| l.contains("leaf-1")));
    }

    #[test]
    fn session_tree_remove_session_reparents_children() {
        let mut tree = SessionTree::new();
        tree.branch("main", "mid").unwrap();
        tree.branch("mid", "leaf").unwrap();

        assert!(tree.remove_session("mid"));
        assert!(tree.node("mid").is_none());
        let leaf = tree.node("leaf").unwrap();
        assert_eq!(leaf.parent.as_deref(), Some("main"));
        assert!(tree.node("main").unwrap().children.contains(&"leaf".to_string()));
    }
}
