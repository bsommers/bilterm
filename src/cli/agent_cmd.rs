//! `bilterm agent ...` parsing (`HERDR-01`).

use super::{CliEnv, Parsed, PollSpec, RpcCall, base_params, str_val, usage};
use crate::api::JsonValue;

/// Default and maximum client-side wait.
const DEFAULT_WAIT_MS: u64 = 60_000;
const MAX_WAIT_MS: u64 = 3_600_000;
const POLL_INTERVAL_MS: u64 = 250;

fn call(method: &str, params: JsonValue) -> RpcCall {
    RpcCall { method: method.to_string(), params, poll: None }
}

pub(super) fn parse_agent(p: &Parsed, env: &CliEnv) -> Result<RpcCall, String> {
    let (sub, rest) = p.pos.split_first().ok_or_else(usage)?;
    let selector = || {
        rest.first()
            .map(|s| ("selector", str_val(s)))
            .ok_or_else(|| format!("agent {sub} needs a SELECTOR"))
    };
    match sub.as_str() {
        "list" | "ls" => Ok(call("agents.list", base_params(env, vec![]))),
        "prompt" => {
            if rest.len() < 2 {
                return Err("agent prompt needs SELECTOR and TEXT".to_string());
            }
            let mut e = vec![selector()?, ("text", str_val(&rest[1..].join(" ")))];
            if p.has("allow-echo-lock") {
                e.push(("allow_echo_lock", JsonValue::Bool(true)));
            }
            Ok(call("agent.prompt", base_params(env, e)))
        }
        "wait" => {
            let until = p.flag("until").unwrap_or("idle");
            let timeout_ms = match p.flag("timeout-ms") {
                Some(v) => v.parse::<u64>().map_err(|_| format!("invalid --timeout-ms '{v}'"))?.min(MAX_WAIT_MS),
                None => DEFAULT_WAIT_MS,
            };
            let mut c = call("agent.wait", base_params(env, vec![selector()?, ("until", str_val(until))]));
            c.poll = Some(PollSpec { timeout_ms, interval_ms: POLL_INTERVAL_MS });
            Ok(c)
        }
        "approve" => {
            let mut e = vec![selector()?];
            if let Some(a) = p.flag("answer") {
                e.push(("answer", str_val(a)));
            }
            if p.has("force") {
                e.push(("force", JsonValue::Bool(true)));
            }
            Ok(call("agent.approve", base_params(env, e)))
        }
        "interrupt" => Ok(call(
            "pane.send",
            base_params(env, vec![selector()?, ("text", str_val("\u{3}"))]),
        )),
        "last-turn" | "last_turn" => {
            let mut e = vec![selector()?];
            if let Some(m) = p.flag("max-bytes") {
                let n = m.parse::<i64>().map_err(|_| format!("invalid --max-bytes '{m}'"))?;
                e.push(("max_bytes", JsonValue::Number(n)));
            }
            Ok(call("agent.last_turn", base_params(env, e)))
        }
        other => Err(format!("unknown agent subcommand '{other}'\n{}", usage())),
    }
}
