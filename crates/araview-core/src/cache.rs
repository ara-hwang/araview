//! Persistent/session image cache inventory and maintenance commands.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::app_error::{AppError, ErrorCode};
use crate::process_temp::{self, CacheStorageMode};

pub const ARCHIVES_SUBDIR: &str = "archives";

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheCategory {
    Thumbnails,
    Converted,
    Scaled,
    Archives,
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CacheScope {
    All,
    Thumbnails,
    Converted,
    Scaled,
    Archives,
    Other,
}

#[derive(Clone, Debug, Serialize)]
pub struct CacheCategoryStats {
    pub key: CacheCategory,
    pub bytes: u64,
    pub file_count: u64,
    pub protected_bytes: u64,
    pub protected_file_count: u64,
    pub limit_bytes: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CacheStats {
    pub storage_mode: CacheStorageMode,
    pub persistent_available: bool,
    pub total_bytes: u64,
    pub file_count: u64,
    pub protected_bytes: u64,
    pub protected_file_count: u64,
    pub total_limit_bytes: u64,
    pub categories: Vec<CacheCategoryStats>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CacheClearResult {
    pub removed_bytes: u64,
    pub removed_file_count: u64,
    pub failed_file_count: u64,
    pub stats: CacheStats,
}

#[derive(Default)]
struct CategoryAccumulator {
    bytes: u64,
    file_count: u64,
    protected_bytes: u64,
    protected_file_count: u64,
}

#[derive(Default)]
struct ClearAccumulator {
    removed_bytes: u64,
    removed_file_count: u64,
    failed_file_count: u64,
}

pub fn get_cache_stats() -> Result<CacheStats, AppError> {
    stats_for_root(&process_temp::process_temp_dir()?)
}

pub fn clear_cache(scope: CacheScope) -> Result<CacheClearResult, AppError> {
    let root = process_temp::process_temp_dir()?;
    let cleared = clear_in_root(&root, scope)?;
    process_temp::reset_dir_totals();
    // 인덱스도 같이 비워 재검증되게 한다. 추출물이 지워진 뒤 stale 인덱스가
    // 남으면 페이지 수와 실제 캐시 내용이 어긋난다.
    crate::archive_index::reset();
    remove_empty_dirs(&root)?;
    let stats = stats_for_root(&root)?;
    Ok(CacheClearResult {
        removed_bytes: cleared.removed_bytes,
        removed_file_count: cleared.removed_file_count,
        failed_file_count: cleared.failed_file_count,
        stats,
    })
}

fn stats_for_root(root: &Path) -> Result<CacheStats, AppError> {
    let mut categories: HashMap<CacheCategory, CategoryAccumulator> = HashMap::new();
    for category in [
        CacheCategory::Thumbnails,
        CacheCategory::Converted,
        CacheCategory::Scaled,
        CacheCategory::Archives,
        CacheCategory::Other,
    ] {
        categories.insert(category, CategoryAccumulator::default());
    }

    let mut total_bytes = 0u64;
    let mut file_count = 0u64;
    let mut protected_bytes = 0u64;
    let mut protected_file_count = 0u64;
    if root.is_dir() {
        for (_, size, path) in process_temp::collect_files(root, true)? {
            let category = category_for_path(root, &path);
            let entry = categories.entry(category).or_default();
            entry.bytes = entry.bytes.saturating_add(size);
            entry.file_count = entry.file_count.saturating_add(1);
            total_bytes = total_bytes.saturating_add(size);
            file_count = file_count.saturating_add(1);
            if process_temp::is_in_use(&path) {
                entry.protected_bytes = entry.protected_bytes.saturating_add(size);
                entry.protected_file_count = entry.protected_file_count.saturating_add(1);
                protected_bytes = protected_bytes.saturating_add(size);
                protected_file_count = protected_file_count.saturating_add(1);
            }
        }
    }

    let mut result = Vec::with_capacity(4);
    for category in [
        CacheCategory::Thumbnails,
        CacheCategory::Converted,
        CacheCategory::Scaled,
        CacheCategory::Archives,
    ] {
        result.push(category_stats(
            category,
            categories.remove(&category).unwrap_or_default(),
        ));
    }
    let other = categories.remove(&CacheCategory::Other).unwrap_or_default();
    if other.file_count > 0 {
        result.push(category_stats(CacheCategory::Other, other));
    }

    Ok(CacheStats {
        storage_mode: process_temp::cache_storage_mode(),
        persistent_available: process_temp::persistent_cache_available(),
        total_bytes,
        file_count,
        protected_bytes,
        protected_file_count,
        total_limit_bytes: process_temp::MAX_TOTAL_CACHE_BYTES,
        categories: result,
    })
}

fn category_stats(key: CacheCategory, value: CategoryAccumulator) -> CacheCategoryStats {
    let limit_bytes = match key {
        CacheCategory::Thumbnails => Some(crate::thumbnail::MAX_CACHE_BYTES),
        CacheCategory::Converted => Some(crate::sidecar::MAX_PAINT_BYTES),
        CacheCategory::Scaled => Some(crate::scaled::MAX_SCALED_BYTES),
        CacheCategory::Archives | CacheCategory::Other => None,
    };
    CacheCategoryStats {
        key,
        bytes: value.bytes,
        file_count: value.file_count,
        protected_bytes: value.protected_bytes,
        protected_file_count: value.protected_file_count,
        limit_bytes,
    }
}

fn category_for_path(root: &Path, path: &Path) -> CacheCategory {
    let Ok(relative) = path.strip_prefix(root) else {
        return CacheCategory::Other;
    };
    let Some(first) = relative.components().next() else {
        return CacheCategory::Other;
    };
    let name = first.as_os_str().to_string_lossy();
    match name.as_ref() {
        crate::thumbnail::THUMBS_SUBDIR => CacheCategory::Thumbnails,
        crate::sidecar::PAINT_SUBDIR => CacheCategory::Converted,
        crate::scaled::SCALED_SUBDIR => CacheCategory::Scaled,
        ARCHIVES_SUBDIR => CacheCategory::Archives,
        value if value.starts_with("archive-") => CacheCategory::Archives,
        _ => CacheCategory::Other,
    }
}

fn scope_matches(scope: CacheScope, category: CacheCategory) -> bool {
    scope == CacheScope::All
        || matches!(
            (scope, category),
            (CacheScope::Thumbnails, CacheCategory::Thumbnails)
                | (CacheScope::Converted, CacheCategory::Converted)
                | (CacheScope::Scaled, CacheCategory::Scaled)
                | (CacheScope::Archives, CacheCategory::Archives)
                | (CacheScope::Other, CacheCategory::Other)
        )
}

fn clear_in_root(root: &Path, scope: CacheScope) -> Result<ClearAccumulator, AppError> {
    let mut result = ClearAccumulator::default();
    if !root.is_dir() {
        return Ok(result);
    }
    for (_, size, path) in process_temp::collect_files(root, true)? {
        let category = category_for_path(root, &path);
        if !scope_matches(scope, category) || process_temp::is_in_use(&path) {
            continue;
        }
        match fs::remove_file(&path) {
            Ok(()) => {
                result.removed_bytes = result.removed_bytes.saturating_add(size);
                result.removed_file_count = result.removed_file_count.saturating_add(1);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => result.failed_file_count = result.failed_file_count.saturating_add(1),
        }
    }
    Ok(result)
}

fn remove_empty_dirs(root: &Path) -> Result<(), AppError> {
    fn visit(dir: &Path) -> Result<(), AppError> {
        let mut children = Vec::new();
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                return Err(AppError::io(
                    "Failed to list cache directory",
                    e,
                    ErrorCode::Unknown,
                ))
            }
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                let path = entry.path();
                visit(&path)?;
                children.push(path);
            }
        }
        for child in children.into_iter().rev() {
            let _ = fs::remove_dir(child);
        }
        Ok(())
    }

    if root.is_dir() {
        visit(root)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn unique_file(root: &Path, relative: &str, bytes: usize) -> PathBuf {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, vec![0u8; bytes]).unwrap();
        path
    }

    fn category(stats: &CacheStats, key: CacheCategory) -> &CacheCategoryStats {
        stats
            .categories
            .iter()
            .find(|entry| entry.key == key)
            .unwrap()
    }

    #[test]
    fn stats_groups_known_categories_and_protected_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        unique_file(root, "thumbs/a.jpg", 10);
        unique_file(root, "paint/a.jpg", 20);
        unique_file(root, "scaled/a.jpg", 30);
        unique_file(root, "archives/archive-a/a.jpg", 40);
        unique_file(root, "unexpected/a.bin", 50);
        let protected = unique_file(root, "thumbs/protected.jpg", 60);
        process_temp::mark_in_use(&protected);

        let stats = stats_for_root(root).unwrap();
        assert_eq!(stats.total_bytes, 210);
        assert_eq!(stats.file_count, 6);
        assert_eq!(stats.protected_bytes, 60);
        assert_eq!(stats.protected_file_count, 1);
        assert_eq!(category(&stats, CacheCategory::Thumbnails).bytes, 70);
        assert_eq!(category(&stats, CacheCategory::Converted).bytes, 20);
        assert_eq!(category(&stats, CacheCategory::Scaled).bytes, 30);
        assert_eq!(category(&stats, CacheCategory::Archives).bytes, 40);
        assert_eq!(category(&stats, CacheCategory::Other).bytes, 50);
    }

    #[test]
    fn clear_scope_preserves_other_categories_and_in_use_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        unique_file(root, "thumbs/remove.jpg", 10);
        let protected = unique_file(root, "thumbs/protected.jpg", 20);
        unique_file(root, "paint/keep.jpg", 30);
        process_temp::mark_in_use(&protected);

        let cleared = clear_in_root(root, CacheScope::Thumbnails).unwrap();
        assert_eq!(cleared.removed_bytes, 10);
        assert_eq!(cleared.removed_file_count, 1);
        assert!(!root.join("thumbs/remove.jpg").exists());
        assert!(protected.exists());
        assert!(root.join("paint/keep.jpg").exists());
    }

    #[test]
    fn clear_all_removes_every_unprotected_file_and_empty_dirs() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        unique_file(root, "thumbs/a.jpg", 10);
        unique_file(root, "archives/a/b.jpg", 20);
        unique_file(root, "other/c.bin", 30);

        let cleared = clear_in_root(root, CacheScope::All).unwrap();
        remove_empty_dirs(root).unwrap();
        assert_eq!(cleared.removed_bytes, 60);
        assert_eq!(cleared.removed_file_count, 3);
        assert!(root.is_dir());
        assert!(!root.join("thumbs").exists());
        assert!(!root.join("archives").exists());
        assert!(!root.join("other").exists());
    }

    #[test]
    fn clear_result_recomputes_stats() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        unique_file(root, "scaled/a.jpg", 100);
        let outcome = clear_in_root(root, CacheScope::Scaled).unwrap();
        assert_eq!(outcome.removed_file_count, 1);
        let stats = stats_for_root(root).unwrap();
        assert_eq!(stats.total_bytes, 0);
        assert_eq!(stats.file_count, 0);
    }
}
