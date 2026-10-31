//! Archive entry index cache (S12).
//!
//! `list_archive_images` used to re-open and re-scan the archive on every
//! call (`get_archive_images`, cover thumbnails, batch covers). Archives are
//! files, so a single file-handle metadata lookup per cache access is enough to
//! detect replacement; no filesystem watcher or content hashing is needed. Cached listings are
//! shared as `Arc` (no clone), and eviction is LRU by last access.
//!
//! Cache keys use the canonicalized archive path, mirroring `dir_cache`.
//! When the archive file changes, the derived-extract subdirectory name
//! changes with it (`sidecar::archive_file_identity_hash` in `ops/archive.rs`), so a
//! stale index never serves extracts for the wrong archive content.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Instant;

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
    identity: crate::file_identity::ArchiveCacheIdentity,
    last_access: Instant,
}

static INDEX_CACHE: LazyLock<Mutex<HashMap<PathBuf, CachedIndex>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Canonical path for identity. Falls back to the raw path when the archive
/// vanished between the caller check and here.
fn normalize(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn identity(path: &Path) -> Result<crate::file_identity::ArchiveCacheIdentity, AppError> {
    let file = fs::File::open(path)
        .map_err(|e| AppError::io("Failed to open archive", e, ErrorCode::Corrupt))?;
    crate::file_identity::ArchiveCacheIdentity::from_file(&file)
        .map_err(|e| AppError::io("Failed to read archive metadata", e, ErrorCode::Corrupt))
}

/// Sorted image entries for the archive, served from cache when valid.
/// Also returns every non-hidden entry so `comic_info` can locate
/// `ComicInfo.xml` without a second full scan.
pub(crate) fn get_archive_entries(archive_path: &Path) -> Result<ArchiveEntries, AppError> {
    let canonical = normalize(archive_path);
    let identity = identity(archive_path)?;

    {
        let Ok(mut cache) = INDEX_CACHE.lock() else {
            return list_archive_entries(archive_path);
        };
        if let Some(entry) = cache.get_mut(&canonical) {
            if entry.identity == identity {
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
        identity,
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

#[cfg(test)]
mod tests {
    use std::fs::{self, FileTimes, OpenOptions};
    use std::io::{Cursor, Write};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::get_archive_entries;

    fn archive_bytes(second_entry: &str, first_page: &[u8]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, contents) in [("001.jpg", first_page), (second_entry, &[0x5a; 8][..])] {
            writer
                .start_file(
                    name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn set_modified(path: &std::path::Path, modified: SystemTime) {
        OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(FileTimes::new().set_modified(modified))
            .unwrap();
    }

    fn replace_contents(path: &std::path::Path, bytes: &[u8], modified: SystemTime) {
        assert_eq!(fs::metadata(path).unwrap().len(), bytes.len() as u64);
        let replacement = path.with_extension("replacement");
        fs::write(&replacement, bytes).unwrap();
        set_modified(&replacement, modified);
        assert_eq!(
            fs::metadata(&replacement).unwrap().modified().unwrap(),
            modified
        );
        // Keep the original alive while creating the replacement so its OS file
        // ID must differ even when the filesystem has coarse change-time ticks.
        fs::remove_file(path).unwrap();
        fs::rename(replacement, path).unwrap();
        assert_eq!(fs::metadata(path).unwrap().len(), bytes.len() as u64);
        assert_eq!(fs::metadata(path).unwrap().modified().unwrap(), modified);
    }

    #[test]
    fn same_size_and_mtime_replacement_refreshes_cached_index() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("same.cbz");
        let old_page = vec![0x31; 16_825];
        let new_page = vec![0xc2; 16_825];
        let old_archive = archive_bytes("002.jpg", &old_page);
        let new_archive = archive_bytes("003.jpg", &new_page);
        assert_eq!(old_archive.len(), 17_035);
        assert_eq!(old_archive.len(), new_archive.len());
        fs::write(&path, &old_archive).unwrap();

        let modified =
            UNIX_EPOCH + Duration::from_secs(1_234_567_890) + Duration::from_nanos(123_456_789);
        set_modified(&path, modified);
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let old_entries = get_archive_entries(&path).unwrap();
        assert_eq!(&*old_entries.images, &["001.jpg", "002.jpg"]);

        replace_contents(&path, &new_archive, modified);
        let new_entries = get_archive_entries(&path).unwrap();

        assert_eq!(&*new_entries.images, &["001.jpg", "003.jpg"]);
    }

    #[test]
    fn same_size_and_mtime_replacement_uses_fresh_extracted_page() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("same.cbz");
        let old_page = vec![0x31; 16_825];
        let new_page = vec![0xc2; 16_825];
        let old_archive = archive_bytes("002.jpg", &old_page);
        let new_archive = archive_bytes("003.jpg", &new_page);
        assert_eq!(old_archive.len(), 17_035);
        assert_eq!(old_archive.len(), new_archive.len());
        fs::write(&path, &old_archive).unwrap();

        let modified =
            UNIX_EPOCH + Duration::from_secs(1_234_567_890) + Duration::from_nanos(123_456_789);
        set_modified(&path, modified);
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let extraction_root = dir.path().join("extracts");
        let old_identity = crate::sidecar::archive_file_identity_hash(&path).unwrap();
        let old_dir = extraction_root.join(format!("{old_identity:016x}"));
        let old_extracted =
            crate::archive::extract_archive_image(&path, "001.jpg", &old_dir).unwrap();
        assert_eq!(fs::read(&old_extracted).unwrap(), old_page);

        replace_contents(&path, &new_archive, modified);
        let new_identity = crate::sidecar::archive_file_identity_hash(&path).unwrap();
        let new_dir = extraction_root.join(format!("{new_identity:016x}"));
        let new_extracted =
            crate::archive::extract_archive_image(&path, "001.jpg", &new_dir).unwrap();

        assert_ne!(new_identity, old_identity);
        assert_ne!(new_extracted, old_extracted);
        assert_eq!(fs::read(new_extracted).unwrap(), new_page);
    }
}
