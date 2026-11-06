//! Tests for Phase 51.1 persistent SSH argv builder, remote options, and redaction (PST-01, PST-04).

use std::path::Path;

use bilterm::remote::{redact_ssh_argv, RemoteHostProfile};

#[test]
fn link_args_use_tt_keepalive_and_no_mux() {
    let mut prof = RemoteHostProfile::new("prod", "ubuntu@10.0.0.42");
    prof.port = Some(2202);
    prof.identity_file = Some("/home/ubuntu/.ssh/id_ed25519".to_string());

    let control_dir = Path::new("/run/user/1000/bilterm");
    let args = prof.build_ssh_tmux_cc_args("main", Some(control_dir)).unwrap();

    assert_eq!(args.first().map(String::as_str), Some("-tt"));
    assert!(!args.iter().any(|a| a == "-T"));

    // Keepalive options
    assert!(args.contains(&"ServerAliveInterval=15".to_string()));
    assert!(args.contains(&"ServerAliveCountMax=3".to_string()));
    assert!(args.contains(&"TCPKeepAlive=yes".to_string()));
    assert!(args.contains(&"ConnectTimeout=15".to_string()));

    // Dedicated link: no multiplexing to avoid stale master sockets
    assert!(args.contains(&"ControlMaster=no".to_string()));
    assert!(args.contains(&"ControlPath=none".to_string()));
    assert!(!args.iter().any(|a| a.contains("ControlMaster=auto")));
    assert!(!args.iter().any(|a| a.contains("ControlPersist")));

    // Port and identity file preserved
    assert!(args.contains(&"-p".to_string()));
    assert!(args.contains(&"2202".to_string()));
    assert!(args.contains(&"-i".to_string()));
    assert!(args.contains(&"/home/ubuntu/.ssh/id_ed25519".to_string()));
}

#[test]
fn link_args_end_with_dd_dest_remote() {
    let prof = RemoteHostProfile::new("node", "ubuntu@10.0.0.42");
    let args = prof.build_ssh_tmux_cc_args("main", None).unwrap();
    let len = args.len();
    assert!(len >= 3);
    assert_eq!(args[len - 3], "--");
    assert_eq!(args[len - 2], "ubuntu@10.0.0.42");
    assert_eq!(
        args[len - 1],
        "exec $SHELL -lc 'exec env PATH=\"$PATH:/opt/homebrew/bin:/usr/local/bin:/opt/local/bin\" tmux -CC new-session -A -s main'"
    );
}

#[test]
fn link_args_wrap_login_shell_with_macos_path() {
    let prof = RemoteHostProfile::new("mac", "admin@macos.local");
    let args = prof.build_ssh_tmux_cc_args("main", None).unwrap();
    let last = args.last().expect("argv has remote command");

    assert!(last.starts_with("exec $SHELL -lc '"));
    assert!(last.ends_with("-s main'"));
    assert!(last.contains("/opt/homebrew/bin"));
    assert!(last.contains("/usr/local/bin"));
    assert!(last.contains("/opt/local/bin"));
}

#[test]
fn link_args_sanitise_session_and_keep_two_quotes() {
    let prof = RemoteHostProfile::new("node", "ubuntu@10.0.0.42");

    let test_cases = [
        ("prod/main!@#", "prodmain"),
        ("!!!", "bilterm"),
        ("'; rm -rf ~", "rm-rf"),
        ("test\n`$(x)\"payload", "testxpayload"),
    ];

    for (input, expected_session) in test_cases {
        let mut args = prof
            .build_ssh_tmux_cc_args(input, None)
            .unwrap_or_else(|e| panic!("unexpected error for session {input:?}: {e}"));
        let remote_cmd = args.pop().expect("remote command present");

        // Exactly two single quotes: opening and closing the $SHELL -lc string
        let quote_count = remote_cmd.chars().filter(|&c| c == '\'').count();
        assert_eq!(
            quote_count, 2,
            "session {input:?} produced {quote_count} quotes in {remote_cmd:?}"
        );

        // Disallowed injection characters must be absent
        assert!(!remote_cmd.contains('`'));
        assert!(!remote_cmd.contains("$("));
        assert!(!remote_cmd.contains('\n'));
        assert!(!remote_cmd.contains(';'));

        // Extracted session token matches expected
        let suffix = format!("-s {expected_session}'");
        assert!(
            remote_cmd.ends_with(&suffix),
            "expected remote_cmd to end with {suffix:?}, got {remote_cmd:?}"
        );

        // Verify session name token characters are strictly [A-Za-z0-9_-]
        let token = &remote_cmd[remote_cmd.rfind("-s ").unwrap() + 3..remote_cmd.len() - 1];
        assert_eq!(token, expected_session);
        assert!(token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }
}

#[test]
fn link_args_reject_option_destination() {
    let bad_destinations = [
        "-oProxyCommand=evil",
        "-p 22",
        "host name",
        "",
        "host\u{0007}",
    ];
    for bad in bad_destinations {
        let prof = RemoteHostProfile::new("bad", bad);
        assert!(
            prof.build_ssh_tmux_cc_args("main", None).is_err(),
            "expected destination {bad:?} to be rejected"
        );
    }
}

#[test]
fn ssh_args_gain_keepalive_and_short_persist() {
    let prof = RemoteHostProfile::new("node", "ubuntu@10.0.0.42");
    let control_dir = Path::new("/run/user/1000/bilterm");
    let args = prof.build_ssh_args(Some(control_dir)).unwrap();

    assert_eq!(args.first().map(String::as_str), Some("-tt"));
    assert!(args.contains(&"ServerAliveInterval=15".to_string()));
    assert!(args.contains(&"ServerAliveCountMax=3".to_string()));
    assert!(args.contains(&"TCPKeepAlive=yes".to_string()));
    assert!(args.contains(&"ConnectTimeout=15".to_string()));
    assert!(args.contains(&"ControlMaster=auto".to_string()));
    assert!(args.contains(&"ControlPersist=10s".to_string()));
    assert!(!args.iter().any(|a| a.contains("ControlPersist=60s")));
    assert!(args.contains(&format!("ControlPath={}/ssh-%C", control_dir.display())));
}

#[test]
fn ssh_args_without_control_dir_have_no_control_flags() {
    let prof = RemoteHostProfile::new("node", "ubuntu@10.0.0.42");

    // Case 1: None
    let no_dir_args = prof.build_ssh_args(None).unwrap();
    assert!(no_dir_args.contains(&"ServerAliveInterval=15".to_string()));
    assert!(!no_dir_args.iter().any(|a| a.contains("Control")));

    // Case 2: Space in path
    let bad_space = Path::new("/run/user/1000/bad dir");
    let space_args = prof.build_ssh_args(Some(bad_space)).unwrap();
    assert!(space_args.contains(&"ServerAliveInterval=15".to_string()));
    assert!(!space_args.iter().any(|a| a.contains("Control")));

    // Case 3: Percent in path
    let bad_percent = Path::new("/run/user/1000/bad%dir");
    let percent_args = prof.build_ssh_args(Some(bad_percent)).unwrap();
    assert!(percent_args.contains(&"ServerAliveInterval=15".to_string()));
    assert!(!percent_args.iter().any(|a| a.contains("Control")));

    // Case 4: Excessively long path (> 40 bytes)
    let bad_long = Path::new("/run/user/1000/this_path_is_way_too_long_for_sun_path_limits");
    let long_args = prof.build_ssh_args(Some(bad_long)).unwrap();
    assert!(long_args.contains(&"ServerAliveInterval=15".to_string()));
    assert!(!long_args.iter().any(|a| a.contains("Control")));
}

#[test]
fn redact_argv_hides_identity_and_control_path() {
    let argv = vec![
        "ssh".to_string(),
        "-tt".to_string(),
        "-o".to_string(),
        "ServerAliveInterval=15".to_string(),
        "-o".to_string(),
        "ControlPath=/run/user/1000/bilterm/ssh-%C".to_string(),
        "-i".to_string(),
        "/home/u/.ssh/id_ed25519".to_string(),
        "ubuntu@10.0.0.42".to_string(),
    ];

    let redacted = redact_ssh_argv(&argv);
    assert!(redacted.contains("-i <key>"));
    assert!(redacted.contains("ControlPath=<private>"));
    assert!(!redacted.contains("/home/u"));
    assert!(!redacted.contains("/run/user"));
    assert!(redacted.contains("ServerAliveInterval=15"));
    assert!(redacted.contains("ubuntu@10.0.0.42"));

    // Trailing -i without a value must not panic
    let trailing_i = vec!["ssh".to_string(), "-i".to_string()];
    let trailing_redacted = redact_ssh_argv(&trailing_i);
    assert_eq!(trailing_redacted, "ssh -i");

    // Empty argv must not panic
    let empty: Vec<String> = Vec::new();
    assert_eq!(redact_ssh_argv(&empty), "");
}

#[test]
fn redact_argv_keeps_remote_command_for_diagnosis() {
    let prof = RemoteHostProfile::new("node", "ubuntu@10.0.0.42");
    let args = prof.build_ssh_tmux_cc_args("main", None).unwrap();
    let redacted = redact_ssh_argv(&args);

    assert!(redacted.contains("tmux -CC new-session -A -s main"));
    assert!(redacted.contains("ubuntu@10.0.0.42"));
    assert!(redacted.contains("ServerAliveInterval=15"));
}
