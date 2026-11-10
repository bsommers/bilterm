//! `bilterm coords resolve|format` parsing (`UNI-R01`, Phase 71).
//!
//! Lowers to the same `coords.resolve` / `coords.format` JSON-RPC methods that MCP exposes.

use super::{CliEnv, Parsed, RpcCall, base_params, str_val, usage};
use crate::api::JsonValue;
use crate::wm::coordinates::{CoordDisplayStyle, TargetCoord};

fn call(method: &str, params: JsonValue) -> RpcCall {
    RpcCall { method: method.to_string(), params, poll: None }
}

pub(super) fn parse_coords(p: &Parsed, env: &CliEnv) -> Result<RpcCall, String> {
    let (sub, rest) = p.pos.split_first().ok_or_else(usage)?;
    match sub.as_str() {
        "resolve" => {
            let coord = rest.first().ok_or("coords resolve needs a COORD (e.g. d0:w1:main:y2)")?;
            TargetCoord::parse(coord).map_err(|e| format!("invalid coord '{coord}': {e}"))?;
            Ok(call("coords.resolve", base_params(env, vec![("coord", str_val(coord))])))
        }
        "format" => {
            let mut e = Vec::new();
            if let Some(n) = p.flag("pane") {
                let id: i64 = n.parse().map_err(|_| format!("--pane needs a numeric pane id, got '{n}'"))?;
                e.push(("pane_id", JsonValue::Number(id)));
            }
            if let Some(s) = p.flag("style") {
                if CoordDisplayStyle::from_name(s).is_none() {
                    return Err(format!("unknown style '{s}' (accepted: verbose, compact, daemon, off)"));
                }
                e.push(("style", str_val(s)));
            }
            Ok(call("coords.format", base_params(env, e)))
        }
        other => Err(format!("unknown coords subcommand '{other}'\n{}", usage())),
    }
}
