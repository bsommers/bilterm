//! Config loading / dotted keys and CLI argument parsing: the error paths and round trips that
//! the feature suites do not pin (unknown keys, type errors, atomic failure, unreadable files,
//! option syntax, endpoint resolution). Pure and platform-neutral: runs on every CI cell.

mod common;

use std::path::Path;

use bilterm::api::JsonValue;
use bilterm::cli::{self, CliEnv};
use bilterm::config::BiltermConfig;
use bilterm::config::dotted::{get_key, list_keys, save_changes, set_key};

// ---------------------------------------------------------------- dotted config keys

#[test]
fn every_listed_key_reads_back_and_re_sets_to_itself() {
    let cfg = BiltermConfig::default();
    let keys = list_keys(&cfg);
    assert!(keys.len() > 30, "the default config should expose many leaf keys (got {})", keys.len());
    assert!(keys.windows(2).all(|w| w[0].0 < w[1].0), "list_keys is sorted and free of duplicates");
    let mut failures = Vec::new();
    for (key, text) in &keys {
        match get_key(&cfg, key) {
            Ok(got) if &got == text => {}
            other => failures.push(format!("get {key}: listed {text:?}, got {other:?}")),
        }
        let mut copy = cfg.clone();
        match set_key(&mut copy, key, text) {
            Ok(()) if copy == cfg => {}
            Ok(()) => failures.push(format!("set {key} = {text} changed the config")),
            Err(e) => failures.push(format!("set {key} = {text} failed: {e}")),
        }
    }
    assert!(failures.is_empty(), "{} key(s) do not round trip:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn malformed_and_unknown_keys_are_rejected_and_leave_the_config_untouched() {
    let original = BiltermConfig::default();
    let mut cfg = original.clone();
    for key in ["", ".", "..", ".theme", "theme.", "a..b", "theme..x"] {
        let e = get_key(&cfg, key).unwrap_err();
        assert!(e.contains("invalid config key"), "get {key:?}: {e}");
        let e = set_key(&mut cfg, key, "1").unwrap_err();
        assert!(e.contains("invalid config key"), "set {key:?}: {e}");
    }
    for key in ["no_such_key", "theme.sub.key", "keys.no_such_field", "agents.watchdog.nope", "appearance.nope.deeper"] {
        let e = get_key(&cfg, key).unwrap_err();
        assert!(e.contains("unknown config key"), "get {key:?}: {e}");
        assert!(set_key(&mut cfg, key, "1").is_err(), "set {key:?} must fail");
    }
    assert_eq!(cfg, original, "a rejected set must not modify the config");
}

#[test]
fn type_errors_are_reported_and_atomic() {
    let original = BiltermConfig::default();
    let keys = list_keys(&original);
    let mut cfg = original.clone();
    let mut rejected = 0;
    for (key, text) in &keys {
        // A table can never be a valid value for a scalar leaf, whatever its real type is.
        let before = cfg.clone();
        if set_key(&mut cfg, key, "{ nested = { deeper = [1, 2] } }").is_err() {
            rejected += 1;
            assert_eq!(cfg, before, "failed set of {key} (was {text}) modified the config");
        } else {
            cfg = before; // string-typed keys keep the raw text as a bare string, by design
        }
    }
    // Numbers, booleans and enums (the large majority of leaves) must all refuse it.
    assert!(rejected * 2 > keys.len(), "only {rejected}/{} keys rejected a table value", keys.len());
}

#[test]
fn a_bare_word_is_accepted_as_a_string_value() {
    let mut cfg = BiltermConfig::default();
    let (key, _) = list_keys(&cfg).into_iter().find(|(k, _)| k == "history_session").expect("history_session is a listed key");
    set_key(&mut cfg, &key, "work-notes").expect("bare word becomes a string");
    assert_eq!(get_key(&cfg, &key).unwrap(), "\"work-notes\"");
    set_key(&mut cfg, &key, "\"quoted value\"").expect("quoted TOML string");
    assert_eq!(get_key(&cfg, &key).unwrap(), "\"quoted value\"");
}

// ---------------------------------------------------------------- loading and saving

#[test]
fn load_distinguishes_missing_unreadable_and_invalid_files() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("absent.toml");
    assert_eq!(BiltermConfig::load(&missing), Ok(None), "a missing file is not an error");
    assert_eq!(BiltermConfig::load_or_default(&missing), BiltermConfig::default());

    // A directory where a file is expected: reported, not silently defaulted.
    let e = BiltermConfig::load(dir.path()).unwrap_err();
    assert!(e.contains("cannot read"), "{e}");

    let bad = dir.path().join("bad.toml");
    for text in ["theme = [", "this is not toml", "[keys]\nprofile = 7", "\u{0}\u{1}\u{2}"] {
        std::fs::write(&bad, text).unwrap();
        let e = BiltermConfig::load(&bad).unwrap_err();
        assert!(e.contains("invalid") && e.contains("bad.toml"), "error must name the file: {e}");
        assert_eq!(BiltermConfig::load_or_default(&bad), BiltermConfig::default(), "fallback is the defaults");
    }
    std::fs::write(&bad, [0xff, 0xfe, 0x00, 0x41]).unwrap();
    assert!(BiltermConfig::load(&bad).is_err(), "non-UTF-8 bytes are an error, not a panic");

    let empty = dir.path().join("empty.toml");
    std::fs::write(&empty, "").unwrap();
    assert_eq!(BiltermConfig::load(&empty), Ok(Some(BiltermConfig::default())), "an empty file is the defaults");
}

#[test]
fn a_fresh_install_differs_from_the_defaults_only_by_profile() {
    let fresh = BiltermConfig::fresh_install();
    let default = BiltermConfig::default();
    assert_ne!(fresh, default);
    let changed: Vec<String> = list_keys(&fresh)
        .into_iter()
        .filter(|(k, v)| get_key(&default, k).ok().as_ref() != Some(v))
        .map(|(k, _)| k)
        .collect();
    assert!(!changed.is_empty() && changed.iter().all(|k| k.starts_with("appearance")), "{changed:?}");
}

#[test]
fn save_changes_writes_only_the_changed_keys_and_keeps_the_rest_of_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "# my hand-written comment\nhistory_session = \"old\"\n").unwrap();

    let mut cfg = BiltermConfig::load(&path).unwrap().unwrap();
    set_key(&mut cfg, "history_session", "new").unwrap();
    let written = save_changes(&cfg, &path, None).unwrap();
    assert_eq!(written, vec!["history_session".to_string()]);

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("# my hand-written comment"), "comments must survive: {text}");
    assert!(text.contains("history_session = \"new\""), "{text}");
    assert_eq!(BiltermConfig::load(&path).unwrap().unwrap(), cfg, "what was saved is what loads");

    // Nothing changed: nothing written. A prefix that matches nothing: nothing written.
    assert!(save_changes(&cfg, &path, None).unwrap().is_empty());
    set_key(&mut cfg, "history_session", "third").unwrap();
    assert!(save_changes(&cfg, &path, Some("no.such.prefix")).unwrap().is_empty());

    // A corrupt file on disk is reported instead of being overwritten.
    std::fs::write(&path, "theme = [").unwrap();
    assert!(save_changes(&cfg, &path, None).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "theme = [", "the corrupt file is left for the user to fix");

    // A missing file is created with just the changed keys.
    let fresh = dir.path().join("new").join("config.toml");
    std::fs::create_dir_all(fresh.parent().unwrap()).unwrap();
    let written = save_changes(&cfg, &fresh, None).unwrap();
    assert_eq!(written, vec!["history_session".to_string()]);
    assert_eq!(BiltermConfig::load(&fresh).unwrap().unwrap(), cfg);
}

// ---------------------------------------------------------------- CLI argument parsing

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(ToString::to_string).collect()
}

fn parse(v: &[&str]) -> Result<cli::RpcCall, String> {
    cli::parse(&args(v), &CliEnv::default())
}

#[test]
fn only_the_documented_words_are_cli_commands() {
    for word in cli::CLI_COMMANDS {
        assert!(cli::is_cli_command(Some(word)), "{word}");
        assert!(cli::usage().contains(&format!("bilterm {word} ")), "usage must document `{word}`");
    }
    for word in ["", "ctl", "ls", "run", "Agent", "agents", "--agent", "pane ", "coord"] {
        assert!(!cli::is_cli_command(Some(word)), "{word:?} must not be routed to the CLI");
    }
    assert!(!cli::is_cli_command(None));
}

#[test]
fn coords_subcommand_validates_before_any_rpc_is_sent() {
    let ok = parse(&["coords", "resolve", "d0:w1:main:y2"]).unwrap();
    assert_eq!(ok.method, "coords.resolve");
    assert_eq!(ok.params.get("coord").and_then(JsonValue::as_str), Some("d0:w1:main:y2"));
    assert!(ok.poll.is_none());

    assert!(parse(&["coords", "resolve"]).unwrap_err().contains("needs a COORD"));
    assert!(parse(&["coords", "resolve", "((unbalanced"]).unwrap_err().contains("invalid coord"));
    assert!(parse(&["coords", "bogus"]).unwrap_err().contains("unknown coords subcommand 'bogus'"));
    assert!(parse(&["coords"]).unwrap_err().contains("Usage:"));

    let fmt = parse(&["coords", "format", "--pane", "3", "--style", "compact"]).unwrap();
    assert_eq!(fmt.method, "coords.format");
    assert_eq!(fmt.params.get("pane_id").and_then(JsonValue::as_i64), Some(3));
    assert_eq!(fmt.params.get("style").and_then(JsonValue::as_str), Some("compact"));
    assert_eq!(parse(&["coords", "format"]).unwrap().params.to_json_string(), "{}");
    assert!(parse(&["coords", "format", "--pane", "three"]).unwrap_err().contains("numeric pane id"));
    assert!(parse(&["coords", "format", "--style", "fancy"]).unwrap_err().contains("unknown style 'fancy'"));
    assert!(parse(&["coords", "format", "--pane"]).unwrap_err().contains("--pane needs a value"));
}

#[test]
fn option_syntax_equals_form_literal_separator_and_unknown_options() {
    // `--flag=value` is the same as `--flag value`.
    let a = parse(&["coords", "format", "--style=verbose"]).unwrap();
    let b = parse(&["coords", "format", "--style", "verbose"]).unwrap();
    assert_eq!(a, b);

    // After `--` everything is text, even things that look like options.
    let sent = parse(&["pane", "send", "p2", "--", "--force", "-rf", "--"]).unwrap();
    assert_eq!(sent.method, "pane.send");
    assert_eq!(sent.params.get("text").and_then(JsonValue::as_str), Some("--force -rf --"));
    assert!(sent.params.get("enter").is_none());
    let entered = parse(&["pane", "send", "p2", "ls", "-la", "--enter"]).unwrap();
    assert_eq!(entered.params.get("text").and_then(JsonValue::as_str), Some("ls -la"));
    assert_eq!(entered.params.get("enter").and_then(JsonValue::as_bool), Some(true));

    for (bad, needle) in [
        (&["pane", "list", "--verbose"][..], "unknown option --verbose"),
        (&["agent", "list", "--"][..], ""), // a lone separator is fine
        (&["frobnicate"][..], "unknown command 'frobnicate'"),
        (&[][..], "Usage:"),
        (&["pane", "split", "--direction"][..], "--direction needs a value"),
    ] {
        match parse(bad) {
            Err(e) => assert!(!needle.is_empty() && e.contains(needle), "{bad:?}: {e}"),
            Ok(_) => assert!(needle.is_empty(), "{bad:?} must fail with {needle:?}"),
        }
    }
}

#[test]
fn pane_split_direction_can_be_positional_or_an_option() {
    for (argv, dir, selector) in [
        (&["pane", "split"][..], "h", None),
        (&["pane", "split", "v"][..], "v", None),
        (&["pane", "split", "vertical", "w1:p2"][..], "vertical", Some("w1:p2")),
        (&["pane", "split", "--direction", "v", "p3"][..], "v", Some("p3")),
        (&["pane", "split", "p3"][..], "h", Some("p3")),
    ] {
        let call = parse(argv).unwrap();
        assert_eq!(call.method, "pane.split", "{argv:?}");
        assert_eq!(call.params.get("direction").and_then(JsonValue::as_str), Some(dir), "{argv:?}");
        assert_eq!(call.params.get("selector").and_then(JsonValue::as_str), selector, "{argv:?}");
    }
}

#[test]
fn calls_made_from_inside_a_pane_are_attributed_to_it() {
    let inside = CliEnv { pane_id: Some(7), socket: None };
    let call = cli::parse(&args(&["pane", "list"]), &inside).unwrap();
    assert_eq!(call.params.get("from_pane").and_then(JsonValue::as_usize), Some(7));
    let outside = cli::parse(&args(&["pane", "list"]), &CliEnv::default()).unwrap();
    assert!(outside.params.get("from_pane").is_none());
    // Workspace calls carry only what they need.
    let ws = cli::parse(&args(&["workspace", "select", "w2"]), &inside).unwrap();
    assert_eq!(ws.params.to_json_string(), "{\"name\":\"w2\"}");
}

#[test]
fn endpoint_resolution_prefers_the_environment_and_normalises_its_form() {
    let env = |s: &str| CliEnv { pane_id: None, socket: Some(s.to_string()) };
    let empty = tempfile::tempdir().unwrap();
    // A bare absolute path is a Unix socket; an explicit scheme or host:port is passed through.
    assert_eq!(cli::resolve_endpoint(&env("/run/x/bilterm.sock"), None).as_deref(), Some("unix:/run/x/bilterm.sock"));
    assert_eq!(cli::resolve_endpoint(&env("unix:/run/x.sock"), None).as_deref(), Some("unix:/run/x.sock"));
    assert_eq!(cli::resolve_endpoint(&env("127.0.0.1:7777"), Some(empty.path())).as_deref(), Some("127.0.0.1:7777"));
    // No environment and nothing registered: the caller must fall back to its default.
    assert_eq!(cli::resolve_endpoint(&CliEnv::default(), None), None);
    assert_eq!(cli::resolve_endpoint(&CliEnv::default(), Some(empty.path())), None);
    assert_eq!(cli::resolve_endpoint(&CliEnv::default(), Some(Path::new("definitely/not/a/dir"))), None);
}
