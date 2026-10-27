//! 파일 identity: 크기와 수정 시각으로 내용 교체를 감지한다.
//!
//! 경로는 파일 내용을 보증하지 않으므로(같은 경로에 다른 CBZ가 들어옴) 경로로
//! 저장한 상태(이어보기 위치 등)를 다시 믿을 때 이 identity로 내용이 그대로인지
//! 확인한다. 관행은 아카이브 인덱스 캐시(`archive_index.rs`)와 같은 mtime + size다.

use serde::{Deserialize, Serialize};
use std::{fs, path::Path, time::UNIX_EPOCH};

/// 크기와 수정 시각으로 쓴 파일 identity. 경로는 담지 않는다. mtime을 읽을 수
/// 없는 파일은 0으로 둔다(`sidecar::file_identity_hash`와 같다).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIdentity {
    pub size: u64,
    pub modified_nanos: i64,
}

impl FileIdentity {
    /// 파일 메타데이터에서 identity를 읽는다. 파일이 없거나 메타데이터를
    /// 못 읽으면 `None`이다.
    pub fn capture(path: &Path) -> Option<Self> {
        let meta = fs::metadata(path).ok()?;
        Some(Self::from_metadata(&meta))
    }

    /// 메타데이터에서 identity를 만든다. epoch 이전 시각은 0으로 둔다.
    pub fn from_metadata(meta: &fs::Metadata) -> Self {
        let modified_nanos = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        Self {
            size: meta.len(),
            modified_nanos,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 파일이 없으면 identity를 읽을 수 없다.
    #[test]
    fn capture_returns_none_for_a_missing_file() {
        assert!(FileIdentity::capture(Path::new("/nonexistent/araview-test-file")).is_none());
    }

    /// 크기가 바뀐 파일은 identity가 달라진다(들어 있던 진행 기록을 무효화한다).
    #[test]
    fn identity_changes_when_the_file_content_changes() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("araview-identity-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.bin");
        std::fs::write(&path, vec![0u8; 10]).unwrap();
        let first = FileIdentity::capture(&path).unwrap();

        std::fs::write(&path, vec![0u8; 20]).unwrap();
        let second = FileIdentity::capture(&path).unwrap();

        assert_ne!(first, second);
        std::fs::remove_dir_all(&dir).ok();
    }
}
