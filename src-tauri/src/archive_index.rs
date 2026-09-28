//! Archive entry index cache (S12).
//!
//! `list_archive_images` used to re-open and re-scan the archive on every
//! call (`get_archive_images`, cover thumbnails, batch covers). Archives are
//! files, so a single `fs::metadata` per lookup (mtime + size) is enough to
//! detect replacement; no filesystem watcher is needed. Cached listings are
//! shared as `Arc` (no clone), and eviction is LRU by last access.
//!
//! Cache keys use the canonicalized archive path, mirroring `dir_cache`.
//! When the archive file changes, the derived-extract subdirectory name
//! changes with it (`sidecar::file_identity_hash` in `commands.rs`), so a
//! stale index never serves extracts for the wrong archive content; the
//! mtime+size check still clears the old index entry promptly.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Instant, SystemTime};

use crate::app_error::{AppError, ErrorCode};
use crate::archive::{list_archive_entries, ArchiveEntries};

/// Upper bound for cached archive indexes; oldest entry evicted past this.
/// Folder-cover batches walk dozens of archives at once, so this must exceed
/// a typical folder's archive count (a 1000-entry listing is tens of KB).
const MAX_CACHE_ENTRIES: usize = 64;

struct CachedIndex {
    /// Image entries only (what the viewer pages through), sorted.
    images: Arc<Vec<String>>,
    /// Every non-directory, non-hidden entry (sorted), including non-image
    /// files such as ComicInfo.xml.
    all: Arc<Vec<String>>,
    mtime: Option<SystemTime>,
    size: u64,
    last_access: Instant,
}

static INDEX_CACHE: LazyLock<Mutex<HashMap<PathBuf, CachedIndex>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Canonical path for identity. Falls back to the raw path when the archive
/// vanished between the caller check and here.
fn normalize(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn identity(path: &Path) -> Result<(Option<SystemTime>, u64), AppError> {
    let meta = fs::metadata(path)
        .map_err(|e| AppError::io("Failed to read archive metadata", e, ErrorCode::Corrupt))?;
    Ok((meta.modified().ok(), meta.len()))
}

/// Sorted image entries for the archive, served from cache when valid.
/// Also returns every non-hidden entry so `comic_info` can locate
/// `ComicInfo.xml` without a second full scan.
pub(crate) fn get_archive_entries(archive_path: &Path) -> Result<ArchiveEntries, AppError> {
    let canonical = normalize(archive_path);
    let (mtime, size) = identity(archive_path)?;

    {
        let Ok(mut cache) = INDEX_CACHE.lock() else {
            return list_archive_entries(archive_path);
        };
        if let Some(entry) = cache.get_mut(&canonical) {
            if entry.mtime == mtime && entry.size == size {
                entry.last_access = Instant::now();
                return Ok(ArchiveEntries {
                    images: Arc::clone(&entry.images),
                    all: Arc::clone(&entry.all),
                });
            }
            // Identity changed: drop the stale entry and re-scan.
            cache.remove(&canonical);
        }
    }

    let entries = list_archive_entries(archive_path)?;
    let cached = CachedIndex {
        images: Arc::clone(&entries.images),
        all: Arc::clone(&entries.all),
        mtime,
        size,
        last_access: Instant::now(),
    };
    if let Ok(mut cache) = INDEX_CACHE.lock() {
        if cache.len() >= MAX_CACHE_ENTRIES {
            if let Some(oldest) = cache
                .iter()
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(key, _)| key.clone())
            {
                cache.remove(&oldest);
            }
        }
        cache.insert(canonical, cached);
    }
    Ok(entries)
}

/// Test/`.cbz`-table helper: whether the cache currently holds an entry.
#[cfg(test)]
pub(crate) fn contains_for_tests(archive_path: &Path) -> bool {
    let canonical = normalize(archive_path);
    INDEX_CACHE
        .lock()
        .map(|cache| cache.contains_key(&canonical))
        .unwrap_or(false)
}

/// Drop every cached index. Called from `cache::clear_cache` so "캐시 삭제"
/// in Settings also revalidates archive listings.
pub fn reset() {
    if let Ok(mut cache) = INDEX_CACHE.lock() {
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn unique_dir(suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tiv-arcidx-{suffix}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_cbz(dir: &Path, file_name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let archive_path = dir.join(file_name);
        let file = fs::File::create(&archive_path).expect("create cbz");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            writer.start_file(*name, options).expect("start entry");
            writer.write_all(data).expect("write entry");
        }
        writer.finish().expect("finish zip");
        archive_path
    }

    const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\nfake-png-bytes";

    #[test]
    fn cache_hit_serves_same_entries() {
        let dir = unique_dir("hit");
        let archive = write_cbz(
            &dir,
            "comic.cbz",
            &[("001.png", PNG_MAGIC), ("ComicInfo.xml", b"<x/>")],
        );

        let first = get_archive_entries(&archive).expect("first scan");
        assert_eq!(first.images.len(), 1);
        assert_eq!(first.all.len(), 2);
        assert!(contains_for_tests(&archive));

        let second = get_archive_entries(&archive).expect("cached");
        assert_eq!(second.images.len(), 1);
        assert_eq!(second.all.len(), 2);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cache_invalidates_when_archive_changes() {
        let dir = unique_dir("invalidate");
        let archive = write_cbz(&dir, "comic.cbz", &[("001.png", PNG_MAGIC)]);

        let first = get_archive_entries(&archive).expect("first scan");
        assert_eq!(first.images.len(), 1);
        assert!(contains_for_tests(&archive));

        // Rewrite with one more page (mtime + size both change).
        std::thread::sleep(std::time::Duration::from_millis(20));
        let rewritten = write_cbz(
            &dir,
            "comic.cbz",
            &[("001.png", PNG_MAGIC), ("002.png", PNG_MAGIC)],
        );

        let second = get_archive_entries(&rewritten).expect("rescan");
        assert_eq!(second.images.len(), 2);
        assert_eq!(second.all.len(), 2);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reset_clears_all_entries() {
        let dir = unique_dir("reset");
        let archive = write_cbz(&dir, "comic.cbz", &[("001.png", PNG_MAGIC)]);
        get_archive_entries(&archive).expect("populate");
        assert!(contains_for_tests(&archive));
        reset();
        assert!(!contains_for_tests(&archive));
        fs::remove_dir_all(&dir).ok();
    }
}
