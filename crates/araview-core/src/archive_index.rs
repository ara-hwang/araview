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
//! changes with it (`sidecar::archive_identity_hash` in `ops/archive.rs`), so a
//! stale index never serves extracts for the wrong archive content.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};

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

/// One request's view of an archive: its canonical path and OS identity, read
/// once so the index check and the extract directory name agree without
/// opening the file twice.
pub(crate) struct ArchiveSource {
    canonical: PathBuf,
    identity: crate::file_identity::ArchiveCacheIdentity,
}

impl ArchiveSource {
    pub(crate) fn open(archive_path: &Path) -> Result<Self, AppError> {
        Ok(Self {
            canonical: normalize(archive_path),
            identity: identity(archive_path)?,
        })
    }

    /// Stable key for this archive's derived caches (extract directory name).
    pub(crate) fn cache_hash(&self) -> u64 {
        crate::sidecar::archive_identity_hash(&self.canonical, self.identity)
    }
}

/// Sorted image entries for the archive, served from cache when valid.
/// Also returns every non-hidden entry so `comic_info` can locate
/// `ComicInfo.xml` without a second full scan.
pub(crate) fn get_archive_entries(archive_path: &Path) -> Result<ArchiveEntries, AppError> {
    get_archive_entries_for(archive_path, &ArchiveSource::open(archive_path)?)
}

/// [`get_archive_entries`] for an archive whose identity the caller already read.
pub(crate) fn get_archive_entries_for(
    archive_path: &Path,
    source: &ArchiveSource,
) -> Result<ArchiveEntries, AppError> {
    if let Some(entries) = cached_entries(source) {
        return Ok(entries);
    }
    let entries = list_archive_entries(archive_path)?;
    remember(source, &entries);
    Ok(entries)
}

/// Entry list file kept in the archive's extract directory. The directory name
/// already carries the archive identity, so the file needs no validation key of
/// its own; a format change gets a new file name.
const STORED_INDEX_NAME: &str = "entries-v1.json";

/// Larger files are ignored and rebuilt from the archive.
const MAX_STORED_INDEX_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Deserialize)]
struct StoredIndex {
    entries: Vec<String>,
}

#[derive(Serialize)]
struct StoredIndexRef<'a> {
    entries: &'a [String],
}

/// [`get_archive_entries_for`] that also keeps the entry list on disk in
/// `sub_dir` (the archive's extract directory), so a restart reads a small file
/// instead of parsing the archive's central directory again.
pub(crate) fn get_archive_entries_stored(
    archive_path: &Path,
    source: &ArchiveSource,
    sub_dir: &Path,
) -> Result<ArchiveEntries, AppError> {
    if let Some(entries) = cached_entries(source) {
        return Ok(entries);
    }
    let stored_path = sub_dir.join(STORED_INDEX_NAME);
    let entries = match read_stored(&stored_path) {
        Some(entries) => {
            crate::process_temp::touch_cache_file(&stored_path);
            entries
        }
        None => {
            let entries = list_archive_entries(archive_path)?;
            // The listing reopened the file. If it was replaced in between, these
            // entries belong to another identity's directory.
            if identity(archive_path).is_ok_and(|now| now == source.identity) {
                write_stored(&stored_path, &entries.all);
            }
            entries
        }
    };
    remember(source, &entries);
    Ok(entries)
}

fn read_stored(path: &Path) -> Option<ArchiveEntries> {
    let len = fs::metadata(path).ok()?.len();
    if len > MAX_STORED_INDEX_BYTES {
        return None;
    }
    let stored: StoredIndex = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    crate::archive::entries_from_stored(stored.entries)
}

/// Best effort: a failed write only means the next restart parses the archive.
fn write_stored(path: &Path, all: &[String]) {
    let Ok(bytes) = serde_json::to_vec(&StoredIndexRef { entries: all }) else {
        return;
    };
    let written =
        crate::sidecar::write_cache_atomic(path, "Failed to publish archive index", |tmp| {
            fs::write(tmp, &bytes)
                .map_err(|e| AppError::io("Failed to write archive index", e, ErrorCode::Unknown))
        });
    match written {
        Ok(()) => {
            if let Some(dir) = path.parent() {
                crate::process_temp::note_written(
                    dir,
                    bytes.len() as u64,
                    crate::archive::MAX_ARCHIVE_DIR_BYTES,
                );
            }
        }
        Err(e) => log::warn!("[archive-index] {}", e.message),
    }
}

fn cached_entries(source: &ArchiveSource) -> Option<ArchiveEntries> {
    let mut cache = INDEX_CACHE.lock().ok()?;
    let entry = cache.get_mut(&source.canonical)?;
    if entry.identity != source.identity {
        cache.remove(&source.canonical);
        return None;
    }
    entry.last_access = Instant::now();
    Some(ArchiveEntries {
        images: Arc::clone(&entry.images),
        all: Arc::clone(&entry.all),
    })
}

fn remember(source: &ArchiveSource, entries: &ArchiveEntries) {
    let canonical = source.canonical.clone();
    let cached = CachedIndex {
        images: Arc::clone(&entries.images),
        all: Arc::clone(&entries.all),
        identity: source.identity,
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

    use super::{
        get_archive_entries, get_archive_entries_stored, read_stored, ArchiveSource, INDEX_CACHE,
        STORED_INDEX_NAME,
    };

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
        let old_identity = super::ArchiveSource::open(&path).unwrap().cache_hash();
        let old_dir = extraction_root.join(format!("{old_identity:016x}"));
        let old_extracted =
            crate::archive::extract_archive_image(&path, "001.jpg", &old_dir).unwrap();
        assert_eq!(fs::read(&old_extracted).unwrap(), old_page);

        replace_contents(&path, &new_archive, modified);
        let new_identity = super::ArchiveSource::open(&path).unwrap().cache_hash();
        let new_dir = extraction_root.join(format!("{new_identity:016x}"));
        let new_extracted =
            crate::archive::extract_archive_image(&path, "001.jpg", &new_dir).unwrap();

        assert_ne!(new_identity, old_identity);
        assert_ne!(new_extracted, old_extracted);
        assert_eq!(fs::read(new_extracted).unwrap(), new_page);
    }

    /// Drops one archive from the memory index, as a restart would.
    fn forget(source: &ArchiveSource) {
        INDEX_CACHE.lock().unwrap().remove(&source.canonical);
    }

    /// After a restart the entry list comes from the extract directory, not
    /// from parsing the archive again.
    #[test]
    fn stored_index_is_read_once_the_memory_index_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.cbz");
        fs::write(&path, archive_bytes("002.jpg", b"page")).unwrap();
        let sub_dir = dir.path().join("extract");
        fs::create_dir_all(&sub_dir).unwrap();
        let source = ArchiveSource::open(&path).unwrap();

        let first = get_archive_entries_stored(&path, &source, &sub_dir).unwrap();
        assert_eq!(&*first.images, &["001.jpg", "002.jpg"]);
        let stored = sub_dir.join(STORED_INDEX_NAME);
        assert_eq!(
            &*read_stored(&stored).unwrap().images,
            &["001.jpg", "002.jpg"]
        );

        // Only the stored file says this, so seeing it proves the disk read.
        // Names are re-sorted and re-filtered with the current rules.
        fs::write(
            &stored,
            r#"{"entries":["010.jpg","ComicInfo.xml","9.jpg"]}"#,
        )
        .unwrap();
        forget(&source);
        let second = get_archive_entries_stored(&path, &source, &sub_dir).unwrap();

        assert_eq!(&*second.images, &["9.jpg", "010.jpg"]);
        assert_eq!(second.all.len(), 3);
    }

    /// A damaged or rule-breaking stored list is ignored, rebuilt from the
    /// archive, and replaced on disk.
    #[test]
    fn invalid_stored_index_falls_back_to_the_archive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.cbz");
        fs::write(&path, archive_bytes("002.jpg", b"page")).unwrap();
        let sub_dir = dir.path().join("extract");
        fs::create_dir_all(&sub_dir).unwrap();
        let source = ArchiveSource::open(&path).unwrap();
        let stored = sub_dir.join(STORED_INDEX_NAME);

        for bad in [
            "not json",
            r#"{"entries":["001.jpg","__MACOSX/._001.jpg"]}"#,
            r#"{"entries":["chapter/"]}"#,
            r#"{"entries":[""]}"#,
        ] {
            fs::write(&stored, bad).unwrap();
            forget(&source);

            let entries = get_archive_entries_stored(&path, &source, &sub_dir).unwrap();

            assert_eq!(&*entries.images, &["001.jpg", "002.jpg"], "{bad}");
            assert_eq!(
                &*read_stored(&stored).unwrap().images,
                &["001.jpg", "002.jpg"],
                "{bad}"
            );
        }
    }
}
