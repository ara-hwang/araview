//! 이미지 속 글자 인식(OCR)의 UI 비의존 부분: 결과 타입, 엔진 trait, 언어 병합.
//!
//! 실제 인식은 OS 내장 엔진이 맡는다. 엔진은 호스트 앱이 구현해 넘기므로(Windows는
//! `src-gpui/src/platform/`) 코어는 OS 연동 없이 디코드와 좌표 정규화, 여러 언어
//! 결과 합치기만 한다.

use image::{imageops, RgbaImage};
use rayon::prelude::*;
use serde::Serialize;

use crate::app_error::AppError;

/// 이미지 전체를 가로세로 1로 본 좌표계의 사각형.
#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
pub struct OcrRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl OcrRect {
    pub fn union(self, other: Self) -> Self {
        let left = self.x.min(other.x);
        let top = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        }
    }

    fn area(self) -> f32 {
        self.width.max(0.0) * self.height.max(0.0)
    }

    fn intersection_area(self, other: Self) -> f32 {
        let width = (self.x + self.width).min(other.x + other.width) - self.x.max(other.x);
        let height = (self.y + self.height).min(other.y + other.height) - self.y.max(other.y);
        width.max(0.0) * height.max(0.0)
    }

    /// 두 사각형이 겹치는 넓이가 더 작은 쪽의 절반을 넘는지.
    fn mostly_overlaps(self, other: Self) -> bool {
        let smaller = self.area().min(other.area());
        smaller > 0.0 && self.intersection_area(other) / smaller > OVERLAP_RATIO
    }

    /// 픽셀 좌표를 `width` x `height` 이미지 기준 0~1 좌표로 바꾼다.
    fn normalized(self, width: f32, height: f32) -> Self {
        Self {
            x: self.x / width,
            y: self.y / height,
            width: self.width / width,
            height: self.height / height,
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct OcrWord {
    pub text: String,
    pub rect: OcrRect,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct OcrLine {
    pub text: String,
    pub rect: OcrRect,
    pub words: Vec<OcrWord>,
    /// 이 줄을 인식한 언어(BCP-47 태그).
    pub language: String,
}

impl OcrLine {
    /// 공백을 뺀 글자 수. 여러 언어 결과 가운데 고르는 기준이다.
    fn glyph_count(&self) -> usize {
        self.text.chars().filter(|c| !c.is_whitespace()).count()
    }
}

/// 인식 결과. 좌표는 이미지 전체를 1로 본 값이라 표시 크기와 무관하다.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct OcrResult {
    pub lines: Vec<OcrLine>,
    /// 인식에 넣은 이미지의 치수(px). 엔진 한계로 줄였다면 줄인 뒤 값이다.
    pub width: u32,
    pub height: u32,
}

impl OcrResult {
    /// 줄을 위에서 아래 순서 그대로 이어 붙인 전체 텍스트.
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|line| line.glyph_count() == 0)
    }
}

/// 어떤 언어로 인식할지.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OcrLanguage {
    /// 설치된 OCR 언어를 모두 돌려 결과를 합친다.
    Auto,
    /// BCP-47 태그(`ko`, `en-US`)로 하나만 돌린다.
    Tag(String),
}

/// OS가 제공하는 OCR 엔진. 호스트 앱이 구현한다.
pub trait OcrEngine: Sync {
    /// 지금 인식할 수 있는 언어 태그(언어 팩이 설치된 것).
    fn available_languages(&self) -> Result<Vec<String>, AppError>;

    /// 엔진이 받는 이미지의 한 변 상한(px).
    fn max_image_dimension(&self) -> u32;

    /// 한 언어로 인식한다. 반환 좌표는 `image`의 픽셀 단위이고 `language`를 채워
    /// 돌려준다. 글자가 없으면 빈 목록이다.
    fn recognize(&self, image: &RgbaImage, language: &str) -> Result<Vec<OcrLine>, AppError>;
}

/// 여러 언어 결과에서 같은 자리를 읽은 줄이 겹치면 같은 줄로 본다.
const OVERLAP_RATIO: f32 = 0.5;

/// `image`를 인식한다. 엔진 한계보다 크면 줄여서 넘기고, 좌표는 정규화해 돌려준다.
pub fn recognize_image(
    engine: &dyn OcrEngine,
    image: RgbaImage,
    language: &OcrLanguage,
) -> Result<OcrResult, AppError> {
    if image.width() == 0 || image.height() == 0 {
        return Err(AppError::invalid_input("Image has no pixels"));
    }
    let image = fit_to_engine(image, engine.max_image_dimension());
    let (width, height) = image.dimensions();

    let lines = match language {
        OcrLanguage::Tag(tag) => engine.recognize(&image, tag)?,
        OcrLanguage::Auto => {
            let tags = engine.available_languages()?;
            if tags.is_empty() {
                return Err(AppError::unsupported("No OCR language is installed"));
            }
            recognize_all(engine, &image, &tags)?
        }
    };

    let (norm_w, norm_h) = (width as f32, height as f32);
    let lines = lines
        .into_iter()
        .map(|line| OcrLine {
            rect: line.rect.normalized(norm_w, norm_h),
            words: line
                .words
                .into_iter()
                .map(|word| OcrWord {
                    text: word.text,
                    rect: word.rect.normalized(norm_w, norm_h),
                })
                .collect(),
            ..line
        })
        .collect();
    Ok(OcrResult {
        lines,
        width,
        height,
    })
}

/// 가장 긴 변이 `limit`을 넘으면 비율을 지켜 줄인다.
fn fit_to_engine(image: RgbaImage, limit: u32) -> RgbaImage {
    let longest = image.width().max(image.height());
    if limit == 0 || longest <= limit {
        return image;
    }
    let ratio = f64::from(limit) / f64::from(longest);
    let width = ((f64::from(image.width()) * ratio).round() as u32).max(1);
    let height = ((f64::from(image.height()) * ratio).round() as u32).max(1);
    imageops::resize(&image, width, height, imageops::FilterType::Lanczos3)
}

/// 설치된 언어를 병렬로 모두 돌려 합친다. 일부 언어가 실패해도 나머지로 이어가고,
/// 전부 실패했을 때만 첫 오류를 돌려준다.
fn recognize_all(
    engine: &dyn OcrEngine,
    image: &RgbaImage,
    tags: &[String],
) -> Result<Vec<OcrLine>, AppError> {
    let results: Vec<Result<Vec<OcrLine>, AppError>> = tags
        .par_iter()
        .map(|tag| engine.recognize(image, tag))
        .collect();
    let mut recognized = Vec::new();
    let mut first_error = None;
    for (tag, result) in tags.iter().zip(results) {
        match result {
            Ok(lines) => recognized.push(lines),
            Err(error) => {
                log::warn!("OCR failed for `{tag}`: {}", error.message);
                first_error.get_or_insert(error);
            }
        }
    }
    match (recognized.is_empty(), first_error) {
        (true, Some(error)) => Err(error),
        _ => Ok(merge_languages(recognized)),
    }
}

/// 언어별 결과를 한 목록으로 합친다. 같은 자리를 여러 언어가 읽었으면 글자가 가장
/// 많이 인식된 줄 하나만 남기고, 결과는 위에서 아래, 왼쪽에서 오른쪽 순서로 둔다.
/// 엔진이 줄별 신뢰도를 주지 않아서 쓰는 임시 기준이다(틀린 언어가 잡음 글자를 더
/// 많이 만들 수 있다).
fn merge_languages(per_language: Vec<Vec<OcrLine>>) -> Vec<OcrLine> {
    let mut non_empty: Vec<Vec<OcrLine>> = per_language
        .into_iter()
        .filter(|lines| !lines.is_empty())
        .collect();
    if non_empty.len() <= 1 {
        return non_empty.pop().unwrap_or_default();
    }
    let mut candidates: Vec<OcrLine> = non_empty.into_iter().flatten().collect();
    // 안정 정렬이라 글자 수가 같으면 언어 순서가 앞선 쪽이 남는다.
    candidates.sort_by_key(|line| std::cmp::Reverse(line.glyph_count()));
    let mut kept: Vec<OcrLine> = Vec::new();
    for line in candidates {
        if line.glyph_count() == 0 {
            continue;
        }
        if !kept
            .iter()
            .any(|other| other.rect.mostly_overlaps(line.rect))
        {
            kept.push(line);
        }
    }
    kept.sort_by(|a, b| {
        let key = |line: &OcrLine| (line.rect.y + line.rect.height / 2.0, line.rect.x);
        key(a)
            .partial_cmp(&key(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    kept
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    fn line(text: &str, language: &str, x: f32, y: f32, width: f32, height: f32) -> OcrLine {
        let rect = OcrRect {
            x,
            y,
            width,
            height,
        };
        OcrLine {
            text: text.to_owned(),
            rect,
            words: vec![OcrWord {
                text: text.to_owned(),
                rect,
            }],
            language: language.to_owned(),
        }
    }

    /// 언어별로 정해 둔 줄을 돌려주고, 받은 이미지 치수를 기록하는 가짜 엔진.
    struct FakeEngine {
        max_dimension: u32,
        lines: HashMap<&'static str, Result<Vec<OcrLine>, AppError>>,
        seen: Mutex<Vec<(u32, u32)>>,
    }

    impl FakeEngine {
        fn new(max_dimension: u32) -> Self {
            Self {
                max_dimension,
                lines: HashMap::new(),
                seen: Mutex::new(Vec::new()),
            }
        }

        fn with(mut self, tag: &'static str, lines: Result<Vec<OcrLine>, AppError>) -> Self {
            self.lines.insert(tag, lines);
            self
        }
    }

    impl OcrEngine for FakeEngine {
        fn available_languages(&self) -> Result<Vec<String>, AppError> {
            let mut tags: Vec<String> = self.lines.keys().map(|tag| (*tag).to_owned()).collect();
            tags.sort();
            Ok(tags)
        }

        fn max_image_dimension(&self) -> u32 {
            self.max_dimension
        }

        fn recognize(&self, image: &RgbaImage, language: &str) -> Result<Vec<OcrLine>, AppError> {
            self.seen.lock().unwrap().push(image.dimensions());
            self.lines
                .get(language)
                .cloned()
                .unwrap_or_else(|| Err(AppError::unsupported("language not installed")))
        }
    }

    fn blank(width: u32, height: u32) -> RgbaImage {
        RgbaImage::new(width, height)
    }

    #[test]
    fn rejects_an_empty_image() {
        let engine = FakeEngine::new(4096);
        let error = recognize_image(&engine, blank(0, 10), &OcrLanguage::Auto).unwrap_err();
        assert_eq!(error.code, crate::app_error::ErrorCode::InvalidInput);
    }

    #[test]
    fn normalizes_pixel_coordinates_to_the_image() {
        let engine =
            FakeEngine::new(4096).with("en", Ok(vec![line("Hello", "en", 20.0, 10.0, 40.0, 10.0)]));
        let result =
            recognize_image(&engine, blank(200, 100), &OcrLanguage::Tag("en".into())).unwrap();
        assert_eq!((result.width, result.height), (200, 100));
        let rect = result.lines[0].rect;
        assert_eq!(
            rect,
            OcrRect {
                x: 0.1,
                y: 0.1,
                width: 0.2,
                height: 0.1
            }
        );
        assert_eq!(result.lines[0].words[0].rect, rect);
        assert_eq!(result.text(), "Hello");
    }

    #[test]
    fn shrinks_images_the_engine_cannot_take() {
        let engine = FakeEngine::new(100).with("en", Ok(Vec::new()));
        let result =
            recognize_image(&engine, blank(400, 200), &OcrLanguage::Tag("en".into())).unwrap();
        assert_eq!(*engine.seen.lock().unwrap(), vec![(100, 50)]);
        assert_eq!((result.width, result.height), (100, 50));
        assert!(result.is_empty());
    }

    #[test]
    fn keeps_images_within_the_limit_untouched() {
        let engine = FakeEngine::new(400).with("en", Ok(Vec::new()));
        recognize_image(&engine, blank(400, 200), &OcrLanguage::Tag("en".into())).unwrap();
        assert_eq!(*engine.seen.lock().unwrap(), vec![(400, 200)]);
    }

    #[test]
    fn auto_without_installed_languages_is_unsupported() {
        let engine = FakeEngine::new(4096);
        let error = recognize_image(&engine, blank(10, 10), &OcrLanguage::Auto).unwrap_err();
        assert_eq!(error.code, crate::app_error::ErrorCode::Unsupported);
    }

    #[test]
    fn auto_keeps_the_language_that_read_more_glyphs() {
        // 같은 자리를 ko는 제대로, en은 일부만 읽었다. 아래 줄은 en만 읽었다.
        let engine = FakeEngine::new(4096)
            .with(
                "ko",
                Ok(vec![line("안녕하세요 세계", "ko", 0.0, 0.0, 100.0, 10.0)]),
            )
            .with(
                "en",
                Ok(vec![
                    line("l", "en", 0.0, 0.0, 100.0, 10.0),
                    line("Second line", "en", 0.0, 50.0, 100.0, 10.0),
                ]),
            );
        let result = recognize_image(&engine, blank(100, 100), &OcrLanguage::Auto).unwrap();
        let texts: Vec<&str> = result.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["안녕하세요 세계", "Second line"]);
        assert_eq!(result.lines[0].language, "ko");
        assert_eq!(result.lines[1].language, "en");
    }

    #[test]
    fn auto_orders_merged_lines_top_to_bottom() {
        let engine = FakeEngine::new(4096)
            .with("en", Ok(vec![line("bottom", "en", 0.0, 80.0, 50.0, 10.0)]))
            .with("ko", Ok(vec![line("위쪽", "ko", 0.0, 5.0, 50.0, 10.0)]));
        let result = recognize_image(&engine, blank(100, 100), &OcrLanguage::Auto).unwrap();
        assert_eq!(result.text(), "위쪽\nbottom");
    }

    #[test]
    fn auto_survives_one_failing_language() {
        let engine = FakeEngine::new(4096)
            .with("en", Ok(vec![line("Hello", "en", 0.0, 0.0, 10.0, 10.0)]))
            .with("ja", Err(AppError::unknown("engine failed")));
        let result = recognize_image(&engine, blank(100, 100), &OcrLanguage::Auto).unwrap();
        assert_eq!(result.text(), "Hello");
    }

    #[test]
    fn auto_reports_the_error_when_every_language_fails() {
        let engine = FakeEngine::new(4096).with("en", Err(AppError::unknown("engine failed")));
        let error = recognize_image(&engine, blank(100, 100), &OcrLanguage::Auto).unwrap_err();
        assert_eq!(error.message, "engine failed");
    }

    #[test]
    fn single_language_result_keeps_engine_order() {
        let lines = vec![
            line("right column", "en", 60.0, 0.0, 40.0, 10.0),
            line("left column", "en", 0.0, 0.0, 40.0, 10.0),
        ];
        let engine = FakeEngine::new(4096).with("en", Ok(lines));
        let result = recognize_image(&engine, blank(100, 100), &OcrLanguage::Auto).unwrap();
        assert_eq!(result.text(), "right column\nleft column");
    }

    #[test]
    fn whitespace_only_result_counts_as_empty() {
        let engine =
            FakeEngine::new(4096).with("en", Ok(vec![line("  ", "en", 0.0, 0.0, 5.0, 5.0)]));
        let result =
            recognize_image(&engine, blank(10, 10), &OcrLanguage::Tag("en".into())).unwrap();
        assert!(result.is_empty());
    }
}
