//! Comprehensive unit tests for `SignalablePid` invariants (`DAEM-05`, `SEC-R01..03`).
//!
//! Validates:
//! - Rejection of 0, -1, 1 (init), negative numbers, and `pid > i32::MAX`.
//! - Acceptance of valid PID range `2 <= pid <= i32::MAX`.
//! - Trait conversions (`TryFrom`, `Display`, `From`).
//! - Serde serialization and deserialization invariant enforcement.
//! - Lock record integration via `DaemonLockRecord::signalable_pid()`.
//! - Process signaling invariants via `kill_daemon_process` and `check_signalable_pid_alive`.
// Daemon sockets and signals are Unix-only; the imports they need are unused on other targets.
#![cfg_attr(not(unix), allow(unused_imports))]

use std::convert::TryFrom;
use std::process::Command;

use bilterm::daemon::supervisor::{
    check_pid_alive, check_signalable_pid_alive, kill_daemon_process, parse_daemon_pid,
    DaemonLockRecord,
};
use bilterm::daemon::types::{DaemonError, SignalablePid};

#[test]
fn signalable_pid_rejects_zero() {
    assert_eq!(
        SignalablePid::try_from(0_i64),
        Err(DaemonError::InvalidPid(0))
    );
    assert_eq!(
        <SignalablePid as TryFrom<u32>>::try_from(0_u32),
        Err(DaemonError::InvalidPid(0))
    );
    assert_eq!(
        <SignalablePid as TryFrom<i32>>::try_from(0_i32),
        Err(DaemonError::InvalidPid(0))
    );
    assert_eq!(
        <SignalablePid as TryFrom<usize>>::try_from(0_usize),
        Err(DaemonError::InvalidPid(0))
    );
}

#[test]
fn signalable_pid_rejects_negative_one() {
    assert_eq!(
        SignalablePid::try_from(-1_i64),
        Err(DaemonError::InvalidPid(-1))
    );
    assert_eq!(
        <SignalablePid as TryFrom<i32>>::try_from(-1_i32),
        Err(DaemonError::InvalidPid(-1))
    );
}

#[test]
fn signalable_pid_rejects_init_process_one() {
    assert_eq!(
        SignalablePid::try_from(1_i64),
        Err(DaemonError::InvalidPid(1))
    );
    assert_eq!(
        <SignalablePid as TryFrom<u32>>::try_from(1_u32),
        Err(DaemonError::InvalidPid(1))
    );
    assert_eq!(
        <SignalablePid as TryFrom<i32>>::try_from(1_i32),
        Err(DaemonError::InvalidPid(1))
    );
    assert_eq!(
        <SignalablePid as TryFrom<usize>>::try_from(1_usize),
        Err(DaemonError::InvalidPid(1))
    );
}

#[test]
fn signalable_pid_rejects_arbitrary_negative_numbers() {
    for neg in [-2_i64, -5, -42, -999, -65535, i32::MIN as i64, i64::MIN] {
        assert_eq!(
            SignalablePid::try_from(neg),
            Err(DaemonError::InvalidPid(neg)),
            "must reject negative PID {neg}"
        );
    }
}

#[test]
fn signalable_pid_rejects_overflow_beyond_i32_max() {
    let overflow_boundary = i64::from(i32::MAX) + 1;
    assert_eq!(
        SignalablePid::try_from(overflow_boundary),
        Err(DaemonError::InvalidPid(overflow_boundary))
    );

    assert_eq!(
        SignalablePid::try_from(i64::MAX),
        Err(DaemonError::InvalidPid(i64::MAX))
    );

    let u32_max_as_i64 = i64::from(u32::MAX);
    assert_eq!(
        SignalablePid::try_from(u32_max_as_i64),
        Err(DaemonError::InvalidPid(u32_max_as_i64))
    );

    assert_eq!(
        <SignalablePid as TryFrom<u32>>::try_from(u32::MAX),
        Err(DaemonError::InvalidPid(u32_max_as_i64))
    );
}

#[test]
fn signalable_pid_accepts_valid_boundary_values() {
    // Smallest allowed valid PID is 2 (non-zero, non-init)
    let min_pid = SignalablePid::try_from(2_i64).expect("PID 2 must be valid");
    assert_eq!(min_pid.as_raw(), 2);
    assert_eq!(min_pid.as_u32(), 2);
    assert_eq!(u32::from(min_pid), 2);

    // Largest allowed valid PID is i32::MAX
    let max_raw = i64::from(i32::MAX);
    let max_pid = SignalablePid::try_from(max_raw).expect("PID i32::MAX must be valid");
    assert_eq!(max_pid.as_raw(), i32::MAX as u32);
    assert_eq!(max_pid.as_u32(), i32::MAX as u32);
    assert_eq!(u32::from(max_pid), i32::MAX as u32);

    // Typical process IDs
    for valid in [42, 100, 1337, 32768, 65535, 1000000] {
        let pid = SignalablePid::try_from(valid as i64).expect("valid PID");
        assert_eq!(pid.as_raw(), valid);
    }
}

#[test]
fn signalable_pid_display_and_formatting() {
    let pid = SignalablePid::try_from(4242_i64).unwrap();
    assert_eq!(format!("{pid}"), "4242");
    #[cfg(unix)]
    assert_eq!(pid.as_pid_t(), 4242 as libc::pid_t);
}

#[test]
fn signalable_pid_serde_roundtrip_and_invariants() {
    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, Debug)]
    struct Wrapper {
        pid: SignalablePid,
    }

    let valid_pid = SignalablePid::try_from(5050_i64).unwrap();
    let toml_str = toml::to_string(&Wrapper { pid: valid_pid }).expect("serialize to toml");
    let deserialized: Wrapper = toml::from_str(&toml_str).expect("deserialize from toml");
    assert_eq!(deserialized.pid, valid_pid);

    // Deserialization must enforce the same invariants as try_from
    for bad in ["0", "-1", "1", "-99", "2147483648", "999999999999"] {
        let res: Result<Wrapper, _> = toml::from_str(&format!("pid = {bad}"));
        assert!(res.is_err(), "deserializing pid = {bad} must fail");
    }
}

#[test]
fn daemon_lock_record_signalable_pid_validation() {
    let rec = DaemonLockRecord {
        pid: 8888,
        name: "test-daemon".to_string(),
        version: "1.0.0".to_string(),
        started_at_ms: 12345678,
        socket_path: std::path::PathBuf::from("/tmp/test.sock"),
    };
    let sig_pid = rec.signalable_pid().expect("valid PID");
    assert_eq!(sig_pid.as_raw(), 8888);

    // Corrupted record with invalid PID 0 or 1
    let bad_rec = DaemonLockRecord {
        pid: 0,
        name: "bad".to_string(),
        version: "0".to_string(),
        started_at_ms: 0,
        socket_path: std::path::PathBuf::from("/tmp/bad.sock"),
    };
    assert_eq!(
        bad_rec.signalable_pid(),
        Err(DaemonError::InvalidPid(0))
    );

    let init_rec = DaemonLockRecord {
        pid: 1,
        name: "init".to_string(),
        version: "0".to_string(),
        started_at_ms: 0,
        socket_path: std::path::PathBuf::from("/tmp/init.sock"),
    };
    assert_eq!(
        init_rec.signalable_pid(),
        Err(DaemonError::InvalidPid(1))
    );
}

#[test]
fn parse_daemon_pid_delegates_to_signalable_pid() {
    assert_eq!(parse_daemon_pid(0), None);
    assert_eq!(parse_daemon_pid(-1), None);
    assert_eq!(parse_daemon_pid(1), None);
    assert_eq!(parse_daemon_pid(-42), None);
    assert_eq!(parse_daemon_pid(i64::from(i32::MAX) + 1), None);
    assert_eq!(parse_daemon_pid(i64::MAX), None);
    assert_eq!(parse_daemon_pid(4242), Some(4242));
    assert_eq!(parse_daemon_pid(i64::from(i32::MAX)), Some(i32::MAX as u32));
}

#[test]
fn check_pid_alive_rejects_unsafe_values() {
    assert!(!check_pid_alive(0));
    assert!(!check_pid_alive(1));
    assert!(!check_pid_alive(u32::MAX));
}

#[cfg(unix)]
#[test]
fn kill_daemon_process_with_signalable_pid() {
    // Spawn real child process
    let mut child = Command::new("sleep").arg("30").spawn().unwrap();
    let raw_pid = child.id();
    assert!(raw_pid > 1);

    let sig_pid = SignalablePid::try_from(i64::from(raw_pid)).expect("valid child pid");
    assert!(check_signalable_pid_alive(sig_pid));

    // Terminate via kill_daemon_process
    kill_daemon_process(sig_pid, true).expect("kill succeeds");

    let _ = child.wait();
    assert!(!check_signalable_pid_alive(sig_pid));
}
