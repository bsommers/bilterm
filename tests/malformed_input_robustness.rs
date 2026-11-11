//! Parsers and protocol handlers fed malformed, hostile and randomly mutated input.
//!
//! Every text entry point an untrusted peer, config file or keyboard can reach must reject
//! garbage with an error value, never with a panic, a hang or a silent acceptance. Inputs come
//! from fixed-seed PRNG streams (`common::Rng`) so a failure is reproducible on every OS.

mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

use bilterm::api::selector::parse_selector;
use bilterm::api::static_serve::{sanitize_request_path, validate_cors_origin};
use bilterm::api::{AutomationControlPlane, JsonValue, Playbook, parse_ctl_args, parse_key_chord_str, validate_http_bind};
use bilterm::colon_command::parse_colon_command;
use bilterm::config::BiltermConfig;
use bilterm::input::chord::KeyChord;
use bilterm::swarm::SwarmPlan;
use bilterm::tmux::TmuxEnv;
use bilterm::tmux::hydrate::parse_block_header;
use bilterm::tmux::splitter::ControlLineSplitter;
use bilterm::ui::theme_import::parse_itermcolors_xml;
use bilterm::vt::codec::base64_decode;
use bilterm::wm::coordinates::TargetCoord;
use common::Rng;

/// Tokens that are meaningful to at least one of the parsers under test.
const HOSTILE: &[&str] = &[
    "", " ", "\t", "\n", "\r\n", "\0", "\u{feff}", "\u{202e}", "\u{1f680}", "e\u{301}", "\u{fffd}",
    "0", "-0", "1", "-1", "9", "255", "256", "65535", "65536", "4294967296", "18446744073709551616", "1e400", "NaN",
    "0x10", "00", ".", "..", "...", "/", "//", "\\", "/etc/passwd", "C:\\Windows", "../..", "~", "%00", "%2e%2e", "%zz", "%",
    ":", "::", ";", ",", "=", "==", "'", "\"", "`", "$(x)", "${x}", "|", "&", "&&", ">", "<", "*", "?", "#", "@", "!", "[", "]", "{", "}",
    "(", ")", "--", "---", "-h", "--help", "--force", "--squash", "true", "false", "null", "on", "off", "yes", "no",
    "h", "v", "p1", "w1:p1", "w0", "d0:w1:main:y2", "@idle", "@claude", "@nope", "#2", "%3", "main",
    "ctrl", "ctrl+", "+c", "ctrl+shift+alt+super+f24", "F0", "F99", "enter", "esc",
    "%begin", "%end", "%error", "%output", "%begin 1 2 3", "%end 1 2", "unix:", "unix:/x", "127.0.0.1:0", "[::1]:1", "localhost:99999",
    "http://", "https://a.b:1", "http://a/b", "null", "file://", "*",
    "a.b.c", "a..b", ".a", "a.", "theme", "keys.profile", "agents.watchdog", "scrollback.lines",
];

fn token(rng: &mut Rng) -> String {
    match rng.below(8) {
        0 => {
            let n = rng.below(24);
            String::from_utf8_lossy(&rng.bytes(n)).into_owned()
        }
        1 => {
            let max = if rng.chance(30) { 70_000 } else { 300 };
            "A".repeat(rng.below(max))
        }
        2 => rng.next_u64().to_string(),
        _ => (*rng.pick(HOSTILE)).to_string(),
    }
}

fn line(rng: &mut Rng, sep: &str) -> String {
    (0..rng.below(7)).map(|_| token(rng)).collect::<Vec<_>>().join(sep)
}

/// Runs `f(input)` for `cases` PRNG inputs per seed; fails naming the exact input on a panic or
/// when the whole run exceeds a generous time budget.
fn never_panics(name: &str, seeds: &[u64], cases: usize, mut make: impl FnMut(&mut Rng) -> String, f: impl Fn(&str)) {
    let started = Instant::now();
    for &seed in seeds {
        let mut rng = Rng::new(seed);
        for case in 0..cases {
            let input = make(&mut rng);
            if catch_unwind(AssertUnwindSafe(|| f(&input))).is_err() {
                let shown: String = input.chars().take(200).collect();
                panic!("{name}: panicked on seed={seed} case={case} len={} input={shown:?}", input.len());
            }
        }
    }
    assert!(started.elapsed() < Duration::from_secs(60), "{name}: took {:?}", started.elapsed());
}

// ---------------------------------------------------------------- JSON

fn gen_json(rng: &mut Rng, depth: usize) -> JsonValue {
    match rng.below(if depth > 4 { 5 } else { 7 }) {
        0 => JsonValue::Null,
        1 => JsonValue::Bool(rng.chance(2)),
        2 => JsonValue::Number((rng.next_u64() >> rng.below(64)) as i64 * if rng.chance(2) { -1 } else { 1 }),
        3 | 4 => JsonValue::String(token(rng)),
        5 => JsonValue::Array((0..rng.below(5)).map(|_| gen_json(rng, depth + 1)).collect()),
        _ => JsonValue::obj((0..rng.below(5)).map(|_| (token(rng), gen_json(rng, depth + 1))).collect::<Vec<_>>()),
    }
}

#[test]
fn json_round_trips_every_generated_value() {
    for seed in [1u64, 2, 3, 4, 5, 6, 7, 8] {
        let mut rng = Rng::new(seed);
        for case in 0..150 {
            let value = gen_json(&mut rng, 0);
            let text = value.to_json_string();
            let back = JsonValue::parse(&text).unwrap_or_else(|| panic!("seed={seed} case={case}: own output does not parse: {text:?}"));
            assert_eq!(back, value, "seed={seed} case={case}: round trip changed the value: {text:?}");
        }
    }
}

#[test]
fn json_rejects_malformed_documents_without_panicking() {
    for bad in [
        "", " ", "{", "}", "[", "]", "[1,]", "[,1]", "{\"a\"}", "{\"a\":}", "{:1}", "{\"a\":1,}", "{\"a\":1 \"b\":2}", "{a:1}",
        "\"unterminated", "\"bad \\q escape\"", "\"\\u12\"", "\"\\ud800\"", "\"raw \n newline\"", "nul", "tru", "True", "NULL",
        "01", "1.", ".5", "+1", "--1", "1e", "0x10", "1 2", "[1] [2]", "{} x", "\u{feff}{}", "'single'", "[1;2]",
    ] {
        assert_eq!(JsonValue::parse(bad), None, "must be rejected: {bad:?}");
    }
    never_panics(
        "JsonValue::parse",
        &[11, 12, 13, 14],
        400,
        |rng| {
            // Mutate a valid document: the interesting bugs live one byte away from valid input.
            let mut bytes = gen_json(rng, 0).to_json_string().into_bytes();
            for _ in 0..1 + rng.below(4) {
                if bytes.is_empty() {
                    break;
                }
                let i = rng.below(bytes.len());
                match rng.below(4) {
                    0 => bytes[i] = *rng.pick(b"{}[]\",:\\ \n0e-.tfnu"),
                    1 => bytes.truncate(i),
                    2 => bytes.insert(i, rng.byte() & 0x7f),
                    _ => {
                        bytes.remove(i);
                    }
                }
            }
            String::from_utf8_lossy(&bytes).into_owned()
        },
        |s| {
            if let Some(v) = JsonValue::parse(s) {
                // Whatever is accepted must survive its own serialisation.
                assert_eq!(JsonValue::parse(&v.to_json_string()), Some(v));
            }
        },
    );
}

#[test]
fn json_nesting_bombs_are_rejected_not_stack_overflowed() {
    for (open, close) in [("[", "]"), ("{\"a\":", "}")] {
        let ok = format!("{}1{}", open.repeat(100), close.repeat(100));
        assert!(JsonValue::parse(&ok).is_some(), "100 levels are legitimate");
        for depth in [200usize, 5_000, 200_000] {
            let bomb = format!("{}1{}", open.repeat(depth), close.repeat(depth));
            assert_eq!(JsonValue::parse(&bomb), None, "depth {depth} must be refused");
            assert_eq!(JsonValue::parse(&open.repeat(depth)), None, "unterminated depth {depth}");
        }
    }
}

// ---------------------------------------------------------------- JSON-RPC / MCP dispatch

#[test]
fn rpc_dispatch_survives_wrongly_typed_and_hostile_params_for_every_method() {
    let snapshots = tempfile::tempdir().unwrap();
    let mut cp = AutomationControlPlane::new_headless(24, 80).with_snapshot_dir(snapshots.path().to_path_buf());
    // Methods that deliberately reach outside the process (spawn daemons, ssh, browsers, audio,
    // real git worktrees) are exercised by their own suites, not by a fuzzer.
    let skip_prefix = ["daemon.", "remote.", "fleet.", "browser.", "video.", "voice.", "web.", "swarm.", "playbook.", "session."];
    let keys = ["pane_id", "command", "data", "keys", "selector", "name", "path", "direction", "text", "kind", "query", "id", "op", "key", "value", "coord", "style", "slug", "node", "to", "from", "body", "limit", "action", "size"];
    let mut rng = Rng::new(0xD15_7A7C);
    let mut calls = 0;
    for method in bilterm::api::RPC_METHODS {
        if skip_prefix.iter().any(|p| method.starts_with(p)) {
            continue;
        }
        for _ in 0..12 {
            let params = match rng.below(6) {
                0 => JsonValue::Null,
                1 => JsonValue::Array(vec![JsonValue::Number(1)]),
                2 => JsonValue::String(token(&mut rng)),
                _ => JsonValue::obj(
                    (0..1 + rng.below(4))
                        .map(|_| {
                            let k = *rng.pick(&keys);
                            let v = match rng.below(5) {
                                0 => JsonValue::Number(*rng.pick(&[-1i64, 0, 1, 2, 999_999, i64::MAX, i64::MIN])),
                                1 => JsonValue::Bool(rng.chance(2)),
                                2 => JsonValue::Null,
                                3 => JsonValue::Array(vec![]),
                                _ => JsonValue::String((*rng.pick(HOSTILE)).to_string()),
                            };
                            (k, v)
                        })
                        .collect::<Vec<_>>(),
                ),
            };
            let shown = params.to_json_string();
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                let _ = cp.dispatch_method(method, &params);
            }));
            assert!(outcome.is_ok(), "{method} panicked on params {shown}");
            calls += 1;
        }
    }
    assert!(calls > 500, "the fuzzer must actually reach the dispatch table (made {calls} calls)");
    // The plane is still coherent afterwards.
    assert!(cp.dispatch_method("workspace.status", &JsonValue::Null).is_ok());
    assert!(cp.dispatch_method("no.such.method", &JsonValue::Null).is_err());
}

#[test]
fn mcp_line_handler_survives_garbage_lines() {
    let mut cp = AutomationControlPlane::new_headless(24, 80);
    for line in [
        "", "{", "[]", "null", "42", "\"str\"", "{}", "{\"id\":1}", "{\"method\":\"ping\"}", "{\"id\":1,\"method\":1}",
        "{\"id\":1,\"method\":\"tools/call\"}", "{\"id\":1,\"method\":\"tools/call\",\"params\":[]}",
        "{\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":7}}",
        "{\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"bilterm_find\",\"arguments\":{\"kind\":3}}}",
        "{\"id\":1,\"method\":\"resources/read\",\"params\":{\"uri\":\"../../etc/passwd\"}}",
        "{\"id\":1,\"method\":\"resources/read\",\"params\":{}}",
        "{\"id\":{\"nested\":[]},\"method\":\"ping\"}",
    ] {
        let reply = catch_unwind(AssertUnwindSafe(|| cp.handle_mcp_line(line)));
        let reply = reply.unwrap_or_else(|_| panic!("handle_mcp_line panicked on {line:?}"));
        if let Some(r) = reply {
            assert!(JsonValue::parse(&r).is_some(), "reply to {line:?} is not JSON: {r}");
        }
    }
    let mut rng = Rng::new(77);
    for _ in 0..300 {
        let l = format!("{{\"jsonrpc\":\"2.0\",\"id\":{},\"method\":{:?},\"params\":{}}}", rng.below(9), token(&mut rng), gen_json(&mut rng, 3).to_json_string());
        assert!(catch_unwind(AssertUnwindSafe(|| cp.handle_mcp_line(&l))).is_ok(), "panicked on {l:?}");
    }
}

// ---------------------------------------------------------------- colon commands

#[test]
fn colon_command_parser_never_panics_on_any_registered_head_with_hostile_arguments() {
    let heads: Vec<&'static str> = bilterm::command_registry::catalog().iter().map(|c| c.name).collect();
    assert!(heads.len() > 40, "the registry should list the colon commands (got {})", heads.len());
    never_panics(
        "parse_colon_command",
        &[21, 22, 23, 24, 25],
        1200,
        |rng| match rng.below(10) {
            0 => line(rng, " "),
            1 => format!(":{}", line(rng, " ")),
            _ => format!("{} {}", rng.pick(&heads), line(rng, " ")),
        },
        |s| {
            let _ = parse_colon_command(s);
        },
    );
    // Things that are not commands at all are refused or ignored, never accepted as an action.
    for not_a_command in [":", ": ", ":\u{1f680}", ":definitely-not-a-command", ":split\u{0}h"] {
        assert!(!matches!(parse_colon_command(not_a_command), Some(Ok(_))), "{not_a_command:?} must not become an Action");
    }
}

#[test]
fn every_registered_example_parses() {
    // The palette advertises these lines to users; a registry entry that does not parse is a lie.
    let mut checked = 0;
    for info in bilterm::command_registry::catalog() {
        for ex in &info.examples {
            if ex.ends_with(' ') || ex.contains('<') || ex.contains('[') {
                continue; // fill-in templates (`:ssh `, `:theme <name>`)
            }
            checked += 1;
            assert!(!matches!(parse_colon_command(ex), Some(Err(_)) | None), "advertised example does not parse: {ex:?}");
        }
    }
    assert!(checked > 20, "expected to check many ready-to-run examples (checked {checked})");
}

// ---------------------------------------------------------------- selectors, coordinates, chords

#[test]
fn selector_coordinate_and_chord_parsers_never_panic() {
    let mk = |rng: &mut Rng| if rng.chance(3) { line(rng, ":") } else { token(rng) };
    never_panics("parse_selector", &[31, 32, 33], 1500, mk, |s| {
        let _ = parse_selector(s);
    });
    never_panics("TargetCoord::parse", &[34, 35, 36], 1500, mk, |s| {
        let _ = TargetCoord::parse(s);
    });
    never_panics("KeyChord::parse", &[37, 38], 1500, |rng| line(rng, "+"), |s| {
        let _ = KeyChord::parse(s);
        let _ = parse_key_chord_str(s);
    });
    for bad in ["", " ", "w", "p", "w1:", ":p1", "w-1:p1", "w1:p1:p2", "@", "@bad!name", "@a b", "99999999999999999999"] {
        assert!(parse_selector(bad).is_err(), "selector {bad:?} must be rejected");
    }
    for bad in ["", "   ", "(", ")", "(a))", "((a)", "(x(a,b))", "(d0:(a,b)", ")("] {
        assert!(TargetCoord::parse(bad).is_err(), "coordinate {bad:?} must be rejected");
    }
    for bad in ["", "  ", "ctrl+", "bogusmod+c", "ctrl+ctrl+c", "primary+primary+c", "ctrl+shift+"] {
        assert!(KeyChord::parse(bad).is_err(), "chord {bad:?} must be rejected");
    }
}

// ---------------------------------------------------------------- HTTP surface

#[test]
fn static_path_sanitizer_never_lets_a_traversal_through() {
    never_panics(
        "sanitize_request_path",
        &[41, 42, 43, 44],
        2000,
        |rng| format!("/{}", line(rng, "/")),
        |s| {
            if let Ok(segments) = sanitize_request_path(s) {
                for seg in &segments {
                    assert!(seg != "." && seg != ".." && !seg.is_empty(), "{s:?} -> dot/empty segment {seg:?}");
                    assert!(!seg.contains(['/', '\\', ':', '\0']), "{s:?} -> separator in segment {seg:?}");
                    assert!(!seg.bytes().any(|b| b < 0x20 || b == 0x7f), "{s:?} -> control byte in {seg:?}");
                }
            }
        },
    );
    for attack in [
        "/../etc/passwd", "/..", "/a/../../b", "/%2e%2e/secret", "/%2E%2E%2Fsecret", "/a/%2e%2e", "/..%2f..%2f", "/a\\..\\b", "/%5c..%5c",
        "/C:/Windows", "/c%3a/x", "/a%00.html", "/%0d%0aSet-Cookie:x", "/a/./b", "/%2e", "/\u{7f}", "/%",  "/%z1", "/%f",
    ] {
        assert!(sanitize_request_path(attack).is_err(), "{attack:?} must be refused");
    }
    assert_eq!(sanitize_request_path("/assets/app.js?v=1#frag"), Ok(vec!["assets".to_string(), "app.js".to_string()]));
    assert_eq!(sanitize_request_path("//a///b/"), Ok(vec!["a".to_string(), "b".to_string()]));
}

#[test]
fn http_bind_and_cors_origin_validation_reject_everything_but_the_safe_shapes() {
    for bad in ["", "0.0.0.0:8080", "192.168.1.1:80", "[::]:80", "example.com:80", "127.0.0.1", "127.0.0.1:", "127.0.0.1:99999", ":80", "localhost"] {
        assert!(validate_http_bind(bad).is_err(), "bind {bad:?} must be refused (loopback only)");
    }
    assert!(validate_http_bind("127.0.0.1:0").is_ok());
    for bad in ["", "*", "null", "example.com", "http://", "http://a/b", "http://a?x", "http://a#x", "ftp://a", "http://a b", "http://a\r\nX: y", "http://*", "javascript:alert(1)"] {
        assert!(validate_cors_origin(bad).is_err(), "origin {bad:?} must be refused");
    }
    assert!(validate_cors_origin("http://localhost:5173").is_ok());
    never_panics("validate_http_bind/cors", &[51, 52], 1500, |rng| line(rng, ":"), |s| {
        let _ = validate_http_bind(s);
        let _ = validate_cors_origin(s);
    });
}

#[test]
fn ctl_argument_parsing_defaults_and_tolerates_bad_json() {
    let args = |v: &[&str]| v.iter().map(ToString::to_string).collect::<Vec<_>>();
    let (ep, method, params) = parse_ctl_args(&args(&[]), "unix:/default.sock");
    assert_eq!((ep.as_str(), method.as_str()), ("unix:/default.sock", "workspace.status"));
    assert_eq!(params.to_json_string(), "{}");
    let (ep, method, params) = parse_ctl_args(&args(&["127.0.0.1:9", "pane.read", "{\"pane_id\":2}"]), "unix:/d");
    assert_eq!((ep.as_str(), method.as_str()), ("127.0.0.1:9", "pane.read"));
    assert_eq!(params.get("pane_id").and_then(JsonValue::as_usize), Some(2));
    // Unparseable params degrade to `{}` instead of aborting the client.
    let (_, method, params) = parse_ctl_args(&args(&["pane.read", "{not json"]), "unix:/d");
    assert_eq!((method.as_str(), params.to_json_string().as_str()), ("pane.read", "{}"));
    never_panics("parse_ctl_args", &[53], 800, |rng| line(rng, "\u{1}"), |s| {
        let v: Vec<String> = s.split('\u{1}').map(ToString::to_string).collect();
        let _ = parse_ctl_args(&v, "unix:/d");
    });
}

// ---------------------------------------------------------------- tmux control mode

#[test]
fn tmux_control_parsers_never_panic_and_bound_their_buffers() {
    never_panics("parse_block_header", &[61, 62], 1500, |rng| line(rng, " "), |s| {
        let _ = parse_block_header(s);
        let _ = TmuxEnv::from_env_str(s);
    });
    for bad in ["", "%begin", "%begin 1 2", "%begin a b c", "%begin 1 2 3 4", "%begin -1 2 3", "begin 1 2 3", "%BEGIN 1 2 3", "%begin  1 2 3"] {
        assert!(parse_block_header(bad).is_none(), "{bad:?} is not a block header");
    }
    assert!(parse_block_header("%end 1700000000 42 1\r\n").is_some());
    for blank in ["", " ", "\t\n"] {
        assert!(TmuxEnv::from_env_str(blank).is_none(), "TMUX={blank:?} must be rejected");
    }
    // The parser is lenient by design: a non-numeric pid is dropped, a missing session id is "0".
    let lenient = TmuxEnv::from_env_str("/run/tmux/default,not-a-pid").expect("socket path alone is enough");
    assert_eq!((lenient.socket_path.as_str(), lenient.server_pid, lenient.session_id.as_str()), ("/run/tmux/default", None, "0"));

    // The splitter sees raw remote bytes: a peer that never sends a newline must not grow it forever.
    let mut rng = Rng::new(63);
    let mut sp = ControlLineSplitter::new();
    let mut peak = 0;
    for _ in 0..400 {
        let chunk = match rng.below(4) {
            0 => {
                let n = rng.below(512);
                rng.bytes(n)
            }
            1 => vec![b'%'; 4096],
            2 => b"\x1bP1000p%begin 1 2 3\n%output %1 hi\n%end 1 2 3\n".to_vec(),
            _ => vec![b'x'; 16 * 1024], // no newline at all
        };
        let _ = sp.push(&chunk);
        peak = peak.max(sp.buffered_len());
    }
    let _ = sp.flush_partial();
    assert!(peak < 4 * 1024 * 1024, "splitter buffered {peak} bytes of newline-free input");
}

// ---------------------------------------------------------------- config, playbooks, plans, themes

#[test]
fn config_playbook_swarm_and_theme_loaders_reject_garbage_without_panicking() {
    let toml_ish = |rng: &mut Rng| {
        (0..rng.below(8))
            .map(|_| match rng.below(5) {
                0 => format!("[{}]", token(rng)),
                1 => format!("[[{}]]", token(rng)),
                2 => format!("{} = {}", rng.pick(HOSTILE), rng.pick(HOSTILE)),
                3 => format!("{} = \"{}\"", rng.pick(&["theme", "font_size", "history_session", "id", "prompt", "name", "steps", "depends_on"]), token(rng)),
                _ => token(rng),
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    never_panics("BiltermConfig::from_toml_str", &[71, 72, 73], 600, toml_ish, |s| {
        let _ = BiltermConfig::from_toml_str(s);
    });
    never_panics("Playbook::from_toml_str", &[74, 75], 600, toml_ish, |s| {
        let _ = Playbook::from_toml_str(s);
    });
    never_panics("SwarmPlan::parse", &[76, 77], 600, toml_ish, |s| {
        let _ = SwarmPlan::parse(s);
    });
    never_panics("parse_itermcolors_xml / parse_ini_theme / base64", &[78, 79], 600, |rng| line(rng, "<key>"), |s| {
        let _ = parse_itermcolors_xml("fuzz", s);
        let _ = bilterm::config::parse_ini_theme(s);
        let _ = base64_decode(s);
    });

    for bad in ["theme = [", "font_size_pt = \"big\"", "[keys]\nprofile = 7", "= 1", "[a]\n[a]", "theme = 1\ntheme = 2"] {
        assert!(BiltermConfig::from_toml_str(bad).is_err(), "config {bad:?} must be rejected");
    }
    // A plan with a dependency cycle, a duplicate id or a dangling dependency is refused up front.
    for bad in [
        "[[node]]\nid='a'\nprompt='p'\ndepends_on=['a']\n",
        "[[node]]\nid='a'\nprompt='p'\ndepends_on=['b']\n[[node]]\nid='b'\nprompt='q'\ndepends_on=['a']\n",
        "[[node]]\nid='a'\nprompt='p'\n[[node]]\nid='a'\nprompt='q'\n",
        "[[node]]\nid='a'\nprompt='p'\ndepends_on=['ghost']\n",
        "[[node]]\nid='../escape'\nprompt='p'\n",
        "",
    ] {
        assert!(SwarmPlan::parse(bad).is_err(), "swarm plan must be rejected: {bad:?}");
    }
}
