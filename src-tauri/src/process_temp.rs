use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use crate::app_error::AppError;

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
