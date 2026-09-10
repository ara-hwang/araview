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

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use once_cell::sync::Lazy;

use crate::app_error::{AppError, ErrorCode};
use crate::commands::{DirListOptions, DirSortKey};
use crate::image::is_supported_file;

/// Upper bound for cached directory listings; cleared wholesale when hit.
const MAX_CACHE_ENTRIES: usize = 128;

#[derive(Debug, Clone)]
pub(crate) struct ImageEntry {
    pub(crate) path: String,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) size: u64,
}

struct CachedListing {
    parent: PathBuf,
    dir_mtime: Option<SystemTime>,
    images: Vec<ImageEntry>,
}

static DIR_CACHE: Lazy<Mutex<HashMap<String, CachedListing>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static WATCHED_DIRS: Lazy<Mutex<HashSet<PathBuf>>> = Lazy::new(|| Mutex::new(HashSet::new()));
static WATCHER: Lazy<Mutex<Option<notify::RecommendedWatcher>>> = Lazy::new(|| Mutex::new(None));

pub(crate) fn cache_key(parent: &Path, opts: &DirListOptions) -> String {
    let sort = match opts.sort_key {
        DirSortKey::Name => "name",
        DirSortKey::Date => "date",
        DirSortKey::Size => "size",
    };
    format!(
        "{}|{}|{}|{}|{}",
        parent.to_string_lossy(),
        sort,
        opts.descending,
        opts.shuffle,
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
    if !opts.recursive {
        let current = dir_mtime(parent);
        if let Ok(cache) = DIR_CACHE.lock() {
            if let Some(entry) = cache.get(&cache_key(parent, opts)) {
                if entry.dir_mtime == current {
                    return Ok(entry.images.clone());
                }
            }
        }
        let mut images = Vec::new();
        collect_images(parent, false, &mut images)?;
        sort_images(&mut images, opts, parent);
        ensure_watched(parent, false);
        insert_cache(parent, opts, current, images.clone());
        Ok(images)
    } else {
        let key = cache_key(parent, opts);
        let watching = WATCHER.lock().map(|guard| guard.is_some()).unwrap_or(false);
        if watching {
            if let Ok(cache) = DIR_CACHE.lock() {
                if let Some(entry) = cache.get(&key) {
                    return Ok(entry.images.clone());
                }
            }
        }
        let mut images = Vec::new();
        collect_images(parent, true, &mut images)?;
        sort_images(&mut images, opts, parent);
        ensure_watched(parent, true);
        insert_cache(parent, opts, None, images.clone());
        Ok(images)
    }
}

fn insert_cache(
    parent: &Path,
    opts: &DirListOptions,
    dir_mtime: Option<SystemTime>,
    images: Vec<ImageEntry>,
) {
    let Ok(mut cache) = DIR_CACHE.lock() else {
        return;
    };
    if cache.len() >= MAX_CACHE_ENTRIES {
        cache.clear();
    }
    cache.insert(
        cache_key(parent, opts),
        CachedListing {
            parent: parent.to_path_buf(),
            dir_mtime,
            images,
        },
    );
}

fn ensure_watched(parent: &Path, recursive: bool) {
    use notify::{RecursiveMode, Watcher};

    let mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };

    if let Ok(watched) = WATCHED_DIRS.lock() {
        if watched.contains(parent) {
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
                Err(e) => eprintln!("[dir-cache] watch error: {e}"),
            },
            notify::Config::default(),
        ) {
            Ok(watcher) => *guard = Some(watcher),
            Err(e) => {
                eprintln!("[dir-cache] failed to start watcher: {e}");
                return;
            }
        }
    }
    if let Some(watcher) = guard.as_mut() {
        match watcher.watch(parent, mode) {
            Ok(()) => {
                if let Ok(mut watched) = WATCHED_DIRS.lock() {
                    watched.insert(parent.to_path_buf());
                }
            }
            Err(e) => eprintln!("[dir-cache] failed to watch {}: {e}", parent.display()),
        }
    }
}

fn invalidate_for_path(path: &Path) {
    let Ok(mut cache) = DIR_CACHE.lock() else {
        return;
    };
    cache.retain(|_, entry| !path.starts_with(&entry.parent));
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

fn sort_images(images: &mut [ImageEntry], opts: &DirListOptions, dir: &Path) {
    if opts.shuffle {
        // 새로고침해도 순서가 바뀌지 않게 폴더 경로 해시를 시드로 사용
        deterministic_shuffle(images, dir_seed(dir));
    } else {
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
    }
    if opts.descending {
        images.reverse();
    }
}

fn dir_seed(dir: &Path) -> u64 {
    // djb2 해시 (추가 크레이트 없이 결정적 셔플용)
    let mut hash: u64 = 5381;
    for b in dir.to_string_lossy().bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    hash
}

fn deterministic_shuffle(images: &mut [ImageEntry], mut seed: u64) {
    if seed == 0 {
        seed = 0x9E3779B97F4A7C15;
    }
    // xorshift64* + Fisher-Yates
    let mut rand = move || {
        seed ^= seed >> 12;
        seed ^= seed << 25;
        seed ^= seed >> 27;
        seed.wrapping_mul(0x2545F4914F6CDD1D)
    };
    for i in (1..images.len()).rev() {
        let j = (rand() % (i as u64 + 1)) as usize;
        images.swap(i, j);
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
        assert!(contains_for_tests(&cache_key(&parent, &opts)));

        // No FS change: second call must hit the cache and agree.
        let second = get_sorted_images(&parent, &opts).expect("cached");
        assert_eq!(file_names(&second), vec!["a.png"]);
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
        let key = cache_key(&parent, &opts);
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
}
