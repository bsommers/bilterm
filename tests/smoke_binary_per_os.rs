//! Per-OS product smoke test: launches the real `bilterm` binary headlessly on every CI cell
//! (Linux / macOS / Windows, x86_64 / ARM64) so each cell proves the product *starts*, not merely
//! that it compiles. Everything runs in an isolated HOME / XDG / runtime tree (`common::IsolatedEnv`)
//! and never talks to a running instance.
//!
//! Layers, cheapest first:
//!   1. `--version` / `--help` and the other print-and-exit switches;
//!   2. CLI argument errors (`agent` / `pane` / `coords` / `ctl`) with exit codes;
//!   3. a scripted MCP session over stdio (`bilterm --mcp`), no sockets, no PTY;
//!   4. a scripted RPC session against a real headless daemon on loopback TCP, driven through
//!      `bilterm ctl` and the `bilterm pane|workspace|coords` CLI;
//!   5. a real PTY child through the product's PTY layer (ConPTY on Windows).

mod common;

use std::io::{BufRead, BufReader, Write};
use std::process::Stdio;
use std::sync::mpsc;
use std::time::Duration;

use bilterm::api::JsonValue;
use common::{IsolatedEnv, KillOnDrop, PROCESS_DEADLINE, stderr_of, stdout_of};

const VERSION: &str = env!("BILTERM_VERSION");

fn result_of(resp: &str) -> JsonValue {
    let v = JsonValue::parse(resp.trim()).unwrap_or_else(|| panic!("response is not JSON: {resp}"));
    assert!(v.get("error").is_none(), "RPC error: {resp}");
    v.get("result").cloned().unwrap_or_else(|| panic!("response has no result: {resp}"))
}

// ---------------------------------------------------------------- 1. print-and-exit switches

#[test]
fn version_switches_print_the_crate_version() {
    let env = IsolatedEnv::new();
    for sw in ["--version", "-V", "-v"] {
        let out = env.run(&[sw]);
        assert!(out.status.success(), "{sw}: {}", stderr_of(&out));
        assert_eq!(stdout_of(&out).trim(), format!("bilterm {VERSION}"), "{sw}");
    }
}

#[test]
fn help_lists_every_entry_point() {
    let env = IsolatedEnv::new();
    for sw in ["--help", "-h"] {
        let out = env.run(&[sw]);
        assert!(out.status.success(), "{sw}: {}", stderr_of(&out));
        let text = stdout_of(&out);
        for needle in [
            VERSION,
            "Usage:",
            "bilterm ctl",
            "bilterm agent|pane|workspace|coords",
            "bilterm run <PLAYBOOK.toml>",
            "--mcp",
            "--headless",
            "--token-file",
            "--version",
        ] {
            assert!(text.contains(needle), "{sw} output lacks {needle:?}:\n{text}");
        }
    }
}

#[test]
fn generator_switches_emit_their_artifacts() {
    let env = IsolatedEnv::new();
    for shell in ["bash", "zsh", "fish"] {
        let out = env.run(&["--completions", shell]);
        assert!(out.status.success() && stdout_of(&out).contains("bilterm"), "completions {shell}");
    }
    let out = env.run(&["--completions", "tcsh"]);
    assert!(stderr_of(&out).contains("Supported shells"), "{}", stderr_of(&out));

    let man = env.run(&["--man"]);
    assert!(man.status.success() && stdout_of(&man).contains(".TH"), "manpage must be roff");

    let desktop = env.run(&["--desktop-file"]);
    assert!(stdout_of(&desktop).contains("[Desktop Entry]"));

    let ppm = env.dir("tmp").join("workspace.ppm");
    let dump = env.run(&["--gui-dump", ppm.to_str().unwrap()]);
    assert!(dump.status.success(), "{}", stderr_of(&dump));
    let bytes = std::fs::read(&ppm).expect("gui dump written");
    assert!(bytes.starts_with(b"P6") && bytes.len() > 1024, "PPM header + pixels expected");

    let plot = env.run(&["plot", "bar", "1,2,3"]);
    assert!(plot.status.success() && stdout_of(&plot).contains("bilterm plot (bar"), "{}", stdout_of(&plot));
}

#[test]
fn ls_in_a_fresh_profile_reports_nothing_and_touches_no_real_state() {
    let env = IsolatedEnv::new();
    let out = env.run(&["ls"]);
    assert!(out.status.success(), "{}", stderr_of(&out));
    let text = stdout_of(&out);
    assert!(text.contains("Active Background Daemons (0)"), "{text}");
    assert!(text.contains("Saved Workspace Sessions (0)"), "{text}");
}

// ---------------------------------------------------------------- 2. argument errors

#[test]
fn cli_subcommand_mistakes_exit_2_with_usage() {
    let env = IsolatedEnv::new();
    for (args, needle) in [
        (&["agent"][..], "Usage:"),
        (&["coords"][..], "Usage:"),
        (&["pane"][..], "Usage:"),
        (&["workspace"][..], "Usage:"),
        (&["pane", "bogus"][..], "unknown pane subcommand 'bogus'"),
        (&["workspace", "bogus"][..], "unknown workspace subcommand 'bogus'"),
        (&["agent", "list", "--nope"][..], "unknown option --nope"),
        (&["agent", "wait", "@idle", "--timeout-ms"][..], "--timeout-ms needs a value"),
        (&["pane", "focus"][..], "pane focus needs a SELECTOR"),
        (&["pane", "send", "p1"][..], "pane send needs SELECTOR and TEXT"),
        (&["workspace", "new"][..], "workspace new needs a NAME"),
    ] {
        let out = env.run(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: stdout {} stderr {}", stdout_of(&out), stderr_of(&out));
        assert!(stderr_of(&out).contains(needle), "{args:?} stderr lacks {needle:?}: {}", stderr_of(&out));
        assert!(stdout_of(&out).is_empty(), "{args:?} must not print a result");
    }
}

#[test]
fn cli_and_ctl_fail_fast_when_no_instance_is_listening() {
    let env = IsolatedEnv::new();
    // A loopback port nothing listens on: an explicit TCP endpoint never auto-starts a daemon.
    let dead = format!("127.0.0.1:{}", common::free_loopback_port());

    let mut cmd = env.bilterm(&["coords", "resolve", "d0:w1:main:y2"]);
    cmd.env("BILTERM_SOCKET", &dead);
    let out = common::run_with_deadline(cmd, PROCESS_DEADLINE);
    assert_eq!(out.status.code(), Some(1), "{}", stderr_of(&out));
    assert!(stderr_of(&out).contains("cannot reach a running bilterm instance"), "{}", stderr_of(&out));

    let out = env.run(&["ctl", &dead, "workspace.status"]);
    assert!(!out.status.success() && stdout_of(&out).is_empty(), "ctl must fail: {}", stdout_of(&out));
}

#[test]
fn missing_token_file_and_missing_playbook_are_reported_not_ignored() {
    let env = IsolatedEnv::new();
    let missing = env.dir("tmp").join("no-such.token");
    let out = env.run(&["--token-file", missing.to_str().unwrap(), "ls"]);
    assert!(!out.status.success());
    assert!(stderr_of(&out).contains("token file"), "{}", stderr_of(&out));

    let out = env.run(&["run", env.dir("tmp").join("no-such-playbook.toml").to_str().unwrap()]);
    assert!(!out.status.success(), "a missing playbook must fail");

    let bad = env.dir("tmp").join("bad.toml");
    std::fs::write(&bad, "this is = = not toml [[[").unwrap();
    let out = env.run(&["--playbook", bad.to_str().unwrap()]);
    assert!(!out.status.success(), "a malformed playbook must fail");
    assert!(!stdout_of(&out).contains("\"passed\":true"));
}

/// Current behaviour, pinned so a change is deliberate: an unrecognised top-level switch is not
/// rejected, it falls through to the interactive launch, which fails without a terminal.
#[test]
#[cfg_attr(not(unix), ignore = "non-TTY interactive launch is only characterised on Unix so far")]
fn unknown_top_level_switch_without_a_tty_exits_instead_of_hanging() {
    let env = IsolatedEnv::new();
    let out = env.run(&["--definitely-not-a-bilterm-switch"]);
    assert!(!out.status.success(), "no TTY: the interactive launch cannot succeed");
    assert!(!stdout_of(&out).contains("Usage:"), "unknown switches do not print usage today");
}

// ---------------------------------------------------------------- 3. MCP over stdio

struct McpSession {
    child: KillOnDrop,
    stdin: std::process::ChildStdin,
    lines: mpsc::Receiver<String>,
    next_id: u64,
}

impl McpSession {
    fn start(env: &IsolatedEnv) -> Self {
        let mut cmd = env.bilterm(&["--mcp", "--snapshot-dir", env.dir("tmp").to_str().unwrap()]);
        let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().expect("spawn bilterm --mcp");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Self { child: KillOnDrop(child), stdin, lines, next_id: 1 }
    }

    fn raw(&mut self, line: &str) -> String {
        writeln!(self.stdin, "{line}").expect("write request");
        self.stdin.flush().expect("flush request");
        self.lines.recv_timeout(PROCESS_DEADLINE).unwrap_or_else(|_| panic!("no MCP response to: {line}"))
    }

    fn request(&mut self, method: &str, params: &str) -> String {
        let id = self.next_id;
        self.next_id += 1;
        self.raw(&format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#))
    }

    fn tool(&mut self, name: &str, arguments: &str) -> String {
        self.request("tools/call", &format!(r#"{{"name":"{name}","arguments":{arguments}}}"#))
    }
}

#[test]
fn mcp_stdio_scripted_session() {
    let env = IsolatedEnv::new();
    let mut mcp = McpSession::start(&env);

    let init = result_of(&mcp.request("initialize", "{}"));
    let info = init.get("serverInfo").expect("serverInfo");
    assert_eq!(info.get("name").and_then(JsonValue::as_str), Some("bilterm-mcp"));
    assert_eq!(info.get("version").and_then(JsonValue::as_str), Some(VERSION));

    assert!(result_of(&mcp.request("ping", "{}")).get("anything").is_none());
    let tools = mcp.request("tools/list", "{}");
    for tool in ["bilterm_workspace_status", "bilterm_exec_command", "bilterm_inspect_pane", "bilterm_colon_command"] {
        assert!(tools.contains(tool), "tools/list lacks {tool}");
    }

    let status = mcp.tool("bilterm_workspace_status", "{}");
    assert!(status.contains("pane") && !status.contains("\"error\""), "{status}");

    // Malformed traffic must produce JSON-RPC errors (or be dropped) and leave the server alive.
    let bad_args = mcp.request("tools/call", r#"{"name":"bilterm_workspace_status","arguments":[1,2]}"#);
    assert!(bad_args.contains("-32602"), "{bad_args}");
    let unknown = mcp.request("no/such/method", "{}");
    assert!(unknown.contains("\"error\""), "{unknown}");
    let unknown_tool = mcp.tool("no_such_tool", "{}");
    assert!(unknown_tool.contains("\"error\"") || unknown_tool.contains("isError"), "{unknown_tool}");
    // Not JSON at all, and a notification without an id: neither gets a reply, so the next
    // well-formed request must be answered with *its* id.
    writeln!(mcp.stdin, "this is not json").unwrap();
    writeln!(mcp.stdin, r#"{{"jsonrpc":"2.0","method":"ping"}}"#).unwrap();
    let pong = mcp.raw(r#"{"jsonrpc":"2.0","id":4242,"method":"ping","params":{}}"#);
    assert!(pong.contains("4242") && !pong.contains("\"error\""), "{pong}");

    // Closing stdin ends the server cleanly.
    let McpSession { mut child, stdin, .. } = mcp;
    drop(stdin);
    let mut status = None;
    assert!(
        common::wait_until(PROCESS_DEADLINE, || {
            status = child.0.try_wait().expect("poll mcp child");
            status.is_some()
        }),
        "bilterm --mcp must exit when stdin closes"
    );
    assert!(status.unwrap().success());
}

// ---------------------------------------------------------------- 4. headless daemon over loopback TCP

#[test]
fn headless_daemon_scripted_session_over_loopback_tcp() {
    let env = IsolatedEnv::new();
    let token = env.token_file("smoke-token-0123456789abcdef0123456789");
    let token_path = token.to_str().unwrap().to_string();
    let endpoint = format!("127.0.0.1:{}", common::free_loopback_port());

    let mut cmd = env.bilterm(&["--headless", "--listen", &endpoint, "--token-file", &token_path, "--name", "smoke"]);
    let daemon = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).spawn().expect("spawn headless daemon");
    let mut daemon = KillOnDrop(daemon);

    let ctl = |method: &str, params: &str| {
        let mut args = vec!["ctl", "--token-file", token_path.as_str(), endpoint.as_str(), method];
        if !params.is_empty() {
            args.push(params);
        }
        env.run(&args)
    };

    // Readiness: poll the real RPC surface with a deadline (no fixed sleep).
    let mut last = String::new();
    let ready = common::wait_until(PROCESS_DEADLINE, || {
        if let Ok(Some(st)) = daemon.0.try_wait() {
            panic!("daemon exited early: {st}");
        }
        let out = ctl("workspace.status", "");
        last = format!("{} {}", stdout_of(&out), stderr_of(&out));
        out.status.success()
    });
    assert!(ready, "daemon never answered workspace.status: {last}");

    let status = result_of(&stdout_of(&ctl("workspace.status", "")));
    let panes = status.get("panes").and_then(JsonValue::as_array).expect("panes array").len();
    assert!(panes >= 1, "a fresh workspace has a pane");

    let split = result_of(&stdout_of(&ctl("pane.split", r#"{"direction":"v"}"#)));
    assert_eq!(split.get("pane_count").and_then(JsonValue::as_usize), Some(panes + 1));

    // Drive the terminal engine end to end: feed bytes, read the screen back.
    let fed = result_of(&stdout_of(&ctl("pane.feed_output", r#"{"data":"SMOKE-MARKER-42\r\n"}"#)));
    assert_eq!(fed.get("status").and_then(JsonValue::as_str), Some("processed"));
    let read = stdout_of(&ctl("pane.read", ""));
    assert!(read.contains("SMOKE-MARKER-42"), "pane.read lacks the fed marker: {read}");

    // Errors come back as JSON-RPC errors, not as a dead daemon.
    let unknown = stdout_of(&ctl("no.such.method", ""));
    assert!(unknown.contains("\"error\""), "{unknown}");
    let bad_pane = stdout_of(&ctl("pane.feed_output", r#"{"pane_id":987654,"data":"x"}"#));
    assert!(bad_pane.contains("\"error\""), "{bad_pane}");

    // The token is enforced.
    let anon = env.run(&["ctl", &endpoint, "workspace.status"]);
    let anon_text = format!("{}{}", stdout_of(&anon), stderr_of(&anon));
    assert!(!anon.status.success() || anon_text.contains("\"error\""), "unauthenticated call must be refused: {anon_text}");

    // The human CLI reaches the same instance through BILTERM_SOCKET (the subcommand must be the
    // first argument, so the token travels in BILTERM_TOKEN_FILE).
    let cli = |args: &[&str]| {
        let mut cmd = env.bilterm(args);
        cmd.env("BILTERM_SOCKET", &endpoint).env("BILTERM_TOKEN_FILE", &token_path);
        common::run_with_deadline(cmd, PROCESS_DEADLINE)
    };
    let list = cli(&["pane", "list"]);
    assert!(list.status.success(), "pane list: {}", stderr_of(&list));
    result_of(&stdout_of(&list));
    let ws = cli(&["workspace", "list"]);
    assert!(ws.status.success(), "workspace list: {}", stderr_of(&ws));
    result_of(&stdout_of(&ws));
    let fmt = cli(&["coords", "format"]);
    assert!(fmt.status.success(), "coords format: {}", stderr_of(&fmt));
    result_of(&stdout_of(&fmt));

    // Still alive after the whole script.
    assert!(daemon.0.try_wait().expect("poll daemon").is_none(), "daemon died during the session");
}

// ---------------------------------------------------------------- 5. real PTY child

#[test]
fn pty_child_output_reaches_the_screen() {
    use bilterm::pane::CompositePane;
    use bilterm::pty::PtyConfig;

    let (program, args) = common::shell_command(&common::echo_script("PTY-SMOKE-7391"));
    let cfg = PtyConfig { program, args, ..PtyConfig::default() };
    let mut pane = match CompositePane::spawn(1, cfg) {
        Ok(p) => p,
        Err(e) => {
            // Sandboxes without /dev/ptmx (or ConPTY) cannot run this layer; say so loudly.
            common::skip(&format!("this platform refused to open a PTY: {e}"));
            return;
        }
    };
    let mut screen = String::new();
    let seen = common::wait_until(Duration::from_secs(30), || {
        pane.poll_pty_events();
        let (rows, _) = pane.vt().size();
        screen = (0..rows).map(|r| pane.vt().row_text(r)).collect::<Vec<_>>().join("\n");
        screen.contains("PTY-SMOKE-7391")
    });
    assert!(seen, "PTY child output never reached the VT screen; screen was: {screen:?}");
}
