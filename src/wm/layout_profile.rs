//! Multi-profile pane/window layout manager (`src/wm/layout_profile.rs`).
//!
//! Supports:
//! - Default saving of pane/window layout profiles (`:profile save [name]`, `:layout save [name]`)
//! - Automatic layout saving by default (`config.layout_autosave = true`) on pane/window mutations
//! - Runtime toggle to enable/disable layout autosave (`:layout autosave [on|off|toggle]`, `:autosave`)
//! - Multiple named layout profiles (persisted `.toml` profiles + built-in starter templates:
//!   `default`, `single`, `dual-v`, `dual-h`, `main-side`, `quad`, `cascade`)
//! - Selecting which profile is set as the active default (`:profile default <name>`, `:layout default <name>`)
//! - Switching between layout profiles dynamically (`:profile switch <name>`, `:layout switch <name>`, `:profile next|prev`)

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::{SavedWorkspaceSession, SplitDirection, WindowManager, WorkspaceLayoutMode};
use crate::error::Result;
use crate::pane::{CompositePane, PaneId};
use crate::pty::PtyConfig;
use crate::wm::split_tree::SplitTree;
use crate::wm::z_stack::ZStack;

/// Built-in layout profile templates available out of the box alongside user-saved `.toml` profiles.
pub const BUILTIN_LAYOUT_PROFILES: &[(&str, &str)] = &[
    ("default", "User default workspace layout profile"),
    ("single", "Single full-screen composite terminal pane (Tiled)"),
    ("dual-v", "2 composite panes side-by-side (Left | Right)"),
    ("dual-h", "2 composite panes stacked (Top / Bottom)"),
    ("main-side", "3 composite panes: Left main + 2 Right stacked panes"),
    ("quad", "4 composite panes in a 2x2 grid"),
    ("cascade", "3 overlapping Borland Turbo Vision cascaded windows (Z-Stack)"),
];

/// Persisted metadata tracking the user's selected default layout profile and autosave preference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutProfileMeta {
    pub default_profile: String,
    pub active_profile: String,
    pub autosave: bool,
}

impl Default for LayoutProfileMeta {
    fn default() -> Self {
        Self {
            default_profile: "default".to_string(),
            active_profile: "default".to_string(),
            autosave: true,
        }
    }
}

/// Sanitizes a layout profile name for safe filesystem/registry use.
pub fn sanitize_profile_name(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect();
    if cleaned.is_empty() {
        "default".to_string()
    } else {
        cleaned
    }
}

impl WindowManager {
    /// Returns `true` if layout profile changes should also be persisted to disk (`<session_dir>`).
    fn should_persist_profiles_to_disk(&self) -> bool {
        !self.headless_mode || self.config.session_dir.is_some()
    }

    /// Path to `layout_profile_meta.toml` inside `<session_dir>`.
    fn layout_profile_meta_path(&self) -> Result<PathBuf> {
        Ok(self.resolve_session_dir()?.join("layout_profile_meta.toml"))
    }

    /// Saves `LayoutProfileMeta` (`default_profile`, `active_profile`, `autosave`) to disk when enabled.
    pub fn persist_layout_profile_meta(&self) {
        if !self.should_persist_profiles_to_disk() {
            return;
        }
        let Ok(dir) = self.resolve_session_dir() else {
            return;
        };
        let _ = std::fs::create_dir_all(&dir);
        let meta = LayoutProfileMeta {
            default_profile: self.config.default_layout_profile.clone(),
            active_profile: self.config.active_layout_profile.clone(),
            autosave: self.config.layout_autosave,
        };
        if let Ok(toml_str) = toml::to_string_pretty(&meta) {
            let _ = std::fs::write(dir.join("layout_profile_meta.toml"), toml_str);
        }
    }

    /// Loads `LayoutProfileMeta` from `<session_dir>/layout_profile_meta.toml` if present.
    pub fn load_layout_profile_meta(&mut self) -> Option<LayoutProfileMeta> {
        let path = self.layout_profile_meta_path().ok()?;
        let raw = std::fs::read_to_string(path).ok()?;
        let meta: LayoutProfileMeta = toml::from_str(&raw).ok()?;
        self.config.default_layout_profile = sanitize_profile_name(&meta.default_profile);
        self.config.active_layout_profile = sanitize_profile_name(&meta.active_profile);
        self.config.layout_autosave = meta.autosave;
        Some(meta)
    }

    /// Returns `true` if automatic layout saving is enabled (`true` by default).
    pub fn is_layout_autosave_enabled(&self) -> bool {
        self.config.layout_autosave
    }

    /// Sets or toggles automatic layout saving (`:layout autosave [on|off|toggle]`, `:autosave`).
    pub fn set_layout_autosave(&mut self, enabled: Option<bool>) -> bool {
        let next = enabled.unwrap_or(!self.config.layout_autosave);
        self.config.layout_autosave = next;
        self.persist_layout_profile_meta();
        if next {
            self.maybe_autosave_layout();
        }
        next
    }

    /// Returns the name of the currently active layout profile.
    pub fn active_layout_profile(&self) -> &str {
        &self.config.active_layout_profile
    }

    /// Returns the name of the user's selected default layout profile.
    pub fn default_layout_profile(&self) -> &str {
        &self.config.default_layout_profile
    }

    /// Automatically saves the current pane/window layout to the active layout profile
    /// when `config.layout_autosave` is `true`.
    pub fn maybe_autosave_layout(&mut self) -> bool {
        if !self.config.layout_autosave {
            return false;
        }
        let active = sanitize_profile_name(&self.config.active_layout_profile);
        self.config.active_layout_profile = active.clone();
        let snap = self.snapshot_session(&active);
        self.layout_profiles_mem.insert(active.clone(), snap);
        if self.should_persist_profiles_to_disk() {
            let _ = self.save_session(&active);
            self.persist_layout_profile_meta();
        }
        true
    }

    /// Explicitly saves the current workspace pane/window layout under profile `name` (or the active profile if `None`).
    pub fn save_layout_profile(&mut self, name: Option<&str>) -> Result<String> {
        let profile = match name.filter(|s| !s.trim().is_empty()) {
            Some(n) => sanitize_profile_name(n),
            None => sanitize_profile_name(&self.config.active_layout_profile),
        };
        self.config.active_layout_profile = profile.clone();
        let snap = self.snapshot_session(&profile);
        self.layout_profiles_mem.insert(profile.clone(), snap);
        if self.should_persist_profiles_to_disk() {
            let _ = self.save_session(&profile)?;
            self.persist_layout_profile_meta();
        }
        Ok(profile)
    }

    /// Sets `name` as the active default layout profile (`default_layout_profile`) loaded on startup.
    /// If `name` does not exist yet, saves the current layout under `name`.
    pub fn set_default_layout_profile(&mut self, name: Option<&str>) -> Result<String> {
        let profile = match name.filter(|s| !s.trim().is_empty()) {
            Some(n) => sanitize_profile_name(n),
            None => sanitize_profile_name(&self.config.active_layout_profile),
        };
        self.config.default_layout_profile = profile.clone();
        if !self.has_saved_layout_profile(&profile)
            && !Self::is_builtin_template_only(&profile)
        {
            let _ = self.save_layout_profile(Some(&profile))?;
        }
        self.persist_layout_profile_meta();
        Ok(profile)
    }

    /// Returns `true` if `name` has a saved snapshot in memory or on disk (`<session_dir>/<name>.toml`).
    pub fn has_saved_layout_profile(&self, name: &str) -> bool {
        let profile = sanitize_profile_name(name);
        if self.layout_profiles_mem.contains_key(&profile) {
            return true;
        }
        if let Ok(dir) = self.resolve_session_dir() {
            let p = dir.join(format!("{profile}.toml"));
            if p.is_file() {
                return true;
            }
        }
        false
    }

    fn is_builtin_template_only(name: &str) -> bool {
        matches!(
            name,
            "single" | "dual-v" | "dual-h" | "main-side" | "quad" | "cascade"
        )
    }

    /// Lists all available layout profiles (saved custom profiles + built-in templates) in deterministic order.
    pub fn list_layout_profiles(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for (builtin, _) in BUILTIN_LAYOUT_PROFILES {
            names.push((*builtin).to_string());
        }
        for k in self.layout_profiles_mem.keys() {
            if k != "layout_profile_meta" && !names.contains(k) {
                names.push(k.clone());
            }
        }
        if self.should_persist_profiles_to_disk() {
            for s in self.list_sessions() {
                if s != "layout_profile_meta" && !names.contains(&s) {
                    names.push(s);
                }
            }
        }
        if !names.contains(&self.config.default_layout_profile) {
            names.insert(0, self.config.default_layout_profile.clone());
        }
        if !names.contains(&self.config.active_layout_profile) {
            names.push(self.config.active_layout_profile.clone());
        }
        names
    }

    /// Deletes a saved layout profile from memory and disk (cannot delete `"default"`).
    pub fn delete_layout_profile(&mut self, name: &str) -> std::result::Result<String, String> {
        let profile = sanitize_profile_name(name);
        if profile == "default" {
            return Err("Cannot delete the 'default' layout profile".to_string());
        }
        let mut removed = self.layout_profiles_mem.remove(&profile).is_some();
        if let Ok(dir) = self.resolve_session_dir() {
            let p = dir.join(format!("{profile}.toml"));
            if p.is_file() && std::fs::remove_file(p).is_ok() {
                removed = true;
            }
        }
        if self.config.default_layout_profile == profile {
            self.config.default_layout_profile = "default".to_string();
        }
        if self.config.active_layout_profile == profile {
            self.config.active_layout_profile = self.config.default_layout_profile.clone();
        }
        self.persist_layout_profile_meta();
        if removed {
            Ok(profile)
        } else {
            Err(format!("Layout profile '{profile}' not found"))
        }
    }

    /// Restores a `SavedWorkspaceSession` directly in-place inside `self`.
    fn apply_saved_session_snapshot(&mut self, saved: &SavedWorkspaceSession) {
        let (rows, cols) = self.default_size;
        let mut new_panes = BTreeMap::new();
        let mut max_id = 1usize;
        for sp in &saved.panes {
            max_id = max_id.max(sp.id);
            let mut pane = if self.headless_mode {
                CompositePane::new_headless(sp.id, rows, cols)
            } else {
                CompositePane::spawn(
                    sp.id,
                    PtyConfig {
                        rows,
                        cols,
                        cwd: Some(PathBuf::from(&sp.cwd)),
                        ..Default::default()
                    },
                )
                .unwrap_or_else(|_| CompositePane::new_headless(sp.id, rows, cols))
            };
            pane.set_title(sp.title.clone());
            pane.prompt_mut().anchor = sp.prompt_anchor;
            pane.prompt_mut().height = sp.prompt_height.clamp(3, 12);
            pane.prompt_mut().width_cols = sp.prompt_width_cols;
            pane.prompt_mut().x_offset = sp.prompt_x_offset;
            if !sp.prompt_input.is_empty() {
                pane.prompt_mut().set_text(&sp.prompt_input);
            }
            pane.font_options_mut().scale_pct = sp.font_scale_pct;
            if let Some(sm) = sp.scroll_mode {
                pane.set_scroll_mode(sm);
            }
            if !sp.scrollback_lines.is_empty() {
                let replayed = format!("{}\r\n", sp.scrollback_lines.join("\r\n"));
                pane.process_pty_output(replayed.as_bytes());
            }
            pane.set_read_only(sp.read_only);
            Self::attach_history(&mut pane, &self.config.history_settings());
            pane.prompt_mut().history = sp.history.clone();
            new_panes.insert(sp.id, pane);
        }

        if new_panes.is_empty() {
            new_panes.insert(1, CompositePane::new_headless(1, rows, cols));
        }

        self.panes = new_panes;
        self.split_tree = saved.split_tree.clone();
        let geom_map: BTreeMap<PaneId, crate::wm::FloatingWindowGeometry> = saved
            .z_geometries
            .iter()
            .filter_map(|(k, v)| k.parse::<PaneId>().ok().map(|id| (id, *v)))
            .collect();
        self.z_stack = ZStack::from_order_and_geometries(saved.z_order.clone(), geom_map);
        self.layout_mode = saved.layout_mode;
        self.zoomed_pane = saved.zoomed_pane;
        self.next_pane_id = max_id + 1;
        let vp = ratatui::layout::Rect::new(0, 0, self.default_size.1, self.default_size.0);
        let _ = self.resize_all(vp);
    }

    /// Applies one of the built-in layout templates (`single`, `dual-v`, `dual-h`, `main-side`, `quad`, `cascade`) dynamically.
    fn apply_builtin_layout_template(&mut self, template: &str) -> Result<usize> {
        let target_panes = match template {
            "single" => 1,
            "dual-v" | "side-by-side" | "dual-h" | "top-bottom" => 2,
            "main-side" | "ide" | "cascade" | "borland-stack" => 3,
            "quad" | "grid-4" => 4,
            _ => return Ok(self.panes.len()),
        };

        let prev_auto = self.config.layout_autosave;
        self.config.layout_autosave = false;
        // Ensure we have at least 1 pane, then trim or spawn to match `target_panes`
        while self.panes.len() > target_panes {
            if let Some(&last_id) = self.panes.keys().next_back() {
                if !self.remove_pane(last_id) {
                    break;
                }
            } else {
                break;
            }
        }
        while self.panes.len() < target_panes {
            let res = self.split_focused(SplitDirection::Vertical);
            if let Err(e) = res {
                self.config.layout_autosave = prev_auto;
                return Err(e);
            }
        }
        self.config.layout_autosave = prev_auto;

        let ids: Vec<PaneId> = self.panes.keys().copied().collect();
        let first = ids[0];
        let mut tree = SplitTree::new(first);
        let mut mode = WorkspaceLayoutMode::Tiled;

        match template {
            "single" => {}
            "dual-v" | "side-by-side" => {
                if ids.len() >= 2 {
                    tree.split_leaf(ids[0], ids[1], SplitDirection::Vertical);
                }
            }
            "dual-h" | "top-bottom" => {
                if ids.len() >= 2 {
                    tree.split_leaf(ids[0], ids[1], SplitDirection::Horizontal);
                }
            }
            "main-side" | "ide" => {
                if ids.len() >= 3 {
                    tree.split_leaf(ids[0], ids[1], SplitDirection::Vertical);
                    tree.split_leaf(ids[1], ids[2], SplitDirection::Horizontal);
                }
            }
            "quad" | "grid-4" => {
                if ids.len() >= 4 {
                    tree.split_leaf(ids[0], ids[1], SplitDirection::Vertical);
                    tree.split_leaf(ids[0], ids[2], SplitDirection::Horizontal);
                    tree.split_leaf(ids[1], ids[3], SplitDirection::Horizontal);
                }
            }
            "cascade" | "borland-stack" => {
                mode = WorkspaceLayoutMode::Stacked;
                if ids.len() >= 3 {
                    tree.split_leaf(ids[0], ids[1], SplitDirection::Vertical);
                    tree.split_leaf(ids[1], ids[2], SplitDirection::Horizontal);
                }
            }
            _ => {}
        }

        self.split_tree = tree;
        self.z_stack = ZStack::from_order_and_geometries(ids, BTreeMap::new());
        self.layout_mode = mode;
        self.zoomed_pane = None;
        let vp = ratatui::layout::Rect::new(0, 0, self.default_size.1, self.default_size.0);
        let _ = self.resize_all(vp);
        self.rebalance_scrollback(Instant::now());
        Ok(self.panes.len())
    }

    /// Dynamically switches to layout profile `name` (`:profile switch <name>`, `:layout switch <name>`).
    ///
    /// 1. If `layout_autosave` is enabled, autosaves the outgoing profile first.
    /// 2. Restores `name` from memory or disk if previously saved, or applies a built-in template
    ///    (`single`, `dual-v`, `dual-h`, `main-side`, `quad`, `cascade`), or snapshots the current
    ///    layout into the new profile name.
    pub fn switch_layout_profile(&mut self, name: &str) -> Result<usize> {
        let target = sanitize_profile_name(name);
        let outgoing = sanitize_profile_name(&self.config.active_layout_profile);
        if self.config.layout_autosave && outgoing != target {
            let snap = self.snapshot_session(&outgoing);
            self.layout_profiles_mem.insert(outgoing.clone(), snap);
            if self.should_persist_profiles_to_disk() {
                let _ = self.save_session(&outgoing);
            }
        }

        self.config.active_layout_profile = target.clone();

        if let Some(saved) = self.layout_profiles_mem.get(&target).cloned() {
            self.apply_saved_session_snapshot(&saved);
        } else if self.should_persist_profiles_to_disk()
            && self
                .resolve_session_dir()
                .map(|d| d.join(format!("{target}.toml")).is_file())
                .unwrap_or(false)
        {
            let _ = self.restore_session(&target)?;
            let snap = self.snapshot_session(&target);
            self.layout_profiles_mem.insert(target.clone(), snap);
        } else if Self::is_builtin_template_only(&target) {
            self.apply_builtin_layout_template(&target)?;
            if self.config.layout_autosave {
                let _ = self.save_layout_profile(Some(&target));
            }
        } else {
            let _ = self.save_layout_profile(Some(&target))?;
        }

        self.persist_layout_profile_meta();
        Ok(self.panes.len())
    }

    /// Cycles dynamically to the next (`forward = true`) or previous (`forward = false`) layout profile.
    pub fn cycle_layout_profile(&mut self, forward: bool) -> Result<(String, usize)> {
        let profiles = self.list_layout_profiles();
        if profiles.is_empty() {
            return Ok((self.config.active_layout_profile.clone(), self.panes.len()));
        }
        let cur = &self.config.active_layout_profile;
        let idx = profiles.iter().position(|p| p == cur).unwrap_or(0);
        let next_idx = if forward {
            (idx + 1) % profiles.len()
        } else if idx == 0 {
            profiles.len() - 1
        } else {
            idx - 1
        };
        let target = profiles[next_idx].clone();
        let count = self.switch_layout_profile(&target)?;
        Ok((target, count))
    }

    /// Restores the configured active default layout profile on startup (`default_layout_profile`).
    pub fn restore_default_layout_profile_on_startup(&mut self) -> Option<(String, usize)> {
        let _ = self.load_layout_profile_meta();
        let default_prof = sanitize_profile_name(&self.config.default_layout_profile);
        self.config.default_layout_profile = default_prof.clone();
        self.config.active_layout_profile = default_prof.clone();
        if (self.has_saved_layout_profile(&default_prof)
            || Self::is_builtin_template_only(&default_prof))
            && let Ok(count) = self.switch_layout_profile(&default_prof)
        {
            return Some((default_prof, count));
        }
        None
    }

    /// Executes a `:profile ...` or `:layout ...` profile subcommand and sets an informative status message.
    pub fn execute_layout_profile_command(&mut self, subcmd: &str) {
        let parts: Vec<&str> = subcmd.split_whitespace().collect();
        let head = parts.first().copied().unwrap_or("list");

        match head {
            "list" | "ls" | "profiles" | "status" => {
                let profiles = self.list_layout_profiles();
                let active = &self.config.active_layout_profile;
                let default = &self.config.default_layout_profile;
                let autosave = if self.config.layout_autosave {
                    "ON"
                } else {
                    "OFF"
                };
                let formatted: Vec<String> = profiles
                    .iter()
                    .map(|p| {
                        let mut tags = Vec::new();
                        if p == active {
                            tags.push("ACTIVE");
                        }
                        if p == default {
                            tags.push("DEFAULT");
                        }
                        if tags.is_empty() {
                            p.clone()
                        } else {
                            format!("{p}[{}]", tags.join(","))
                        }
                    })
                    .collect();
                let msg = format!(
                    "Layout Profiles (autosave={autosave}): {}",
                    formatted.join(" | ")
                );
                self.active_pane_mut().set_status_message(Some(msg));
            }
            "autosave" | "auto" => {
                let arg = match parts.get(1).copied() {
                    Some("on" | "true" | "1" | "enable" | "enabled") => Some(true),
                    Some("off" | "false" | "0" | "disable" | "disabled") => Some(false),
                    _ => None,
                };
                let state = self.set_layout_autosave(arg);
                let msg = format!(
                    "Layout profile autosave: {} (active='{}', default='{}')",
                    if state { "ON" } else { "OFF" },
                    self.config.active_layout_profile,
                    self.config.default_layout_profile
                );
                self.active_pane_mut().set_status_message(Some(msg));
            }
            "save" => {
                let name = parts.get(1).copied();
                let msg = match self.save_layout_profile(name) {
                    Ok(saved) => format!(
                        "Saved layout profile '{saved}' ({} pane(s), mode={:?})",
                        self.panes.len(),
                        self.layout_mode
                    ),
                    Err(e) => format!("Profile save error: {e}"),
                };
                self.active_pane_mut().set_status_message(Some(msg));
            }
            "default" | "set-default" => {
                let name = parts.get(1).copied();
                let msg = match self.set_default_layout_profile(name) {
                    Ok(def) => {
                        let _ = self.switch_layout_profile(&def);
                        format!(
                            "Default layout profile set to '{def}' (active='{}', autosave={})",
                            self.config.active_layout_profile,
                            if self.config.layout_autosave { "ON" } else { "OFF" }
                        )
                    }
                    Err(e) => format!("Set default profile error: {e}"),
                };
                self.active_pane_mut().set_status_message(Some(msg));
            }
            "switch" | "load" | "use" | "select" | "apply" => {
                let Some(&name) = parts.get(1) else {
                    self.active_pane_mut().set_status_message(Some(
                        "Usage: :profile switch <name> (e.g. default, single, dual-v, dual-h, main-side, quad, cascade)"
                            .to_string(),
                    ));
                    return;
                };
                let msg = match self.switch_layout_profile(name) {
                    Ok(count) => format!(
                        "Switched to layout profile '{}' ({count} pane(s), mode={:?})",
                        self.config.active_layout_profile, self.layout_mode
                    ),
                    Err(e) => format!("Profile switch error: {e}"),
                };
                self.active_pane_mut().set_status_message(Some(msg));
            }
            "next" => {
                let msg = match self.cycle_layout_profile(true) {
                    Ok((prof, count)) => format!(
                        "Switched to next layout profile '{prof}' ({count} pane(s), mode={:?})",
                        self.layout_mode
                    ),
                    Err(e) => format!("Profile cycle error: {e}"),
                };
                self.active_pane_mut().set_status_message(Some(msg));
            }
            "prev" | "previous" => {
                let msg = match self.cycle_layout_profile(false) {
                    Ok((prof, count)) => format!(
                        "Switched to previous layout profile '{prof}' ({count} pane(s), mode={:?})",
                        self.layout_mode
                    ),
                    Err(e) => format!("Profile cycle error: {e}"),
                };
                self.active_pane_mut().set_status_message(Some(msg));
            }
            "delete" | "rm" | "remove" => {
                let Some(&name) = parts.get(1) else {
                    self.active_pane_mut().set_status_message(Some(
                        "Usage: :profile delete <name>".to_string(),
                    ));
                    return;
                };
                let msg = match self.delete_layout_profile(name) {
                    Ok(del) => format!(
                        "Deleted layout profile '{del}' (active='{}', default='{}')",
                        self.config.active_layout_profile, self.config.default_layout_profile
                    ),
                    Err(e) => e,
                };
                self.active_pane_mut().set_status_message(Some(msg));
            }
            other => {
                let msg = match self.switch_layout_profile(other) {
                    Ok(count) => format!(
                        "Switched to layout profile '{}' ({count} pane(s), mode={:?})",
                        self.config.active_layout_profile, self.layout_mode
                    ),
                    Err(e) => format!("Profile switch error: {e}"),
                };
                self.active_pane_mut().set_status_message(Some(msg));
            }
        }
    }
}
