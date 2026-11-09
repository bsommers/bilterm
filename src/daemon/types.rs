//! Daemon types and invariants (`DAEM-05`, `DAEM-06`, `SEC-R01..03`).
//!
//! Provides [`SignalablePid`], a strongly-typed PID wrapper guaranteeing
//! `2 <= pid <= i32::MAX as u32`, preventing accidental transmission of
//! special or negative PIDs (like 0, -1, 1) to `kill(2)`.

use std::fmt;
use std::io;
use serde::{Deserialize, Serialize};

/// Errors relating to daemon management and PID operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DaemonError {
    #[error("invalid signalable process id: {0} (must satisfy 2 <= pid <= {max})", max = i32::MAX)]
    InvalidPid(i64),

    #[error("daemon process not alive or already exited: pid {0}")]
    ProcessNotAlive(u32),

    #[error("daemon lock acquisition failed: {0}")]
    LockError(String),

    #[error("invalid daemon name: {0}")]
    InvalidName(String),

    #[error("daemon I/O error: {0}")]
    Io(String),

    #[error("daemon error: {0}")]
    Other(String),
}

impl From<io::Error> for DaemonError {
    fn from(err: io::Error) -> Self {
        DaemonError::Io(err.to_string())
    }
}

impl From<DaemonError> for io::Error {
    fn from(err: DaemonError) -> Self {
        match err {
            DaemonError::Io(s) => io::Error::other(s),
            DaemonError::InvalidPid(p) => {
                io::Error::new(io::ErrorKind::InvalidInput, format!("invalid pid: {p}"))
            }
            DaemonError::InvalidName(n) => {
                io::Error::new(io::ErrorKind::InvalidInput, format!("invalid daemon name: {n}"))
            }
            other => io::Error::other(other.to_string()),
        }
    }
}

/// Strongly typed, non-zero, non-init, non-negative process identifier.
///
/// Invariant: `2 <= pid <= i32::MAX as u32`.
///
/// Eliminates primitive obsession around raw integer PIDs, preventing
/// accidental transmission of dangerous special values to `kill(2)`:
/// - `pid == 0`: signals all processes in the current process group.
/// - `pid == -1`: signals all processes the calling process has permission to signal.
/// - `pid < -1`: signals process group `|pid|`.
/// - `pid == 1`: signals the `init` process (`systemd` / PID 1).
/// - `pid > i32::MAX`: overflows POSIX `pid_t`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "u32")]
pub struct SignalablePid(u32);

impl SignalablePid {
    /// Minimum allowed PID (2, since 0 and -1 are special groups/broadcast, and 1 is init).
    pub const MIN: u32 = 2;
    /// Maximum allowed PID (`i32::MAX as u32` to fit safely in POSIX `pid_t`).
    pub const MAX: u32 = i32::MAX as u32;

    /// Attempts to construct a `SignalablePid` from an `i64`.
    pub fn try_from(raw: i64) -> Result<Self, DaemonError> {
        <Self as TryFrom<i64>>::try_from(raw)
    }

    /// Returns the underlying raw PID as a `u32`.
    #[inline]
    pub const fn as_raw(&self) -> u32 {
        self.0
    }

    /// Returns the underlying raw PID as a `u32`.
    #[inline]
    pub const fn as_u32(&self) -> u32 {
        self.0
    }

    /// Converts to POSIX `libc::pid_t`.
    #[cfg(unix)]
    #[inline]
    pub fn as_pid_t(&self) -> libc::pid_t {
        self.0 as libc::pid_t
    }
}

impl TryFrom<i64> for SignalablePid {
    type Error = DaemonError;

    fn try_from(raw: i64) -> Result<Self, Self::Error> {
        if raw >= 2 && raw <= i64::from(i32::MAX) {
            Ok(Self(raw as u32))
        } else {
            Err(DaemonError::InvalidPid(raw))
        }
    }
}

impl TryFrom<u32> for SignalablePid {
    type Error = DaemonError;

    fn try_from(raw: u32) -> Result<Self, Self::Error> {
        Self::try_from(i64::from(raw))
    }
}

impl TryFrom<i32> for SignalablePid {
    type Error = DaemonError;

    fn try_from(raw: i32) -> Result<Self, Self::Error> {
        Self::try_from(i64::from(raw))
    }
}

impl TryFrom<usize> for SignalablePid {
    type Error = DaemonError;

    fn try_from(raw: usize) -> Result<Self, Self::Error> {
        if let Ok(val) = i64::try_from(raw) {
            Self::try_from(val)
        } else {
            Err(DaemonError::InvalidPid(i64::MAX))
        }
    }
}

impl From<SignalablePid> for u32 {
    #[inline]
    fn from(pid: SignalablePid) -> Self {
        pid.0
    }
}

impl fmt::Display for SignalablePid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
