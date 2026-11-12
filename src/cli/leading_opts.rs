//! Global client options placed BEFORE the client subcommand
//! (`bilterm --daemon-name X pane read ...`). [`normalize_leading_options`] rewrites such argv so
//! the subcommand is first (the form the rest of the dispatch expects), and refuses an unsupported
//! leading option rather than letting a client invocation fall through to the GUI/TUI launcher.

/// Options that may precede a client subcommand, with whether they take a separate value.
const HOISTABLE: &[(&str, bool)] = &[("--daemon-name", true), ("-t", true), ("--token-file", true)];

/// Launcher options that take a value; their value is never mistaken for a subcommand.
const VALUE_FLAGS: &[&str] = &[
    "--name", "-s", "--session", "-F", "--font", "--gui-font", "--font-size", "--gui-font-size", "--font-file",
    "--gui-font-file", "-T", "--theme", "-l", "--listen", "--http", "--web-dir", "--cors-origin", "-S",
    "--snapshot-dir", "-p", "--playbook", "-g", "--gui-dump", "-P", "--profile", "-c", "--config", "-C",
    "--completions", "-o", "--output", "-b", "--backend", "-a", "--attach", "--raw-fkeys",
];

fn is_client_word(a: &str) -> bool {
    a == "ctl" || a == "daemon" || super::is_cli_command(Some(a))
}

/// Moves leading `--daemon-name`/`-t` options behind a client subcommand. `Ok(())` leaves `args`
/// untouched when the invocation is not a client command with leading options.
pub fn normalize_leading_options(args: &mut Vec<String>) -> Result<(), String> {
    let mut i = 0;
    let mut unsupported: Option<String> = None;
    while i < args.len() {
        let a = args[i].as_str();
        if is_client_word(a) {
            break;
        }
        if !a.starts_with('-') || a == "--" {
            return Ok(()); // a non-client positional (new/attach/ls/...) first: not a client call
        }
        let (name, has_eq) = a.split_once('=').map_or((a, false), |(n, _)| (n, true));
        if let Some((_, takes)) = HOISTABLE.iter().find(|(n, _)| *n == name) {
            i += if *takes && !has_eq { 2 } else { 1 };
        } else if VALUE_FLAGS.contains(&name) {
            unsupported.get_or_insert_with(|| name.to_string());
            i += if has_eq { 1 } else { 2 };
        } else {
            unsupported.get_or_insert_with(|| name.to_string());
            i += 1;
        }
    }
    if i == 0 || i >= args.len() {
        return Ok(());
    }
    if let Some(flag) = unsupported {
        return Err(format!(
            "bilterm: option '{flag}' cannot precede the client subcommand '{}'; only --daemon-name and -t/--token-file may (put other options after it)",
            args[i]
        ));
    }
    let cmd = args[i].clone();
    let mut lead: Vec<String> = args.drain(..i).collect();
    args.remove(0); // the subcommand itself, now first
    if cmd != "ctl" {
        // `-t/--token-file` was already consumed from the original argv; only `ctl` re-reads it.
        let mut k = 0;
        while k < lead.len() {
            let tok = lead[k].as_str();
            if tok == "-t" || tok == "--token-file" {
                lead.drain(k..(k + 2).min(lead.len()));
            } else if tok.starts_with("--token-file=") {
                lead.remove(k);
            } else {
                k += 1;
            }
        }
    }
    let mut out = vec![cmd];
    out.extend(lead);
    out.append(args);
    *args = out;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }
    fn norm(a: &[&str]) -> Result<Vec<String>, String> {
        let mut x = v(a);
        normalize_leading_options(&mut x).map(|()| x)
    }

    #[test]
    fn leading_daemon_name_moves_behind_each_client_subcommand() {
        for cmd in ["pane", "agent", "workspace", "coords", "daemon", "ctl"] {
            assert_eq!(norm(&["--daemon-name", "x", cmd, "read", "p1"]).unwrap(), v(&[cmd, "--daemon-name", "x", "read", "p1"]));
            assert_eq!(norm(&["--daemon-name=x", cmd, "list"]).unwrap(), v(&[cmd, "--daemon-name=x", "list"]));
        }
    }

    #[test]
    fn both_orders_resolve_to_the_same_target_and_call() {
        let env = crate::cli::CliEnv::default();
        let mut before = norm(&["--daemon-name", "alpha", "pane", "read", "p1"]).unwrap();
        let mut after = v(&["pane", "read", "p1", "--daemon-name", "alpha"]);
        let t1 = crate::cli::daemon_cmd::take_daemon_target(&mut before, None).unwrap();
        let t2 = crate::cli::daemon_cmd::take_daemon_target(&mut after, None).unwrap();
        assert_eq!(t1, Some("alpha".into()));
        assert_eq!(t1, t2);
        assert_eq!(before, after);
        assert!(crate::cli::is_cli_command(before.first().map(String::as_str)));
        assert_eq!(crate::cli::parse(&before, &env).unwrap().method, crate::cli::parse(&after, &env).unwrap().method);
    }

    #[test]
    fn env_daemon_with_leading_token_flag() {
        let mut a = norm(&["-t", "/tok", "pane", "list"]).unwrap();
        assert_eq!(a, v(&["pane", "list"]));
        assert_eq!(crate::cli::daemon_cmd::take_daemon_target(&mut a, Some("envd")).unwrap(), Some("envd".into()));
        assert_eq!(norm(&["-t", "/tok", "ctl", "m"]).unwrap(), v(&["ctl", "-t", "/tok", "m"]));
    }

    #[test]
    fn unsupported_leading_flag_before_client_subcommand_errors() {
        assert!(norm(&["--bogus", "pane", "list"]).unwrap_err().contains("--bogus"));
        assert!(norm(&["--daemon-name", "x", "--gui", "agent", "list"]).is_err());
        assert!(norm(&["-s", "work", "pane", "list"]).is_err());
    }

    #[test]
    fn non_client_invocations_are_untouched() {
        for a in [&["--name", "pane"][..], &["-s", "pane"], &["ls"], &["new", "pane"], &[], &["--daemon-name", "x"], &["--headless", "--name", "d"]] {
            assert_eq!(norm(a).unwrap(), v(a), "{a:?}");
        }
    }

    #[test]
    fn already_ordered_args_are_untouched() {
        assert_eq!(norm(&["pane", "list", "--daemon-name", "x"]).unwrap(), v(&["pane", "list", "--daemon-name", "x"]));
    }
}
