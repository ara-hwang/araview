//! 이미지 속 글자 인식(SPEC §15 `recognize_text`).

use std::path::Path;

use crate::app_error::AppError;
use crate::display::decode_for_display;
use crate::ocr::{recognize_image, OcrEngine, OcrLanguage, OcrResult};

/// 벡터 이미지를 래스터화할 때의 긴 변 상한(px). 화면 표시와 같은 값이다.
const SVG_MAX_SIDE: u32 = 8192;

/// `path`(화면이 디코드하는 파일 경로)의 글자를 인식한다. 애니메이션은 첫 프레임만 쓴다.
pub fn recognize_text_blocking(
    engine: &dyn OcrEngine,
    path: &Path,
    language: &OcrLanguage,
) -> Result<OcrResult, AppError> {
    if !path.exists() {
        return Err(AppError::not_found("File not found"));
    }
    let decoded = decode_for_display(path, SVG_MAX_SIDE)?;
    let frame = decoded
        .frames
        .into_iter()
        .next()
        .ok_or_else(|| AppError::corrupt("Image has no frames"))?;
    recognize_image(engine, frame.image, language)
}
