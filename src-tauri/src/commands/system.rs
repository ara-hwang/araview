//! 파일 연결, 기본 앱 설정, 라이선스 번들 같은 시스템 연동 커맨드.

use crate::app_error::AppError;

use super::run_blocking;

#[tauri::command]
pub async fn get_file_associations() -> Result<Vec<crate::file_assoc::FileAssociation>, AppError> {
    run_blocking("associations", crate::file_assoc::list_associations).await
}

#[tauri::command]
pub async fn set_file_association(
    window: tauri::WebviewWindow,
    extension: String,
    associate: bool,
) -> Result<crate::file_assoc::FileAssociation, AppError> {
    run_blocking("association", move || {
        crate::file_assoc::set_association(&extension, associate, window_hwnd(&window))
    })
    .await
}

/// 소유자 창의 HWND 값(없으면 0). `file_assoc`가 raw 포인터를 safe
/// 시그니처로 받지 않도록 정수로 넘긴다.
fn window_hwnd(window: &tauri::WebviewWindow) -> isize {
    window.hwnd().map(|hwnd| hwnd.0 as isize).unwrap_or(0)
}

#[tauri::command]
pub async fn open_default_apps_settings() -> Result<(), AppError> {
    run_blocking(
        "default apps",
        crate::file_assoc::open_default_apps_settings,
    )
    .await
}

/// 설치 프로그램에 동봉된 서드파티 라이선스 문서(고지 문서 + 라이브러리 원문).
#[derive(serde::Serialize)]
pub struct LicenseDocument {
    pub name: String,
    pub content: String,
}

/// 라이선스 대화상자용 묶음: 문서 + 패키지별 라이선스 목록과 본문(id로 중복 제거).
/// `packages`/`texts`는 `THIRD_PARTY_LICENSES.json`(scripts/generate-license-data.mjs)을 그대로 넘긴다.
#[derive(serde::Serialize)]
pub struct LicenseBundle {
    pub documents: Vec<LicenseDocument>,
    pub packages: serde_json::Value,
    pub texts: serde_json::Value,
}

const LICENSE_DATA_FILE: &str = "THIRD_PARTY_LICENSES.json";

/// 동봉된 라이선스 자료를 읽어 돌려준다. 고지 문서가 문서 목록 맨 앞, 나머지는 이름순.
#[tauri::command]
pub async fn get_license_bundle(app: tauri::AppHandle) -> Result<LicenseBundle, AppError> {
    use tauri::Manager;

    let dir = app
        .path()
        .resource_dir()
        .map_err(|e| AppError::unknown(format!("Failed to resolve resource dir: {e}")))?
        .join("licenses");
    run_blocking("licenses", move || {
        let entries = std::fs::read_dir(&dir).map_err(|e| {
            AppError::io(
                &format!("read licenses dir {}", dir.display()),
                e,
                crate::app_error::ErrorCode::NotFound,
            )
        })?;
        let mut docs: Vec<LicenseDocument> = entries
            .flatten()
            .filter(|entry| entry.path().is_file())
            .filter(|entry| entry.file_name().to_string_lossy() != LICENSE_DATA_FILE)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let bytes = std::fs::read(entry.path()).ok()?;
                Some(LicenseDocument {
                    name,
                    content: String::from_utf8_lossy(&bytes).into_owned(),
                })
            })
            .collect();
        docs.sort_by(|a, b| {
            let notice = |d: &LicenseDocument| d.name != "THIRD_PARTY_LICENSES.md";
            (notice(a), a.name.to_lowercase()).cmp(&(notice(b), b.name.to_lowercase()))
        });
        if docs.is_empty() {
            return Err(AppError::not_found("No license documents are bundled"));
        }

        // 패키지 데이터가 없거나 깨져도 문서는 보여준다.
        let data: serde_json::Value = std::fs::read(dir.join(LICENSE_DATA_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(serde_json::Value::Null);
        Ok(LicenseBundle {
            documents: docs,
            packages: data
                .get("packages")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([])),
            texts: data
                .get("texts")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
        })
    })
    .await
}
