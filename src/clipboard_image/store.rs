//! On-disk image store: private (`0700` dir, `0600` files via `secure_fs`), content-addressed,
//! LRU-evicted to a byte budget, never persisted across runs (orphans are swept at start).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::detect::{ImageInfo, ImageKind};
use crate::secure_fs;

/// File name prefix of every stored image (and of its temp siblings, `.img-*`).
const PREFIX: &str = "img-";
/// Files older than this are orphans regardless of owner (covers pid reuse and Windows).
const ORPHAN_AGE: Duration = Duration::from_secs(24 * 3600);

/// One image resident in the store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredImage {
    pub path: PathBuf,
    pub kind: ImageKind,
    pub bytes: usize,
    pub width: u32,
    pub height: u32,
    /// `true` when identical content was already stored (dedupe hit).
    pub reused: bool,
}

struct Entry {
    path: PathBuf,
    kind: ImageKind,
    bytes: usize,
    width: u32,
    height: u32,
}

/// Two independent 64-bit FNV-1a passes, hex-joined: 128 bits of non-cryptographic content id.
fn content_id(data: &[u8]) -> String {
    let (mut a, mut b) = (0xcbf2_9ce4_8422_2325u64, 0x8422_2325_cbf2_9ce4u64);
    for &x in data {
        a = (a ^ u64::from(x)).wrapping_mul(0x0100_0000_01b3);
        b = (b ^ u64::from(x)).wrapping_mul(0x0000_0100_0000_01b3).rotate_left(5);
    }
    format!("{a:016x}{b:016x}")
}

/// The store. Entries are kept least-recently-used first.
pub struct ImageStore {
    dir: PathBuf,
    entries: Vec<Entry>,
    total: u64,
}

impl ImageStore {
    /// Opens (creating `0700`) `dir` and sweeps orphans left by earlier runs.
    pub fn open(dir: &Path) -> io::Result<Self> {
        secure_fs::ensure_private_dir(dir)?;
        let store = Self { dir: dir.to_path_buf(), entries: Vec::new(), total: 0 };
        store.sweep_orphans();
        Ok(store)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn total_bytes(&self) -> u64 {
        self.total
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn sweep_orphans(&self) {
        let me = std::process::id();
        let Ok(rd) = fs::read_dir(&self.dir) else { return };
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().into_owned();
            let stem = name.strip_prefix('.').unwrap_or(&name);
            let Some(rest) = stem.strip_prefix(PREFIX) else { continue };
            let owner: Option<u32> = rest.split('-').next().and_then(|p| p.parse().ok());
            let old = ent
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| SystemTime::now().duration_since(t).ok())
                .is_some_and(|age| age > ORPHAN_AGE);
            let dead_owner = owner.is_none_or(|p| p == me || !pid_alive(p));
            if old || dead_owner {
                let _ = fs::remove_file(ent.path());
            }
        }
    }

    /// Stores `bytes` (already sniffed as `info`), deduping by content and evicting the least
    /// recently used files until the total stays within `max_disk`.
    pub fn put(&mut self, bytes: &[u8], info: ImageInfo, max_disk: u64) -> io::Result<StoredImage> {
        let name = format!("{PREFIX}{}-{}.{}", std::process::id(), content_id(bytes), info.kind.ext());
        let path = self.dir.join(&name);
        if let Some(i) = self.entries.iter().position(|e| e.path == path) {
            if path.is_file() {
                let e = self.entries.remove(i);
                let out = StoredImage { path: e.path.clone(), kind: e.kind, bytes: e.bytes, width: e.width, height: e.height, reused: true };
                self.entries.push(e);
                return Ok(out);
            }
            let gone = self.entries.remove(i);
            self.total -= gone.bytes as u64;
        }
        let need = bytes.len() as u64;
        if need > max_disk {
            return Err(io::Error::other("image exceeds the disk budget"));
        }
        while self.total + need > max_disk && !self.entries.is_empty() {
            let old = self.entries.remove(0);
            let _ = fs::remove_file(&old.path);
            self.total -= old.bytes as u64;
        }
        secure_fs::write_private_file(&path, bytes)?;
        self.total += need;
        self.entries.push(Entry { path: path.clone(), kind: info.kind, bytes: bytes.len(), width: info.width, height: info.height });
        Ok(StoredImage { path, kind: info.kind, bytes: bytes.len(), width: info.width, height: info.height, reused: false })
    }

    /// Deletes every stored file (used by `:clipboard clear`-style cleanup and tests).
    pub fn clear(&mut self) {
        for e in self.entries.drain(..) {
            let _ = fs::remove_file(e.path);
        }
        self.total = 0;
    }
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    let Ok(p) = i32::try_from(pid) else { return false };
    if p <= 0 {
        return false;
    }
    // SAFETY: signal 0 only probes existence/permission; it delivers nothing.
    let r = unsafe { libc::kill(p, 0) };
    r == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn pid_alive(_pid: u32) -> bool {
    // No cheap portable probe: rely on the age rule for other owners' files.
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(kind: ImageKind) -> ImageInfo {
        ImageInfo { kind, width: 2, height: 3 }
    }

    fn tmp(tag: &str) -> PathBuf {
        let base = std::env::var_os("CARGO_TARGET_TMPDIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let d = base.join(format!("bimg-{tag}-{}", secure_fs::unique_suffix()));
        fs::create_dir_all(&d).unwrap();
        d.join("store")
    }

    #[test]
    fn put_writes_private_file_and_dedupes() {
        let dir = tmp("dedupe");
        let mut s = ImageStore::open(&dir).unwrap();
        let a = s.put(b"abcdef", info(ImageKind::Png), 1000).unwrap();
        assert!(!a.reused && a.path.is_file());
        assert_eq!(fs::read(&a.path).unwrap(), b"abcdef");
        let b = s.put(b"abcdef", info(ImageKind::Png), 1000).unwrap();
        assert!(b.reused);
        assert_eq!(a.path, b.path);
        assert_eq!((s.len(), s.total_bytes()), (1, 6));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&a.path).unwrap().permissions().mode() & 0o777, 0o600);
            assert_eq!(fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        }
        let name = a.path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("img-") && name.ends_with(".png"));
    }

    #[test]
    fn lru_eviction_keeps_total_within_budget() {
        let dir = tmp("lru");
        let mut s = ImageStore::open(&dir).unwrap();
        let a = s.put(b"aaaa", info(ImageKind::Png), 10).unwrap();
        let b = s.put(b"bbbb", info(ImageKind::Png), 10).unwrap();
        // touch `a` so `b` becomes least recently used
        assert!(s.put(b"aaaa", info(ImageKind::Png), 10).unwrap().reused);
        let c = s.put(b"cccc", info(ImageKind::Png), 10).unwrap();
        assert!(a.path.is_file() && c.path.is_file());
        assert!(!b.path.exists(), "LRU entry evicted from disk");
        assert!(s.total_bytes() <= 10);
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn single_image_over_budget_is_refused_without_writing() {
        let dir = tmp("over");
        let mut s = ImageStore::open(&dir).unwrap();
        assert!(s.put(&[0u8; 20], info(ImageKind::Png), 10).is_err());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
    }

    #[test]
    fn deleted_file_is_rewritten_not_reported_reused() {
        let dir = tmp("gone");
        let mut s = ImageStore::open(&dir).unwrap();
        let a = s.put(b"xyz", info(ImageKind::Gif), 100).unwrap();
        fs::remove_file(&a.path).unwrap();
        let b = s.put(b"xyz", info(ImageKind::Gif), 100).unwrap();
        assert!(!b.reused && b.path.is_file());
        assert_eq!(s.total_bytes(), 3);
    }

    #[test]
    fn startup_sweep_removes_orphans_but_not_foreign_files() {
        let dir = tmp("sweep");
        secure_fs::ensure_private_dir(&dir).unwrap();
        let mine = dir.join(format!("img-{}-deadbeef.png", std::process::id()));
        let temp = dir.join(format!(".img-{}-x.tmp.1", std::process::id()));
        let malformed = dir.join("img-notapid-1.png");
        let foreign = dir.join("keep.txt");
        for p in [&mine, &temp, &malformed, &foreign] {
            fs::write(p, b"x").unwrap();
        }
        let _s = ImageStore::open(&dir).unwrap();
        assert!(!mine.exists() && !temp.exists() && !malformed.exists());
        assert!(foreign.exists());
    }

    #[test]
    fn clear_removes_files() {
        let dir = tmp("clear");
        let mut s = ImageStore::open(&dir).unwrap();
        let a = s.put(b"q", info(ImageKind::Bmp), 100).unwrap();
        s.clear();
        assert!(!a.path.exists() && s.is_empty() && s.total_bytes() == 0);
    }
}
