use std::process::Command;

#[test]
fn headless_refuses_non_loopback_without_allow_remote() {
    let output = Command::new(env!("CARGO_BIN_EXE_bilterm"))
        .args(["--headless", "--listen", "0.0.0.0:0"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--allow-remote"),
        "stderr should mention --allow-remote: {stderr}"
    );
}

#[test]
fn headless_allow_remote_requires_token_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_bilterm"))
        .args(["--headless", "--listen", "0.0.0.0:0", "--allow-remote"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--token-file"),
        "stderr should mention --token-file: {stderr}"
    );
}

#[test]
#[cfg(unix)]
fn headless_rejects_world_readable_token_file() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().unwrap();
    let token_file = temp_dir.path().join("insecure.token");
    let sock_path = temp_dir.path().join("test.sock");
    std::fs::write(&token_file, b"0123456789abcdef01234567\n").unwrap();
    std::fs::set_permissions(&token_file, std::fs::Permissions::from_mode(0o644)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_bilterm"))
        .args([
            "--headless",
            "--listen",
            &format!("unix:{}", sock_path.display()),
            "--token-file",
            token_file.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("600") || stderr.contains("private"),
        "stderr should mention 600 or private permissions: {stderr}"
    );
    assert!(
        !stderr.contains("0123456789abcdef"),
        "stderr must not leak secret token: {stderr}"
    );
}

#[test]
#[cfg(unix)]
fn headless_unix_socket_roundtrip_with_ctl() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().unwrap();
    let sock_path = temp_dir.path().join("roundtrip.sock");

    let mut child = Command::new(env!("CARGO_BIN_EXE_bilterm"))
        .args([
            "--headless",
            "--listen",
            &format!("unix:{}", sock_path.display()),
        ])
        .spawn()
        .unwrap();

    let mut ready = false;
    for _ in 0..50 {
        if sock_path.exists() {
            ready = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(ready, "daemon did not create socket in time");

    let meta = std::fs::symlink_metadata(&sock_path).unwrap();
    assert_eq!(
        meta.permissions().mode() & 0o777,
        0o600,
        "socket permissions should be 0600"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_bilterm"))
        .args([
            "ctl",
            &format!("unix:{}", sock_path.display()),
            "workspace.status",
        ])
        .output()
        .unwrap();

    let _ = child.kill();
    let _ = child.wait();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("pane_count"),
        "ctl stdout should contain pane_count: {stdout}"
    );
}

#[test]
#[cfg(unix)]
fn ctl_token_flow() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sock_path = temp_dir.path().join("auth.sock");
    let token_file = temp_dir.path().join("bilterm.token");
    let token = "my-secret-auth-token-12345";
    bilterm::secure_fs::write_private_file(&token_file, format!("{token}\n").as_bytes()).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_bilterm"))
        .args([
            "--headless",
            "--listen",
            &format!("unix:{}", sock_path.display()),
            "--token-file",
            token_file.to_str().unwrap(),
        ])
        .spawn()
        .unwrap();

    let mut ready = false;
    for _ in 0..50 {
        if sock_path.exists() {
            ready = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(ready, "daemon did not create socket in time");

    // 1. Without token: expect auth error
    let unauthed = Command::new(env!("CARGO_BIN_EXE_bilterm"))
        .args([
            "ctl",
            &format!("unix:{}", sock_path.display()),
            "workspace.status",
        ])
        .output()
        .unwrap();
    let unauthed_out = String::from_utf8_lossy(&unauthed.stdout);
    assert!(
        unauthed_out.contains("-32001"),
        "expected -32001 error without token: {unauthed_out}"
    );

    // 2. With --token-file flag: expect success
    let authed_flag = Command::new(env!("CARGO_BIN_EXE_bilterm"))
        .args([
            "ctl",
            "--token-file",
            token_file.to_str().unwrap(),
            &format!("unix:{}", sock_path.display()),
            "workspace.status",
        ])
        .output()
        .unwrap();
    let authed_flag_out = String::from_utf8_lossy(&authed_flag.stdout);
    assert!(
        authed_flag_out.contains("pane_count"),
        "expected pane_count with --token-file: {authed_flag_out}"
    );

    // 3. With BILTERM_TOKEN_FILE env: expect success
    let authed_env = Command::new(env!("CARGO_BIN_EXE_bilterm"))
        .env("BILTERM_TOKEN_FILE", token_file.to_str().unwrap())
        .args([
            "ctl",
            &format!("unix:{}", sock_path.display()),
            "workspace.status",
        ])
        .output()
        .unwrap();
    let authed_env_out = String::from_utf8_lossy(&authed_env.stdout);
    assert!(
        authed_env_out.contains("pane_count"),
        "expected pane_count with BILTERM_TOKEN_FILE: {authed_env_out}"
    );

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn help_and_completions_list_new_flags() {
    let help_out = Command::new(env!("CARGO_BIN_EXE_bilterm"))
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&help_out.stdout);
    assert!(help.contains("allow-remote"), "--help missing allow-remote");
    assert!(help.contains("token-file"), "--help missing token-file");
    assert!(help.contains("snapshot-dir"), "--help missing snapshot-dir");

    for shell in ["bash", "zsh", "fish"] {
        let comp_out = Command::new(env!("CARGO_BIN_EXE_bilterm"))
            .args(["--completions", shell])
            .output()
            .unwrap();
        let comp = String::from_utf8_lossy(&comp_out.stdout);
        assert!(
            comp.contains("allow-remote"),
            "{shell} completions missing allow-remote: {comp}"
        );
        assert!(
            comp.contains("token-file"),
            "{shell} completions missing token-file: {comp}"
        );
        assert!(
            comp.contains("snapshot-dir"),
            "{shell} completions missing snapshot-dir: {comp}"
        );

        // Verify no bare --token flag is present (tokens only via file)
        let mut idx = 0;
        while let Some(pos) = comp[idx..].find("--token") {
            let actual_pos = idx + pos;
            assert!(
                comp[actual_pos..].starts_with("--token-file"),
                "bare --token found in {shell} completions: {}",
                &comp[actual_pos..actual_pos + 20.min(comp.len() - actual_pos)]
            );
            idx = actual_pos + "--token-file".len();
        }
    }
}

#[test]
fn man_page_documents_new_flags() {
    let man_out = Command::new(env!("CARGO_BIN_EXE_bilterm"))
        .arg("--man")
        .output()
        .unwrap();
    let man = String::from_utf8_lossy(&man_out.stdout);
    assert!(man.contains("allow\\-remote"), "man page missing allow-remote");
    assert!(man.contains("token\\-file"), "man page missing token-file");
    assert!(
        man.contains("snapshot\\-dir"),
        "man page missing snapshot-dir"
    );
    assert!(man.contains(".SH SECURITY"), "man page missing .SH SECURITY");
}
