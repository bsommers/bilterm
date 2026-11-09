//! Pane discovery for idle remote sessions: provisional `%0` mapping and `list-panes` reconcile.

use super::{TmuxControlBridge, link};
use crate::pane::PaneId;
use crate::wm::WindowManager;

/// Parses `@<win> %<pane> <name>` lines; malformed lines are skipped, count is capped.
fn parse_list_panes(lines: &[String]) -> Vec<(usize, usize, String)> {
    lines
        .iter()
        .take(link::MAPPED_PANES_CAP.saturating_mul(2))
        .filter_map(|l| {
            let mut it = l.splitn(3, ' ');
            let win = it.next()?.strip_prefix('@')?.parse().ok()?;
            let pane = it.next()?.strip_prefix('%')?.parse().ok()?;
            Some((win, pane, it.next().unwrap_or("").to_string()))
        })
        .collect()
}

impl TmuxControlBridge {
    /// Binds the pending primary pane to its previously known tmux id (or `%0`).
    pub(crate) fn map_provisional_primary(&mut self, wm: &mut WindowManager) -> Option<usize> {
        let primary = self.primary?;
        let tmux = self
            .stale
            .iter()
            .find_map(|(&t, &p)| (p == primary).then_some(t))
            .unwrap_or(0);
        let id = self.ensure_tmux_pane(wm, tmux)?;
        self.stale.retain(|_, p| *p != id);
        self.provisional = Some(id);
        (id == primary).then_some(tmux)
    }

    /// A real pane id was discovered while the primary only holds a guessed one: the primary
    /// gives up the guess so the discovered pane adopts it (no duplicate pane is created).
    pub(crate) fn rebind_provisional(&mut self, discovered: usize) {
        let Some(p) = self.provisional else { return };
        if self.tmux_to_bilterm.get(&discovered) == Some(&p) {
            return;
        }
        if let Some(&t) = self.bilterm_to_tmux.get(&p) {
            self.tmux_to_bilterm.remove(&t);
            self.bilterm_to_tmux.remove(&p);
            self.window_of_pane.remove(&t);
            self.primary = Some(p);
        }
        self.provisional = None;
    }

    /// Reconciles the bridge with a `list-panes` reply: fixes a wrong provisional primary id,
    /// records windows, and maps every listed pane (extra ones become pty-less remote panes).
    pub(crate) fn apply_list_panes(&mut self, wm: &mut WindowManager, lines: &[String]) {
        let listed = parse_list_panes(lines);
        if listed.is_empty() {
            return;
        }
        // Provisional primary mapping that the server does not have: rebind the same pane.
        let provisional: Vec<(usize, PaneId)> = self
            .tmux_to_bilterm
            .iter()
            .filter(|(t, p)| !listed.iter().any(|(_, lp, _)| lp == *t) && self.primary_hint(**p))
            .map(|(&t, &p)| (t, p))
            .collect();
        for (t, p) in provisional {
            self.tmux_to_bilterm.remove(&t);
            self.bilterm_to_tmux.remove(&p);
            self.window_of_pane.remove(&t);
            self.primary = Some(p);
            self.provisional = None;
        }
        for (win, pane, name) in listed {
            if self.ensure_tmux_pane(wm, pane).is_some() {
                self.window_of_pane.insert(pane, win);
                if !name.is_empty()
                    && !self.window_names.contains_key(&win)
                    && self.window_names.len() < 1024
                {
                    self.window_names.insert(win, name.chars().take(256).collect());
                }
            }
        }
        self.provisional = None;
    }

    /// Only the link primary (marked via `provisional_primary`) may be rebound by a reply.
    fn primary_hint(&self, pane: PaneId) -> bool {
        self.provisional == Some(pane)
    }
}
