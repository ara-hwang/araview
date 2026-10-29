//! 파일 identity: 크기와 수정 시각으로 내용 교체를 감지한다.
//!
//! 경로는 파일 내용을 보증하지 않으므로(같은 경로에 다른 CBZ가 들어옴) 경로로
//! 저장한 상태(이어보기 위치 등)를 다시 믿을 때 이 identity로 내용이 그대로인지
//! 확인한다. 읽기 진도에는 mtime + size를 쓰고, 아카이브 파생 캐시는 OS 파일
//! identity와 변경 시각까지 포함한 [`ArchiveCacheIdentity`]를 쓴다.

use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, Metadata},
    io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

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

/// Fast source identity for archive-derived caches. It uses filesystem metadata
/// only, so validating a cache hit never reads or hashes archive contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ArchiveCacheIdentity {
    pub(crate) size: u64,
    pub(crate) modified: Option<SystemTime>,
    file_id: [u64; 2],
    change_time: Option<i128>,
}

impl ArchiveCacheIdentity {
    pub(crate) fn from_file(file: &File) -> io::Result<Self> {
        let metadata = file.metadata()?;
        let (file_id, change_time) = platform_file_identity(file, &metadata);
        Ok(Self {
            size: metadata.len(),
            modified: metadata.modified().ok(),
            file_id,
            change_time,
        })
    }

    pub(crate) fn write_hash(self, hasher: &mut crate::stable_hash::StableHasher) {
        hasher.write_u64_le(self.size);
        if let Some(modified) = self
            .modified
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        {
            hasher.write(&[1]);
            hasher.write_u128_le(modified.as_nanos());
        } else {
            hasher.write(&[0]);
        }
        hasher.write_u64_le(self.file_id[0]);
        hasher.write_u64_le(self.file_id[1]);
        if let Some(change_time) = self.change_time {
            hasher.write(&[1]);
            hasher.write(&change_time.to_le_bytes());
        } else {
            hasher.write(&[0]);
        }
    }
}

#[cfg(unix)]
fn platform_file_identity(_file: &File, metadata: &Metadata) -> ([u64; 2], Option<i128>) {
    use std::os::unix::fs::MetadataExt;

    let change_time =
        i128::from(metadata.ctime()) * 1_000_000_000 + i128::from(metadata.ctime_nsec());
    ([metadata.dev(), metadata.ino()], Some(change_time))
}

#[cfg(windows)]
fn platform_file_identity(file: &File, metadata: &Metadata) -> ([u64; 2], Option<i128>) {
    use std::os::windows::fs::MetadataExt;

    let file_id = [
        u64::from(metadata.volume_serial_number().unwrap_or_default()),
        metadata.file_index().unwrap_or_default(),
    ];
    (file_id, windows_change_time(file))
}

#[cfg(windows)]
fn windows_change_time(file: &File) -> Option<i128> {
    use std::{
        ffi::c_void,
        mem::{size_of, MaybeUninit},
        os::windows::io::AsRawHandle,
    };

    #[repr(C)]
    struct FileBasicInfo {
        creation_time: i64,
        last_access_time: i64,
        last_write_time: i64,
        change_time: i64,
        file_attributes: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandleEx(
            file: *mut c_void,
            information_class: u32,
            information: *mut c_void,
            buffer_size: u32,
        ) -> i32;
    }

    const FILE_BASIC_INFO: u32 = 0;
    let mut information = MaybeUninit::<FileBasicInfo>::uninit();
    // SAFETY: `file` is a live std::fs::File handle and the output buffer has
    // the exact C layout and size required for FILE_BASIC_INFO.
    let succeeded = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle().cast(),
            FILE_BASIC_INFO,
            information.as_mut_ptr().cast(),
            size_of::<FileBasicInfo>() as u32,
        )
    };
    if succeeded == 0 {
        return None;
    }
    // SAFETY: a nonzero result means Windows initialized the full output struct.
    Some(i128::from(unsafe { information.assume_init() }.change_time))
}

#[cfg(not(any(unix, windows)))]
fn platform_file_identity(_file: &File, _metadata: &Metadata) -> ([u64; 2], Option<i128>) {
    ([0, 0], None)
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
