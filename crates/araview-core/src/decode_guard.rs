//! 디코더 panic 격리.
//!
//! 이미지와 아카이브는 신뢰할 수 없는 입력이고, 그 내용을 해석하는 서드파티
//! 디코더에는 panic 경로가 남아 있다. 예를 들어 rawler 0.8은 파일에서 읽은
//! 오프셋으로 버퍼를 바로 잘라, 복사가 끊겨 앞부분만 남은 MRW에서 범위를 벗어난다.
//! 이 panic이 호출자까지 올라가면 프로세스가 끝난다. GPUI의 Windows 스레드 풀
//! 콜백은 unwind를 받지 못해 abort하고, 탐색기 썸네일 DLL은 호스트 프로세스
//! 안에서 돈다.
//!
//! 그래서 파일 내용을 해석하는 공개 진입점은 여기서 panic을 잡아 `Corrupt`
//! 에러로 바꾼다. 한 파일의 손상은 그 파일의 에러로 끝나고, 같은 폴더의 다른
//! 파일과 앱은 계속 동작한다.
//!
//! 스택 오버플로와 `abort`는 잡지 못한다. 디코드 중에 쥐는 잠금은 poison을
//! 복구하고(`sidecar::with_file_lock`), 쓰던 캐시 임시 파일은 unwind에서
//! 지워지므로(`sidecar::write_cache_atomic`), panic 뒤에도 같은 파일을 다시
//! 요청할 수 있다.

use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::app_error::AppError;

/// panic한 디코드가 돌려주는 에러 문구. PSD 디코더가 쓰던 문구를 그대로 쓴다.
const PANIC_MESSAGE: &str = "Failed to decode image: decoder panicked";

/// `decode`를 실행하고, panic하면 `Corrupt` 에러로 바꾼다. `operation`은 로그에
/// 남기는 연산 이름이다(파일 경로는 남기지 않는다).
pub(crate) fn guard<T>(
    operation: &'static str,
    decode: impl FnOnce() -> Result<T, AppError>,
) -> Result<T, AppError> {
    guard_or_else(operation, decode, || Err(AppError::corrupt(PANIC_MESSAGE)))
}

/// 실패를 `None`으로 알리는 디코드용. panic하면 `None`이다.
pub(crate) fn guard_option<T>(
    operation: &'static str,
    decode: impl FnOnce() -> Option<T>,
) -> Option<T> {
    guard_or_else(operation, decode, || None)
}

fn guard_or_else<T>(
    operation: &'static str,
    decode: impl FnOnce() -> T,
    panicked: impl FnOnce() -> T,
) -> T {
    // 디코드가 건드리는 공유 상태는 위 모듈 설명대로 panic 뒤에도 일관되므로
    // unwind 안전성을 단언한다.
    match catch_unwind(AssertUnwindSafe(decode)) {
        Ok(value) => value,
        Err(_) => {
            log::warn!("[decode] {operation}: decoder panicked on malformed input");
            panicked()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_error::ErrorCode;

    #[test]
    fn results_pass_through_unchanged() {
        assert_eq!(guard("test", || Ok(7)), Ok(7));
        let error = AppError::too_large("too big");
        assert_eq!(guard("test", || Err::<(), _>(error.clone())), Err(error));
        assert_eq!(guard_option("test", || Some(7)), Some(7));
        assert_eq!(guard_option("test", || None::<u8>), None);
    }

    #[test]
    fn a_panic_becomes_a_corrupt_error() {
        let error =
            guard("test", || -> Result<(), AppError> { panic!("decoder bug") }).unwrap_err();
        assert_eq!(error.code, ErrorCode::Corrupt);
        assert_eq!(error.message, PANIC_MESSAGE);
        assert_eq!(
            guard_option("test", || -> Option<u8> { panic!("boom") }),
            None
        );
    }

    /// 복사가 끊겨 머리글 앞부분만 남은 MRW. rawler 0.8은 파일에서 읽은 블록
    /// 오프셋으로 버퍼를 바로 잘라 이 파일에서 범위를 벗어난다(`bits.rs`).
    fn truncated_mrw(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("truncated.mrw");
        std::fs::write(&path, b"\0MRM\0\0\x04\x3fV33").unwrap();
        path
    }

    /// 손상된 RAW 하나가 어느 공개 진입점에서도 panic으로 새지 않는다. 격리가
    /// 없으면 이 테스트는 에러를 받기 전에 rawler의 panic으로 실패한다.
    #[test]
    fn a_truncated_raw_fails_every_entry_point_without_panicking() {
        use crate::transcode::Decoder;

        let dir = tempfile::tempdir().unwrap();
        let path = truncated_mrw(dir.path());
        let text = path.to_str().unwrap();
        let failures = [
            crate::transcode::decode_rgb8(&path, Decoder::Raw).map(drop),
            crate::image::load_viewable(&path).map(drop),
            crate::ops::load_image_blocking(text, Some(512), None).map(drop),
            crate::display::decode_for_display(&path, 4096).map(drop),
            crate::thumbnail::generate_thumbnail(&path, 128).map(drop),
            crate::image_info::histogram_for_path(&path).map(drop),
            crate::image_info::details_for_path(&path).map(drop),
            crate::clipboard_png::export_clipboard_png(&path).map(drop),
            crate::ops::decode_shell_thumbnail(&path, 128).map(drop),
        ];
        for (index, failure) in failures.into_iter().enumerate() {
            let error = failure.expect_err("a truncated RAW must not decode");
            // 업스트림이 panic을 고치면 일반 오류 경로의 분류가 대신 온다.
            assert!(
                matches!(error.code, ErrorCode::Corrupt | ErrorCode::Unsupported),
                "entry point {index}: {error:?}"
            );
        }
        assert!(crate::display::decode_preview_for_display(&path, 512).is_none());

        // panic한 디코더가 다음 파일까지 막지 않는다. 같은 rawler 경로의 정상 MRW는
        // 그 뒤에도 열린다.
        let sound =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/sample.mrw");
        let decoded = crate::transcode::decode_rgb8(&sound, Decoder::Raw).unwrap();
        assert!(decoded.width > 0 && decoded.height > 0);
    }

    /// 썸네일 배치는 손상된 항목만 실패로 돌려주고 나머지를 그대로 만든다.
    #[test]
    fn a_thumbnail_batch_survives_a_panicking_item() {
        let dir = tempfile::tempdir().unwrap();
        let broken = truncated_mrw(dir.path());
        let sound = dir.path().join("sound.png");
        image::RgbImage::from_pixel(64, 32, image::Rgb([20, 80, 160]))
            .save(&sound)
            .unwrap();
        let sources = [
            broken.to_string_lossy().into_owned(),
            sound.to_string_lossy().into_owned(),
        ];

        let batch = crate::thumbnail::generate_thumbnails_batch(&sources, 64);

        assert_eq!(batch.len(), 2);
        assert!(batch[0].thumb.is_none() && batch[0].error.is_some());
        assert!(batch[1].thumb.is_some() && batch[1].error.is_none());
    }

    /// panic이 잠금을 쥔 채 일어나도 같은 키를 다시 쓸 수 있다.
    #[test]
    fn a_panic_under_a_file_lock_leaves_the_key_usable() {
        let key = "decode-guard-test-key";
        let first = guard("test", || {
            crate::sidecar::with_file_lock(key, "test lock", || -> Result<(), AppError> {
                panic!("decoder bug")
            })
        });
        assert_eq!(first.unwrap_err().code, ErrorCode::Corrupt);
        let second = guard("test", || {
            crate::sidecar::with_file_lock(key, "test lock", || Ok(42))
        });
        assert_eq!(second, Ok(42));
    }
}
