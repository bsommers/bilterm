//! `:agent guide [PANE|all|--print]`: hand the bilterm control guide to agent panes (`AGUIDE-01`).
//!
//! The guide (`crate::agent_guide::GUIDE`) is several KiB, so it is not typed into a pane. It is
//! written `0600` to the private runtime dir (the same idea as `:agent send --file`) and one line
//! pointing at that file is delivered:
//! - a raw agent TUI gets it through [`crate::pane::CompositePane::inject_prompt`], the path
//!   `agent.prompt` and `:swarm` use (bracketed paste when the TUI enabled it, then Enter);
//! - a shell pane gets a `#` comment line (like the `:agent send --nudge` notice), so nothing runs.
//!
//! Read-only panes and `!ECHO LOCK` password prompts are refused; a busy shell pane is refused too.
//! ECHO being off is only treated as a password prompt when the program has not declared itself a
//! TUI (bracketed paste, kitty keyboard, alternate screen): every raw agent TUI turns ECHO off.

use std::time::Instant;

use crate::api::JsonValue;
use crate::pane::{PaneId, PromptInjectionKind};
use crate::wm::WindowManager;
use crate::wm::traffic::{TrafficEndpoint, TrafficKind};

/// Which panes `:agent guide` targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuideTarget {
    /// The focused pane.
    Focused,
    /// One pane by id.
    Pane(PaneId),
    /// Every detected agent pane.
    All,
}

/// Canonical operations of `:agent guide` (`ARCH-R01`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentGuideOp {
    /// Deliver the pointer line to the target pane(s).
    Inject(GuideTarget),
    /// Write the guide file and report its path without touching any pane.
    Print,
}

impl AgentGuideOp {
    /// Canonical colon form.
    pub fn colon_form(&self) -> String {
        match self {
            Self::Inject(GuideTarget::Focused) => ":agent guide".to_string(),
            Self::Inject(GuideTarget::Pane(id)) => format!(":agent guide {id}"),
            Self::Inject(GuideTarget::All) => ":agent guide all".to_string(),
            Self::Print => ":agent guide --print".to_string(),
        }
    }
}

fn s(v: &str) -> JsonValue {
    JsonValue::String(v.to_string())
}

impl WindowManager {
    /// Delivers the pointer line to one pane. `Ok(mode)` names the delivery path.
    fn agent_guide_inject_pane(&mut self, id: PaneId, line: &str) -> Result<&'static str, String> {
        let pane = self.panes.get_mut(&id).ok_or_else(|| format!("pane #{id} not found"))?;
        if pane.is_read_only() {
            return Err(format!("pane #{id} is locked read-only (PROT)"));
        }
        // A raw agent TUI runs with ECHO off, which the termios signal alone reads as a password
        // prompt. A program that declared itself a TUI (bracketed paste, kitty keyboard or the
        // alternate screen) is not one; a password line on the canvas always is.
        let declared_tui = pane.vt().is_bracketed_paste() || pane.vt().kitty_keyboard_flags() > 0 || pane.vt().is_alternate_screen();
        if pane.is_active_canvas_password_prompt() || (pane.is_password_prompt() && !declared_tui) {
            return Err(format!("pane #{id} is in !ECHO LOCK password prompt; guide not injected"));
        }
        let kind = pane.prompt_injection_kind();
        let mode = if kind == PromptInjectionKind::ShellPrompt {
            if pane.resolve_running_command().is_some() {
                return Err(format!("pane #{id} is busy running a command; guide not injected"));
            }
            pane.write_to_pty(format!("# [bilterm] {line}\n").as_bytes()).map_err(|e| e.to_string())?;
            "shell_comment"
        } else {
            pane.inject_prompt(line).map_err(|e| e.to_string())?.as_str()
        };
        self.record_traffic_with(TrafficEndpoint::Api, TrafficEndpoint::Pane(id), TrafficKind::Message, line, false, Instant::now());
        if kind != PromptInjectionKind::ShellPrompt {
            self.agents_hud_note_prompt(id, line);
        }
        Ok(mode)
    }

    /// Applies an [`AgentGuideOp`]; the structured result is left in the command data.
    pub fn agent_guide_apply(&mut self, op: &AgentGuideOp) -> Result<String, String> {
        let path = crate::agent_guide::write_guide_file()?;
        let path_str = path.to_string_lossy().into_owned();
        let bytes = JsonValue::Number(crate::agent_guide::GUIDE.len() as i64);
        let target = match op {
            AgentGuideOp::Print => {
                let data = JsonValue::obj([("status", s("written")), ("path", s(&path_str)), ("bytes", bytes), ("text", s(crate::agent_guide::GUIDE))]);
                self.set_command_data(data.to_json_string());
                return Ok(format!("Agent guide written to {path_str} (also: bilterm agents-md)"));
            }
            AgentGuideOp::Inject(t) => *t,
        };
        let ids: Vec<PaneId> = match target {
            GuideTarget::Focused => vec![self.active_pane_id()],
            GuideTarget::Pane(id) => vec![id],
            GuideTarget::All => {
                self.update_agent_states(Instant::now());
                self.agent_statuses().into_iter().filter(|st| st.profile.is_some() || st.reported).map(|st| st.pane_id).collect()
            }
        };
        if ids.is_empty() {
            return Err("no agent panes detected (name a pane: :agent guide <pane-id>)".to_string());
        }
        let line = crate::agent_guide::pointer_line(&path);
        let mut injected = Vec::new();
        let mut refused = Vec::new();
        for id in ids {
            match self.agent_guide_inject_pane(id, &line) {
                Ok(mode) => injected.push(JsonValue::obj([("pane_id", JsonValue::Number(id as i64)), ("mode", s(mode))])),
                Err(e) if target == GuideTarget::All => {
                    refused.push(JsonValue::obj([("pane_id", JsonValue::Number(id as i64)), ("reason", s(&e))]));
                }
                Err(e) => return Err(e),
            }
        }
        let (n, r) = (injected.len(), refused.len());
        let data = JsonValue::obj([
            ("status", s(if n > 0 { "injected" } else { "refused" })),
            ("path", s(&path_str)),
            ("bytes", bytes),
            ("injected", JsonValue::Array(injected)),
            ("refused", JsonValue::Array(refused)),
        ]);
        self.set_command_data(data.to_json_string());
        if n == 0 {
            return Err(format!("agent guide: all {r} agent pane(s) refused (locked, password prompt or busy)"));
        }
        Ok(match r {
            0 => format!("Agent guide sent to {n} pane(s): {path_str}"),
            _ => format!("Agent guide sent to {n} pane(s), {r} refused: {path_str}"),
        })
    }
}
