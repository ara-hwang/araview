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
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime};

use crate::app_error::{AppError, ErrorCode};
use crate::file_availability::FileAvailability;
use crate::image::is_supported_file;
use crate::natural_sort::sort_by_natural_path;
use crate::ops::{DirListOptions, DirSortKey};

/// Upper bound for cached directory listings; oldest entry evicted past this.
const MAX_CACHE_ENTRIES: usize = 128;

/// Upper bound for OS watcher slots. Trimmed together with cache eviction.
const MAX_WATCHED_DIRS: usize = 64;

#[derive(Debug, Clone)]
pub(crate) struct ImageEntry {
    /// IPC 응답이 문자열을 복사하지 않고 참조만 늘리도록 공유한다.
    pub(crate) path: Arc<str>,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) size: u64,
    pub(crate) availability: FileAvailability,
}

struct CachedListing {
    parent: PathBuf,
    parent_key: String,
    dir_mtime: Option<SystemTime>,
    /// 캐시 히트가 목록을 복사하지 않고 잠금 밖에서 읽도록 공유한다.
    images: Arc<Vec<ImageEntry>>,
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

/// 목록이 어디서 왔는지. 방금 스캔한 목록은 디스크 상태 그대로라 호출자가
/// 항목별 존재 확인을 다시 할 필요가 없다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ListingSource {
    Scanned,
    Cached,
}

/// [`get_sorted_images`]에 목록의 출처를 더해 돌려준다.
pub(crate) fn get_sorted_images_with_source(
    parent: &Path,
    opts: &DirListOptions,
) -> Result<(Arc<Vec<ImageEntry>>, ListingSource), AppError> {
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
                    return Ok((Arc::clone(&entry.images), ListingSource::Cached));
                }
            }
        }
        let mut images = Vec::new();
        collect_images(parent, false, &mut images)?;
        sort_images(&mut images, opts);
        ensure_watched(&canonical, false);
        let images = Arc::new(images);
        insert_cache(canonical, opts, current, Arc::clone(&images));
        Ok((images, ListingSource::Scanned))
    } else {
        let key = cache_key(&canonical, opts);
        // 재귀 목록은 부모 mtime으로 검증할 수 없어 워처가 살아 있을 때만
        // 캐시를 신뢰한다. `ensure_watched`의 watch()가 실패했는데(네트워크
        // 드라이브, 핸들 고갈 등) 캐시를 넣으면 그 폴더는 세션 동안
        // stale해진다. 그래서 watch 성공 여부가 조회·저장을 함께 가른다.
        let watched = ensure_watched(&canonical, true);
        if watched {
            if let Ok(mut cache) = DIR_CACHE.lock() {
                if let Some(entry) = cache.get_mut(&key) {
                    entry.last_access = Instant::now();
                    return Ok((Arc::clone(&entry.images), ListingSource::Cached));
                }
            }
        }
        let mut images = Vec::new();
        collect_images(parent, true, &mut images)?;
        sort_images(&mut images, opts);
        let images = Arc::new(images);
        if watched {
            insert_cache(canonical, opts, None, Arc::clone(&images));
        }
        Ok((images, ListingSource::Scanned))
    }
}

fn insert_cache(
    canonical_parent: PathBuf,
    opts: &DirListOptions,
    dir_mtime: Option<SystemTime>,
    images: Arc<Vec<ImageEntry>>,
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
    images: Arc<Vec<ImageEntry>>,
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

/// Watch `canonical_parent` (idempotent) and report whether it is now watched.
/// The recursive cache only trusts itself while the watcher is live, so a
/// failed registration must be visible to the caller instead of being logged
/// and forgotten.
fn ensure_watched(canonical_parent: &Path, recursive: bool) -> bool {
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
            return true;
        }
    }

    let Ok(mut guard) = WATCHER.lock() else {
        return false;
    };
    if guard.is_none() {
        match notify::RecommendedWatcher::new(
            |res: Result<notify::Event, notify::Error>| match res {
                Ok(event) => {
                    for path in event.paths {
                        invalidate_for_path(&path);
                    }
                }
                Err(e) => {
                    log::warn!("[dir-cache] watch error: {e}");
                    // 이벤트가 유실됐을 수 있다. 재귀 목록은 mtime 검증 없이
                    // 워처만 신뢰하므로, 유실을 그대로 두면 캐시가 세션 내내
                    // stale해진다. 보수적으로 전체를 지우고 재스캔에 맡긴다.
                    if let Ok(mut cache) = DIR_CACHE.lock() {
                        cache.clear();
                    }
                }
            },
            notify::Config::default(),
        ) {
            Ok(watcher) => *guard = Some(watcher),
            Err(e) => {
                log::warn!("[dir-cache] failed to start watcher: {e}");
                return false;
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
                return true;
            }
            Err(e) => log::warn!(
                "[dir-cache] failed to watch {}: {e}",
                canonical_parent.display()
            ),
        }
    }
    false
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

/// 직전 이벤트의 부모 폴더 해석 결과. 대량 복사·삭제는 같은 폴더의 이벤트가
/// 연달아 오므로 이벤트마다 canonicalize하지 않게 잠깐만 재사용한다.
static LAST_EVENT_PARENT: Mutex<Option<(PathBuf, Instant, String)>> = Mutex::new(None);
const EVENT_PARENT_TTL: Duration = Duration::from_secs(1);

fn canonical_event_parent(parent: &Path) -> Option<String> {
    let mut last = LAST_EVENT_PARENT.lock().ok()?;
    if let Some((path, at, key)) = last.as_ref() {
        if path == parent && at.elapsed() < EVENT_PARENT_TTL {
            return Some(key.clone());
        }
    }
    let canonical = fs::canonicalize(parent).ok()?;
    let key = with_trailing_sep(canonical.to_string_lossy().to_lowercase());
    *last = Some((parent.to_path_buf(), Instant::now(), key.clone()));
    Some(key)
}

fn invalidate_for_path(path: &Path) {
    // 이벤트 경로는 raw/canonical 어느 쪽으로 와도 매칭되게 후보를 모은다.
    // 삭제된 파일은 canonicalize가 실패하므로 부모 기준도 함께 검사한다.
    // 후보에도 구분자를 붙여야 `C:\foo`가 `C:\foobar`에 걸리지 않으면서
    // 감시 중인 디렉터리 자신에 대한 이벤트(삭제·이름 변경)까지 잡힌다.
    // 무효화할 목록이 없으면 경로 해석(syscall)도 필요 없다. 대량 변경은 첫
    // 이벤트가 목록을 비운 뒤 나머지가 여기서 끝난다.
    if DIR_CACHE.lock().is_ok_and(|cache| cache.is_empty()) {
        return;
    }
    let mut candidates = vec![with_trailing_sep(path.to_string_lossy().to_lowercase())];
    if let Ok(canonical) = fs::canonicalize(path) {
        candidates.push(with_trailing_sep(
            canonical.to_string_lossy().to_lowercase(),
        ));
    }
    if let Some(parent) = path.parent().and_then(canonical_event_parent) {
        candidates.push(parent);
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
    let mut is_root = true;
    while let Some(current) = stack.pop() {
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            // 루트 실패는 호출자가 알아야 하지만, 재귀 중 하위 폴더 하나의
            // ACL/일시 오류가 전체 목록을 죽이면 안 된다 (`System Volume
            // Information` 같은 폴더 하나로 폴더 전체가 안 보이던 문제).
            Err(e) if is_root => {
                return Err(AppError::io(
                    "Failed to read directory",
                    e,
                    ErrorCode::Corrupt,
                ))
            }
            Err(e) => {
                log::warn!("[dir-cache] skip {}: {e}", current.display());
                continue;
            }
        };
        is_root = false;
        for entry in entries {
            // 스캔 중 삭제된 항목 등 개별 실패는 건너뛴다 (TOCTOU).
            let Ok(entry) = entry else {
                continue;
            };
            let entry_path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                // junction/symlink 같은 reparse point는 따라가지 않는다.
                // 순환 링크로 인한 무한 순회와 범위 밖 스캔을 막는다.
                if recursive && !is_reparse_point(&entry) {
                    stack.push(entry_path);
                }
                continue;
            }
            // `file_type`은 디렉터리 열거 결과라 추가 조회가 없다. 그 사이 사라진
            // 파일은 아래 `metadata` 실패나 로드 시점의 not_found로 드러난다.
            if !file_type.is_file() || !is_supported_file(&entry_path) {
                continue;
            }
            let Some(path_str) = entry_path.to_str().map(Arc::<str>::from) else {
                continue;
            };
            let (availability, modified, size) = match entry.metadata() {
                Ok(meta) => (
                    crate::file_availability::availability_from_metadata(&meta),
                    meta.modified().ok(),
                    meta.len(),
                ),
                Err(_) => (FileAvailability::Unknown, None, 0),
            };
            out.push(ImageEntry {
                path: path_str,
                modified,
                size,
                availability,
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
    let by_name = |group: &mut [ImageEntry]| sort_by_natural_path(group, |e| &e.path);
    match opts.sort_key {
        DirSortKey::Name => by_name(images),
        // 이름 키(Win32 호출)는 날짜나 크기가 같은 묶음에만 계산한다.
        DirSortKey::Date => {
            images.sort_unstable_by_key(|e| e.modified);
            for group in images.chunk_by_mut(|a, b| a.modified == b.modified) {
                by_name(group);
            }
        }
        DirSortKey::Size => {
            images.sort_unstable_by_key(|e| e.size);
            for group in images.chunk_by_mut(|a, b| a.size == b.size) {
                by_name(group);
            }
        }
    }
    if opts.descending {
        images.reverse();
    }
}
