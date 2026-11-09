//! Generic dotted-key access to [`BiltermConfig`] (`:get`, `:set`, `:config list|save`, Phase 49-04).
//!
//! The config is round-tripped through `toml::Value`, so every serde field is addressable by its
//! TOML path (`appearance.profile`, `theme`, `drawer.width`). Limitation: `Option` fields that are
//! `None` are omitted by serde; `get_key` reports them as `(unset)` only when a probe proves the
//! key exists, and `set_key` accepts them because the typed deserialisation validates the value.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::BiltermConfig;

/// `[api]` section: policy for the automation plane (JSON-RPC / MCP / HTTP).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiConfig {
    /// Allows `:set`, `:config save` and `:appearance save` over the API (default `false`).
    pub allow_config_write: bool,
    /// Allows agent-to-agent messaging and coordination (`AGT-01..03`, Phase 52).
    pub allow_agent_messaging: bool,
}

fn to_value(cfg: &BiltermConfig) -> Result<toml::Value, String> {
    toml::Value::try_from(cfg).map_err(|e| format!("cannot serialize config: {e}"))
}

fn walk<'a>(root: &'a toml::Value, key: &str) -> Option<&'a toml::Value> {
    let mut cur = root;
    for part in key.split('.') {
        cur = cur.as_table()?.get(part)?;
    }
    Some(cur)
}

fn render(v: &toml::Value) -> String {
    match v {
        toml::Value::Table(t) => toml::to_string(t).unwrap_or_default().trim_end().to_string(),
        other => other.to_string(),
    }
}

fn valid_key(key: &str) -> Result<(), String> {
    if key.is_empty() || key.split('.').any(str::is_empty) {
        return Err(format!("invalid config key `{key}`"));
    }
    Ok(())
}

/// Returns the TOML text of the value at `key` (`(unset)` for a known but unset optional key).
pub fn get_key(cfg: &BiltermConfig, key: &str) -> Result<String, String> {
    valid_key(key)?;
    let root = to_value(cfg)?;
    if let Some(v) = walk(&root, key) {
        return Ok(render(v));
    }
    // A path that descends through a non-table value can never be valid.
    let parts: Vec<&str> = key.split('.').collect();
    for n in 1..parts.len() {
        if walk(&root, &parts[..n].join(".")).is_some_and(|v| !v.is_table()) {
            return Err(format!("unknown config key `{key}`"));
        }
    }
    // Known-but-None probe: a type error (not "unknown key") proves the field exists.
    for probe in ["true", "0", "\"x\""] {
        let mut copy = cfg.clone();
        match set_key(&mut copy, key, probe) {
            Ok(()) => return Ok("(unset)".to_string()),
            Err(e) if !e.starts_with("unknown config key") => return Ok("(unset)".to_string()),
            Err(_) => {}
        }
    }
    Err(format!("unknown config key `{key}`"))
}

fn flatten(prefix: &str, v: &toml::Value, out: &mut BTreeMap<String, toml::Value>) {
    match v {
        toml::Value::Table(t) => {
            for (k, child) in t {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten(&key, child, out);
            }
        }
        leaf => {
            out.insert(prefix.to_string(), leaf.clone());
        }
    }
}

fn flat(cfg: &BiltermConfig) -> Result<BTreeMap<String, toml::Value>, String> {
    let mut out = BTreeMap::new();
    flatten("", &to_value(cfg)?, &mut out);
    Ok(out)
}

/// Flattened `dotted.key => TOML text` for every set leaf, sorted by key.
pub fn list_keys(cfg: &BiltermConfig) -> Vec<(String, String)> {
    flat(cfg)
        .map(|m| m.into_iter().map(|(k, v)| (k, v.to_string())).collect())
        .unwrap_or_default()
}

fn parse_raw(raw: &str) -> toml::Value {
    toml::from_str::<toml::Table>(&format!("v = {raw}"))
        .ok()
        .and_then(|mut t| t.remove("v"))
        .unwrap_or_else(|| toml::Value::String(raw.to_string()))
}

fn insert_at(root: &mut toml::Value, key: &str, val: toml::Value) -> Result<(), String> {
    let parts: Vec<&str> = key.split('.').collect();
    let mut cur = root;
    for part in &parts[..parts.len() - 1] {
        let table = cur.as_table_mut().ok_or_else(|| format!("`{key}` is not inside a table"))?;
        cur = table
            .entry((*part).to_string())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    }
    let table = cur.as_table_mut().ok_or_else(|| format!("`{key}` is not inside a table"))?;
    table.insert(parts[parts.len() - 1].to_string(), val);
    Ok(())
}

fn try_apply(cfg: &BiltermConfig, key: &str, val: toml::Value) -> Result<BiltermConfig, String> {
    let mut root = to_value(cfg)?;
    insert_at(&mut root, key, val)?;
    let mut next: BiltermConfig = root.clone().try_into().map_err(|e| e.to_string())?;
    // serde ignores unknown fields: prove the key survived the round trip.
    if walk(&to_value(&next)?, key).is_none() {
        return Err(format!("unknown config key `{key}`"));
    }
    crate::input::keyconflict::revalidate_after_set(key, &next)?;
    next.custom_palette_override = cfg.custom_palette_override.clone();
    next.cursor_blink_hidden = cfg.cursor_blink_hidden;
    Ok(next)
}

/// Sets `key` to `raw` (a TOML value, or a bare string). `cfg` is untouched on error.
pub fn set_key(cfg: &mut BiltermConfig, key: &str, raw: &str) -> Result<(), String> {
    valid_key(key)?;
    let raw = raw.trim();
    let parsed = parse_raw(raw);
    let next = match try_apply(cfg, key, parsed.clone()) {
        Ok(n) => n,
        Err(first) => {
            if matches!(parsed, toml::Value::String(_)) || first.starts_with("unknown config key") {
                return Err(first);
            }
            try_apply(cfg, key, toml::Value::String(raw.to_string())).map_err(|_| first)?
        }
    };
    *cfg = next;
    Ok(())
}

fn item_from(v: &toml::Value) -> Result<toml_edit::Item, String> {
    v.to_string()
        .parse::<toml_edit::Value>()
        .map(toml_edit::Item::Value)
        .map_err(|e| format!("cannot encode value: {e}"))
}

fn table_for<'a>(
    doc: &'a mut toml_edit::DocumentMut,
    parts: &[&str],
) -> Result<&'a mut dyn toml_edit::TableLike, String> {
    let mut cur: &mut dyn toml_edit::TableLike = doc.as_table_mut();
    for p in parts {
        if !cur.contains_key(p) {
            let mut t = toml_edit::Table::new();
            t.set_implicit(true);
            cur.insert(p, toml_edit::Item::Table(t));
        }
        cur = cur
            .get_mut(p)
            .and_then(toml_edit::Item::as_table_like_mut)
            .ok_or_else(|| format!("`{p}` is not a table in the config file"))?;
    }
    Ok(cur)
}

/// Writes only the leaf keys that differ between `cfg` and the file at `path` (or defaults when
/// the file is missing), optionally restricted to keys starting with `prefix`. Comments, formatting
/// and unrelated keys are preserved. Returns the keys written (or removed).
pub fn save_changes(cfg: &BiltermConfig, path: &Path, prefix: Option<&str>) -> Result<Vec<String>, String> {
    let disk_cfg = BiltermConfig::load(path)?.unwrap_or_default();
    let (running, disk) = (flat(cfg)?, flat(&disk_cfg)?);
    let wanted = |k: &str| prefix.is_none_or(|p| k.starts_with(p));
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let mut doc = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("invalid {}: {e}", path.display()))?;
    let mut written = Vec::new();
    for (k, v) in &running {
        if !wanted(k) || disk.get(k) == Some(v) {
            continue;
        }
        let parts: Vec<&str> = k.split('.').collect();
        let (last, parents) = parts.split_last().ok_or("empty key")?;
        let table = table_for(&mut doc, parents)?;
        let mut item = item_from(v)?;
        if let Some(slot) = table.get_mut(last) {
            // Replace in place so the key (and its leading comments) is untouched.
            if let (Some(old), Some(new)) = (slot.as_value(), item.as_value_mut()) {
                *new.decor_mut() = old.decor().clone();
            }
            *slot = item;
        } else {
            table.insert(last, item);
        }
        written.push(k.clone());
    }
    for k in disk.keys().filter(|k| wanted(k) && !running.contains_key(*k)) {
        let parts: Vec<&str> = k.split('.').collect();
        let (last, parents) = parts.split_last().ok_or("empty key")?;
        if let Ok(table) = table_for(&mut doc, parents) {
            table.remove(last);
            written.push(k.clone());
        }
    }
    if written.is_empty() {
        return Ok(written);
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let mut out = doc.to_string();
    if text.contains("\r\n") {
        // toml_edit re-emits key/value line ends as `\n`; keep the file's CRLF convention.
        out = out.replace("\r\n", "\n").replace('\n', "\r\n");
    }
    crate::secure_fs::write_private_file(path, out.as_bytes())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("bilterm-dotted-{name}-{}", crate::secure_fs::unique_suffix()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn round_trip_bool_enum_number_string() {
        let mut c = BiltermConfig::default();
        set_key(&mut c, "crosshair_cursor", "true").unwrap();
        assert!(c.crosshair_cursor);
        assert_eq!(get_key(&c, "crosshair_cursor").unwrap(), "true");
        set_key(&mut c, "appearance.profile", "midnight").unwrap();
        assert_eq!(get_key(&c, "appearance.profile").unwrap(), "\"midnight\"");
        set_key(&mut c, "font_size_pt", "18").unwrap();
        assert_eq!(c.font_size_pt, 18);
        set_key(&mut c, "font_family", "Fira Code").unwrap();
        assert_eq!(c.font_family, "Fira Code");
        set_key(&mut c, "leader_key", "123").unwrap();
        assert_eq!(c.leader_key, "123");
    }

    #[test]
    fn invalid_value_and_unknown_key_leave_config_unchanged() {
        let mut c = BiltermConfig::default();
        let before = c.clone();
        assert!(set_key(&mut c, "theme", "no-such-theme").is_err());
        assert!(set_key(&mut c, "font_size_pt", "banana").is_err());
        assert!(set_key(&mut c, "bogus.key", "1").is_err());
        assert!(get_key(&c, "bogus").unwrap_err().contains("unknown config key"));
        assert_eq!(c, before);
    }

    #[test]
    fn unset_option_is_reported_and_list_is_sorted() {
        let c = BiltermConfig::default();
        assert_eq!(get_key(&c, "appearance.profile").unwrap(), "(unset)");
        let keys = list_keys(&c);
        assert!(keys.windows(2).all(|w| w[0].0 <= w[1].0));
        assert!(keys.iter().any(|(k, _)| k == "api.allow_config_write"));
    }

    #[test]
    fn save_preserves_comments_and_only_changes_target() {
        let dir = tmp("save");
        let path = dir.join("config.toml");
        std::fs::write(&path, "# my config\nfont_size_pt = 16 # size\n\n[appearance]\n# profile below\n").unwrap();
        let mut c = BiltermConfig::load(&path).unwrap().unwrap();
        set_key(&mut c, "appearance.profile", "arctic").unwrap();
        let written = save_changes(&c, &path, None).unwrap();
        assert_eq!(written, vec!["appearance.profile".to_string()]);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# my config") && text.contains("font_size_pt = 16 # size") && text.contains("# profile below"));
        assert!(text.contains("profile = \"arctic\""));
        assert_eq!(BiltermConfig::load(&path).unwrap().unwrap().font_size_pt, 16);
    }

    #[test]
    fn save_prefix_restricts_keys_and_creates_missing_file() {
        let dir = tmp("prefix");
        let path = dir.join("sub").join("config.toml");
        let mut c = BiltermConfig::default();
        set_key(&mut c, "appearance.profile", "liquid").unwrap();
        set_key(&mut c, "font_size_pt", "20").unwrap();
        let written = save_changes(&c, &path, Some("appearance.")).unwrap();
        assert_eq!(written, vec!["appearance.profile".to_string()]);
        let back = BiltermConfig::load(&path).unwrap().unwrap();
        assert_eq!(back.font_size_pt, BiltermConfig::default().font_size_pt);
        assert!(back.appearance.profile.is_some());
    }

    fn save_with(orig: &str, key: &str, raw: &str) -> String {
        let p = tmp("edit").join("c.toml");
        std::fs::write(&p, orig).unwrap();
        let mut cfg = BiltermConfig::load(&p).unwrap().unwrap();
        set_key(&mut cfg, key, raw).unwrap();
        save_changes(&cfg, &p, None).unwrap();
        std::fs::read_to_string(&p).unwrap()
    }

    #[test]
    fn save_keeps_leading_and_inline_comments() {
        let t = save_with("# hdr\nfont_size_pt = 14  # note\n", "font_size_pt", "19");
        assert_eq!(t, "# hdr\nfont_size_pt = 19  # note\n");
    }

    #[test]
    fn save_keeps_table_comments_and_neighbours() {
        let orig = "# top\n\n# about appearance\n[appearance]\n# p\nprofile = \"midnight\"\n# between\n";
        let t = save_with("# top\n\n# about appearance\n[appearance]\n# p\nprofile = \"midnight\"\n# between\n", "appearance.profile", "arctic");
        assert!(t.contains("# about appearance\n[appearance]\n# p\nprofile = \"arctic\"\n# between\n"), "{t}");
        let _ = orig;
    }

    #[test]
    fn save_new_key_does_not_disturb_neighbours_and_crlf_kept() {
        let t = save_with("# hdr\r\nfont_size_pt = 14\r\nfont_family = \"x\"\r\n", "font_size_pt", "19");
        assert_eq!(t, "# hdr\r\nfont_size_pt = 19\r\nfont_family = \"x\"\r\n");
        let t = save_with("# hdr\nfont_size_pt = 14 # n\n", "appearance.profile", "arctic");
        assert!(t.starts_with("# hdr\nfont_size_pt = 14 # n\n"), "{t}");
        assert!(t.contains("profile = \"arctic\""), "{t}");
    }

    #[test]
    fn get_child_of_scalar_is_unknown() {
        let c = BiltermConfig::default();
        assert!(get_key(&c, "font_size_pt.sub").unwrap_err().starts_with("unknown config key"));
    }
}
