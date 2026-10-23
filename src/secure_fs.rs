//! Shared secure filesystem helpers for Bilterm (SEC-R03, D-06).
//!
//! Provides atomic, race-free private directory and file manipulation routines:
//! - 0700 private directories with owner, mode, and symlink validation
//! - 0600 private files with `O_CREAT | O_EXCL` and `O_NOFOLLOW`
//! - Secret-file reader for bearer authentication tokens
//! - Constant-time byte comparisons to prevent timing side channels

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};

pub const MIN_SECRET_LEN: usize = 16;
static SEQ: AtomicU64 = AtomicU64::new(0);

/// Returns the effective user ID of the calling process.
pub fn current_euid() -> u32 {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// Ensures `path` exists as a secure directory owned by the current process with 0700 permissions.
///
/// If the directory already exists, its ownership, file type, and permissions are verified.
/// Existing directories failing verification are rejected with [`ErrorKind::PermissionDenied`]
/// and are NEVER modified or chmod-ed.
pub fn ensure_private_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        match builder.create(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                let md = fs::symlink_metadata(path)?;
                if !md.is_dir() {
                    return Err(io::Error::new(
                        ErrorKind::PermissionDenied,
                        format!("private dir target is not a directory: {:?}", path),
                    ));
                }
                if md.file_type().is_symlink() {
                    return Err(io::Error::new(
                        ErrorKind::PermissionDenied,
                        format!("private dir target is a symlink: {:?}", path),
                    ));
                }
                if md.uid() != current_euid() {
                    return Err(io::Error::new(
                        ErrorKind::PermissionDenied,
                        format!(
                            "private dir {:?} owned by uid {}, expected {}",
                            path,
                            md.uid(),
                            current_euid()
                        ),
                    ));
                }
                if md.mode() & 0o077 != 0 {
                    return Err(io::Error::new(
                        ErrorKind::PermissionDenied,
                        format!(
                            "private dir {:?} has unsafe mode {:o}; must be 0700",
                            path,
                            md.mode() & 0o777
                        ),
                    ));
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(path)
    }
}

/// Creates or verifies a private 0700 subdirectory named `name` inside pre-existing `base`.
pub fn private_dir_in(base: &Path, name: &str) -> io::Result<PathBuf> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!("invalid private dir leaf name: {:?}", name),
        ));
    }
    let base_md = fs::symlink_metadata(base)?;
    if !base_md.is_dir() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!("base path is not a directory: {:?}", base),
        ));
    }
    let target = base.join(name);
    ensure_private_dir(&target)?;
    Ok(target)
}

/// Resolves the runtime base directory and leaf directory name.
///
/// Pure function taking parameters without reading environment variables or performing I/O.
pub fn resolve_runtime_base(
    xdg_runtime_dir: Option<&std::ffi::OsStr>,
    temp_dir: &Path,
    uid: u32,
) -> (PathBuf, String) {
    if let Some(xdg) = xdg_runtime_dir
        && !xdg.is_empty()
        && Path::new(xdg).is_absolute()
    {
        return (PathBuf::from(xdg), "bilterm".to_string());
    }
    (temp_dir.to_path_buf(), format!("bilterm-{uid}"))
}

/// Resolves and ensures the process-wide private runtime directory.
pub fn private_runtime_dir() -> io::Result<PathBuf> {
    let (base, leaf) = resolve_runtime_base(
        std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
        &std::env::temp_dir(),
        current_euid(),
    );
    private_dir_in(&base, &leaf)
}

/// Returns standard [`OpenOptions`] configured for private 0600 file access with `O_NOFOLLOW`.
pub fn private_open_options() -> OpenOptions {
    #[allow(unused_mut)]
    let mut opts = OpenOptions::new();
    #[cfg(unix)]
    {
        opts.mode(0o600);
        opts.custom_flags(libc::O_NOFOLLOW);
    }
    opts
}

/// Atomically creates a private 0600 file with `O_CREAT | O_EXCL` and `O_NOFOLLOW`.
pub fn create_private_file(path: &Path) -> io::Result<File> {
    private_open_options()
        .write(true)
        .create_new(true)
        .open(path)
}

/// Replaces any existing file or dangling/planted symlink at `path` by unlinking it first,
/// then atomically creating a fresh 0600 file and writing `bytes`.
pub fn write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(md) => {
            if md.file_type().is_symlink() {
                fs::remove_file(path)?;
            } else if md.is_dir() {
                return Err(io::Error::new(
                    ErrorKind::PermissionDenied,
                    format!("write_private_file target is a directory: {:?}", path),
                ));
            } else {
                #[cfg(unix)]
                {
                    if md.uid() != current_euid() {
                        return Err(io::Error::new(
                            ErrorKind::PermissionDenied,
                            format!("write_private_file target {:?} owned by other uid {}", path, md.uid()),
                        ));
                    }
                }
                fs::remove_file(path)?;
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    let mut file = create_private_file(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

/// Reads an authentication token from `path`.
///
/// Verifies the file is a regular file owned by the effective UID with mode 0600 (no group/world permissions).
/// Rejects symlinks via `O_NOFOLLOW` / `fstat`. Error messages NEVER contain token contents.
pub fn read_secret_file(path: &Path) -> io::Result<String> {
    #[allow(unused_mut)]
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        opts.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = opts.open(path)?;
    let md = file.metadata()?;
    if !md.is_file() {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "secret file is not a regular file",
        ));
    }
    #[cfg(unix)]
    {
        if md.uid() != current_euid() {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "secret file owned by different user",
            ));
        }
        if md.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "secret file has group or world permissions; chmod 600 required",
            ));
        }
    }

    let mut content = String::new();
    file.read_to_string(&mut content)?;
    let trimmed = content.trim().to_string();
    if trimmed.len() < MIN_SECRET_LEN {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "token file must contain at least 16 characters",
        ));
    }
    Ok(trimmed)
}

/// Compares two byte slices in constant time.
#[inline(never)]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc: u8 = 0;
    for (&x, &y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

/// Generates a process-unique, time-monotonic suffix string (`<pid>-<nanos>-<seq>`).
pub fn unique_suffix() -> String {
    format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::Permissions;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    #[cfg(unix)]
    fn private_dir_in_creates_0700_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let path = private_dir_in(tmp.path(), "bilterm").unwrap();
        assert_eq!(path, tmp.path().join("bilterm"));
        let md = fs::symlink_metadata(&path).unwrap();
        assert!(md.is_dir());
        assert_eq!(md.permissions().mode() & 0o777, 0o700);
    }

    #[test]
    #[cfg(unix)]
    fn private_dir_in_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let path1 = private_dir_in(tmp.path(), "bilterm").unwrap();
        let path2 = private_dir_in(tmp.path(), "bilterm").unwrap();
        assert_eq!(path1, path2);
    }

    #[test]
    #[cfg(unix)]
    fn private_dir_in_rejects_group_or_world_bits() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("bilterm");
        fs::create_dir(&target).unwrap();
        fs::set_permissions(&target, Permissions::from_mode(0o755)).unwrap();

        let err = private_dir_in(tmp.path(), "bilterm").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::PermissionDenied);

        // Verify it was NEVER chmod-ed
        let md = fs::symlink_metadata(&target).unwrap();
        assert_eq!(md.permissions().mode() & 0o777, 0o755);
    }

    #[test]
    #[cfg(unix)]
    fn private_dir_in_rejects_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let real_dir = tmp.path().join("real_0700");
        fs::create_dir(&real_dir).unwrap();
        fs::set_permissions(&real_dir, Permissions::from_mode(0o700)).unwrap();

        let symlink_path = tmp.path().join("bilterm");
        std::os::unix::fs::symlink(&real_dir, &symlink_path).unwrap();

        let err = private_dir_in(tmp.path(), "bilterm").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::PermissionDenied);
    }

    #[test]
    #[cfg(unix)]
    fn private_dir_in_rejects_non_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("bilterm");
        fs::write(&target, b"regular file").unwrap();
        fs::set_permissions(&target, Permissions::from_mode(0o700)).unwrap();

        let err = private_dir_in(tmp.path(), "bilterm").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::PermissionDenied);
    }

    #[test]
    fn private_dir_in_rejects_bad_names() {
        let tmp = tempfile::tempdir().unwrap();
        for bad in &["", ".", "..", "a/b", "../x", "a\\b"] {
            let err = private_dir_in(tmp.path(), bad).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "expected InvalidInput for {:?}", bad);
        }
    }

    #[test]
    fn resolve_runtime_base_prefers_absolute_xdg() {
        let temp = Path::new("/tmp/test_dir");
        let uid = 1000;

        let (base, leaf) = resolve_runtime_base(
            Some(std::ffi::OsStr::new("/run/user/1000")),
            temp,
            uid,
        );
        assert_eq!(base, PathBuf::from("/run/user/1000"));
        assert_eq!(leaf, "bilterm");

        let (base, leaf) = resolve_runtime_base(
            Some(std::ffi::OsStr::new("relative/dir")),
            temp,
            uid,
        );
        assert_eq!(base, temp);
        assert_eq!(leaf, "bilterm-1000");

        let (base, leaf) = resolve_runtime_base(
            Some(std::ffi::OsStr::new("")),
            temp,
            uid,
        );
        assert_eq!(base, temp);
        assert_eq!(leaf, "bilterm-1000");

        let (base, leaf) = resolve_runtime_base(None, temp, uid);
        assert_eq!(base, temp);
        assert_eq!(leaf, "bilterm-1000");
    }

    #[test]
    #[cfg(unix)]
    fn create_private_file_is_0600_and_exclusive() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("secret.txt");

        {
            let _f = create_private_file(&file_path).unwrap();
            let md = fs::symlink_metadata(&file_path).unwrap();
            assert_eq!(md.permissions().mode() & 0o777, 0o600);
        }

        let err = create_private_file(&file_path).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::AlreadyExists);
    }

    #[test]
    #[cfg(unix)]
    fn create_private_file_refuses_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let victim = tmp.path().join("victim.txt");
        fs::write(&victim, b"keep").unwrap();

        let link = tmp.path().join("symlink.txt");
        std::os::unix::fs::symlink(&victim, &link).unwrap();

        let err = create_private_file(&link).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&victim).unwrap(), b"keep");
    }

    #[test]
    #[cfg(unix)]
    fn write_private_file_replaces_own_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test.txt");

        write_private_file(&file_path, b"a").unwrap();
        assert_eq!(fs::read(&file_path).unwrap(), b"a");

        write_private_file(&file_path, b"b").unwrap();
        assert_eq!(fs::read(&file_path).unwrap(), b"b");
        let md = fs::symlink_metadata(&file_path).unwrap();
        assert_eq!(md.permissions().mode() & 0o777, 0o600);
    }

    #[test]
    #[cfg(unix)]
    fn write_private_file_unlinks_planted_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let victim = tmp.path().join("victim.txt");
        fs::write(&victim, b"keep").unwrap();

        let target = tmp.path().join("planted.txt");
        std::os::unix::fs::symlink(&victim, &target).unwrap();

        write_private_file(&target, b"new").unwrap();

        // Victim remains untouched
        assert_eq!(fs::read(&victim).unwrap(), b"keep");
        // Target is now a regular 0600 file
        let md = fs::symlink_metadata(&target).unwrap();
        assert!(md.is_file());
        assert!(!md.file_type().is_symlink());
        assert_eq!(md.permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::read(&target).unwrap(), b"new");
    }

    #[test]
    fn write_private_file_refuses_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let dir_target = tmp.path().join("subdir");
        fs::create_dir(&dir_target).unwrap();

        let err = write_private_file(&dir_target, b"data").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::PermissionDenied);
    }

    #[test]
    #[cfg(unix)]
    fn read_secret_file_accepts_private_token() {
        let tmp = tempfile::tempdir().unwrap();
        let token_path = tmp.path().join("token.txt");
        let token = "0123456789abcdef0123\n";
        fs::write(&token_path, token.as_bytes()).unwrap();
        fs::set_permissions(&token_path, Permissions::from_mode(0o600)).unwrap();

        let read = read_secret_file(&token_path).unwrap();
        assert_eq!(read, "0123456789abcdef0123");
    }

    #[test]
    #[cfg(unix)]
    fn read_secret_file_rejects_group_readable() {
        let tmp = tempfile::tempdir().unwrap();
        let token_path = tmp.path().join("token.txt");
        let token = "0123456789abcdef0123\n";
        fs::write(&token_path, token.as_bytes()).unwrap();
        fs::set_permissions(&token_path, Permissions::from_mode(0o640)).unwrap();

        let err = read_secret_file(&token_path).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::PermissionDenied);
        assert!(!err.to_string().contains("0123456789abcdef"));
    }

    #[test]
    #[cfg(unix)]
    fn read_secret_file_rejects_short_and_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let short_path = tmp.path().join("short.txt");
        fs::write(&short_path, b"short\n").unwrap();
        fs::set_permissions(&short_path, Permissions::from_mode(0o600)).unwrap();

        let err = read_secret_file(&short_path).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);

        let valid_target = tmp.path().join("valid.txt");
        fs::write(&valid_target, b"0123456789abcdef0123\n").unwrap();
        fs::set_permissions(&valid_target, Permissions::from_mode(0o600)).unwrap();

        let symlink = tmp.path().join("symlink_token.txt");
        std::os::unix::fs::symlink(&valid_target, &symlink).unwrap();

        let err = read_secret_file(&symlink).unwrap_err();
        // O_NOFOLLOW on symlink causes ELOOP / OS error
        assert!(err.kind() == ErrorKind::PermissionDenied || err.raw_os_error() == Some(libc::ELOOP) || err.kind() == ErrorKind::InvalidInput || err.kind() == ErrorKind::Other);
    }

    #[test]
    fn constant_time_eq_matches_semantics() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hell"));
        assert!(!constant_time_eq(b"hell", b"hello"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn unique_suffix_differs_on_consecutive_calls() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            let s = unique_suffix();
            assert!(seen.insert(s), "unique_suffix generated duplicate");
        }
    }
}
