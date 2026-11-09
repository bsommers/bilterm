//! Key-conflict engine (`KEY-02`, Phase 63-03).
//!
//! One engine feeds every surface: the load-time check in `main.rs`, `:keys check`, the
//! `keys.check` RPC and the `bilterm_keys` MCP tool. It answers three questions about the active
//! keymap plus the user's `[keys.bind]` / `[keybindings]` entries:
//! - **internal**: do two Bilterm actions share a chord, or does a user binding shadow something
//!   Bilterm must keep (the leader key, `Ctrl+Q`)?
//! - **host**: does the OS, desktop or terminal host consume the chord before Bilterm sees it?
//! - **consumer**: does a program running inside a pane (readline, vim, tmux, Claude Code, ...)
//!   expect the chord?
//!
//! Host `Block` findings and internal `Block` findings on *user* bindings reject the config at
//! load ([`validate_user_bindings`]); everything else is reported. Output is sorted by
//! `(severity, chord, class, consumer)` so snapshots and cross-surface comparisons are stable.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::action::Action;
use crate::config::BiltermConfig;
use crate::config::keys::KeysConfig;
use crate::error::BiltermError;

use super::chord::KeyChord;
use super::keyconflict_data::{
    ACCEPTED_DEFAULT_SHADOWS, Class, Pattern, RESERVED_CONSUMERS, RESERVED_HOST, RESERVED_INTERNAL, Severity,
};
use super::keyenv::{HostOs, KeyEnv, Surface, primary_mods};
use super::keyprofile::EffectiveKeymap;
use super::keyprofile_tables::PRIMARY_BINDINGS;

pub use super::keyconflict_data::{Class as FindingClass, Severity as FindingSeverity};

/// One conflict report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// Canonical ASCII chord with `Primary` resolved for the env (`Ctrl+Shift+W`, `Cmd+Space`).
    pub chord: String,
    /// The chord as the user wrote it (equal to `chord` for shipped bindings).
    pub bound_as: String,
    pub action: Option<Action>,
    pub severity: Severity,
    pub class: Class,
    pub consumer: String,
    pub note: String,
    /// `default`, `keys.bind` or `keybindings`.
    pub source: &'static str,
}

impl Finding {
    /// Config-load error text for a blocking finding.
    pub fn block_message(&self) -> String {
        let shown = if self.bound_as.eq_ignore_ascii_case(&self.chord) {
            format!("'{}'", self.chord)
        } else {
            format!("'{}' ({})", self.bound_as, self.chord)
        };
        format!("key binding {shown} is reserved by {} ({})", self.consumer, self.note)
    }

    /// `(chord, severity, consumer)`: the tuple the parity tests compare across surfaces.
    pub fn key(&self) -> (String, &'static str, String) {
        (self.chord.clone(), self.severity.as_str(), self.consumer.clone())
    }
}

/// A binding under inspection.
struct Bound {
    chord: KeyChord,
    bound_as: String,
    action: Option<Action>,
    source: &'static str,
}

/// Lowercases letters; Shift stays significant (`Ctrl+Shift+W` is not `Ctrl+W`).
fn norm(c: KeyChord) -> KeyChord {
    let mut n = c;
    if let KeyCode::Char(ch) = n.code {
        n.code = KeyCode::Char(ch.to_ascii_lowercase());
    }
    n
}

/// Resolves `Primary` against `env`. `None` when the chord needs Primary and the env has none.
fn resolve(chord: KeyChord, env: &KeyEnv) -> Option<KeyChord> {
    if !chord.primary {
        return Some(norm(chord));
    }
    let p = primary_mods(env)?;
    Some(norm(KeyChord { mods: chord.mods | p, primary: false, code: chord.code }))
}

/// Canonical ASCII form (`Super` is spelled `Cmd` on macOS).
fn ascii(chord: &KeyChord, env: &KeyEnv) -> String {
    chord.format(env)
}

fn pattern_matches(p: &Pattern, c: &KeyChord) -> bool {
    match p {
        Pattern::Chord(s) => KeyChord::parse(s).is_ok_and(|r| norm(r) == *c),
        Pattern::AnySuper => c.mods.contains(KeyModifiers::SUPER),
        Pattern::CtrlAltLetter => {
            c.mods.contains(KeyModifiers::CONTROL | KeyModifiers::ALT)
                && !c.mods.contains(KeyModifiers::SUPER)
                && matches!(c.code, KeyCode::Char(ch) if ch.is_ascii_alphabetic())
        }
        Pattern::AltPrintable => {
            c.mods.contains(KeyModifiers::ALT)
                && !c.mods.intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER)
                && matches!(c.code, KeyCode::Char(ch) if !ch.is_control())
        }
    }
}

fn applies(os: Option<HostOs>, surface: Option<Surface>, env: &KeyEnv) -> bool {
    os.is_none_or(|o| o == env.os) && surface.is_none_or(|s| s == env.surface)
}

/// Host and consumer findings for one resolved chord.
fn lookup(b: &Bound, c: &KeyChord, env: &KeyEnv, out: &mut Vec<Finding>) {
    let mut host: Vec<(Severity, &str, &str)> = Vec::new();
    for r in RESERVED_HOST.iter().filter(|r| applies(r.os, r.surface, env)) {
        if pattern_matches(&r.pattern, c) {
            host.push((r.severity, r.consumer, r.note));
        }
    }
    if let Some(worst) = host.iter().map(|h| h.0).min() {
        let top: Vec<_> = host.iter().filter(|h| h.0 == worst).collect();
        let mut consumers: Vec<&str> = top.iter().map(|h| h.1).collect();
        consumers.dedup();
        out.push(Finding {
            chord: ascii(c, env),
            bound_as: b.bound_as.clone(),
            action: b.action.clone(),
            severity: worst,
            class: Class::Host,
            consumer: consumers.join(", "),
            note: top[0].2.to_string(),
            source: b.source,
        });
    }
    let mut who: Vec<&str> = Vec::new();
    let mut note = "";
    for set in RESERVED_CONSUMERS.iter().filter(|s| applies(s.os, None, env)) {
        if set.chords.iter().any(|s| KeyChord::parse(s).is_ok_and(|r| norm(r) == *c)) {
            if who.is_empty() {
                note = set.note;
            }
            who.push(set.consumer);
        }
    }
    if !who.is_empty() {
        out.push(Finding {
            chord: ascii(c, env),
            bound_as: b.bound_as.clone(),
            action: b.action.clone(),
            severity: Severity::Warn,
            class: Class::Consumer,
            consumer: who.join(", "),
            note: note.to_string(),
            source: b.source,
        });
    }
}

fn accepted_reason(chord: &str) -> Option<&'static str> {
    ACCEPTED_DEFAULT_SHADOWS.iter().find_map(|(pat, why)| match pat.strip_suffix('*') {
        Some(prefix) => chord.to_ascii_lowercase().starts_with(&prefix.to_ascii_lowercase()).then_some(*why),
        None => chord.eq_ignore_ascii_case(pat).then_some(*why),
    })
}

fn command_action(cmd: &str) -> Option<Action> {
    crate::colon_command::parse_colon_command(cmd).and_then(Result::ok)
}

/// The keymap's shipped chords as bindings (global, pane, Primary, macOS extras).
fn default_bindings(km: &EffectiveKeymap, env: &KeyEnv) -> Vec<Bound> {
    let mut v: Vec<Bound> = Vec::new();
    let mut push = |chord: KeyChord, action: Action| {
        if let Some(c) = resolve(chord, env) {
            let shown = ascii(&c, env);
            if !v.iter().any(|b| b.chord == c) {
                v.push(Bound { chord: c, bound_as: shown, action: Some(action), source: "default" });
            }
        }
    };
    for (m, code, a) in km.global_rows().chain(km.pane_rows()) {
        push(KeyChord::new(m, code), a.clone());
    }
    if km.primary_enabled() {
        for (code, a) in PRIMARY_BINDINGS {
            push(KeyChord::primary(*code), a.clone());
        }
    }
    v
}

/// User bindings (`[keys.bind]` wins over `[keybindings]`); entries that do not parse are reported
/// by the caller as unreachable.
fn user_bindings<'a>(keys: &'a KeysConfig, legacy: &'a std::collections::BTreeMap<String, String>) -> Vec<(&'static str, &'a str, &'a str)> {
    let mut v: Vec<(&'static str, &str, &str)> = Vec::new();
    v.extend(keys.bind.iter().map(|(k, c)| ("keys.bind", k.as_str(), c.as_str())));
    v.extend(legacy.iter().map(|(k, c)| ("keybindings", k.as_str(), c.as_str())));
    v
}

fn internal(b: &Bound, chord: &KeyChord, env: &KeyEnv, sev: Severity, who: &str, note: String) -> Finding {
    Finding {
        chord: ascii(chord, env),
        bound_as: b.bound_as.clone(),
        action: b.action.clone(),
        severity: sev,
        class: Class::Internal,
        consumer: who.to_string(),
        note,
        source: b.source,
    }
}

fn run(km: &EffectiveKeymap, env: &KeyEnv, keys: &KeysConfig, legacy: &std::collections::BTreeMap<String, String>, leader: Option<&str>) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let defaults = default_bindings(km, env);
    let leader_chord = leader.and_then(|l| KeyChord::parse(l).ok()).and_then(|c| resolve(c, env));

    // Shipped chords: host/consumer lookups, with accepted shadows demoted to Info.
    let mut shipped: Vec<Bound> = defaults;
    if let Some(l) = leader_chord {
        let shown = ascii(&l, env);
        shipped.push(Bound { chord: l, bound_as: shown, action: None, source: "default" });
    }
    for b in &shipped {
        let mut found = Vec::new();
        lookup(b, &b.chord, env, &mut found);
        for mut f in found {
            if f.severity == Severity::Warn
                && let Some(why) = accepted_reason(&f.chord)
            {
                f.severity = Severity::Info;
                f.note = format!("{} (accepted: {why})", f.note);
            }
            out.push(f);
        }
    }
    for d in km.unexpected_duplicates() {
        out.push(Finding {
            chord: d.chord.clone(),
            bound_as: d.chord.clone(),
            action: d.actions.first().cloned(),
            severity: Severity::Warn,
            class: Class::Internal,
            consumer: "Bilterm".to_string(),
            note: format!("{} table binds {} to {} actions", d.table, d.chord, d.actions.len()),
            source: "default",
        });
    }

    // User chords.
    let mut seen_user: Vec<(KeyChord, &str, &str)> = Vec::new();
    for (source, raw, command) in user_bindings(keys, legacy) {
        let parsed = match KeyChord::parse(raw) {
            Ok(c) => c,
            Err(e) => {
                let b = Bound { chord: KeyChord::new(KeyModifiers::NONE, KeyCode::Null), bound_as: raw.to_string(), action: None, source };
                let mut f = internal(&b, &b.chord, env, Severity::Warn, "Bilterm", format!("not a valid chord, binding is unreachable: {e}"));
                f.chord = raw.to_string();
                out.push(f);
                continue;
            }
        };
        let Some(c) = resolve(parsed, env) else {
            let b = Bound { chord: norm(parsed), bound_as: raw.to_string(), action: command_action(command), source };
            let mut f = internal(&b, &b.chord, env, Severity::Warn, "Bilterm", "Primary is unavailable in this environment (needs a GUI or the kitty keyboard protocol); the binding never fires".to_string());
            f.chord = raw.to_string();
            out.push(f);
            continue;
        };
        let b = Bound { chord: c, bound_as: raw.to_string(), action: command_action(command), source };
        lookup(&b, &c, env, &mut out);
        for (rc, why) in RESERVED_INTERNAL {
            if KeyChord::parse(rc).is_ok_and(|r| norm(r) == c) {
                out.push(internal(&b, &c, env, Severity::Block, "Bilterm Exit Guard", (*why).to_string()));
            }
        }
        if leader_chord == Some(c) {
            out.push(internal(&b, &c, env, Severity::Warn, "Bilterm", "shadows the leader key; the leader prefix becomes unreachable".to_string()));
        }
        if let Some(d) = shipped.iter().find(|d| d.chord == c && d.source == "default" && d.action.is_some()) {
            let what = d.action.as_ref().map_or_else(String::new, Action::colon_form_or_summary);
            out.push(internal(&b, &c, env, Severity::Info, "Bilterm", format!("overrides the shipped binding ({what})")));
        }
        if let Some((_, prev_src, prev_cmd)) = seen_user.iter().find(|(pc, _, _)| *pc == c)
            && *prev_cmd != command
        {
            out.push(internal(&b, &c, env, Severity::Warn, "Bilterm", format!("also bound in {prev_src} to '{prev_cmd}'; {prev_src} wins")));
        }
        seen_user.push((c, source, command));
    }

    out.sort_by(|a, b| {
        (a.severity, &a.chord, a.class, &a.consumer, a.source).cmp(&(b.severity, &b.chord, b.class, &b.consumer, b.source))
    });
    out.dedup();
    out
}

/// Findings for the active keymap and the `[keys]` table. Does not know `[keybindings]` or the
/// leader key; see [`check_config`] for the whole-config form.
pub fn check(effective: &EffectiveKeymap, env: &KeyEnv, cfg: &KeysConfig) -> Vec<Finding> {
    run(effective, env, cfg, &std::collections::BTreeMap::new(), None)
}

/// Findings for a whole config: `[keys]`, legacy `[keybindings]` and `leader_key`.
pub fn check_config(effective: &EffectiveKeymap, env: &KeyEnv, cfg: &BiltermConfig) -> Vec<Finding> {
    run(effective, env, &cfg.keys, &cfg.keybindings, Some(&cfg.leader_key))
}

/// Rejects the config when a user binding is a `Block` finding (OS/host reserved or `Ctrl+Q`).
pub fn validate_user_bindings(cfg: &BiltermConfig, env: &KeyEnv) -> Result<(), BiltermError> {
    let km = crate::input::keyprofile::KeyProfile::resolve(&cfg.keys, env);
    let blocked = run(&km, env, &cfg.keys, &cfg.keybindings, Some(&cfg.leader_key))
        .into_iter()
        .find(|f| f.severity == Severity::Block && f.source != "default");
    match blocked {
        Some(f) => Err(BiltermError::Config(f.block_message())),
        None => Ok(()),
    }
}

/// Findings introduced by the user (everything not from the shipped tables, minus plain overrides).
pub fn user_findings(findings: &[Finding]) -> impl Iterator<Item = &Finding> {
    findings.iter().filter(|f| f.source != "default" && f.severity <= Severity::Warn)
}

/// The one-line load-time status (`N key conflicts: :keys check`), or `None` when there is nothing to say.
pub fn load_status(findings: &[Finding]) -> Option<String> {
    let n = user_findings(findings).count();
    (n > 0).then(|| format!("{n} key conflict{}: :keys check", if n == 1 { "" } else { "s" }))
}

/// Human-readable one-line count used by `:keys check` and the RPC summary.
pub fn summary_line(profile: &str, findings: &[Finding]) -> String {
    let count = |s: Severity| findings.iter().filter(|f| f.severity == s).count();
    let (b, w, i) = (count(Severity::Block), count(Severity::Warn), count(Severity::Info));
    if b + w == 0 {
        return format!("Keys [{profile}]: no conflicts ({i} accepted shadow{})", if i == 1 { "" } else { "s" });
    }
    let first = findings.iter().find(|f| f.severity <= Severity::Warn).map_or(String::new(), |f| {
        format!(", first: {} vs {} ({})", f.chord, f.consumer, f.class.as_str())
    });
    format!("Keys [{profile}]: {b} blocked, {w} warning(s), {i} accepted{first}")
}

/// Explains what `chord` does under `effective` and which findings apply to it.
pub fn explain(chord: &str, effective: &EffectiveKeymap, env: &KeyEnv, cfg: &BiltermConfig) -> Explanation {
    let parsed = KeyChord::parse(chord);
    let Ok(parsed) = parsed else {
        return Explanation { chord: chord.to_string(), error: parsed.err(), action: None, origin: "none", findings: Vec::new() };
    };
    let resolved = resolve(parsed, env);
    let shown = resolved.map_or_else(|| parsed.format(env), |r| ascii(&r, env));
    let user = cfg
        .keys
        .bind
        .iter()
        .chain(cfg.keybindings.iter())
        .find(|(k, _)| KeyChord::parse(k).ok().and_then(|c| resolve(c, env)) == resolved && resolved.is_some());
    let (action, origin) = if let Some((_, cmd)) = user {
        (command_action(cmd), "user")
    } else if let Some(c) = resolved {
        match default_bindings(effective, env).into_iter().find(|b| b.chord == c) {
            Some(b) => {
                let primary = parsed.primary;
                (b.action, if primary || effective.profile() != crate::config::keys::KeyProfileName::Legacy { "profile" } else { "default" })
            }
            None => (None, "none"),
        }
    } else {
        (None, "none")
    };
    let findings = check_config(effective, env, cfg).into_iter().filter(|f| f.chord == shown).collect();
    Explanation { chord: shown, error: None, action, origin, findings }
}

/// Result of [`explain`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Explanation {
    pub chord: String,
    pub error: Option<String>,
    pub action: Option<Action>,
    /// `user`, `profile`, `default` or `none` (unbound).
    pub origin: &'static str,
    pub findings: Vec<Finding>,
}

impl Explanation {
    /// One-line text for the status bar.
    pub fn text(&self, profile: &str) -> String {
        if let Some(e) = &self.error {
            return e.clone();
        }
        let head = match &self.action {
            Some(a) => format!("{} -> {} ({}, profile {profile})", self.chord, a.colon_form_or_summary(), self.origin),
            None if self.origin == "user" => format!("{} -> user command ({}, profile {profile})", self.chord, self.origin),
            None => format!("{} is not bound in profile {profile}", self.chord),
        };
        if self.findings.is_empty() {
            head
        } else {
            let list: Vec<String> = self.findings.iter().map(|f| format!("{} {} {}", f.severity.as_str(), f.class.as_str(), f.consumer)).collect();
            format!("{head}; {}", list.join("; "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::keys::{KeyProfileName, KeysConfig};
    use crate::input::keyprofile::KeyProfile;

    fn cfg_with(bind: &[(&str, &str)]) -> BiltermConfig {
        let mut c = BiltermConfig::default();
        for (k, v) in bind {
            c.keys.bind.insert((*k).to_string(), (*v).to_string());
        }
        c
    }

    fn env(os: HostOs) -> KeyEnv {
        KeyEnv::new(os, Surface::Gui)
    }

    fn findings(os: HostOs, bind: &[(&str, &str)]) -> Vec<Finding> {
        let c = cfg_with(bind);
        let e = env(os);
        check_config(&KeyProfile::resolve(&c.keys, &e), &e, &c)
    }

    #[test]
    fn data_tables_parse_and_carry_reasons() {
        for r in RESERVED_HOST {
            if let Pattern::Chord(s) = r.pattern {
                assert!(KeyChord::parse(s).is_ok(), "host chord {s}");
            }
            assert!(!r.consumer.is_empty() && !r.note.is_empty());
        }
        for s in RESERVED_CONSUMERS {
            assert!(!s.chords.is_empty() && !s.consumer.is_empty());
            for c in s.chords {
                assert!(KeyChord::parse(c).is_ok(), "consumer chord {c} of {}", s.consumer);
            }
        }
        for (c, why) in RESERVED_INTERNAL.iter().chain(ACCEPTED_DEFAULT_SHADOWS.iter()) {
            assert!(!why.trim().is_empty(), "{c} needs a reason");
        }
        for (c, why) in super::super::keyprofile_tables::ALLOWED_SHADOWS {
            assert!(!why.trim().is_empty(), "ALLOWED_SHADOWS {c} needs a reason");
        }
    }

    #[test]
    fn internal_class_user_bind_shadows_shipped_and_quit() {
        let f = findings(HostOs::Linux, &[("Ctrl+Q", ":help"), ("Primary+L", ":help")]);
        assert!(f.iter().any(|x| x.class == Class::Internal && x.severity == Severity::Block && x.chord == "Ctrl+Q"));
        assert!(f.iter().any(|x| x.class == Class::Internal && x.severity == Severity::Info && x.chord == "Ctrl+Shift+L"));
    }

    #[test]
    fn host_class_blocks_per_os() {
        for (os, chord, blocked) in [
            (HostOs::MacOs, "Cmd+Space", true),
            (HostOs::Windows, "Alt+F4", true),
            (HostOs::Linux, "Super+L", true),
            (HostOs::MacOs, "Alt+F4", false),
            (HostOs::Linux, "Ctrl+Alt+Delete", true),
            (HostOs::MacOs, "Super+L", true),
        ] {
            let f = findings(os, &[(chord, ":help")]);
            let any = f.iter().any(|x| x.class == Class::Host && x.severity == Severity::Block && x.source == "keys.bind");
            // macOS Super is Cmd, which is only blocked chord by chord.
            let expect = blocked && !(os == HostOs::MacOs && chord == "Super+L");
            assert_eq!(any, expect, "{os:?} {chord}");
        }
    }

    #[test]
    fn consumer_class_warns_with_named_consumer() {
        let f = findings(HostOs::Linux, &[("Ctrl+A", ":split v")]);
        let w: Vec<_> = f.iter().filter(|x| x.source == "keys.bind" && x.class == Class::Consumer).collect();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].severity, Severity::Warn);
        assert!(w[0].consumer.contains("readline"));
        assert_eq!(load_status(&f).as_deref(), Some("1 key conflict: :keys check"));
    }

    #[test]
    fn user_block_rejected_with_named_error() {
        let mut c = cfg_with(&[("Alt+F4", ":help")]);
        let err = validate_user_bindings(&c, &env(HostOs::Windows)).unwrap_err().to_string();
        assert!(err.contains("key binding 'Alt+F4' is reserved by Windows (close window)"), "{err}");
        assert!(validate_user_bindings(&c, &env(HostOs::MacOs)).is_ok());
        c.keybindings.insert("Ctrl+Q".into(), ":help".into());
        assert!(validate_user_bindings(&c, &env(HostOs::MacOs)).is_err());
    }

    #[test]
    fn output_is_sorted_and_deterministic() {
        let a = findings(HostOs::Linux, &[("Ctrl+A", ":a"), ("Super+L", ":b"), ("Ctrl+Q", ":c")]);
        let b = findings(HostOs::Linux, &[("Ctrl+Q", ":c"), ("Super+L", ":b"), ("Ctrl+A", ":a")]);
        assert_eq!(a, b);
        assert!(a.windows(2).all(|w| (w[0].severity, &w[0].chord) <= (w[1].severity, &w[1].chord)));
    }

    #[test]
    fn shipped_profiles_have_no_block_and_only_accepted_warns() {
        for os in [HostOs::MacOs, HostOs::Windows, HostOs::Linux] {
            for surface in [Surface::Tui, Surface::Gui] {
                for kitty in [false, true] {
                    let e = KeyEnv::new(os, surface).with_kitty(kitty);
                    for name in [KeyProfileName::Auto, KeyProfileName::Portable] {
                        let c = BiltermConfig { keys: KeysConfig { profile: name, ..KeysConfig::default() }, ..BiltermConfig::default() };
                        let f = check_config(&KeyProfile::resolve(&c.keys, &e), &e, &c);
                        let bad: Vec<_> = f.iter().filter(|x| x.severity <= Severity::Warn).map(|x| (x.chord.clone(), x.consumer.clone(), x.severity)).collect();
                        assert!(bad.is_empty(), "{name:?} {os:?} {surface:?} kitty={kitty}: {bad:?}");
                        for x in f.iter().filter(|x| x.severity == Severity::Info && x.class != Class::Internal) {
                            assert!(x.note.contains("accepted:"), "{x:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn explain_reports_origin_and_findings() {
        let c = cfg_with(&[("Ctrl+A", ":split v")]);
        let e = env(HostOs::Linux);
        let km = KeyProfile::resolve(&c.keys, &e);
        let x = explain("Ctrl+A", &km, &e, &c);
        assert_eq!(x.origin, "user");
        assert!(x.findings.iter().any(|f| f.class == Class::Consumer));
        assert!(x.text("linux").contains("readline"));
        assert_eq!(explain("Primary+L", &km, &e, &c).origin, "profile");
        assert!(explain("nonsense+", &km, &e, &c).error.is_some());
    }
}
