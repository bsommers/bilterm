//! First-class `bilterm agent|pane|workspace` subcommands (`HERDR-01`).
//!
//! Parsing is pure: [`parse`] turns argv into one [`RpcCall`] (method + JSON params) so every
//! subcommand lowers to the same JSON-RPC methods that MCP and `bilterm ctl` use, and so the
//! whole surface is unit-testable without a socket. `BILTERM_PANE_ID` becomes `from_pane`
//! (caller attribution and self-exclusion for `@semantic` selectors); `BILTERM_SOCKET` picks the
//! endpoint ([`resolve_endpoint`]).

pub mod agent_cmd;
mod coords_cmd;

use std::collections::BTreeMap;
use std::path::Path;

use crate::api::JsonValue;

/// Subcommand words owned by this module.
pub const CLI_COMMANDS: &[&str] = &["agent", "pane", "workspace", "coords"];

/// True when `first` is one of [`CLI_COMMANDS`].
pub fn is_cli_command(first: Option<&str>) -> bool {
    first.is_some_and(|f| CLI_COMMANDS.contains(&f))
}

/// Environment the CLI inherits from the pane it runs in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliEnv {
    pub pane_id: Option<usize>,
    pub socket: Option<String>,
}

impl CliEnv {
    pub fn from_process() -> Self {
        Self {
            pane_id: std::env::var("BILTERM_PANE_ID").ok().and_then(|v| v.trim().parse().ok()),
            socket: std::env::var("BILTERM_SOCKET").ok().filter(|v| !v.trim().is_empty()),
        }
    }
}

/// Client-side polling of a `pending` result (used by `agent wait`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PollSpec {
    pub timeout_ms: u64,
    pub interval_ms: u64,
}

/// One JSON-RPC call a subcommand lowers to.
#[derive(Clone, Debug, PartialEq)]
pub struct RpcCall {
    pub method: String,
    pub params: JsonValue,
    pub poll: Option<PollSpec>,
}

/// Positionals plus `--flag [value]` options.
pub(crate) struct Parsed {
    pub pos: Vec<String>,
    pub flags: BTreeMap<String, Option<String>>,
}

const VALUE_FLAGS: &[&str] = &["timeout-ms", "until", "answer", "direction", "max-bytes", "name", "pane", "style"];
const BOOL_FLAGS: &[&str] = &["force", "enter", "allow-echo-lock", "blank"];

pub(crate) fn split_args(args: &[String]) -> Result<Parsed, String> {
    let mut pos = Vec::new();
    let mut flags = BTreeMap::new();
    let mut it = args.iter().peekable();
    let mut literal = false;
    while let Some(a) = it.next() {
        if literal || !a.starts_with("--") || a == "--" {
            if a == "--" && !literal {
                literal = true;
                continue;
            }
            pos.push(a.clone());
            continue;
        }
        let key = a.trim_start_matches("--");
        if let Some((k, v)) = key.split_once('=') {
            flags.insert(k.to_string(), Some(v.to_string()));
        } else if VALUE_FLAGS.contains(&key) {
            let v = it.next().ok_or_else(|| format!("--{key} needs a value"))?;
            flags.insert(key.to_string(), Some(v.clone()));
        } else if BOOL_FLAGS.contains(&key) {
            flags.insert(key.to_string(), None);
        } else {
            return Err(format!("unknown option --{key}"));
        }
    }
    Ok(Parsed { pos, flags })
}

impl Parsed {
    pub fn flag(&self, k: &str) -> Option<&str> {
        self.flags.get(k).and_then(|v| v.as_deref())
    }
    pub fn has(&self, k: &str) -> bool {
        self.flags.contains_key(k)
    }
}

pub(crate) fn str_val(v: &str) -> JsonValue {
    JsonValue::String(v.to_string())
}

/// Builds params with `from_pane` attached when the CLI runs inside a pane.
pub(crate) fn base_params(env: &CliEnv, entries: Vec<(&str, JsonValue)>) -> JsonValue {
    let mut v: Vec<(String, JsonValue)> = entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    if let Some(p) = env.pane_id {
        v.push(("from_pane".to_string(), JsonValue::Number(p as i64)));
    }
    JsonValue::obj(v)
}

fn call(method: &str, params: JsonValue) -> RpcCall {
    RpcCall { method: method.to_string(), params, poll: None }
}

/// Top-level usage text.
pub fn usage() -> String {
    "\
Usage:
  bilterm agent list
  bilterm agent prompt <SELECTOR> <TEXT...>
  bilterm agent wait <SELECTOR> [--until idle|working|blocked|done|errored|attention] [--timeout-ms N]
  bilterm agent approve <SELECTOR> [--answer yes|y|n|no|esc] [--force]
  bilterm agent interrupt <SELECTOR>
  bilterm agent last-turn <SELECTOR> [--max-bytes N]
  bilterm pane list
  bilterm pane read [SELECTOR]
  bilterm pane split [h|v] [SELECTOR]
  bilterm pane focus <SELECTOR>
  bilterm pane zoom [SELECTOR]
  bilterm pane send <SELECTOR> <TEXT...> [--enter]
  bilterm workspace list
  bilterm workspace select <NAME|wN>
  bilterm workspace new <NAME>
  bilterm workspace rename <NAME>
  bilterm coords resolve <COORD>   (e.g. d0:w1:main:y2)
  bilterm coords format [--pane N] [--style verbose|compact|daemon|off]

SELECTOR: w<N>:p<M> | p<M> | <id> | @claude | @agy | @idle | @working | @blocked | @done | @errored | @focused
Environment: BILTERM_SOCKET (endpoint), BILTERM_PANE_ID (caller pane, excluded from @selectors)"
        .to_string()
}

/// Parses `args` (starting at the subcommand word) into one RPC call. `Err` carries a message
/// for the user (usage or the specific mistake).
pub fn parse(args: &[String], env: &CliEnv) -> Result<RpcCall, String> {
    let (cmd, rest) = args.split_first().ok_or_else(usage)?;
    let p = split_args(rest)?;
    match cmd.as_str() {
        "agent" => agent_cmd::parse_agent(&p, env),
        "pane" => parse_pane(&p, env),
        "workspace" => parse_workspace(&p),
        "coords" => coords_cmd::parse_coords(&p, env),
        other => Err(format!("unknown command '{other}'\n{}", usage())),
    }
}

fn parse_pane(p: &Parsed, env: &CliEnv) -> Result<RpcCall, String> {
    let (sub, rest) = p.pos.split_first().ok_or_else(usage)?;
    let sel = |i: usize| rest.get(i).map(|s| ("selector", str_val(s)));
    match sub.as_str() {
        "list" | "ls" => Ok(call("pane.list", base_params(env, vec![]))),
        "read" => Ok(call("pane.read", base_params(env, sel(0).into_iter().collect()))),
        "split" => {
            let (dir, selector) = match rest.first().map(String::as_str) {
                Some(d @ ("h" | "v" | "horizontal" | "vertical")) => (d.to_string(), rest.get(1)),
                _ => (p.flag("direction").unwrap_or("h").to_string(), rest.first()),
            };
            let mut e = vec![("direction", str_val(&dir))];
            if let Some(s) = selector {
                e.push(("selector", str_val(s)));
            }
            Ok(call("pane.split", base_params(env, e)))
        }
        "focus" => Ok(call("pane.focus", base_params(env, vec![sel(0).ok_or("pane focus needs a SELECTOR")?]))),
        "zoom" => Ok(call("pane.zoom", base_params(env, sel(0).into_iter().collect()))),
        "send" => {
            let selector = rest.first().ok_or("pane send needs SELECTOR and TEXT")?;
            if rest.len() < 2 {
                return Err("pane send needs SELECTOR and TEXT".to_string());
            }
            let mut e = vec![("selector", str_val(selector)), ("text", str_val(&rest[1..].join(" ")))];
            if p.has("enter") {
                e.push(("enter", JsonValue::Bool(true)));
            }
            if p.has("allow-echo-lock") {
                e.push(("allow_echo_lock", JsonValue::Bool(true)));
            }
            Ok(call("pane.send", base_params(env, e)))
        }
        other => Err(format!("unknown pane subcommand '{other}'\n{}", usage())),
    }
}

fn parse_workspace(p: &Parsed) -> Result<RpcCall, String> {
    let (sub, rest) = p.pos.split_first().ok_or_else(usage)?;
    let name = || {
        rest.first()
            .map(|n| JsonValue::obj([("name", str_val(n))]))
            .ok_or_else(|| format!("workspace {sub} needs a NAME"))
    };
    match sub.as_str() {
        "list" | "ls" => Ok(call("workspace.list", JsonValue::obj([] as [(&str, JsonValue); 0]))),
        "select" => Ok(call("workspace.select", name()?)),
        "new" => Ok(call("workspace.new", name()?)),
        "rename" => Ok(call("workspace.rename", name()?)),
        other => Err(format!("unknown workspace subcommand '{other}'\n{}", usage())),
    }
}

/// Picks the endpoint (`unix:PATH`): `BILTERM_SOCKET`, else the registry's newest live instance /
/// `bilterm.sock` link under `runtime`, else `None` (caller falls back to its default).
pub fn resolve_endpoint(env: &CliEnv, runtime: Option<&Path>) -> Option<String> {
    if let Some(s) = env.socket.as_deref() {
        return Some(if s.contains(':') && !s.starts_with('/') { s.to_string() } else { format!("unix:{s}") });
    }
    let rt = runtime?;
    crate::api::instance_registry::resolve_client_socket(rt, None).map(|p| format!("unix:{}", p.display()))
}

/// Sends `call` and returns the final response line. Polls while `poll` is set and the result
/// status is `pending`, up to the deadline (then returns the last response with status `timeout`).
pub async fn run_call(
    call: &RpcCall,
    endpoint: &str,
    token: Option<&str>,
) -> crate::error::Result<String> {
    use crate::api::send_rpc_request_with_token;
    let started = std::time::Instant::now();
    loop {
        let resp = send_rpc_request_with_token(endpoint, &call.method, call.params.clone(), token).await?;
        let Some(poll) = call.poll else { return Ok(resp) };
        let pending = JsonValue::parse(&resp)
            .and_then(|v| v.get("result").and_then(|r| r.get("status")).and_then(|s| s.as_str().map(str::to_string)))
            .is_some_and(|s| s == "pending");
        if !pending {
            return Ok(resp);
        }
        if started.elapsed().as_millis() as u64 >= poll.timeout_ms {
            return Ok(resp.replacen("\"pending\"", "\"timeout\"", 1));
        }
        tokio::time::sleep(std::time::Duration::from_millis(poll.interval_ms)).await;
    }
}
