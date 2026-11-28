//! Image paste: read an image off the host clipboard, validate and store it privately, and hand
//! back a file path the caller pastes into the pane (the owner-chosen default delivery).
//!
//! Resource bounds (all enforced before or while reading, never after):
//! - one image at most [`ImageLimits::max_image_bytes`] (reader stops one byte past it);
//! - at most [`ImageLimits::max_memory_bytes`] of read buffer reserved process-wide at once;
//! - at most [`ImageLimits::max_disk_bytes`] stored (LRU eviction);
//! - header dimensions capped before anything is written.
//!
//! Layout: `detect` (magic bytes + dimensions), `source` (mockable shell-out), `store` (disk LRU).

pub mod detect;
pub mod source;
pub mod store;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use detect::{ImageInfo, ImageKind};
pub use source::{ImageSource, ShellImageSource};
pub use store::{ImageStore, StoredImage};

const MIB: usize = 1024 * 1024;

/// Why an image paste did not happen. `Display` is the user-facing status line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageError {
    NoImage,
    TooLarge,
    Timeout,
    Busy,
    Disabled,
    NotAnImage(String),
    DimensionsTooLarge { width: u32, height: u32 },
    ToolMissing(String),
    Io(String),
}

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoImage => write!(f, "No image on the clipboard"),
            Self::TooLarge => write!(f, "Clipboard image exceeds the size limit"),
            Self::Timeout => write!(f, "Reading the clipboard image timed out"),
            Self::Busy => write!(f, "Another image paste is in progress (memory budget)"),
            Self::Disabled => write!(f, "Image paste is disabled ([clipboard.images] enabled = false)"),
            Self::NotAnImage(why) => write!(f, "Clipboard data is not a supported image: {why}"),
            Self::DimensionsTooLarge { width, height } => write!(f, "Clipboard image {width}x{height} exceeds the dimension limit"),
            Self::ToolMissing(why) => write!(f, "No clipboard image tool available: {why}"),
            Self::Io(e) => write!(f, "Cannot store clipboard image: {e}"),
        }
    }
}

impl std::error::Error for ImageError {}

/// How an image reaches the application (`[clipboard.images] delivery`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageDelivery {
    /// Save to a private file and bracket-paste its path (default).
    #[default]
    Path,
    /// Forward a raw `Ctrl+V` and let the child read the host clipboard itself (previous behaviour).
    Passthrough,
}

impl ImageDelivery {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Passthrough => "passthrough",
        }
    }
}

/// Resource budgets for one paste (`[clipboard.images]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageLimits {
    pub max_image_bytes: usize,
    pub max_memory_bytes: usize,
    pub max_disk_bytes: u64,
    pub read_timeout: Duration,
    pub max_dimension: u32,
    pub max_pixels: u64,
}

impl Default for ImageLimits {
    fn default() -> Self {
        Self {
            max_image_bytes: 10 * MIB,
            max_memory_bytes: 32 * MIB,
            max_disk_bytes: 256 * MIB as u64,
            read_timeout: Duration::from_millis(3000),
            max_dimension: 16_384,
            max_pixels: 100_000_000,
        }
    }
}

/// Per-pane image paste policy: delivery mode plus budgets. `Copy` so panes hold it by value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImagePasteSettings {
    pub enabled: bool,
    pub delivery: ImageDelivery,
    pub limits: ImageLimits,
}

impl ImagePasteSettings {
    /// Defaults with the feature on (config defaults agree; tests rely on it).
    pub fn on() -> Self {
        Self { enabled: true, ..Self::default() }
    }
}

static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
static STORE: Mutex<Option<ImageStore>> = Mutex::new(None);
static TEST_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
static TEST_SOURCE: Mutex<Option<Arc<dyn ImageSource>>> = Mutex::new(None);

/// Test hook: replace the clipboard image source (`None` restores the platform shell-out).
pub fn set_test_image_source(src: Option<Arc<dyn ImageSource>>) {
    if let Ok(mut g) = TEST_SOURCE.lock() {
        *g = src;
    }
}

/// Test hook: store files in `dir` instead of the runtime dir; drops any open store.
pub fn set_test_store_dir(dir: Option<PathBuf>) {
    if let Ok(mut g) = TEST_DIR.lock() {
        *g = dir;
    }
    if let Ok(mut g) = STORE.lock() {
        *g = None;
    }
}

/// Bytes of read buffer currently reserved by in-progress pastes.
pub fn in_flight_bytes() -> usize {
    IN_FLIGHT.load(Ordering::SeqCst)
}

struct Reservation(usize);

impl Reservation {
    fn take(bytes: usize, cap: usize) -> Result<Self, ImageError> {
        let mut cur = IN_FLIGHT.load(Ordering::SeqCst);
        loop {
            let next = cur.checked_add(bytes).filter(|n| *n <= cap).ok_or(ImageError::Busy)?;
            match IN_FLIGHT.compare_exchange(cur, next, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => return Ok(Self(bytes)),
                Err(seen) => cur = seen,
            }
        }
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        IN_FLIGHT.fetch_sub(self.0, Ordering::SeqCst);
    }
}

fn current_source() -> Result<Arc<dyn ImageSource>, ImageError> {
    if let Ok(g) = TEST_SOURCE.lock()
        && let Some(s) = g.as_ref()
    {
        return Ok(s.clone());
    }
    if crate::clipboard::automated_test_process() {
        return Err(ImageError::ToolMissing("automated test process".into()));
    }
    Ok(Arc::new(ShellImageSource::system()))
}

fn store_dir() -> Result<PathBuf, ImageError> {
    if let Ok(g) = TEST_DIR.lock()
        && let Some(d) = g.as_ref()
    {
        return Ok(d.clone());
    }
    let rt = crate::secure_fs::private_runtime_dir().map_err(|e| ImageError::Io(e.to_string()))?;
    Ok(rt.join("clipboard-images"))
}

/// Checks `info` against the dimension caps (pure).
pub fn check_dimensions(info: &ImageInfo, limits: &ImageLimits) -> Result<(), ImageError> {
    let over = info.width > limits.max_dimension
        || info.height > limits.max_dimension
        || u64::from(info.width) * u64::from(info.height) > limits.max_pixels;
    if over {
        return Err(ImageError::DimensionsTooLarge { width: info.width, height: info.height });
    }
    Ok(())
}

/// Validates `bytes` and stores them; split from [`acquire`] so it is testable without a source.
pub fn store_bytes(bytes: &[u8], limits: &ImageLimits) -> Result<StoredImage, ImageError> {
    if bytes.len() > limits.max_image_bytes {
        return Err(ImageError::TooLarge);
    }
    let info = detect::sniff(bytes)?;
    check_dimensions(&info, limits)?;
    let mut guard = STORE.lock().map_err(|_| ImageError::Io("image store poisoned".into()))?;
    if guard.is_none() {
        *guard = Some(ImageStore::open(&store_dir()?).map_err(|e| ImageError::Io(e.to_string()))?);
    }
    let store = guard.as_mut().ok_or_else(|| ImageError::Io("image store unavailable".into()))?;
    store.put(bytes, info, limits.max_disk_bytes).map_err(|e| ImageError::Io(e.to_string()))
}

/// Reads the clipboard image, validates it and stores it. Never buffers more than the budgets.
pub fn acquire(limits: &ImageLimits) -> Result<StoredImage, ImageError> {
    // Reserve the worst-case read buffer up front (cap enforced before any read).
    let _hold = Reservation::take(limits.max_image_bytes, limits.max_memory_bytes)?;
    let bytes = current_source()?.read(limits)?;
    store_bytes(&bytes, limits)
}

/// Deletes every stored image (diagnostics and tests).
pub fn clear_store() {
    if let Ok(mut g) = STORE.lock()
        && let Some(s) = g.as_mut()
    {
        s.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::detect::fixtures::png;
    use super::*;

    /// All tests below share process-global hooks; serialize them.
    static LOCK: Mutex<()> = Mutex::new(());

    struct Fixed(Result<Vec<u8>, ImageError>);
    impl ImageSource for Fixed {
        fn read(&self, _l: &ImageLimits) -> Result<Vec<u8>, ImageError> {
            self.0.clone()
        }
    }

    fn setup(tag: &str, res: Result<Vec<u8>, ImageError>) -> std::sync::MutexGuard<'static, ()> {
        let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let base = std::env::var_os("CARGO_TARGET_TMPDIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        set_test_store_dir(Some(base.join(format!("bimg-mod-{tag}-{}", crate::secure_fs::unique_suffix()))));
        set_test_image_source(Some(Arc::new(Fixed(res))));
        g
    }

    #[test]
    fn acquire_stores_a_valid_png() {
        let _g = setup("ok", Ok(png(32, 16)));
        let s = acquire(&ImageLimits::default()).unwrap();
        assert_eq!((s.kind, s.width, s.height, s.reused), (ImageKind::Png, 32, 16, false));
        assert!(s.path.is_file());
        assert_eq!(in_flight_bytes(), 0, "reservation released");
        assert!(acquire(&ImageLimits::default()).unwrap().reused);
    }

    #[test]
    fn non_image_and_oversize_dimensions_are_rejected_before_storing() {
        let _g = setup("bad", Ok(b"plain text clipboard".to_vec()));
        assert!(matches!(acquire(&ImageLimits::default()), Err(ImageError::NotAnImage(_))));
        set_test_image_source(Some(Arc::new(Fixed(Ok(png(20_000, 10))))));
        assert!(matches!(acquire(&ImageLimits::default()), Err(ImageError::DimensionsTooLarge { .. })));
        set_test_image_source(Some(Arc::new(Fixed(Ok(png(15_000, 15_000))))));
        assert!(matches!(acquire(&ImageLimits::default()), Err(ImageError::DimensionsTooLarge { .. })), "pixel cap");
        let dir = store_dir().unwrap();
        assert_eq!(std::fs::read_dir(dir).map(|d| d.count()).unwrap_or(0), 0, "nothing written");
    }

    #[test]
    fn size_cap_applies_to_source_output() {
        let _g = setup("big", Ok(png(2, 2)));
        let tiny = ImageLimits { max_image_bytes: 10, ..ImageLimits::default() };
        assert_eq!(acquire(&tiny), Err(ImageError::TooLarge));
        assert_eq!(in_flight_bytes(), 0);
    }

    #[test]
    fn memory_budget_is_reserved_before_reading() {
        let _g = setup("mem", Ok(png(2, 2)));
        let l = ImageLimits { max_image_bytes: 100, max_memory_bytes: 10, ..ImageLimits::default() };
        assert_eq!(acquire(&l), Err(ImageError::Busy));
        let held = Reservation::take(120, 200).unwrap();
        let l = ImageLimits { max_image_bytes: 100, max_memory_bytes: 200, ..ImageLimits::default() };
        assert_eq!(acquire(&l), Err(ImageError::Busy), "concurrent paste would exceed budget");
        drop(held);
        assert!(acquire(&l).is_ok());
    }

    #[test]
    fn source_errors_propagate_and_release_the_reservation() {
        let _g = setup("err", Err(ImageError::NoImage));
        assert_eq!(acquire(&ImageLimits::default()), Err(ImageError::NoImage));
        assert_eq!(in_flight_bytes(), 0);
    }

    #[test]
    fn delivery_serde_names() {
        assert_eq!(ImageDelivery::default().as_str(), "path");
        assert_eq!(ImageDelivery::Passthrough.as_str(), "passthrough");
    }
}
