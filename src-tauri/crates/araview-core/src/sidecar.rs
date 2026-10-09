//! Shared helpers for JPEG sidecars, thumbnails and archive extracts.
//!
//! `heif.rs` / `psd_sidecar.rs` / `thumbnail.rs` previously duplicated the
//! file-identity hash, per-file locks, atomic publish and RGB downscaling.
//! All of that lives here now so cache naming and races are fixed in one place.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    LazyLock, Mutex,
};
use std::time::{Duration, UNIX_EPOCH};

use crate::app_error::{AppError, ErrorCode};
use crate::process_temp::process_temp_dir;
use crate::stable_hash::StableHasher;

/// Decoded RGB8 pixels shared by HEIC/HEIF and PSD pipelines.
#[derive(Debug, Clone)]
pub struct Rgb8 {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

/// Subdirectory holding the JPEG sidecars painted for HEIC/HEIF and PSD.
pub const PAINT_SUBDIR: &str = "paint";

/// Cap for [`PAINT_SUBDIR`]. Both sidecar pipelines share the directory, so the
/// limit lives here rather than being restated per format.
pub const MAX_PAINT_BYTES: u64 = 500 * 1024 * 1024;

/// The paint cache directory, created on first use.
pub fn paint_dir() -> Result<PathBuf, AppError> {
    let dir = process_temp_dir()?.join(PAINT_SUBDIR);
    fs::create_dir_all(&dir)
        .map_err(|e| AppError::io("Failed to create paint dir", e, ErrorCode::Unknown))?;
    Ok(dir)
}

static NAMED_LOCKS: LazyLock<Mutex<HashMap<String, ArcMutex>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

type ArcMutex = std::sync::Arc<Mutex<()>>;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Run `f` under a per-key mutex. The map entry is removed when no other
/// thread holds a clone, so long sessions do not leak lock entries.
/// A poisoned slot (previous holder panicked) is recovered instead of
/// failing that key for the rest of the process.
pub fn with_file_lock<T>(
    key: &str,
    context: &str,
    f: impl FnOnce() -> Result<T, AppError>,
) -> Result<T, AppError> {
    let slot = {
        let mut map = NAMED_LOCKS
            .lock()
            .map_err(|_| AppError::lock_poisoned(context))?;
        map.entry(key.to_string())
            .or_insert_with(|| std::sync::Arc::new(Mutex::new(())))
            .clone()
    };
    let guard = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let result = f();
    drop(guard);
    // 복제는 맵 잠금 안에서만 생기므로, 잠금을 쥔 채 자기 복제본을 놓고 남은
    // 참조가 맵 하나뿐이면 지운다. 동시에 끝난 스레드들 중 마지막 하나가 반드시
    // 지우게 된다 (각자 자기 복제본을 쥔 채 세면 서로를 보고 둘 다 남길 수 있다).
    if let Ok(mut map) = NAMED_LOCKS.lock() {
        drop(slot);
        if map
            .get(key)
            .is_some_and(|entry| std::sync::Arc::strong_count(entry) == 1)
        {
            map.remove(key);
        }
    }
    result
}

/// Stable identity hash for a source file: canonical path + mtime + length
/// plus caller-supplied discriminator bytes (e.g. max_side).
pub fn file_identity_hash(source: &Path, extra: &[u8]) -> Result<u64, AppError> {
    let canonical = fs::canonicalize(source).unwrap_or_else(|_| source.to_path_buf());
    let meta = fs::metadata(source)
        .map_err(|e| AppError::io("Failed to read metadata", e, ErrorCode::Corrupt))?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let is_cache_file = crate::process_temp::is_canonical_cache_path(&canonical);
    let mut hasher = StableHasher::new();
    hasher.write_u64_le(crate::process_temp::CACHE_FORMAT_REVISION);
    hasher.write(canonical.to_string_lossy().as_bytes());
    hasher.write(&[if is_cache_file { 0xfd } else { 0xfc }]);
    if !is_cache_file {
        hasher.write_u128_le(mtime);
    }
    hasher.write_u64_le(meta.len());
    if !extra.is_empty() {
        hasher.write(&[0xfe]);
        hasher.write(extra);
    }
    Ok(hasher.finish())
}

/// Stable hash of an archive entry name (path traversal is irrelevant here
/// because extracts are flattened to basename + hash prefix).
pub fn entry_name_hash(entry_name: &str) -> u64 {
    let mut hasher = StableHasher::new();
    hasher.write(entry_name.as_bytes());
    hasher.finish()
}

/// Unique `pid-threadid-counter` suffix so concurrent publishers of the same
/// `dest` never share a temp file.
fn tmp_suffix() -> String {
    let count = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let thread_id = format!("{:?}", std::thread::current().id());
    let thread_id: String = thread_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let pid = std::process::id();
    format!("{pid}-{thread_id}-{count}")
}

/// Temp path next to `dest` preserving its extension, for writers that infer
/// the encoder from the file name (`image::DynamicImage::save`).
pub fn tmp_path_for(dest: &Path) -> PathBuf {
    let ext = dest.extension().and_then(|e| e.to_str()).unwrap_or("tmp");
    let stem = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("tmp");
    dest.with_file_name(format!("{stem}.tmp-{}.{ext}", tmp_suffix()))
}

/// Extension of [`scratch_path_for`] temp files. Not an image extension, so a
/// leftover from an interrupted write is never listed as a picture.
pub const SCRATCH_EXT: &str = "araview-part";

/// Temp path next to `dest` that drops the image extension. Use it when
/// writing into a user directory, where a crash would otherwise leave a file
/// that `is_supported_file` happily lists as an image.
pub fn scratch_path_for(dest: &Path) -> PathBuf {
    let stem = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("tmp");
    dest.with_file_name(format!(".{stem}.tmp-{}.{SCRATCH_EXT}", tmp_suffix()))
}

/// 다른 pid의 잔재라도 mtime이 이 값보다 최근이면 살려둔다. 설치본/개발본
/// 같은 다른 실행 중인 인스턴스의 진행 중 저장을 pid만으로 구분할 수 없으므로
/// 방금 시작된 쓰기는 건드리지 않는다.
const SCRATCH_MIN_AGE: Duration = Duration::from_secs(5 * 60);

/// Remove leftovers of [`scratch_path_for`] in `dir`. Only files carrying our
/// own extension are touched, so nothing the user owns can be deleted, and
/// only ones from other processes, so a concurrent save is never disturbed.
pub fn sweep_scratch_files(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mine = format!(".tmp-{}-", std::process::id());
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(SCRATCH_EXT) {
            continue;
        }
        let is_mine = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains(&mine));
        if is_mine {
            continue;
        }
        let too_recent = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age < SCRATCH_MIN_AGE);
        if too_recent {
            continue;
        }
        let _ = fs::remove_file(&path);
    }
}

/// Rename `tmp` onto `dest`. When another thread already published `dest`,
/// the redundant temp file is dropped and `Ok` is returned (cache content
/// is interchangeable). Save paths with unique content must not use this;
/// they need a strict rename instead.
pub fn publish_atomic(tmp: &Path, dest: &Path, context: &str) -> Result<(), AppError> {
    match fs::rename(tmp, dest) {
        Ok(()) => Ok(()),
        Err(e) if dest.exists() => {
            // 동시 발행이 이겼거나 rename이 공유 위반으로 막힌 경우다.
            // 전자는 정상, 후자는 구 content가 남는다는 뜻이라 로그로 남긴다.
            log::warn!(
                "[sidecar] rename failed but dest exists, keeping existing file: {context}: {e}"
            );
            let _ = fs::remove_file(tmp);
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_file(tmp);
            Err(AppError::unknown(format!("{context}: {e}")))
        }
    }
}

/// Encode RGB8 pixels as JPEG through a temp file, then publish atomically.
pub fn write_rgb8_jpeg_atomic(dest: &Path, rgb: &Rgb8, quality: u8) -> Result<(), AppError> {
    use jpeg_encoder::{ColorType, Encoder};

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::io("Failed to create paint dir", e, ErrorCode::Unknown))?;
    }
    let tmp = tmp_path_for(dest);
    let width =
        u16::try_from(rgb.width).map_err(|_| AppError::corrupt("image too wide for JPEG"))?;
    let height =
        u16::try_from(rgb.height).map_err(|_| AppError::corrupt("image too tall for JPEG"))?;
    let result = (|| {
        let encoder = Encoder::new_file(&tmp, quality)
            .map_err(|e| AppError::unknown(format!("Failed to open JPEG sidecar: {e}")))?;
        encoder
            .encode(&rgb.bytes, width, height, ColorType::Rgb)
            .map_err(|e| AppError::unknown(format!("Failed to encode JPEG sidecar: {e}")))
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    publish_atomic(&tmp, dest, "Failed to publish JPEG sidecar")
}

/// One JPEG-sidecar pipeline (HEIC/HEIF, PSD, ...). Formats differ only in how
/// they decode to RGB8 and in their cache file names, so the lock, cache-hit,
/// downscale, atomic publish and eviction steps live here once.
pub struct SidecarSpec {
    /// Lock context label.
    pub label: &'static str,
    /// Cache file name prefix for full-size sidecars (`{prefix}{hash}.jpg`).
    pub paint_prefix: &'static str,
    /// Cache file name prefix for thumbnail sidecars.
    pub thumb_prefix: &'static str,
    pub decode: fn(&Path) -> Result<Rgb8, AppError>,
}

impl SidecarSpec {
    pub fn paint_path(&self, source: &Path) -> Result<PathBuf, AppError> {
        let hash = file_identity_hash(source, &[])?;
        Ok(paint_dir()?.join(format!("{}{hash:016x}.jpg", self.paint_prefix)))
    }

    pub fn thumb_path(&self, source: &Path, max_side: u32) -> Result<PathBuf, AppError> {
        let hash = file_identity_hash(source, &max_side.to_le_bytes())?;
        Ok(paint_dir()?.join(format!("{}{hash:016x}.jpg", self.thumb_prefix)))
    }

    pub fn ensure(&self, source: &Path) -> Result<PathBuf, AppError> {
        let dest = self.paint_path(source)?;
        self.publish(&dest, 90, downscale_to_fit_u16, source)
    }

    /// Lightweight thumbnail sidecar: full decode, then downscale to `max_side`
    /// and cache separately under `paint/`.
    pub fn ensure_thumb(&self, source: &Path, max_side: u32) -> Result<PathBuf, AppError> {
        let max_side = max_side.clamp(32, 1024);
        let dest = self.thumb_path(source, max_side)?;
        self.publish(
            &dest,
            80,
            |rgb| downscale_to_fit_u16(downscale_rgb8(rgb, max_side)),
            source,
        )
    }

    /// Cached thumbnail sidecar path only (never generates).
    pub fn cached_thumb(&self, source: &Path, max_side: u32) -> Option<PathBuf> {
        let dest = self.thumb_path(source, max_side.clamp(32, 1024)).ok()?;
        if !dest.exists() {
            return None;
        }
        crate::process_temp::touch_cache_file(&dest);
        Some(dest)
    }

    fn publish(
        &self,
        dest: &Path,
        quality: u8,
        shape: impl FnOnce(Rgb8) -> Rgb8,
        source: &Path,
    ) -> Result<PathBuf, AppError> {
        let key = dest.to_string_lossy().into_owned();
        with_file_lock(&key, self.label, || {
            crate::process_temp::mark_in_use(dest);
            if dest.exists() {
                crate::process_temp::touch_cache_file(dest);
                return Ok(dest.to_path_buf());
            }
            let rgb = shape((self.decode)(source)?);
            write_rgb8_jpeg_atomic(dest, &rgb, quality)?;
            crate::process_temp::note_published(dest, MAX_PAINT_BYTES);
            Ok(dest.to_path_buf())
        })
    }
}

/// Scale to fit inside `max_side` preserving aspect ratio (never upscale).
pub fn downscale_rgb8(rgb: Rgb8, max_side: u32) -> Rgb8 {
    let max_side = max_side.max(1);
    if rgb.width <= max_side && rgb.height <= max_side {
        return rgb;
    }
    let scale = (f64::from(max_side) / f64::from(rgb.width.max(rgb.height))).min(1.0);
    let narrow_wide = (f64::from(rgb.width) * scale).round() as u32;
    let narrow_high = (f64::from(rgb.height) * scale).round() as u32;
    resize_rgb8(rgb, narrow_wide.max(1), narrow_high.max(1))
}

/// Shrink panoramas to fit the JPEG u16 dimension limit.
pub fn downscale_to_fit_u16(rgb: Rgb8) -> Rgb8 {
    const LIMIT: u32 = 65500;
    if rgb.width <= LIMIT && rgb.height <= LIMIT {
        return rgb;
    }
    let scale = (f64::from(LIMIT) / f64::from(rgb.width.max(rgb.height))).min(1.0);
    let narrow_wide = (f64::from(rgb.width) * scale).floor() as u32;
    let narrow_high = (f64::from(rgb.height) * scale).floor() as u32;
    resize_rgb8(rgb, narrow_wide.max(1), narrow_high.max(1))
}

/// Takes `rgb` by value and resizes through a borrowed view. A full-resolution
/// decode is up to `MAX_DECODE_PIXELS * 3` bytes (~450MB), so the source buffer
/// is never copied just to be handed to `imageops::resize`.
fn resize_rgb8(rgb: Rgb8, narrow_wide: u32, narrow_high: u32) -> Rgb8 {
    let src_w = rgb.width.max(1);
    let src_h = rgb.height.max(1);
    let expected = (src_w as usize)
        .saturating_mul(src_h as usize)
        .saturating_mul(3);
    if rgb.bytes.len() < expected || narrow_wide == 0 || narrow_high == 0 {
        return rgb;
    }
    let view: image::ImageBuffer<image::Rgb<u8>, &[u8]> =
        match image::ImageBuffer::from_raw(src_w, src_h, rgb.bytes.as_slice()) {
            Some(value) => value,
            None => return rgb,
        };
    let resized = image::imageops::resize(
        &view,
        narrow_wide,
        narrow_high,
        image::imageops::FilterType::Triangle,
    );
    Rgb8 {
        width: narrow_wide,
        height: narrow_high,
        bytes: resized.into_raw(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmp_path_preserves_extension_and_is_unique() {
        let dest = Path::new("/tmp/abc.jpg");
        let first = tmp_path_for(dest);
        let second = tmp_path_for(dest);
        assert_ne!(first, second);
        assert_eq!(first.extension().and_then(|e| e.to_str()), Some("jpg"));
        assert_eq!(second.extension().and_then(|e| e.to_str()), Some("jpg"));
        assert_eq!(first.parent(), second.parent());
        assert!(crate::process_temp::is_inflight_temp_path(&first));
        assert!(crate::process_temp::is_inflight_temp_path(&second));
    }

    #[test]
    fn lock_entries_are_cleaned_up() {
        with_file_lock("sidecar-test-key", "test", || Ok(())).expect("lock");
        let still_there = NAMED_LOCKS
            .lock()
            .map(|map| map.contains_key("sidecar-test-key"))
            .unwrap_or(true);
        assert!(!still_there);
    }

    #[test]
    fn lock_entries_are_cleaned_up_after_concurrent_holders() {
        let key = "sidecar-test-concurrent";
        for _ in 0..50 {
            let threads: Vec<_> = (0..8)
                .map(|_| std::thread::spawn(move || with_file_lock(key, "test", || Ok(()))))
                .collect();
            for thread in threads {
                thread.join().expect("join").expect("lock");
            }
        }
        let still_there = NAMED_LOCKS
            .lock()
            .map(|map| map.contains_key(key))
            .unwrap_or(true);
        assert!(!still_there);
    }

    #[test]
    fn poisoned_lock_slot_is_recovered() {
        let key = "sidecar-test-poison";
        let panicked = std::panic::catch_unwind(|| {
            let _ = with_file_lock::<()>(key, "test", || panic!("boom"));
        });
        assert!(panicked.is_err());
        // 이전 홀더가 패닉했어도 같은 키의 락은 다시 쓸 수 있어야 한다.
        with_file_lock(key, "test", || Ok(())).expect("recovered lock");
    }
}
