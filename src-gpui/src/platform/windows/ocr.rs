//! Windows 내장 OCR(`Windows.Media.Ocr`)을 코어의 `OcrEngine`으로 감싼다.
//!
//! 언어는 사용자가 OCR 언어 팩을 설치한 것만 쓸 수 있다. 한 번 호출에 언어 하나만
//! 인식하므로 여러 언어를 합치는 일은 코어(`araview_core::ocr`)가 맡는다.

use araview_core::app_error::AppError;
use araview_core::ocr::{OcrEngine, OcrLine, OcrRect, OcrWord};
use image::RgbaImage;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::{OcrEngine as WinOcrEngine, OcrLine as WinOcrLine};
use windows::Storage::Streams::DataWriter;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::core::HSTRING;

/// 엔진이 한 변 상한을 알려주지 못할 때 쓰는 값(Windows OCR의 문서화된 최대치).
const FALLBACK_MAX_DIMENSION: u32 = 10_000;

/// OS 내장 OCR. 상태가 없어서 호출마다 WinRT 객체를 만든다.
pub struct SystemOcr;

fn winrt(context: &str, error: windows::core::Error) -> AppError {
    AppError::unknown(format!("{context}: {error}"))
}

/// 백그라운드 스레드에서 WinRT를 쓰려면 스레드마다 아파트가 있어야 한다.
fn init_winrt() {
    // SAFETY: 인자는 상수 하나뿐이고 현재 스레드의 아파트 모델만 정한다. 이미 다른
    // 모델로 초기화된 스레드는 오류를 돌려주지만 그 상태에서도 WinRT를 쓸 수 있어 무시한다.
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
}

fn create_engine(tag: &str) -> Result<WinOcrEngine, AppError> {
    let language = Language::CreateLanguage(&HSTRING::from(tag))
        .map_err(|_| AppError::invalid_input(format!("Invalid OCR language tag `{tag}`")))?;
    let installed = WinOcrEngine::IsLanguageSupported(&language)
        .map_err(|e| winrt("Failed to query OCR languages", e))?;
    if !installed {
        return Err(AppError::unsupported(format!(
            "OCR language `{tag}` is not installed"
        )));
    }
    WinOcrEngine::TryCreateFromLanguage(&language)
        .map_err(|_| AppError::unsupported(format!("OCR language `{tag}` is not installed")))
}

/// 알파를 흰 배경에 합쳐 불투명 BGRA 비트맵으로 만든다. 투명한 곳이 검게 읽혀
/// 글자가 묻히는 것을 막는다.
fn bitmap_from(image: &RgbaImage) -> Result<SoftwareBitmap, AppError> {
    let mut bgra = Vec::with_capacity(image.as_raw().len());
    for pixel in image.pixels() {
        let [r, g, b, a] = pixel.0;
        let alpha = u32::from(a);
        let over_white = |c: u8| ((u32::from(c) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
        bgra.extend_from_slice(&[over_white(b), over_white(g), over_white(r), 255]);
    }
    let writer = DataWriter::new().map_err(|e| winrt("Failed to create OCR buffer", e))?;
    writer
        .WriteBytes(&bgra)
        .map_err(|e| winrt("Failed to fill OCR buffer", e))?;
    let buffer = writer
        .DetachBuffer()
        .map_err(|e| winrt("Failed to detach OCR buffer", e))?;
    SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        image.width() as i32,
        image.height() as i32,
        BitmapAlphaMode::Ignore,
    )
    .map_err(|e| winrt("Failed to create OCR bitmap", e))
}

/// 단어 사이에 공백을 넣지 않는 문자 체계의 언어인지. 엔진이 중국어와 일본어를
/// 글자마다 공백으로 끊어 돌려준다는 보고에 따른 처리다(실기에서 확인 필요).
fn joins_words_without_spaces(tag: &str) -> bool {
    let primary = tag.split('-').next().unwrap_or(tag);
    primary.eq_ignore_ascii_case("ja") || primary.eq_ignore_ascii_case("zh")
}

fn convert_line(line: &WinOcrLine, tag: &str) -> windows::core::Result<Option<OcrLine>> {
    let mut words = Vec::new();
    let mut rect: Option<OcrRect> = None;
    for word in line.Words()? {
        let bounds = word.BoundingRect()?;
        let word_rect = OcrRect {
            x: bounds.X,
            y: bounds.Y,
            width: bounds.Width,
            height: bounds.Height,
        };
        rect = Some(rect.map_or(word_rect, |acc| acc.union(word_rect)));
        words.push(OcrWord {
            text: word.Text()?.to_string(),
            rect: word_rect,
        });
    }
    let Some(rect) = rect else {
        return Ok(None);
    };
    let text = if joins_words_without_spaces(tag) {
        words.iter().map(|word| word.text.as_str()).collect()
    } else {
        line.Text()?.to_string()
    };
    Ok(Some(OcrLine {
        text,
        rect,
        words,
        language: tag.to_owned(),
    }))
}

impl OcrEngine for SystemOcr {
    fn available_languages(&self) -> Result<Vec<String>, AppError> {
        init_winrt();
        let languages = WinOcrEngine::AvailableRecognizerLanguages()
            .map_err(|e| winrt("Failed to list OCR languages", e))?;
        let mut tags = Vec::new();
        for language in languages {
            let tag = language
                .LanguageTag()
                .map_err(|e| winrt("Failed to read OCR language", e))?;
            tags.push(tag.to_string());
        }
        Ok(tags)
    }

    fn max_image_dimension(&self) -> u32 {
        WinOcrEngine::MaxImageDimension().unwrap_or(FALLBACK_MAX_DIMENSION)
    }

    fn recognize(&self, image: &RgbaImage, language: &str) -> Result<Vec<OcrLine>, AppError> {
        init_winrt();
        let engine = create_engine(language)?;
        let bitmap = bitmap_from(image)?;
        let result = engine
            .RecognizeAsync(&bitmap)
            .and_then(|operation| operation.join())
            .map_err(|e| winrt("OCR failed", e))?;
        let mut lines = Vec::new();
        for line in result
            .Lines()
            .map_err(|e| winrt("Failed to read OCR result", e))?
        {
            if let Some(line) =
                convert_line(&line, language).map_err(|e| winrt("Failed to read OCR line", e))?
            {
                lines.push(line);
            }
        }
        Ok(lines)
    }
}
