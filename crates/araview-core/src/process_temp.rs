use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::app_error::{AppError, ErrorCode};

/// Bump when an on-disk cache key or encoding contract changes.
pub const CACHE_FORMAT_REVISION: u64 = 2;
/// Upper bound for the complete app cache tree.
pub const MAX_TOTAL_CACHE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const PERSISTENT_CACHE_DIR: &str = "cache-v2";
const TEMP_SESSION_DIR: &str = "session-v1";
const FALLBACK_TEMP_DIR: &str = "araview-cache-fallback-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheStorageMode {
    Temporary,
    Persistent,
}

struct ActiveCacheRoot {
    path: PathBuf,
    /// `is_canonical_cache_path`가 호출마다 루트를 canonicalize하지 않게 한 번만 구한다.
    canonical: PathBuf,
    /// Keep the session directory alive for the lifetime of the process.
    _temporary: Option<tempfile::TempDir>,
    mode: CacheStorageMode,
    persistent_available: bool,
}

static ACTIVE_CACHE_ROOT: LazyLock<Mutex<Option<ActiveCacheRoot>>> =
    LazyLock::new(|| Mutex::new(None));

fn reset_temporary_base(base: &Path) -> Result<(), AppError> {
    match fs::remove_dir_all(base) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(AppError::io(
                "Failed to clean temporary cache base",
                e,
                ErrorCode::Unknown,
            ))
        }
    }
    fs::create_dir_all(base).map_err(|e| {
        AppError::io(
            "Failed to create temporary cache base",
            e,
            ErrorCode::Unknown,
        )
    })
}

fn create_temporary_root(base: &Path) -> Result<tempfile::TempDir, AppError> {
    tempfile::TempDir::new_in(base)
        .map_err(|e| AppError::unknown(format!("Failed to create temp dir: {e}")))
}

/// Initialize the cache root before any image command runs.
///
/// Persistent mode uses the per-application cache directory the host passes in. The cache is
/// best-effort: the OS may remove it, so every loader must regenerate missing
/// files. Development and release identifiers resolve to different roots.
pub fn initialize(app_cache: &Path, mode: CacheStorageMode) -> Result<PathBuf, AppError> {
    fs::create_dir_all(app_cache)
        .map_err(|e| AppError::io("Failed to create app cache dir", e, ErrorCode::Unknown))?;

    match mode {
        CacheStorageMode::Temporary => {
            cleanup_old_versions(app_cache, None)?;
            let session_base = app_cache.join(TEMP_SESSION_DIR);
            reset_temporary_base(&session_base)?;
            let temp = create_temporary_root(&session_base)?;
            let path = temp.path().to_path_buf();
            set_active_root(ActiveCacheRoot {
                path: path.clone(),
                canonical: canonical_or_raw(&path),
                _temporary: Some(temp),
                mode,
                persistent_available: true,
            })?;
            Ok(path)
        }
        CacheStorageMode::Persistent => {
            reset_temporary_base(&app_cache.join(TEMP_SESSION_DIR))?;
            let root = app_cache.join(PERSISTENT_CACHE_DIR);
            cleanup_old_versions(app_cache, Some(&root))?;
            fs::create_dir_all(&root).map_err(|e| {
                AppError::io(
                    "Failed to create persistent cache dir",
                    e,
                    ErrorCode::Unknown,
                )
            })?;
            remove_stale_temp_files(&root);
            let path = root.clone();
            set_active_root(ActiveCacheRoot {
                canonical: canonical_or_raw(&path),
                path,
                _temporary: None,
                mode,
                persistent_available: true,
            })?;
            Ok(root)
        }
    }
}

/// Fallback used when Tauri's cache directory cannot be initialized.
pub fn initialize_temporary_fallback() -> Result<PathBuf, AppError> {
    let base = std::env::temp_dir().join(FALLBACK_TEMP_DIR);
    reset_temporary_base(&base)?;
    let temp = create_temporary_root(&base)?;
    let path = temp.path().to_path_buf();
    set_active_root(ActiveCacheRoot {
        path: path.clone(),
        canonical: canonical_or_raw(&path),
        _temporary: Some(temp),
        mode: CacheStorageMode::Temporary,
        persistent_available: false,
    })?;
    Ok(path)
}

fn set_active_root(root: ActiveCacheRoot) -> Result<PathBuf, AppError> {
    let path = root.path.clone();
    let mut guard = ACTIVE_CACHE_ROOT
        .lock()
        .map_err(|_| AppError::lock_poisoned("cache root"))?;
    *guard = Some(root);
    drop(guard);
    // A root switch invalidates every path protected by the previous root.
    if let Ok(mut guard) = IN_USE.lock() {
        guard.0.clear();
        guard.1.clear();
    }
    if let Ok(mut totals) = DIR_TOTALS.lock() {
        totals.clear();
    }
    Ok(path)
}

pub fn cache_storage_mode() -> CacheStorageMode {
    ACTIVE_CACHE_ROOT
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|root| root.mode))
        .unwrap_or(CacheStorageMode::Temporary)
}

pub fn persistent_cache_available() -> bool {
    ACTIVE_CACHE_ROOT
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|root| root.persistent_available))
        .unwrap_or(false)
}

pub fn process_temp_dir() -> Result<PathBuf, AppError> {
    let path = {
        let mut guard = ACTIVE_CACHE_ROOT
            .lock()
            .map_err(|_| AppError::lock_poisoned("cache root"))?;
        if let Some(root) = guard.as_ref() {
            root.path.clone()
        } else {
            // Keep creation under the same lock as the read. Otherwise two
            // first callers can replace one another and drop a TempDir while
            // another thread is still using its path.
            let base = std::env::temp_dir().join(FALLBACK_TEMP_DIR);
            reset_temporary_base(&base)?;
            let temp = create_temporary_root(&base)?;
            let path = temp.path().to_path_buf();
            *guard = Some(ActiveCacheRoot {
                path: path.clone(),
                canonical: canonical_or_raw(&path),
                _temporary: Some(temp),
                mode: CacheStorageMode::Temporary,
                persistent_available: false,
            });
            path
        }
    };
    fs::create_dir_all(&path)
        .map_err(|e| AppError::io("Failed to create cache dir", e, ErrorCode::Unknown))?;
    Ok(path)
}

/// Explicitly remove a session cache root on a normal Tauri exit. Static
/// values are not dropped at process termination, so TempDir's destructor alone
/// is not sufficient. A crash is handled by resetting the named session base
/// during the next startup.
pub fn cleanup_on_exit() {
    let path = ACTIVE_CACHE_ROOT.lock().ok().and_then(|guard| {
        guard.as_ref().and_then(|root| {
            root._temporary
                .as_ref()
                .map(|temp| temp.path().to_path_buf())
        })
    });
    if let Some(path) = path {
        let _ = fs::remove_dir_all(path);
    }
}

fn canonical_or_raw(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// 이미 canonicalize한 경로가 활성 캐시 루트 아래인지 본다.
pub(crate) fn is_canonical_cache_path(canonical: &Path) -> bool {
    ACTIVE_CACHE_ROOT
        .lock()
        .ok()
        .and_then(|guard| {
            guard
                .as_ref()
                .map(|root| canonical.starts_with(&root.canonical))
        })
        .unwrap_or(false)
}

fn cleanup_old_versions(base: &Path, keep: Option<&Path>) -> Result<(), AppError> {
    let entries = match fs::read_dir(base) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(AppError::io(
                "Failed to list app cache dir",
                e,
                ErrorCode::Unknown,
            ))
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("cache-v") || keep.is_some_and(|keep| keep == path) {
            continue;
        }
        let result = if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        if let Err(e) = result {
            if e.kind() != std::io::ErrorKind::NotFound {
                log::warn!("[cache] failed to remove old cache {:?}: {e}", path);
            }
        }
    }
    Ok(())
}

fn remove_stale_temp_files(root: &Path) {
    fn visit(dir: &Path) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                visit(&path);
            } else if file_type.is_file() && is_inflight_temp_path(&path) {
                let _ = fs::remove_file(path);
            }
        }
    }
    if root.is_dir() {
        visit(root);
    }
}

/// Detect the exact temp filename shape emitted by `sidecar::tmp_path_for`:
/// `<stem>.tmp-<pid>-ThreadId<n>-<counter>.<ext>`. Matching the complete
/// suffix avoids treating a legitimate archive name such as
/// `<hash>_photo.tmp-123-page.jpg` as an in-flight file.
pub(crate) fn is_inflight_temp_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let Some(start) = name.rfind(".tmp-") else {
        return false;
    };
    let rest = &name[start + ".tmp-".len()..];
    let Some((suffix, extension)) = rest.rsplit_once('.') else {
        return false;
    };
    if extension.is_empty() {
        return false;
    }
    let mut parts = suffix.split('-');
    let (Some(pid), Some(thread), Some(counter), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    !pid.is_empty()
        && pid.bytes().all(|b| b.is_ascii_digit())
        && thread.starts_with("ThreadId")
        && thread.len() > "ThreadId".len()
        && thread["ThreadId".len()..]
            .bytes()
            .all(|b| b.is_ascii_digit())
        && !counter.is_empty()
        && counter.bytes().all(|b| b.is_ascii_digit())
}

/// A cache hit within this window of the last touch skips the touch.
const TOUCH_MIN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

/// Best-effort LRU touch for persistent cache hits. Reopening the file and
/// setting its current length updates its modification time without adding a
/// platform-specific timestamp dependency.
pub(crate) fn touch_cache_file(path: &Path) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    // 방금 갱신한 파일은 다시 열지 않는다. 축출은 오래된 순서라 이 정도 오차는
    // 순서를 바꾸지 않고, 스크롤 중 반복 hit마다 쓰기 오픈을 하지 않게 된다.
    let fresh = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age < TOUCH_MIN_INTERVAL);
    if fresh {
        return;
    }
    if let Ok(file) = fs::OpenOptions::new().write(true).open(path) {
        let _ = file.set_len(metadata.len());
    }
}

/// Cache paths recently handed to the frontend. The webview holds an
/// `asset://` URL for whatever it renders, and nothing re-creates a cache file
/// once that URL exists, so evicting one leaves a broken image. Bounded ring
/// buffer: the oldest hand-out loses its protection first.
/// Webtoon·대용량 아카이브에서 오래된 페이지 asset URL이 깨지지 않도록 여유를 둔다.
const MAX_IN_USE: usize = 1024;

static IN_USE_OVERFLOW_LOGGED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

static IN_USE: LazyLock<Mutex<(VecDeque<PathBuf>, HashSet<PathBuf>)>> =
    LazyLock::new(|| Mutex::new((VecDeque::new(), HashSet::new())));

/// Mark a cache file as served, protecting it from the next eviction pass.
pub(crate) fn mark_in_use(path: &Path) {
    let Ok(mut guard) = IN_USE.lock() else {
        return;
    };
    let (queue, set) = &mut *guard;
    let path_buf = path.to_path_buf();
    if set.contains(&path_buf) {
        if let Some(pos) = queue.iter().position(|p| p == &path_buf) {
            queue.remove(pos);
        }
        queue.push_back(path_buf);
        return;
    }
    set.insert(path_buf.clone());
    queue.push_back(path_buf);
    while queue.len() > MAX_IN_USE {
        if let Some(old) = queue.pop_front() {
            set.remove(&old);
            // 넘침 자체는 설계된 동작이지만, 화면에 떠 있던 파일이 축출될 수
            // 있어 깨진 이미지 추적에 단서가 되도록 세션당 한 번만 남긴다.
            if !IN_USE_OVERFLOW_LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                log::warn!(
                    "[cache] in-use protection exceeded {MAX_IN_USE} entries; oldest lost protection: {}",
                    old.display()
                );
            }
        }
    }
}

pub(crate) fn is_in_use(path: &Path) -> bool {
    IN_USE
        .lock()
        .map(|guard| guard.1.contains(path))
        .unwrap_or(false)
}

/// Known byte total per capped directory. Writers add what they just wrote and
/// only rescan when the running total says the cap may be exceeded, so paging
/// through a large archive does not restat the whole directory per entry.
static DIR_TOTALS: LazyLock<Mutex<HashMap<PathBuf, u64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn reset_dir_totals() {
    if let Ok(mut totals) = DIR_TOTALS.lock() {
        totals.clear();
    }
}

/// 상한을 넘겨 정리할 때 상한의 이 비율(1/N)만큼 더 비운다. 상한 바로 아래까지만
/// 지우면 꽉 찬 캐시에서는 다음 쓰기가 곧바로 상한을 넘겨 쓸 때마다 폴더를 다시
/// 훑게 된다.
const EVICT_HEADROOM_DIVISOR: u64 = 10;

/// Record `bytes` written into `dir` and enforce `max_bytes` only when the
/// running total suggests the cap is exceeded. Best effort.
pub(crate) fn note_written(dir: &Path, bytes: u64, max_bytes: u64) {
    let needs_scan = {
        let Ok(mut totals) = DIR_TOTALS.lock() else {
            return;
        };
        match totals.get_mut(dir) {
            Some(total) => {
                *total = total.saturating_add(bytes);
                *total > max_bytes
            }
            // Unknown directory: scan once to learn its size.
            None => true,
        }
    };
    if !needs_scan {
        return;
    }
    let target = max_bytes - max_bytes / EVICT_HEADROOM_DIVISOR;
    if let Ok(total) = enforce_cap_in(dir, max_bytes, target) {
        if let Ok(mut totals) = DIR_TOTALS.lock() {
            totals.insert(dir.to_path_buf(), total);
        }
    }
}

/// [`note_written`] for a cache file that was just published. Writers that
/// encode straight to disk do not know the byte count, so one metadata read
/// stands in for rescanning the whole directory on every write.
pub(crate) fn note_published(path: &Path, max_bytes: u64) {
    let Some(dir) = path.parent() else {
        return;
    };
    let bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    note_written(dir, bytes, max_bytes);
}

/// When `dir` holds more than `max_bytes`, delete oldest files until it is
/// under `target_bytes` (unit testable). Returns the directory byte total that
/// remains after eviction.
pub(crate) fn enforce_cap_in(
    dir: &Path,
    max_bytes: u64,
    target_bytes: u64,
) -> Result<u64, AppError> {
    let files = collect_files(dir, false)?;
    Ok(evict_oldest(files, max_bytes, target_bytes))
}

/// Delete oldest files anywhere under the active cache root until the whole
/// tree is under `max_bytes`. Best-effort companion for archive extracts,
/// which live in per-archive subdirectories without their own cap.
pub(crate) fn enforce_total_cap(max_bytes: u64) -> Result<(), AppError> {
    let root = process_temp_dir()?;
    let files = collect_files(&root, true)?;
    evict_oldest(files, max_bytes, max_bytes);
    // Per-directory estimates are stale after a tree-wide sweep; relearn them.
    if let Ok(mut totals) = DIR_TOTALS.lock() {
        totals.clear();
    }
    Ok(())
}

/// Tree-wide cap sweeps are expensive, so concurrent requesters (new archive,
/// startup) coalesce onto one worker. A request arriving while a sweep runs
/// only sets `pending`, and the worker runs one extra pass so the cap is
/// enforced over writes that raced the sweep.
struct TotalCapSweep {
    running: bool,
    pending: bool,
}

static TOTAL_CAP_SWEEP: LazyLock<Mutex<TotalCapSweep>> = LazyLock::new(|| {
    Mutex::new(TotalCapSweep {
        running: false,
        pending: false,
    })
});

/// Request a tree-wide cap sweep. Returns immediately; at most one sweep
/// thread runs at a time and overlapping requests collapse into it.
pub fn request_total_cap_scan() {
    {
        let Ok(mut sweep) = TOTAL_CAP_SWEEP.lock() else {
            return;
        };
        if sweep.running {
            sweep.pending = true;
            return;
        }
        sweep.running = true;
    }
    std::thread::spawn(|| loop {
        enforce_total_cap(MAX_TOTAL_CACHE_BYTES).ok();
        let Ok(mut sweep) = TOTAL_CAP_SWEEP.lock() else {
            break;
        };
        if sweep.pending {
            sweep.pending = false;
            continue;
        }
        sweep.running = false;
        break;
    });
}

/// Collect `(mtime, size, path)` for evictable files. In-flight temp files
/// (`*.tmp-*`) are skipped so partial writes are never deleted. `root` must
/// exist; nested directories are best effort.
pub(crate) fn collect_files(
    root: &Path,
    recursive: bool,
) -> Result<Vec<(u128, u64, PathBuf)>, AppError> {
    let mut stack = vec![root.to_path_buf()];
    let mut files: Vec<(u128, u64, PathBuf)> = Vec::new();
    let mut is_root = true;
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if is_root => {
                return Err(AppError::io(
                    "Failed to list cache dir",
                    e,
                    ErrorCode::Unknown,
                ))
            }
            Err(_) => continue,
        };
        is_root = false;
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                if recursive {
                    stack.push(path);
                }
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            // Skip in-flight temp files so eviction and statistics never expose
            // a partial write. Restrict the match to the numeric pid shape.
            if is_inflight_temp_path(&path) {
                continue;
            }
            let (mtime, size) = entry
                .metadata()
                .map(|m| {
                    (
                        m.modified()
                            .ok()
                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                            .map(|d| d.as_nanos())
                            .unwrap_or(0),
                        m.len(),
                    )
                })
                .unwrap_or((0, 0));
            files.push((mtime, size, path));
        }
    }
    Ok(files)
}

/// When the total exceeds `max_bytes`, delete oldest-first until it is under
/// `target_bytes` (at most `max_bytes`), skipping files the frontend is still
/// displaying. Returns the total that remains.
fn evict_oldest(mut files: Vec<(u128, u64, PathBuf)>, max_bytes: u64, target_bytes: u64) -> u64 {
    let mut total = files
        .iter()
        .fold(0u64, |acc, (_, size, _)| acc.saturating_add(*size));
    if total <= max_bytes {
        return total;
    }
    let target_bytes = target_bytes.min(max_bytes);
    files.sort_by_key(|(mtime, _, _)| *mtime);
    for (_, size, path) in files {
        if total <= target_bytes {
            break;
        }
        // 스냅샷이 아니라 삭제 직전에 확인한다. 수집 이후 mark_in_use된 파일은
        // FE가 이미 asset URL로 들고 있어 지우면 깨진 화면으로 남는다.
        if is_in_use(&path) {
            continue;
        }
        if fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforce_cap_evicts_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["old.jpg", "mid.jpg", "new.jpg"] {
            fs::write(dir.path().join(name), vec![0u8; 100]).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        enforce_cap_in(dir.path(), 150, 150).expect("evict");
        assert!(!dir.path().join("old.jpg").exists());
        assert!(!dir.path().join("mid.jpg").exists());
        assert!(dir.path().join("new.jpg").exists());
    }

    #[test]
    fn enforce_cap_skips_inflight_tmp_files() {
        let dir = tempfile::tempdir().unwrap();
        let inflight = format!("a.tmp-{}-ThreadId4-0.jpg", std::process::id());
        fs::write(dir.path().join(&inflight), vec![0u8; 1000]).unwrap();
        fs::write(dir.path().join("b.jpg"), vec![0u8; 100]).unwrap();
        enforce_cap_in(dir.path(), 150, 150).expect("evict");
        // 진행 중 temp는 축출 대상에서 빠지고, 그 바이트도 상한 계산에
        // 들어가지 않는다 (b.jpg 100B만 세므로 cap 이내).
        assert!(dir.path().join(&inflight).exists());
        assert!(dir.path().join("b.jpg").exists());
    }

    #[test]
    fn enforce_cap_evicts_entry_like_tmp_names() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("deadbeef_x.tmp-1.jpg"), vec![0u8; 1000]).unwrap();
        fs::write(
            dir.path().join("foreign.tmp-part-abc-0.jpg"),
            vec![0u8; 100],
        )
        .unwrap();
        fs::write(dir.path().join("keep.jpg"), vec![0u8; 100]).unwrap();
        // `.tmp-<숫자 pid>-...`만 진행 중 파일로 간주한다. 숫자가 없는 일반
        // 파일명은 임시 파일 exemption을 받지 못하므로 오래된 것부터 축출된다.
        std::thread::sleep(std::time::Duration::from_millis(5));
        fs::write(dir.path().join("keep.jpg"), vec![0u8; 100]).unwrap();
        enforce_cap_in(dir.path(), 150, 150).expect("evict");
        assert!(!dir.path().join("deadbeef_x.tmp-1.jpg").exists());
        assert!(!dir.path().join("foreign.tmp-part-abc-0.jpg").exists());
        assert!(dir.path().join("keep.jpg").exists());
    }

    #[test]
    fn enforce_cap_keeps_files_still_displayed() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.jpg");
        for name in ["old.jpg", "new.jpg"] {
            fs::write(dir.path().join(name), vec![0u8; 100]).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        mark_in_use(&old);
        // The cap forces one eviction, but the oldest file is still on screen.
        enforce_cap_in(dir.path(), 150, 150).expect("evict");
        assert!(old.exists());
        assert!(!dir.path().join("new.jpg").exists());
    }

    #[test]
    fn mark_in_use_touch_refreshes_protection_order() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.jpg");
        let second = dir.path().join("second.jpg");
        mark_in_use(&first);
        mark_in_use(&second);
        // 재터치하면 LRU 맨 뒤로 이동한다. 보호 자체는 절대적이라(화면에 떠 있는
        // 파일은 cap을 넘어도 지우지 않는다, enforce_cap_keeps_files_still_displayed
        // 참고) 터치 순번은 MAX_IN_USE 넘침 때 누가 보호를 잃을지만 결정한다.
        // 다른 테스트가 병행으로 mark해 큐 뒤에 항목이 끼어도 상대 순서는 유지되므로
        // 절대 위치 대신 상대 순서를 단언한다.
        mark_in_use(&first);
        let guard = IN_USE.lock().unwrap();
        let (queue, set) = &*guard;
        assert!(set.contains(&first));
        assert!(set.contains(&second));
        let pos = |p: &PathBuf| {
            queue
                .iter()
                .position(|q| q == p)
                .expect("marked path in queue")
        };
        assert!(pos(&second) < pos(&first));
    }

    #[test]
    fn enforce_cap_reports_total_after_scan() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.jpg"), vec![0u8; 100]).unwrap();
        assert_eq!(enforce_cap_in(dir.path(), 1000, 1000).expect("scan"), 100);
    }

    #[test]
    fn note_written_defers_scan_until_cap_exceeded() {
        let dir = tempfile::tempdir().unwrap();
        let (small, big) = (dir.path().join("a.jpg"), dir.path().join("b.jpg"));
        fs::write(&small, vec![0u8; 100]).unwrap();
        // First call has no running total yet, so it scans once and learns 100.
        note_written(dir.path(), 100, 1000);
        assert!(small.exists());

        // A file appearing behind the cache's back is not noticed while the
        // running total stays under the cap - that is the deferral.
        fs::write(&big, vec![0u8; 5000]).unwrap();
        note_written(dir.path(), 10, 1000);
        assert!(small.exists());
        assert!(big.exists());

        // Once the running total crosses the cap, the rescan sees the truth.
        note_written(dir.path(), 5000, 1000);
        assert!(!small.exists());
        assert!(!big.exists());
    }

    #[test]
    fn note_written_leaves_headroom_after_eviction() {
        let dir = tempfile::tempdir().unwrap();
        let path = |i: usize| dir.path().join(format!("{i:02}.jpg"));
        for i in 0..10 {
            fs::write(path(i), vec![0u8; 100]).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // Exactly at the cap: the first scan only learns the total.
        note_written(dir.path(), 100, 1000);
        assert!(path(0).exists());

        // One more write crosses the cap. Eviction clears down to 90% (900),
        // not merely under 1000, so the two oldest files go.
        fs::write(path(10), vec![0u8; 100]).unwrap();
        note_written(dir.path(), 100, 1000);
        assert!(!path(0).exists());
        assert!(!path(1).exists());
        assert!(path(2).exists());

        // The next small write fits in the headroom and must not rescan. A file
        // that appeared behind the cache's back would be evicted by a rescan.
        fs::write(dir.path().join("behind.jpg"), vec![0u8; 500]).unwrap();
        note_written(dir.path(), 50, 1000);
        assert!(path(2).exists());
    }

    #[test]
    fn note_published_reads_size_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.jpg");
        fs::write(&old, vec![0u8; 100]).unwrap();
        note_published(&old, 1000);
        std::thread::sleep(std::time::Duration::from_millis(5));

        // The new file's real size (2000) pushes the running total over the cap.
        let new = dir.path().join("new.jpg");
        fs::write(&new, vec![0u8; 2000]).unwrap();
        note_published(&new, 1000);
        assert!(!old.exists());
    }

    #[test]
    fn session_base_reset_removes_crashed_roots() {
        let owner = tempfile::tempdir().unwrap();
        let base = owner.path().join("session-v1");
        reset_temporary_base(&base).unwrap();
        let temp = create_temporary_root(&base).unwrap();
        let path = temp.path().to_path_buf();
        assert!(path.is_dir());
        reset_temporary_base(&base).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn cache_file_identity_ignores_lru_touch() {
        let root = process_temp_dir().unwrap();
        let file = root.join(format!("identity-{}.bin", std::process::id()));
        fs::write(&file, b"cache").unwrap();
        let first = crate::sidecar::file_identity_hash(&file, &[]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        touch_cache_file(&file);
        let second = crate::sidecar::file_identity_hash(&file, &[]).unwrap();
        assert_eq!(first, second);
        let _ = fs::remove_file(file);
    }

    #[test]
    fn inflight_temp_detection_requires_exact_generated_suffix() {
        assert!(is_inflight_temp_path(Path::new(
            "a.tmp-123-ThreadId4-0.jpg"
        )));
        assert!(!is_inflight_temp_path(Path::new(
            "0123_photo.tmp-123-page.jpg"
        )));
        assert!(!is_inflight_temp_path(Path::new("photo.tmp-1.jpg")));
        assert!(!is_inflight_temp_path(Path::new("photo.tmp-part.jpg")));
        assert!(!is_inflight_temp_path(Path::new("ordinary.jpg")));
    }
}
