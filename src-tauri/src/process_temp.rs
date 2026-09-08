use std::path::PathBuf;
use std::sync::Mutex;

use once_cell::sync::Lazy;

static PROCESS_TEMP_DIR: Lazy<Mutex<Option<tempfile::TempDir>>> = Lazy::new(|| Mutex::new(None));

pub fn process_temp_dir() -> Result<PathBuf, String> {
    let mut guard = PROCESS_TEMP_DIR.lock().map_err(|e| e.to_string())?;
    if let Some(ref td) = *guard {
        return Ok(td.path().to_path_buf());
    }
    let td = tempfile::TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;
    let path = td.path().to_path_buf();
    *guard = Some(td);
    Ok(path)
}
