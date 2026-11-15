//! 라이선스 번들 같은 시스템 연동 구현.

use std::path::Path;

use crate::app_error::AppError;

/// 설치 프로그램에 동봉된 서드파티 라이선스 문서(고지 문서 + 라이브러리 원문).
#[derive(Debug, Clone, serde::Serialize)]
pub struct LicenseDocument {
    pub name: String,
    pub content: String,
}

/// 라이선스 대화상자용 묶음: 문서 + 패키지별 라이선스 목록과 본문(id로 중복 제거).
/// `packages`/`texts`는 `THIRD_PARTY_LICENSES.json`(scripts/generate-license-data.mjs)을 그대로 넘긴다.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LicenseBundle {
    pub documents: Vec<LicenseDocument>,
    pub packages: serde_json::Value,
    pub texts: serde_json::Value,
}

const LICENSE_DATA_FILE: &str = "THIRD_PARTY_LICENSES.json";

/// `dir`에 동봉된 라이선스 자료를 읽어 돌려준다. 고지 문서가 문서 목록 맨 앞, 나머지는 이름순.
pub fn license_bundle_in(dir: &Path) -> Result<LicenseBundle, AppError> {
    let entries = std::fs::read_dir(dir).map_err(|e| {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, contents: &str) {
        std::fs::write(dir.join(name), contents).unwrap();
    }

    #[test]
    fn license_bundle_puts_the_notice_first() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "zeta-copyright.txt", "Z");
        write(dir.path(), "THIRD_PARTY_LICENSES.md", "notice");
        write(dir.path(), "alpha-copyright.txt", "A");
        write(
            dir.path(),
            LICENSE_DATA_FILE,
            r#"{"packages":[{"name":"libheif"}],"texts":{"MIT":"text"}}"#,
        );

        let bundle = license_bundle_in(dir.path()).expect("bundle");
        let names: Vec<&str> = bundle
            .documents
            .iter()
            .map(|document| document.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "THIRD_PARTY_LICENSES.md",
                "alpha-copyright.txt",
                "zeta-copyright.txt"
            ]
        );
        assert_eq!(bundle.documents[0].content, "notice");
        // 데이터 파일 자체는 문서로 노출하지 않는다.
        assert!(!names.contains(&LICENSE_DATA_FILE));
        assert_eq!(bundle.packages[0]["name"], "libheif");
        assert_eq!(bundle.texts["MIT"], "text");
    }

    #[test]
    fn license_bundle_falls_back_when_the_data_file_is_broken() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "THIRD_PARTY_LICENSES.md", "notice");
        write(dir.path(), LICENSE_DATA_FILE, "not json");

        let bundle = license_bundle_in(dir.path()).expect("bundle");
        assert_eq!(bundle.documents.len(), 1);
        assert_eq!(bundle.packages, serde_json::json!([]));
        assert_eq!(bundle.texts, serde_json::json!({}));
    }

    #[test]
    fn license_bundle_ignores_subdirectories() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "THIRD_PARTY_LICENSES.md", "notice");
        std::fs::create_dir(dir.path().join("nested")).unwrap();

        let bundle = license_bundle_in(dir.path()).expect("bundle");
        assert_eq!(bundle.documents.len(), 1);
    }

    #[test]
    fn license_bundle_errors_without_documents() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            license_bundle_in(dir.path()).unwrap_err().code,
            crate::app_error::ErrorCode::NotFound
        );
        let missing = dir.path().join("missing");
        assert_eq!(
            license_bundle_in(&missing).unwrap_err().code,
            crate::app_error::ErrorCode::NotFound
        );
    }
}
