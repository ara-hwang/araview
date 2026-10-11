//! JPEG XL 디코드.
//!
//! `image` 크레이트가 JXL을 지원하지 않아 순수 Rust 디코더 jxl-oxide를 쓴다
//! (C 라이브러리나 DLL 추가 없음).
//! - 표시와 클립보드는 원본을 RGBA로 직접 디코드하고, 썸네일과 히스토그램은
//!   AVIF처럼 JPEG sidecar 규칙(`SPEC`)을 탄다.
//! - 컨테이너와 베어 코드스트림을 모두 읽는다. 애니메이션은 첫 키프레임만 쓴다.
//! - 코드스트림 헤더의 orientation은 디코더가 픽셀과 치수에 반영하므로 별도의
//!   EXIF 회전은 적용하지 않는다.
//! - 색 공간이 열거형으로 신호된 이미지(XYB 손실 압축, Display P3, PQ/HLG,
//!   선형 등)는 sRGB로 변환해 돌려준다. ICC 프로파일만 실린 이미지는 변환할
//!   CMS가 없어 픽셀을 그대로 두고 프로파일을 함께 돌려준다. CMYK는 같은
//!   이유로 `unsupported`로 보고한다.

use std::fs;
use std::io::Read;
use std::path::Path;

use jxl_oxide::color::{ColourEncoding, ColourSpace};
use jxl_oxide::{
    AllocTracker, EnumColourEncoding, InitializeResult, JxlImage, PixelFormat, RenderingIntent,
};

use crate::app_error::{AppError, ErrorCode};
use crate::sidecar::{Rgb8, SidecarSpec};

pub const JXL_MIME: &str = "image/jxl";

/// 이 디코더의 JPEG sidecar 규칙. `transcode`가 디코더별로 골라 쓴다.
pub(crate) static SPEC: SidecarSpec = SidecarSpec {
    label: "JXL sidecar lock",
    paint_prefix: "jxl-",
    thumb_prefix: "jxl-thumb-",
    decode: decode_rgb8,
};

/// 디코드 허용 픽셀 상한. 표시·클립보드 경로와 같은 값이다.
const MAX_DECODE_PIXELS: u64 = 100_000_000;

/// 디코더가 할당할 수 있는 상한. 내부 버퍼가 채널당 f32라 8비트 출력보다 크다.
const MAX_DECODE_ALLOC: usize = 4 * 1024 * 1024 * 1024;

/// 헤더를 찾을 때 읽는 단위.
const HEADER_CHUNK: usize = 4096;

fn decode_error(e: impl std::fmt::Display) -> AppError {
    AppError::corrupt(format!("Failed to decode image: {e}"))
}

/// 이미지 헤더까지만 읽는다. 치수와 색상 정보에는 픽셀 데이터가 필요 없다.
fn read_header(source: &Path) -> Result<JxlImage, AppError> {
    let mut file = fs::File::open(source)
        .map_err(|e| AppError::io("Failed to open image", e, ErrorCode::Corrupt))?;
    let mut uninit = JxlImage::builder().build_uninit();
    let mut buf = vec![0u8; HEADER_CHUNK];
    let mut filled = 0;
    loop {
        if filled == buf.len() {
            // 헤더 앞에 큰 박스(ICC, 메타데이터)가 있으면 버퍼를 늘린다.
            buf.resize(buf.len() * 2, 0);
        }
        let count = file
            .read(&mut buf[filled..])
            .map_err(|e| AppError::io("Failed to read file", e, ErrorCode::Corrupt))?;
        if count == 0 {
            return Err(AppError::corrupt("JPEG XL header is incomplete"));
        }
        filled += count;
        let consumed = uninit.feed_bytes(&buf[..filled]).map_err(decode_error)?;
        buf.copy_within(consumed..filled, 0);
        filled -= consumed;
        match uninit.try_init().map_err(decode_error)? {
            InitializeResult::NeedMoreData(next) => uninit = next,
            InitializeResult::Initialized(image) => return Ok(image),
        }
    }
}

/// 헤더만 읽어 orientation이 반영된 치수를 구한다.
pub(crate) fn dimensions(source: &Path) -> Option<(u32, u32)> {
    let image = read_header(source).ok()?;
    Some((image.width(), image.height()))
}

/// 상세 패널용 색상 모드와 채널당 비트 수. 원본 헤더 기준이다.
pub(crate) fn color_info(source: &Path) -> Option<(&'static str, u8)> {
    let image = read_header(source).ok()?;
    let mode = match image.pixel_format() {
        PixelFormat::Gray => "grayscale",
        PixelFormat::Graya => "grayscale-alpha",
        PixelFormat::Rgb => "rgb",
        PixelFormat::Rgba => "rgba",
        PixelFormat::Cmyk | PixelFormat::Cmyka => return None,
    };
    let bits = image.image_header().metadata.bit_depth.bits_per_sample();
    Some((mode, u8::try_from(bits).ok()?))
}

/// 첫 키프레임을 8비트로 디코드하고, 픽셀을 설명하는 ICC 프로파일을 함께
/// 돌려준다. 채널 구성은 원본을 따른다(회색조, 알파 유지).
pub(crate) fn decode(source: &Path) -> Result<(image::DynamicImage, Option<Vec<u8>>), AppError> {
    let mut image = JxlImage::builder()
        .alloc_tracker(AllocTracker::with_limit(MAX_DECODE_ALLOC))
        .open(source)
        .map_err(decode_error)?;
    let (width, height) = (image.width(), image.height());
    if u64::from(width).saturating_mul(u64::from(height)) > MAX_DECODE_PIXELS {
        return Err(AppError::too_large("Image dimensions are too large"));
    }
    if image.pixel_format().has_black() {
        return Err(AppError::unsupported("CMYK JPEG XL is not supported"));
    }
    // 열거형 색 공간끼리는 CMS 없이 변환된다. 화면은 프로파일을 해석하지
    // 않으므로 sRGB로 맞춰야 광색역·HDR·선형 이미지가 제 색으로 보인다.
    if let ColourEncoding::Enum(encoding) = &image.image_header().metadata.colour_encoding {
        let target = match encoding.colour_space {
            ColourSpace::Grey => Some(EnumColourEncoding::gray_srgb(RenderingIntent::Relative)),
            ColourSpace::Rgb => Some(EnumColourEncoding::srgb(RenderingIntent::Relative)),
            _ => None,
        };
        if let Some(target) = target.filter(|_| !encoding.is_srgb()) {
            image.request_color_encoding(target);
        }
    }
    if image.num_loaded_keyframes() == 0 {
        return Err(AppError::corrupt("JPEG XL image has no complete frame"));
    }
    let render = image.render_frame(0).map_err(decode_error)?;
    let mut stream = render.stream();
    let channels = stream.channels() as usize;
    let len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| AppError::corrupt("JPEG XL image dimensions overflow"))?;
    let mut bytes = vec![0u8; len];
    stream.write_to_buffer(&mut bytes);
    let invalid = || AppError::corrupt("JPEG XL decode produced invalid buffer");
    let decoded = match channels {
        1 => image::GrayImage::from_raw(width, height, bytes).map(image::DynamicImage::ImageLuma8),
        2 => image::GrayAlphaImage::from_raw(width, height, bytes)
            .map(image::DynamicImage::ImageLumaA8),
        3 => image::RgbImage::from_raw(width, height, bytes).map(image::DynamicImage::ImageRgb8),
        4 => image::RgbaImage::from_raw(width, height, bytes).map(image::DynamicImage::ImageRgba8),
        _ => None,
    }
    .ok_or_else(invalid)?;
    Ok((decoded, Some(image.rendered_icc())))
}

/// 썸네일·히스토그램용 RGB8 디코드. 투명은 흰 배경에 합성한다
/// (JPEG에 알파가 없어 `raster_sidecar`와 같은 규칙).
pub(crate) fn decode_rgb8(source: &Path) -> Result<Rgb8, AppError> {
    let (decoded, _icc) = decode(source)?;
    let (width, height) = (decoded.width(), decoded.height());
    let bytes = if decoded.color().has_alpha() {
        let rgba = decoded.into_rgba8();
        let mut bytes = Vec::with_capacity(width as usize * height as usize * 3);
        for px in rgba.as_raw().as_chunks::<4>().0 {
            let a = u32::from(px[3]);
            let blend = |c: u8| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8;
            bytes.extend_from_slice(&[blend(px[0]), blend(px[1]), blend(px[2])]);
        }
        bytes
    } else {
        decoded.into_rgb8().into_raw()
    };
    Ok(Rgb8 {
        width,
        height,
        bytes,
    })
}
