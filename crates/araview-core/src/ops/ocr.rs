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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocr::{OcrLine, OcrRect};
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingEngine {
        calls: Mutex<Vec<(u32, u32, String)>>,
    }

    impl OcrEngine for RecordingEngine {
        fn available_languages(&self) -> Result<Vec<String>, AppError> {
            Ok(vec!["ko".into()])
        }

        fn max_image_dimension(&self) -> u32 {
            4096
        }

        fn recognize(
            &self,
            image: &image::RgbaImage,
            language: &str,
        ) -> Result<Vec<OcrLine>, AppError> {
            let (width, height) = image.dimensions();
            self.calls
                .lock()
                .unwrap()
                .push((width, height, language.to_owned()));
            Ok(vec![OcrLine {
                text: "안녕".into(),
                rect: OcrRect {
                    x: 0.0,
                    y: 0.0,
                    width: width as f32,
                    height: height as f32,
                },
                words: Vec::new(),
                language: language.to_owned(),
            }])
        }
    }

    fn sample(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples")
            .join(name)
    }

    #[test]
    fn recognizes_the_first_frame_through_the_engine() {
        let engine = RecordingEngine::default();
        let result = recognize_text_blocking(
            &engine,
            &sample("sample.png"),
            &OcrLanguage::Tag("ko".into()),
        )
        .expect("recognize");

        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.lines[0].text, "안녕");
        assert_eq!((result.width, result.height), (800, 600));
        // 디코드한 프레임 하나만 엔진에 넘긴다.
        let calls = engine.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], (800, 600, "ko".to_owned()));
    }

    #[test]
    fn animated_sources_use_the_first_frame() {
        let engine = RecordingEngine::default();
        let result = recognize_text_blocking(&engine, &sample("sample.gif"), &OcrLanguage::Auto)
            .expect("recognize");
        assert!(result.width > 0 && result.height > 0);
        assert_eq!(engine.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn missing_file_is_not_found() {
        let engine = RecordingEngine::default();
        let err = recognize_text_blocking(
            &engine,
            Path::new("D:\\no-such-dir\\missing.png"),
            &OcrLanguage::Auto,
        )
        .unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::NotFound);
        assert!(engine.calls.lock().unwrap().is_empty());
    }
}
