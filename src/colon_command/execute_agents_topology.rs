//! Executors for `:agents fade` and `:agents topology` (communication topology record).
//!
//! The topology holds labels, counts and times only (never previews or content), so reporting it
//! over RPC / MCP is read-only and safe. `export` writes a file, so it is refused for API-origin
//! callers; local callers write `<private runtime dir>/agent-topology/<name>.json` through
//! [`crate::secure_fs`] (`0700` directory, atomic `0600` file) after validating `<name>`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::api::JsonValue;
use crate::config::agents::EDGE_FADE_RANGE;
use crate::wm::WindowManager;
use crate::wm::agents_cmd::TopologyOp;
use crate::wm::agents_topology::{PairRecord, Topology};

/// Longest accepted export name.
const EXPORT_NAME_CAP: usize = 64;
/// Directory (inside the private runtime directory) that holds exports.
const EXPORT_DIR: &str = "agent-topology";

fn status(wm: &mut WindowManager, msg: String) {
    wm.active_pane_mut().set_status_message(Some(msg));
}

/// `:agents fade [secs]`.
pub(super) fn fade(wm: &mut WindowManager, secs: Option<u32>) {
    match secs {
        Some(n) if EDGE_FADE_RANGE.contains(&n) => {
            wm.config_mut().agents_window.edge_fade_secs = n;
            status(wm, format!("Agent graph edges fade over {n}s (agents_window.edge_fade_secs)"));
        }
        Some(_) => status(wm, "Usage: :agents fade [1..3600]".to_string()),
        None => {
            let n = wm.config().agents_window.edge_fade_secs;
            wm.set_command_data(JsonValue::obj([("edge_fade_secs", JsonValue::Number(i64::from(n)))]).to_json_string());
            status(wm, format!("Agent graph edges fade over {n}s (agents_window.edge_fade_secs)"));
        }
    }
}

/// `:agents topology [export <name>]`.
pub(super) fn topology(wm: &mut WindowManager, op: TopologyOp) {
    let now = Instant::now();
    let fade = wm.config().agents_window.edge_fade();
    let data = topology_json(wm.traffic().topology(), wm.traffic().latest_seq(), now, fade);
    match op {
        TopologyOp::Show => {
            let msg = summary(wm.traffic().topology());
            wm.set_command_data(data.to_json_string());
            status(wm, msg);
        }
        TopologyOp::Export(name) => {
            if wm.is_api_origin() {
                return status(wm, "Topology export writes a file and is not available over the API".to_string());
            }
            let msg = match export(&name, &data) {
                Ok(path) => format!("Topology exported to {}", path.display()),
                Err(e) => format!("Topology export failed: {e}"),
            };
            status(wm, msg);
        }
    }
}

fn summary(t: &Topology) -> String {
    let events: u64 = t.iter().map(PairRecord::count).sum();
    match busiest(t) {
        Some(top) => {
            let (a, b) = top.labels();
            format!("Topology: {} pairs, {events} events (busiest {a} - {b} x{})", t.len(), top.count())
        }
        None => "Topology: no communication recorded yet".to_string(),
    }
}

/// Pair with the most events (ties: oldest first, deterministic).
fn busiest(t: &Topology) -> Option<&PairRecord> {
    t.iter().max_by(|x, y| x.count().cmp(&y.count()).then(y.first_seq().cmp(&x.first_seq())))
}

fn ms_since(now: Instant, then: Instant) -> i64 {
    i64::try_from(now.saturating_duration_since(then).as_millis()).unwrap_or(i64::MAX)
}

fn num(n: u64) -> JsonValue {
    JsonValue::Number(i64::try_from(n).unwrap_or(i64::MAX))
}

/// The topology as JSON: pairs by descending count (ties by first event), times as "ms ago".
pub(crate) fn topology_json(t: &Topology, latest_seq: u64, now: Instant, fade: Duration) -> JsonValue {
    let mut pairs: Vec<&PairRecord> = t.iter().collect();
    pairs.sort_by(|x, y| y.count().cmp(&x.count()).then(x.first_seq().cmp(&y.first_seq())));
    let rows: Vec<JsonValue> = pairs
        .into_iter()
        .map(|p| {
            let (a, b) = p.labels();
            let (ka, kb) = p.keys();
            let heat = p.heat(now, fade);
            JsonValue::obj([
                ("a", JsonValue::String(a)),
                ("b", JsonValue::String(b)),
                ("a_key", JsonValue::String(ka.as_str().to_string())),
                ("b_key", JsonValue::String(kb.as_str().to_string())),
                ("count", num(p.count())),
                ("first_seq", num(p.first_seq())),
                ("last_seq", num(p.last_seq())),
                ("first_ago_ms", JsonValue::Number(ms_since(now, p.first_at()))),
                ("last_ago_ms", JsonValue::Number(ms_since(now, p.last_at()))),
                ("heat_permille", JsonValue::Number((heat * 1000.0).round() as i64)),
            ])
        })
        .collect();
    JsonValue::obj([
        ("version", JsonValue::Number(1)),
        ("pair_count", num(t.len() as u64)),
        ("dropped_pairs", num(t.dropped())),
        ("latest_seq", num(latest_seq)),
        ("fade_secs", num(fade.as_secs())),
        ("pairs", JsonValue::Array(rows)),
    ])
}

/// Accepts plain names only: a single safe path component of ASCII letters, digits, `-`, `_`, `.`
/// (not starting with `.`), at most [`EXPORT_NAME_CAP`] characters.
fn validate_export_name(name: &str) -> Result<(), String> {
    crate::wm::journal::validate_safe_name(name).map_err(|_| format!("invalid name {name:?}"))?;
    let ok = !name.is_empty()
        && name.len() <= EXPORT_NAME_CAP
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok {
        Ok(())
    } else {
        Err(format!("invalid name {name:?} (letters, digits, - _ . ; at most {EXPORT_NAME_CAP})"))
    }
}

fn export(name: &str, data: &JsonValue) -> Result<PathBuf, String> {
    validate_export_name(name)?;
    let runtime = crate::secure_fs::private_runtime_dir().map_err(|e| e.to_string())?;
    let dir = crate::secure_fs::private_dir_in(&runtime, EXPORT_DIR).map_err(|e| e.to_string())?;
    let file = if name.ends_with(".json") { name.to_string() } else { format!("{name}.json") };
    let path = dir.join(file);
    crate::secure_fs::write_private_file(&path, data.to_json_string().as_bytes()).map_err(|e| e.to_string())?;
    Ok(path)
}
