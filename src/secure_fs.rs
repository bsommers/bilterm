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

/// Rejects a Unix peer whose uid differs from ours. Shared by the JSON-RPC and attach sockets.
pub fn check_peer_uid(peer_uid: u32, my_uid: u32) -> io::Result<()> {
    if peer_uid != my_uid {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("peer UID {peer_uid} does not match daemon UID {my_uid}"),
        ));
    }
    Ok(())
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

/// Recursively creates `dir`, giving every newly created component 0700 permissions.
///
/// Pre-existing components are left untouched (no chmod), so state directories created by
/// older versions keep their mode; files written inside should use [`write_private_file`].
pub fn create_private_dir_all(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(dir)
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
    let base_md = fs::metadata(base)?;
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
        && (Path::new(xdg).is_absolute() || xdg.to_string_lossy().starts_with('/'))
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

/// Replaces any existing file or dangling/planted symlink at `path` by writing a fresh 0600
/// temporary sibling file (`O_CREAT | O_EXCL | O_NOFOLLOW`) and atomically renaming it into place.
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
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "tmp".to_string());
    let tmp_path = parent.join(format!(".{}.tmp.{}", file_name, unique_suffix()));

    let mut file = create_private_file(&tmp_path)?;
    if let Err(e) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }
    drop(file);

    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }
    Ok(())
}

/// Flushes directory metadata so a completed `rename` survives power loss (best effort).
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Path of the last-good backup kept next to a durable state file (`<name>.bak`).
pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".bak");
    path.with_file_name(name)
}

/// Durable replacement of a state file: the current file becomes `<name>.bak` (hard link, so the
/// old content is never copied or truncated), then the new content is written to a sibling temp
/// file, fsynced, renamed into place, and the directory is fsynced.
pub fn write_private_file_durable(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if fs::symlink_metadata(path).is_ok_and(|md| md.is_file() && md.len() > 0) {
        let bak = backup_path(path);
        let staging = parent.join(format!(".bak.tmp.{}", unique_suffix()));
        if fs::hard_link(path, &staging).is_ok() && fs::rename(&staging, &bak).is_err() {
            let _ = fs::remove_file(&staging);
        }
    }
    write_private_file(path, bytes)?;
    sync_dir(parent);
    Ok(())
}

/// Where the content returned by [`read_with_backup`] came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurableSource {
    Primary,
    /// The primary file was missing, truncated or invalid; `<name>.bak` was used and written
    /// back as the primary. A corrupt primary is kept as `<name>.corrupt`.
    Backup,
}

/// Reads a durable state file, falling back to `<name>.bak` when the primary is unreadable or
/// rejected by `valid`. Errors only when neither copy is usable; never returns invalid content.
pub fn read_with_backup(path: &Path, valid: &dyn Fn(&str) -> bool) -> io::Result<(String, DurableSource)> {
    let primary = fs::read_to_string(path);
    if let Ok(text) = &primary
        && valid(text)
    {
        return Ok((text.clone(), DurableSource::Primary));
    }
    let bak = backup_path(path);
    match fs::read_to_string(&bak) {
        Ok(text) if valid(&text) => {
            if primary.is_ok() {
                let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
                name.push(".corrupt");
                let _ = fs::rename(path, path.with_file_name(name));
            }
            // Put the good copy back without rotating the corrupt file into the backup slot.
            write_private_file(path, text.as_bytes())?;
            sync_dir(path.parent().unwrap_or_else(|| Path::new(".")));
            Ok((text, DurableSource::Backup))
        }
        _ => match primary {
            Ok(_) => Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("{} is corrupt and no usable backup exists", path.display()),
            )),
            Err(e) => Err(e),
        },
    }
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

/// Generates a 256-bit lowercase hex token from the OS CSPRNG
/// (`/dev/urandom` on Unix, `BCryptGenRandom` on Windows).
pub fn random_token_hex() -> io::Result<String> {
    #[cfg(unix)]
    {
        use std::io::Read;
        let mut buf = [0u8; 32];
        fs::File::open("/dev/urandom")?.read_exact(&mut buf)?;
        Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
    }
    #[cfg(windows)]
    {
        #[link(name = "bcrypt")]
        unsafe extern "system" {
            fn BCryptGenRandom(
                h_algorithm: *mut std::ffi::c_void,
                pb_buffer: *mut u8,
                cb_buffer: u32,
                dw_flags: u32,
            ) -> i32;
        }
        const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;
        let mut buf = [0u8; 32];
        // Validated invariants for BCryptGenRandom:
        // 1. Null algorithm handle is valid when BCRYPT_USE_SYSTEM_PREFERRED_RNG is set.
        // 2. Returned NTSTATUS (0 == STATUS_SUCCESS) is checked before reading `buf`.
        // SAFETY: `buf` is a uniquely owned 32-byte writable buffer matching `buf.len() as u32`.
        let status = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                buf.as_mut_ptr(),
                buf.len() as u32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        if status != 0 {
            return Err(io::Error::other(format!("BCryptGenRandom failed: {status:#x}")));
        }
        Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "OS CSPRNG unavailable on this platform",
        ))
    }
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

/// Validates that `name` is a single plain file name (no separators, `.`/`..`, or NUL) so it cannot escape a confining directory.
pub fn validate_plain_file_name(name: &str) -> std::result::Result<(), String> {
    const MSG: &str = "ppm_path must be a file name within the snapshot directory";
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
        || name == "."
        || name == ".."
    {
        return Err(MSG.to_string());
    }
    let mut components = std::path::Path::new(name).components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None) => Ok(()),
        _ => Err(MSG.to_string()),
    }
}

/// `true` for Windows reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM0-9`, `LPT0-9`),
/// case-insensitively; such a stem opens the device instead of a file on Windows.
fn is_windows_device_name(name: &str) -> bool {
    let up = name.to_ascii_uppercase();
    matches!(up.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|p| up.strip_prefix(p).is_some_and(|d| d.len() == 1 && d.as_bytes()[0].is_ascii_digit()))
}

/// Validates that `name` is a safe identifier matching `^[A-Za-z0-9_-]{1,64}$` (`SEC-R01`, `D-03`).
///
/// Rejects empty names, names exceeding 64 characters, names containing directory separators
/// (`/`, `\`), dots (`.`, `..`), whitespace, or NUL bytes.
pub fn validate_safe_name(name: &str) -> std::result::Result<(), String> {
    if name.is_empty() {
        return Err("identifier cannot be empty".to_string());
    }
    if name.len() > 64 {
        return Err("identifier exceeds maximum length of 64 characters".to_string());
    }
    if name == "." || name == ".." {
        return Err("identifier cannot be '.' or '..'".to_string());
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("identifier must contain only ASCII alphanumeric, '-', or '_' characters".to_string());
    }
    if is_windows_device_name(name) {
        return Err("identifier is a reserved Windows device name".to_string());
    }
    Ok(())
}

/// Test-only: parent for temp dirs holding Unix sockets. macOS's per-user `$TMPDIR` (~50 bytes)
/// plus a socket name overflows `sun_path` (104), so socket tests root under `/tmp` instead.
#[cfg(test)]
pub(crate) fn short_test_root() -> std::path::PathBuf {
    if cfg!(unix) && std::path::Path::new("/tmp").is_dir() {
        "/tmp".into()
    } else {
        std::env::temp_dir()
    }
}

/// Test-only drop-in for `tempfile::tempdir()` rooted at [`short_test_root`] (`/tmp/btXXXXXX`).
#[cfg(test)]
pub(crate) fn short_tempdir() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("bt").tempdir_in(short_test_root()).expect("short tempdir")
}

#[cfg(test)]
mod tests {
    #[test]
    fn random_token_hex_is_64_hex_chars_and_unique() {
        let a = random_token_hex().unwrap();
        let b = random_token_hex().unwrap();
        assert_eq!(a.len(), 64, "{a}");
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()), "{a}");
        assert_ne!(a, b);
        assert_ne!(a, "0".repeat(64));
    }

    use super::*;
    #[cfg(unix)]
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

    #[test]
    fn validate_safe_name_accepts_valid_identifiers() {
        assert!(validate_safe_name("default").is_ok());
        assert!(validate_safe_name("worker-1").is_ok());
        assert!(validate_safe_name("sess_01").is_ok());
        assert!(validate_safe_name("A-Z_0-9").is_ok());
    }

    #[test]
    fn validate_safe_name_rejects_windows_device_names() {
        for n in ["CON", "con", "nul", "Aux", "prn", "com1", "COM9", "lpt1", "LPT0"] {
            assert!(validate_safe_name(n).is_err(), "{n}");
        }
        for n in ["console", "com10", "lpt", "com", "nulx", "con-1"] {
            assert!(validate_safe_name(n).is_ok(), "{n}");
        }
    }

    #[test]
    fn validate_safe_name_rejects_unsafe_identifiers() {
        assert!(validate_safe_name("").is_err());
        assert!(validate_safe_name(".").is_err());
        assert!(validate_safe_name("..").is_err());
        assert!(validate_safe_name("../foo").is_err());
        assert!(validate_safe_name("foo/bar").is_err());
        assert!(validate_safe_name("foo\\bar").is_err());
        assert!(validate_safe_name("foo bar").is_err());
        assert!(validate_safe_name("foo\0bar").is_err());
        let too_long = "a".repeat(65);
        assert!(validate_safe_name(&too_long).is_err());
    }
}
