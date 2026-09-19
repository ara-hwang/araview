//! Directory listing cache with filesystem-watcher invalidation (S1).
//!
//! `get_directory_images` used to rescan the whole folder on every call.
//! This module caches the sorted listing per directory + options and
//! invalidates it when:
//! - the directory mtime changed (cheap single-metadata check, covers
//!   changes that happened while unwatched, e.g. before the first scan), or
//! - the `notify` filesystem watcher reports a change under that directory.
//!
//! Recursive listings are watcher-gated only: a parent-dir mtime does not
//! reliably reflect nested changes, so without an active watcher they are
//! always rescanned (previous behavior).
//!
//! Cache keys use the canonicalized directory path (lowercased on Windows)
//! so `C:\Pics` and `c:\pics\` share one entry. Eviction is LRU by last
//! access, and watcher slots are trimmed together with evicted entries.
//! Canonicalization stays internal: the listing itself is scanned from the
//! caller's own path, because `fs::canonicalize` returns extended-length
//! paths (`\\?\D:\...`) that the shell and the UI cannot use. Releasing a
//! watcher slot also drops the listings that depended on it, so a cached
//! recursive listing is never served for an unwatched directory.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Instant, SystemTime};

use crate::app_error::{AppError, ErrorCode};
use crate::commands::{DirListOptions, DirSortKey};
use crate::image::is_supported_file;

/// Upper bound for cached directory listings; oldest entry evicted past this.
const MAX_CACHE_ENTRIES: usize = 128;

/// Upper bound for OS watcher slots. Trimmed together with cache eviction.
const MAX_WATCHED_DIRS: usize = 64;

#[derive(Debug, Clone)]
pub(crate) struct ImageEntry {
    pub(crate) path: String,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) size: u64,
}

struct CachedListing {
    parent: PathBuf,
    parent_key: String,
    dir_mtime: Option<SystemTime>,
    images: Vec<ImageEntry>,
    last_access: Instant,
}

static DIR_CACHE: LazyLock<Mutex<HashMap<String, CachedListing>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static WATCHED_DIRS: LazyLock<Mutex<HashMap<PathBuf, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static WATCHER: LazyLock<Mutex<Option<notify::RecommendedWatcher>>> =
    LazyLock::new(|| Mutex::new(None));

/// Canonicalize for identity. Falls back to the raw path when the directory
/// vanished between the caller check and here.
fn normalize_parent(parent: &Path) -> PathBuf {
    fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf())
}

/// Lowercased canonical string with trailing separator, for prefix matching
/// (`C:\foo` must not match `C:\foobar`). Rust `Path::starts_with` is
/// case-sensitive even on Windows, so string comparison is used.
fn parent_key(canonical: &Path) -> String {
    with_trailing_sep(canonical.to_string_lossy().to_lowercase())
}

fn with_trailing_sep(mut key: String) -> String {
    if !key.ends_with(['/', '\\']) {
        key.push(std::path::MAIN_SEPARATOR);
    }
    key
}

/// Cache key for an already-canonical parent (see [`normalize_parent`]).
/// Canonicalizing here too would cost a second syscall on every lookup.
pub(crate) fn cache_key(canonical: &Path, opts: &DirListOptions) -> String {
    let sort = match opts.sort_key {
        DirSortKey::Name => "name",
        DirSortKey::Date => "date",
        DirSortKey::Size => "size",
    };
    format!(
        "{}|{}|{}|{}",
        canonical.to_string_lossy().to_lowercase(),
        sort,
        opts.descending,
        opts.recursive
    )
}

fn dir_mtime(dir: &Path) -> Option<SystemTime> {
    fs::metadata(dir).and_then(|m| m.modified()).ok()
}

/// Sorted image entries for `parent`, served from cache when valid.
pub(crate) fn get_sorted_images(
    parent: &Path,
    opts: &DirListOptions,
) -> Result<Vec<ImageEntry>, AppError> {
    // `canonical`은 캐시 키/워처 식별에만 쓴다. Windows canonicalize는
    // `\\?\D:\...` 확장 경로를 돌려주는데, 그 문자열이 그대로 FE까지 가면
    // 탐색기 열기(`/select,`)와 경로 표시가 깨진다. 스캔은 호출자가 준
    // 원본 경로로 해서 기존 형태의 경로를 그대로 유지한다.
    let canonical = normalize_parent(parent);
    if !opts.recursive {
        let current = dir_mtime(&canonical);
        if let Ok(mut cache) = DIR_CACHE.lock() {
            if let Some(entry) = cache.get_mut(&cache_key(&canonical, opts)) {
                if entry.dir_mtime == current {
                    entry.last_access = Instant::now();
                    return Ok(entry.images.clone());
                }
            }
        }
        let mut images = Vec::new();
        collect_images(parent, false, &mut images)?;
        sort_images(&mut images, opts);
        ensure_watched(&canonical, false);
        insert_cache(canonical, opts, current, images.clone());
        Ok(images)
    } else {
        let key = cache_key(&canonical, opts);
        let watching = WATCHER.lock().map(|guard| guard.is_some()).unwrap_or(false);
        if watching {
            if let Ok(mut cache) = DIR_CACHE.lock() {
                if let Some(entry) = cache.get_mut(&key) {
                    entry.last_access = Instant::now();
                    return Ok(entry.images.clone());
                }
            }
        }
        let mut images = Vec::new();
        collect_images(parent, true, &mut images)?;
        sort_images(&mut images, opts);
        ensure_watched(&canonical, true);
        insert_cache(canonical, opts, None, images.clone());
        Ok(images)
    }
}

fn insert_cache(
    canonical_parent: PathBuf,
    opts: &DirListOptions,
    dir_mtime: Option<SystemTime>,
    images: Vec<ImageEntry>,
) {
    let Ok(mut cache) = DIR_CACHE.lock() else {
        return;
    };
    if cache.len() >= MAX_CACHE_ENTRIES {
        // LRU: evict the least recently used entry, then unwatch its
        // directory when nothing else references it.
        if let Some(oldest) = cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(key, _)| key.clone())
        {
            if let Some(evicted) = cache.remove(&oldest) {
                drop(cache);
                unwatch_if_unused(&evicted.parent);
                let Ok(mut cache) = DIR_CACHE.lock() else {
                    return;
                };
                insert_entry(&mut cache, canonical_parent, opts, dir_mtime, images);
                return;
            }
        }
    }
    insert_entry(&mut cache, canonical_parent, opts, dir_mtime, images);
}

fn insert_entry(
    cache: &mut HashMap<String, CachedListing>,
    canonical_parent: PathBuf,
    opts: &DirListOptions,
    dir_mtime: Option<SystemTime>,
    images: Vec<ImageEntry>,
) {
    cache.insert(
        cache_key(&canonical_parent, opts),
        CachedListing {
            parent_key: parent_key(&canonical_parent),
            parent: canonical_parent,
            dir_mtime,
            images,
            last_access: Instant::now(),
        },
    );
}

fn ensure_watched(canonical_parent: &Path, recursive: bool) {
    use notify::{RecursiveMode, Watcher};

    let mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };

    if let Ok(mut watched) = WATCHED_DIRS.lock() {
        // 재방문마다 타임스탬프를 갱신해야 trim이 실제 LRU로 동작한다.
        // 갱신하지 않으면 가장 자주 쓰는 폴더가 먼저 해제된다.
        if let Some(seen) = watched.get_mut(canonical_parent) {
            *seen = Instant::now();
            return;
        }
    }

    let Ok(mut guard) = WATCHER.lock() else {
        return;
    };
    if guard.is_none() {
        match notify::RecommendedWatcher::new(
            |res: Result<notify::Event, notify::Error>| match res {
                Ok(event) => {
                    for path in event.paths {
                        invalidate_for_path(&path);
                    }
                }
                Err(e) => log::warn!("[dir-cache] watch error: {e}"),
            },
            notify::Config::default(),
        ) {
            Ok(watcher) => *guard = Some(watcher),
            Err(e) => {
                log::warn!("[dir-cache] failed to start watcher: {e}");
                return;
            }
        }
    }
    if let Some(watcher) = guard.as_mut() {
        match watcher.watch(canonical_parent, mode) {
            Ok(()) => {
                drop(guard);
                trim_watched();
                if let Ok(mut watched) = WATCHED_DIRS.lock() {
                    watched.insert(canonical_parent.to_path_buf(), Instant::now());
                }
            }
            Err(e) => log::warn!(
                "[dir-cache] failed to watch {}: {e}",
                canonical_parent.display()
            ),
        }
    }
}

/// Keep watcher slots bounded. Called without holding the WATCHER lock.
fn trim_watched() {
    let oldest = WATCHED_DIRS.lock().ok().and_then(|watched| {
        if watched.len() < MAX_WATCHED_DIRS {
            return None;
        }
        watched
            .iter()
            .min_by_key(|(_, instant)| *instant)
            .map(|(path, _)| path.clone())
    });
    if let Some(path) = oldest {
        unwatch(&path);
    }
}

fn unwatch(canonical_parent: &Path) {
    if let Ok(mut guard) = WATCHER.lock() {
        if let Some(watcher) = guard.as_mut() {
            use notify::Watcher as _;
            watcher.unwatch(canonical_parent).ok();
        }
    }
    if let Ok(mut watched) = WATCHED_DIRS.lock() {
        watched.remove(canonical_parent);
    }
    // 워처가 없어진 디렉터리의 캐시를 남겨두면 재귀 목록이 영원히 stale해진다
    // (재귀 경로는 전역 `watching` 플래그만 보고 캐시를 그대로 돌려준다).
    // MAX_WATCHED_DIRS < MAX_CACHE_ENTRIES라 반드시 발생하는 조합이다.
    drop_cached_listings(canonical_parent);
}

/// Drop every cache entry whose listing depends on `canonical_parent`.
fn drop_cached_listings(canonical_parent: &Path) {
    let Ok(mut cache) = DIR_CACHE.lock() else {
        return;
    };
    cache.retain(|_, entry| entry.parent != *canonical_parent);
}

/// Remove the watcher slot when no cache entry references the directory.
/// Only called from non-callback threads (insert path), never from the
/// watcher callback itself, to avoid WATCHER/DIR_CACHE lock ordering issues.
fn unwatch_if_unused(canonical_parent: &Path) {
    let still_used = DIR_CACHE
        .lock()
        .map(|cache| {
            cache
                .values()
                .any(|entry| entry.parent == *canonical_parent)
        })
        .unwrap_or(true);
    if !still_used {
        unwatch(canonical_parent);
    }
}

fn invalidate_for_path(path: &Path) {
    // 이벤트 경로는 raw/canonical 어느 쪽으로 와도 매칭되게 후보를 모은다.
    // 삭제된 파일은 canonicalize가 실패하므로 부모 기준도 함께 검사한다.
    // 후보에도 구분자를 붙여야 `C:\foo`가 `C:\foobar`에 걸리지 않으면서
    // 감시 중인 디렉터리 자신에 대한 이벤트(삭제·이름 변경)까지 잡힌다.
    let mut candidates = vec![with_trailing_sep(path.to_string_lossy().to_lowercase())];
    if let Ok(canonical) = fs::canonicalize(path) {
        candidates.push(with_trailing_sep(
            canonical.to_string_lossy().to_lowercase(),
        ));
    }
    if let Some(parent) = path
        .parent()
        .and_then(|parent| fs::canonicalize(parent).ok())
    {
        candidates.push(with_trailing_sep(parent.to_string_lossy().to_lowercase()));
    }
    // Watcher-callback thread: touch DIR_CACHE only, never WATCHER.
    let Ok(mut cache) = DIR_CACHE.lock() else {
        return;
    };
    cache.retain(|_, entry| {
        !candidates
            .iter()
            .any(|candidate| candidate.starts_with(&entry.parent_key))
    });
}

fn collect_images(dir: &Path, recursive: bool, out: &mut Vec<ImageEntry>) -> Result<(), AppError> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = fs::read_dir(&current)
            .map_err(|e| AppError::io("Failed to read directory", e, ErrorCode::Corrupt))?;
        for entry in entries {
            let entry =
                entry.map_err(|e| AppError::corrupt(format!("Failed to read entry: {e}")))?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                // junction/symlink 같은 reparse point는 따라가지 않는다.
                // 순환 링크로 인한 무한 순회와 범위 밖 스캔을 막는다.
                if recursive && !is_reparse_point(&entry) {
                    stack.push(entry_path);
                }
                continue;
            }
            if !entry_path.is_file() || !is_supported_file(&entry_path) {
                continue;
            }
            let Some(path_str) = entry_path.to_str().map(str::to_string) else {
                continue;
            };
            let (modified, size) = entry
                .metadata()
                .map(|m| (m.modified().ok(), m.len()))
                .unwrap_or((None, 0));
            out.push(ImageEntry {
                path: path_str,
                modified,
                size,
            });
        }
    }
    Ok(())
}

/// 디렉터리 엔트리가 reparse point(junction/symlink)인지 판별한다.
fn is_reparse_point(entry: &fs::DirEntry) -> bool {
    if entry.file_type().map(|t| t.is_symlink()).unwrap_or(false) {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        entry
            .metadata()
            .map(|m| m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    false
}

fn sort_images(images: &mut [ImageEntry], opts: &DirListOptions) {
    match opts.sort_key {
        DirSortKey::Name => images.sort_by_key(|e| e.path.to_lowercase()),
        DirSortKey::Date => images.sort_by(|a, b| {
            a.modified
                .cmp(&b.modified)
                .then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase()))
        }),
        DirSortKey::Size => images.sort_by(|a, b| {
            a.size
                .cmp(&b.size)
                .then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase()))
        }),
    }
    if opts.descending {
        images.reverse();
    }
}

#[cfg(test)]
pub(crate) fn contains_for_tests(key: &str) -> bool {
    DIR_CACHE
        .lock()
        .map(|cache| cache.contains_key(key))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::DirListOptions;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_dir(suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tiv-dircache-{suffix}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_sized(dir: &Path, name: &str, size: usize) -> String {
        let path = dir.join(name);
        fs::write(&path, vec![0u8; size]).expect("write dummy");
        path.to_str().unwrap().to_string()
    }

    fn file_names(entries: &[ImageEntry]) -> Vec<String> {
        entries
            .iter()
            .map(|e| {
                Path::new(&e.path)
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn cache_hit_serves_same_listing() {
        let dir = unique_dir("hit");
        let anchor = write_sized(&dir, "a.png", 10);
        let parent = Path::new(&anchor).parent().unwrap().to_path_buf();
        let opts = DirListOptions::default();

        let first = get_sorted_images(&parent, &opts).expect("first scan");
        assert_eq!(file_names(&first), vec!["a.png"]);
        assert!(contains_for_tests(&cache_key(
            &normalize_parent(&parent),
            &opts
        )));

        // No FS change: second call must hit the cache and agree.
        let second = get_sorted_images(&parent, &opts).expect("cached");
        assert_eq!(file_names(&second), vec!["a.png"]);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cache_key_ignores_case_and_trailing_separator() {
        let dir = unique_dir("casekey");
        let opts = DirListOptions::default();
        let lower = cache_key(&normalize_parent(&dir), &opts);
        let upper = cache_key(
            &normalize_parent(Path::new(&dir.to_string_lossy().to_uppercase())),
            &opts,
        );
        assert_eq!(lower, upper);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn listing_keeps_caller_path_spelling() {
        // canonicalize는 Windows에서 확장 경로(`\\?\D:\...`)를 돌려준다. 그
        // 형태가 FE까지 가면 탐색기 열기와 경로 표시가 깨지므로, 목록은
        // 호출자가 준 철자를 그대로 유지해야 한다.
        let dir = unique_dir("spelling");
        write_sized(&dir, "a.png", 10);
        let opts = DirListOptions::default();

        let entries = get_sorted_images(&dir, &opts).expect("scan");
        let expected = dir.join("a.png").to_string_lossy().to_string();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, expected);
        assert!(!entries[0].path.starts_with(r"\\?\"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cache_invalidates_when_directory_changes() {
        let dir = unique_dir("invalidate");
        let anchor = write_sized(&dir, "a.png", 10);
        let parent = Path::new(&anchor).parent().unwrap().to_path_buf();
        let opts = DirListOptions::default();

        let first = get_sorted_images(&parent, &opts).expect("first scan");
        assert_eq!(file_names(&first), vec!["a.png"]);

        write_sized(&dir, "b.png", 10);
        let second = get_sorted_images(&parent, &opts).expect("rescan");
        assert_eq!(file_names(&second), vec!["a.png", "b.png"]);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn watcher_invalidation_drops_matching_entry() {
        let dir = unique_dir("watch-inv");
        let anchor = write_sized(&dir, "a.png", 10);
        let parent = Path::new(&anchor).parent().unwrap().to_path_buf();
        let opts = DirListOptions::default();

        get_sorted_images(&parent, &opts).expect("populate");
        let key = cache_key(&normalize_parent(&parent), &opts);
        assert!(contains_for_tests(&key));

        invalidate_for_path(&parent.join("a.png"));
        assert!(!contains_for_tests(&key));

        // Unrelated paths must not evict the entry.
        get_sorted_images(&parent, &opts).expect("repopulate");
        assert!(contains_for_tests(&key));
        invalidate_for_path(Path::new("D:\\definitely\\elsewhere\\x.png"));
        assert!(contains_for_tests(&key));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn recursive_listing_includes_subfolders() {
        let dir = unique_dir("recursive");
        let anchor = write_sized(&dir, "top.png", 10);
        let parent = Path::new(&anchor).parent().unwrap().to_path_buf();
        let sub = dir.join("sub");
        fs::create_dir_all(&sub).expect("create sub");
        write_sized(&sub, "nested.png", 10);

        let opts = DirListOptions {
            recursive: true,
            ..Default::default()
        };
        let entries = get_sorted_images(&parent, &opts).expect("recursive scan");
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.path.ends_with("nested.png")));

        // Second call exercises the watcher-gated cache path.
        let again = get_sorted_images(&parent, &opts).expect("cached recursive");
        assert_eq!(again.len(), 2);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn event_on_the_watched_directory_itself_invalidates() {
        // 디렉터리 자체가 지워지거나 이름이 바뀌면 그 목록도 버려야 한다.
        let dir = unique_dir("selfevent");
        write_sized(&dir, "a.png", 10);
        let opts = DirListOptions::default();

        get_sorted_images(&dir, &opts).expect("populate");
        let key = cache_key(&normalize_parent(&dir), &opts);
        assert!(contains_for_tests(&key));

        invalidate_for_path(&dir);
        assert!(!contains_for_tests(&key));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sibling_prefix_does_not_invalidate() {
        // `pics`의 이벤트가 `pics-backup` 캐시를 지우면 안 된다.
        let dir = unique_dir("prefix");
        let short = dir.join("pics");
        let long = dir.join("pics-backup");
        fs::create_dir_all(&short).expect("create short");
        fs::create_dir_all(&long).expect("create long");
        write_sized(&long, "a.png", 10);
        let opts = DirListOptions::default();

        get_sorted_images(&long, &opts).expect("populate");
        let key = cache_key(&normalize_parent(&long), &opts);
        assert!(contains_for_tests(&key));

        invalidate_for_path(&short.join("b.png"));
        assert!(contains_for_tests(&key));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unwatching_drops_the_listings_that_depended_on_it() {
        // 워처 슬롯이 회수되면 그 폴더 캐시도 같이 버려야 한다. 남겨두면
        // 재귀 목록이 전역 `watching` 플래그만 보고 영원히 stale해진다.
        let dir = unique_dir("unwatch");
        write_sized(&dir, "a.png", 10);
        let opts = DirListOptions::default();

        get_sorted_images(&dir, &opts).expect("populate");
        let key = cache_key(&normalize_parent(&dir), &opts);
        assert!(contains_for_tests(&key));

        unwatch(&normalize_parent(&dir));
        assert!(!contains_for_tests(&key));
        fs::remove_dir_all(&dir).ok();
    }
}
