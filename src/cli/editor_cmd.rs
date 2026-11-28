//! `bilterm editor <verb>` parsing (`UNI-R01`, `EDIT-01`): lowers to the `editor.*` JSON-RPC
//! methods, which build the same `Action::Editor` as `:edit` / `:scratch` / `:editor ...`.
//! Talks to a running bilterm (daemon or `BILTERM_SOCKET`); file verbs need `[api] allow_config_write`.

use super::{CliEnv, Parsed, RpcCall, base_params, str_val, usage};
use crate::api::JsonValue;

pub(super) fn parse_editor(p: &Parsed, env: &CliEnv) -> Result<RpcCall, String> {
    let (verb, rest) = p.pos.split_first().ok_or_else(usage)?;
    let joined = rest.join(" ");
    let force = ("force", JsonValue::Bool(p.has("force")));
    let need = |what: &str| if joined.is_empty() { Err(format!("editor {verb} needs {what}")) } else { Ok(str_val(&joined)) };
    let (method, params): (&str, Vec<(&str, JsonValue)>) = match verb.as_str() {
        "open" | "edit" => ("editor.open", if joined.is_empty() { vec![] } else { vec![("path", str_val(&joined))] }),
        "scratch" => ("editor.scratch", p.flag("name").map(|n| ("name", str_val(n))).or_else(|| rest.first().map(|n| ("name", str_val(n)))).into_iter().collect()),
        "artifact" => ("editor.artifact", p.flag("name").map(|n| ("name", str_val(n))).or_else(|| rest.first().map(|n| ("name", str_val(n)))).into_iter().collect()),
        "board" => ("editor.board", p.flag("name").map(|n| ("name", str_val(n))).or_else(|| rest.first().map(|n| ("name", str_val(n)))).into_iter().collect()),
        "save" => ("editor.save", vec![force]),
        "saveas" | "save-as" => ("editor.save_as", vec![("path", need("a PATH")?), force]),
        "wrap" => ("editor.wrap", match rest.first().map(String::as_str) {
            None => vec![],
            Some("on") => vec![("enabled", JsonValue::Bool(true))],
            Some("off") => vec![("enabled", JsonValue::Bool(false))],
            Some(o) => return Err(format!("editor wrap takes on|off, got '{o}'")),
        }),
        "render" | "view" => ("editor.render", match rest.first().map(String::as_str) {
            None => vec![],
            Some("on" | "rendered") => vec![("enabled", JsonValue::Bool(true))],
            Some("off" | "edit") => vec![("enabled", JsonValue::Bool(false))],
            Some(o) => return Err(format!("editor render takes on|off, got '{o}'")),
        }),
        "check" | "toggle-check" => ("editor.check", vec![]),
        "broadcast" => ("editor.broadcast", if joined.is_empty() { vec![] } else { vec![("note", str_val(&joined))] }),
        "preview" => ("editor.preview", vec![]),
        "find" | "findprev" => ("editor.find", vec![("query", need("TEXT")?), ("backward", JsonValue::Bool(verb == "findprev"))]),
        "undo" => ("editor.undo", vec![]),
        "redo" => ("editor.redo", vec![]),
        "goto" => {
            let n: i64 = joined.parse().map_err(|_| "editor goto needs a line number".to_string())?;
            ("editor.goto", vec![("line", JsonValue::Number(n))])
        }
        "revert" => ("editor.revert", vec![]),
        "insert" => ("editor.insert", vec![("text", need("TEXT")?)]),
        "text" => ("editor.text", vec![]),
        "status" => ("editor.status", vec![]),
        "close" => ("editor.close", vec![force]),
        "menu" => match rest.first().map(String::as_str) {
            None | Some("open") => ("editor.menu", rest.get(1).map(|m| vec![("menu", str_val(m))]).unwrap_or_default()),
            Some("close") => ("editor.menu", vec![("op", str_val("close"))]),
            Some("do") => ("editor.menu", vec![("op", str_val("do")), ("item", rest.get(1).map(|i| str_val(i)).ok_or("editor menu do needs an ITEM")?)]),
            Some(o) => return Err(format!("editor menu takes open [MENU] | close | do ITEM, got '{o}'")),
        },
        other => return Err(format!("unknown editor subcommand '{other}'\n{}", usage())),
    };
    Ok(RpcCall { method: method.to_string(), params: base_params(env, params), poll: None })
}
