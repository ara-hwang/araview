use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::UNIX_EPOCH;

use crate::app_error::{AppError, ErrorCode};

static PROCESS_TEMP_DIR: LazyLock<Mutex<Option<tempfile::TempDir>>> =
    LazyLock::new(|| Mutex::new(None));

pub fn process_temp_dir() -> Result<PathBuf, AppError> {
    let mut guard = PROCESS_TEMP_DIR
        .lock()
        .map_err(|_| AppError::lock_poisoned("temp dir"))?;
    if let Some(ref td) = *guard {
        return Ok(td.path().to_path_buf());
    }
    let td = tempfile::TempDir::new()
        .map_err(|e| AppError::unknown(format!("Failed to create temp dir: {e}")))?;
    let path = td.path().to_path_buf();
    *guard = Some(td);
    Ok(path)
}

/// Cache paths recently handed to the frontend. The webview holds an
/// `asset://` URL for whatever it renders, and nothing re-creates a cache file
/// once that URL exists, so evicting one leaves a broken image. Bounded ring
/// buffer: the oldest hand-out loses its protection first.
/// Webtoon·대용량 아카이브에서 오래된 페이지 asset URL이 깨지지 않도록 여유를 둔다.
const MAX_IN_USE: usize = 1024;

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
        }
    }
}

fn in_use_snapshot() -> HashSet<PathBuf> {
    IN_USE
        .lock()
        .map(|guard| guard.1.clone())
        .unwrap_or_default()
}

/// Known byte total per capped directory. Writers add what they just wrote and
/// only rescan when the running total says the cap may be exceeded, so paging
/// through a large archive does not restat the whole directory per entry.
static DIR_TOTALS: LazyLock<Mutex<HashMap<PathBuf, u64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

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
    if let Ok(total) = enforce_cap_in(dir, max_bytes) {
        if let Ok(mut totals) = DIR_TOTALS.lock() {
            totals.insert(dir.to_path_buf(), total);
        }
    }
}

/// Delete oldest files in `process_temp/<sub_dir>` until under `max_bytes`.
pub(crate) fn enforce_cap(sub_dir: &str, max_bytes: u64) -> Result<(), AppError> {
    let dir = process_temp_dir()?.join(sub_dir);
    enforce_cap_in(&dir, max_bytes).map(|_| ())
}

/// Core of [`enforce_cap`] over an explicit directory (unit testable).
/// Returns the directory byte total that remains after eviction.
pub(crate) fn enforce_cap_in(dir: &Path, max_bytes: u64) -> Result<u64, AppError> {
    let files = collect_files(dir, false)?;
    Ok(evict_oldest(files, max_bytes))
}

/// Delete oldest files anywhere under the process temp dir until the whole
/// tree is under `max_bytes`. Best-effort companion for archive extracts,
/// which live in per-archive subdirectories without their own cap.
pub(crate) fn enforce_total_cap(max_bytes: u64) -> Result<(), AppError> {
    let root = process_temp_dir()?;
    let files = collect_files(&root, true)?;
    evict_oldest(files, max_bytes);
    // Per-directory estimates are stale after a tree-wide sweep; relearn them.
    if let Ok(mut totals) = DIR_TOTALS.lock() {
        totals.clear();
    }
    Ok(())
}

/// Collect `(mtime, size, path)` for evictable files. In-flight temp files
/// (`*.tmp-*`) are skipped so partial writes are never deleted. `root` must
/// exist; nested directories are best effort.
fn collect_files(root: &Path, recursive: bool) -> Result<Vec<(u128, u64, PathBuf)>, AppError> {
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
            // Skip in-flight temp files, never evict partial writes.
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(".tmp-"))
            {
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

/// Delete oldest-first until the total is under `max_bytes`, skipping files the
/// frontend is still displaying. Returns the total that remains.
fn evict_oldest(mut files: Vec<(u128, u64, PathBuf)>, max_bytes: u64) -> u64 {
    let mut total = files
        .iter()
        .fold(0u64, |acc, (_, size, _)| acc.saturating_add(*size));
    if total <= max_bytes {
        return total;
    }
    let protected = in_use_snapshot();
    files.sort_by_key(|(mtime, _, _)| *mtime);
    for (_, size, path) in files {
        if total <= max_bytes {
            break;
        }
        if protected.contains(&path) {
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
        enforce_cap_in(dir.path(), 150).expect("evict");
        assert!(!dir.path().join("old.jpg").exists());
        assert!(!dir.path().join("mid.jpg").exists());
        assert!(dir.path().join("new.jpg").exists());
    }

    #[test]
    fn enforce_cap_skips_inflight_tmp_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.tmp-123-abc-0.jpg"), vec![0u8; 1000]).unwrap();
        fs::write(dir.path().join("b.jpg"), vec![0u8; 100]).unwrap();
        enforce_cap_in(dir.path(), 150).expect("evict");
        assert!(dir.path().join("a.tmp-123-abc-0.jpg").exists());
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
        enforce_cap_in(dir.path(), 150).expect("evict");
        assert!(old.exists());
        assert!(!dir.path().join("new.jpg").exists());
    }

    #[test]
    fn mark_in_use_touch_refreshes_protection_order() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.jpg");
        let second = dir.path().join("second.jpg");
        for path in [&first, &second] {
            fs::write(path, vec![0u8; 100]).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        mark_in_use(&first);
        mark_in_use(&second);
        // Re-touch the older file so it should survive eviction before the other.
        mark_in_use(&first);
        enforce_cap_in(dir.path(), 150).expect("evict");
        assert!(first.exists());
        assert!(!second.exists());
    }

    #[test]
    fn enforce_cap_reports_total_after_scan() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.jpg"), vec![0u8; 100]).unwrap();
        assert_eq!(enforce_cap_in(dir.path(), 1000).expect("scan"), 100);
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
}
