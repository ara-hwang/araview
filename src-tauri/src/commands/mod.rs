//! 프런트엔드가 `invoke()`로 부르는 Tauri 커맨드. 도메인별 하위 모듈에 두고
//! 여기서 다시 내보내 `lib.rs`의 등록부와 기존 `crate::commands::*` 경로를 유지한다.

mod archive;
mod cache;
mod directory;
mod file_ops;
mod load;
mod metadata;
mod system;
#[cfg(test)]
mod test_support;
mod thumbnail;

pub use archive::*;
pub use cache::*;
pub use directory::*;
pub use file_ops::*;
pub use load::*;
pub use metadata::*;
pub use system::*;
pub use thumbnail::*;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use crate::app_error::AppError;
use crate::process_temp::process_temp_dir;
use tauri::Manager;

/// 이미 허용한 asset 디렉터리. 파일마다 scope를 추가하면 세션 내내 팽창하므로
/// 부모 디렉터리 단위로 1회만 허용한다. 활성 파생 이미지 캐시 루트는 시작 시
/// 통째로 허용되므로 여기서 건너뛴다.
static ALLOWED_ASSET_DIRS: LazyLock<Mutex<HashSet<PathBuf>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// asset 프로토콜 scope 매칭은 canonicalize된 요청 경로 기준이므로
/// 허용할 때도 canonicalize한 경로를 등록한다.
pub(crate) fn allow_asset_path(app: &tauri::AppHandle, path: &Path) -> Result<(), AppError> {
    if let Ok(temp) = process_temp_dir() {
        let temp_canon = fs::canonicalize(&temp).unwrap_or(temp.clone());
        let path_canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if path_canon.starts_with(&temp_canon) || path.starts_with(&temp) {
            return Ok(());
        }
    }
    let parent = path.parent().unwrap_or(path);
    let canon_parent = fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
    if ALLOWED_ASSET_DIRS
        .lock()
        .map(|set| set.contains(&canon_parent))
        .unwrap_or(false)
    {
        return Ok(());
    }
    // 재귀 허용은 금지한다. `D:\a.jpg`의 부모는 `D:\`라서 recursive=true면
    // 드라이브 전체가 webview에 열린다. 하위 폴더 이미지는 자기 파일을 열 때
    // 그 폴더가 개별로 허용되므로 재귀 없이도 충분하다.
    if app
        .asset_protocol_scope()
        .allow_directory(&canon_parent, false)
        .is_ok()
    {
        if let Ok(mut set) = ALLOWED_ASSET_DIRS.lock() {
            set.insert(canon_parent);
        }
        return Ok(());
    }
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    app.asset_protocol_scope()
        .allow_file(&canonical)
        .map_err(|e| AppError::unknown(format!("Failed to allow asset path: {e}")))
}

/// 블로킹 작업을 런타임 밖 스레드에서 실행한다. join 실패(패닉 등)는
/// `AppError::unknown("Failed to join <task> task: ...")`로 통일한다.
async fn run_blocking<T, F>(task: &'static str, f: F) -> Result<T, AppError>
where
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::unknown(format!("Failed to join {task} task: {e}")))?
}
