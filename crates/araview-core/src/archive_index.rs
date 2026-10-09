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
//! changes with it (`sidecar::file_identity_hash` in `commands/archive.rs`), so a
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

/// Drop every cached index. Called from `cache::clear_cache` so "캐시 삭제"
/// in Settings also revalidates archive listings.
pub fn reset() {
    if let Ok(mut cache) = INDEX_CACHE.lock() {
        cache.clear();
    }
}
