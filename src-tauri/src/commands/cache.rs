//! 파생 이미지 캐시 통계와 삭제 커맨드.

use crate::app_error::AppError;

use super::run_blocking;

/// 현재 활성 이미지 캐시의 종류별 사용량. 큰 트리 조회는 UI를 막지 않도록
/// blocking task 안에서 실행한다.
#[tauri::command]
pub async fn get_cache_stats() -> Result<crate::cache::CacheStats, AppError> {
    run_blocking("cache stats", crate::cache::get_cache_stats).await
}

/// 종류별 또는 전체 캐시 삭제. 화면에 전달된 in-use 파일은 보존한다.
#[tauri::command]
pub async fn clear_cache(
    scope: crate::cache::CacheScope,
) -> Result<crate::cache::CacheClearResult, AppError> {
    run_blocking("cache clear", move || crate::cache::clear_cache(scope)).await
}
