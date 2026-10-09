//! EXIF 패널용 부가 정보: RGB 히스토그램 + 파일 상세.
//!
//! - 히스토그램: `image` 크레이트로 디코드 가능한 포맷 + HEIC/HEIF/PSD(전용 디코더).
//!   최대 변 256px로 다운샘플 후 집계하므로 대용량 파일도 빠르게 처리한다.
//!   `image` 크레이트가 디코드 불가한 입력(SVG, AVIF 등)은 에러를 내고,
//!   프론트는 해당 섹션을 숨긴다.
//! - 파일 상세: 크기/치수/색상 모드/비트뎁스/생성·수정 시각/EXIF 해상도(DPI)/
//!   ICC 프로파일(JPEG APP2, PNG iCCP 스캔). EXIF가 없어도 명령은 성공한다.

use std::fs;
use std::io::BufReader;
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::app_error::{AppError, ErrorCode};
use crate::image::PaintStrategy;

#[derive(Serialize, Debug, PartialEq)]
pub struct Histogram {
    pub r: Vec<u32>,
    pub g: Vec<u32>,
    pub b: Vec<u32>,
    /// 집계에 실제 사용한 픽셀 수 (다운샘플 시 축소본 기준).
    pub sampled_pixels: u64,
}

/// 히스토그램 집계 전 다운샘플 상한 (정사각형 기준 한 변).
const HISTOGRAM_MAX_SIDE: u32 = 256;

/// 히스토그램/색상 판정용 RGB 디코드. HEIC/HEIF/AVIF/PSD는 전용 디코더를 쓴다.
fn decode_rgb8(path: &Path) -> Result<image::RgbImage, AppError> {
    if !path.is_file() {
        return Err(AppError::not_found("File not found"));
    }
    if let Some(decoder) = crate::transcode::decoder_for(path) {
        let rgb = crate::transcode::decode_rgb8(path, decoder)?;
        return image::RgbImage::from_raw(rgb.width, rgb.height, rgb.bytes)
            .ok_or_else(|| AppError::corrupt("Decode produced invalid buffer"));
    }
    if crate::image::get_mime_type(path).is_none() {
        return Err(AppError::unsupported("Unsupported image format"));
    }
    if crate::image::get_mime_type(path) == Some("image/svg+xml") {
        return crate::svg_raster::rasterize(path, HISTOGRAM_MAX_SIDE);
    }
    // JPEG는 집계 크기까지 DCT 단계에서 줄여 디코드한다. 어차피 256px로
    // 다운샘플하므로 풀해상도 디코드와 큰 버퍼를 건너뛴다.
    if let Some(scaled) = crate::thumbnail::decode_jpeg_scaled(path, HISTOGRAM_MAX_SIDE) {
        return Ok(scaled.into_rgb8());
    }
    let dyn_img =
        image::open(path).map_err(|e| AppError::image_error("Cannot compute histogram", e))?;
    Ok(dyn_img.to_rgb8())
}

pub fn compute_histogram(rgb: &image::RgbImage) -> Histogram {
    // 다운샘플은 `RgbImage`를 직접 받는 `thumbnail`을 쓴다. 예전에는
    // DynamicImage/RGBA 왕복에 풀해상도 clone까지 겹쳐 150MP에서 수백 MB를
    // 더 복사했다. 256px 이하 입력도 clone 없이 그대로 집계한다.
    let sampled = if rgb.width().max(rgb.height()) > HISTOGRAM_MAX_SIDE {
        Some(image::imageops::thumbnail(
            rgb,
            HISTOGRAM_MAX_SIDE,
            HISTOGRAM_MAX_SIDE,
        ))
    } else {
        None
    };
    let pixels = sampled.as_ref().unwrap_or(rgb);
    let mut r = vec![0u32; 256];
    let mut g = vec![0u32; 256];
    let mut b = vec![0u32; 256];
    for pixel in pixels.pixels() {
        r[pixel[0] as usize] += 1;
        g[pixel[1] as usize] += 1;
        b[pixel[2] as usize] += 1;
    }
    Histogram {
        r,
        g,
        b,
        sampled_pixels: u64::from(pixels.width()) * u64::from(pixels.height()),
    }
}

pub fn histogram_for_path(path: &Path) -> Result<Histogram, AppError> {
    Ok(compute_histogram(&decode_rgb8(path)?))
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum IccStatus {
    /// JPEG APP2 또는 PNG iCCP에서 프로파일을 찾음.
    Present,
    /// 검사 대상 포맷(jpg/png)이나 프로파일 없음.
    Absent,
    /// 검사하지 않는 포맷.
    Unchecked,
}

#[derive(Serialize, Debug)]
pub struct ImageDetails {
    pub file_path: String,
    pub file_size: u64,
    /// 렌더 바이트 기준 치수. SVG는 헤더 파싱으로 복원하며, 해석 불가분만 null.
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// "rgb" | "rgba" | "grayscale" | "grayscale-alpha" | "unknown".
    /// 디코드 표현 기준이다 (예: GIF는 RGBA로 보고될 수 있다).
    pub color_mode: String,
    pub bits_per_channel: Option<u8>,
    /// 유닉스 초. FS가 지원하지 않으면 null.
    pub created_unix: Option<i64>,
    pub modified_unix: Option<i64>,
    /// EXIF X/YResolution + ResolutionUnit 기준 DPI. EXIF가 없으면 null.
    pub dpi_x: Option<f32>,
    pub dpi_y: Option<f32>,
    pub icc_status: IccStatus,
    pub icc_name: Option<String>,
    pub icc_bytes: Option<u64>,
}

pub fn details_for_path(path: &Path) -> Result<ImageDetails, AppError> {
    if !path.is_file() {
        return Err(AppError::not_found("File not found"));
    }
    let mime = crate::image::resolve_mime(path)
        .ok_or_else(|| AppError::unsupported("Unsupported image format"))?;
    let meta = fs::metadata(path)
        .map_err(|e| AppError::io("Failed to read metadata", e, ErrorCode::Corrupt))?;
    if crate::image::is_archive_file(path) {
        // CBZ/ZIP 자체는 이미지가 아니므로 파일 수준 정보만 채운다.
        return Ok(ImageDetails {
            file_path: path.to_string_lossy().to_string(),
            file_size: meta.len(),
            width: None,
            height: None,
            color_mode: "unknown".to_string(),
            bits_per_channel: None,
            created_unix: unix_time(meta.created().ok()),
            modified_unix: unix_time(meta.modified().ok()),
            dpi_x: None,
            dpi_y: None,
            icc_status: IccStatus::Unchecked,
            icc_name: None,
            icc_bytes: None,
        });
    }
    // 치수는 렌더 경로 기준 (HEIC/PSD는 JPEG sidecar). JPEG/TIFF는 EXIF
    // orientation까지 반영해 표시 치수와 맞춘다.
    let paint_path = match crate::image::paint_strategy(mime) {
        PaintStrategy::Native => path.to_path_buf(),
        PaintStrategy::TranscodeJpeg => crate::transcode::ensure_paint(path, mime)?,
    };
    let (width, height) = crate::image::render_dimensions(mime, path, &paint_path);
    let (color_mode, bits_per_channel) = color_info(path, mime);
    let (dpi_x, dpi_y) = resolution_dpi(path);
    let (icc_status, icc_name, icc_bytes) = icc_info(path);
    Ok(ImageDetails {
        file_path: path.to_string_lossy().to_string(),
        file_size: meta.len(),
        width,
        height,
        color_mode,
        bits_per_channel,
        created_unix: unix_time(meta.created().ok()),
        modified_unix: unix_time(meta.modified().ok()),
        dpi_x,
        dpi_y,
        icc_status,
        icc_name,
        icc_bytes,
    })
}

fn unix_time(time: Option<std::time::SystemTime>) -> Option<i64> {
    time?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn color_info(path: &Path, mime: &str) -> (String, Option<u8>) {
    if mime == "image/heic" || mime == "image/heif" {
        return ("rgb".to_string(), Some(8));
    }
    if mime == crate::psd_sidecar::PSD_MIME {
        // 렌더 출력은 흰 배경에 합성된 JPEG sidecar이므로 rgb로 보고한다.
        return ("rgb".to_string(), Some(8));
    }
    // 색상 모드 한 줄을 위해 픽셀을 전부 디코드하지 않는다. `image::open`이
    // 반환하는 표현은 디코더의 `color_type()`과 1:1이라 헤더 파싱만으로 같다.
    let color_type = image::ImageReader::open(path)
        .ok()
        .and_then(|reader| reader.with_guessed_format().ok())
        .and_then(|reader| reader.into_decoder().ok())
        .map(|decoder| image::ImageDecoder::color_type(&decoder));
    let (mode, bits) = match color_type {
        Some(image::ColorType::L8) => ("grayscale", 8),
        Some(image::ColorType::La8) => ("grayscale-alpha", 8),
        Some(image::ColorType::Rgb8) => ("rgb", 8),
        Some(image::ColorType::Rgba8) => ("rgba", 8),
        Some(image::ColorType::L16) => ("grayscale", 16),
        Some(image::ColorType::La16) => ("grayscale-alpha", 16),
        Some(image::ColorType::Rgb16) => ("rgb", 16),
        Some(image::ColorType::Rgba16) => ("rgba", 16),
        Some(image::ColorType::Rgb32F) => ("rgb", 32),
        Some(image::ColorType::Rgba32F) => ("rgba", 32),
        _ => ("unknown", 0),
    };
    let bits = if mode == "unknown" { None } else { Some(bits) };
    (mode.to_string(), bits)
}

/// EXIF X/YResolution + ResolutionUnit을 DPI로. EXIF가 없으면 (None, None).
fn resolution_dpi(path: &Path) -> (Option<f32>, Option<f32>) {
    let read = || -> Option<(Option<f32>, Option<f32>)> {
        let file = fs::File::open(path).ok()?;
        let mut reader = BufReader::new(file);
        let exif = exif::Reader::new().read_from_container(&mut reader).ok()?;
        let number = |tag: exif::Tag| {
            let field = exif.get_field(tag, exif::In::PRIMARY)?;
            match &field.value {
                exif::Value::Rational(v) => v.first().map(|r| r.to_f32()),
                exif::Value::SRational(v) => v.first().map(|r| r.to_f32()),
                exif::Value::Short(v) => v.first().map(|n| f32::from(*n)),
                exif::Value::Long(v) => v.first().map(|n| *n as f32),
                exif::Value::Ascii(v) => v
                    .first()
                    .and_then(|bytes| std::str::from_utf8(bytes).ok())
                    .and_then(|s| s.trim_matches('\0').trim().parse::<f32>().ok()),
                _ => None,
            }
        };
        let x = number(exif::Tag::XResolution);
        let y = number(exif::Tag::YResolution);
        if x.is_none() && y.is_none() {
            return None;
        }
        Some(dpi_from_values(x, y, unit_factor(&exif)))
    };
    read().unwrap_or((None, None))
}

/// ResolutionUnit(2=inch 기본, 3=cm)을 DPI 환산 계수로.
fn unit_factor(exif: &exif::Exif) -> f32 {
    let unit = exif
        .get_field(exif::Tag::ResolutionUnit, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0));
    if unit == Some(3) {
        2.54
    } else {
        1.0
    }
}

fn dpi_from_values(x: Option<f32>, y: Option<f32>, unit_factor: f32) -> (Option<f32>, Option<f32>) {
    // EXIF XResolution은 ResolutionUnit당 픽셀 수다. inch(2)가 기준이고
    // cm(3)일 때는 inch당으로 환산해야 DPI가 되므로 곱한다.
    let factor = if unit_factor > 0.0 { unit_factor } else { 1.0 };
    let convert = |v: Option<f32>| {
        v.filter(|n| n.is_finite() && *n > 0.0)
            .map(|n| (n * factor * 10.0).round() / 10.0)
    };
    (convert(x), convert(y))
}

/// ICC 프로파일 검사. jpg/png만 스캔하고 나머지는 Unchecked.
fn icc_info(path: &Path) -> (IccStatus, Option<String>, Option<u64>) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    let found = match ext.as_str() {
        "jpg" | "jpeg" => icc_from_jpeg(path),
        "png" | "apng" => icc_from_png(path),
        _ => return (IccStatus::Unchecked, None, None),
    };
    match found {
        Some((name, bytes)) => (IccStatus::Present, name, Some(bytes)),
        None => (IccStatus::Absent, None, None),
    }
}

/// JPEG APP2("ICC_PROFILE") 세그먼트 스캔. SOS 이후는 보지 않는다.
/// 스캔 상한(1MiB) + 최대 세그먼트 길이(64KiB)만 읽어 파일 전체를
/// 메모리에 올리지 않는다.
fn icc_from_jpeg(path: &Path) -> Option<(Option<String>, u64)> {
    use std::io::Read as _;

    const SCAN_LIMIT: usize = 1_048_576;
    const MAX_SEGMENT: usize = 65_535;
    let file = fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take((SCAN_LIMIT + MAX_SEGMENT) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() < 4 || bytes[0..2] != [0xFF, 0xD8] {
        return None;
    }
    let mut pos = 2usize;
    let mut total: u64 = 0;
    let mut found = false;
    while pos + 4 <= bytes.len() && pos < SCAN_LIMIT {
        if bytes[pos] != 0xFF {
            pos += 1;
            continue;
        }
        let marker = bytes[pos + 1];
        // 연속 FF 패딩, 단독 마커는 길이 없이 건너뛴다.
        if marker == 0xFF || marker == 0x00 {
            pos += 1;
            continue;
        }
        if marker == 0xDA {
            break; // SOS: 이후는 압축 스트림이라 메타데이터 없음.
        }
        if marker == 0xD8 || marker == 0xD9 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            pos += 2;
            continue;
        }
        let len = u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]) as usize;
        let end = pos.checked_add(2)?.checked_add(len)?;
        if len < 2 || end > bytes.len() {
            break;
        }
        if marker == 0xE2 {
            let seg = &bytes[pos + 4..end];
            // "ICC_PROFILE\0"(12) + seq(1) + count(1) + 프로파일 데이터.
            if seg.starts_with(b"ICC_PROFILE\0") {
                found = true;
                total += (seg.len() as u64).saturating_sub(14);
            }
        }
        pos = end;
    }
    found.then_some((None, total))
}

/// PNG iCCP 청크 스캔. 프로파일 이름과 압축 데이터 길이를 반환.
/// 청크를 스트리밍으로 걸어 iCCP 데이터만 읽는다 (전체 파일 로드 없음).
fn icc_from_png(path: &Path) -> Option<(Option<String>, u64)> {
    use std::io::{Read as _, Seek as _, SeekFrom};

    const SIG: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    /// 실제 ICC 프로파일보다 훨씬 큰 상한. 넘는 길이 선언은 손상으로 본다.
    const MAX_ICC_CHUNK: u64 = 64 * 1024 * 1024;
    let mut file = fs::File::open(path).ok()?;
    let mut sig = [0u8; 8];
    file.read_exact(&mut sig).ok()?;
    if sig != *SIG {
        return None;
    }
    loop {
        let mut header = [0u8; 8];
        if file.read_exact(&mut header).is_err() {
            return None;
        }
        let len = u64::from(u32::from_be_bytes(header[..4].try_into().ok()?));
        let typ = &header[4..8];
        if typ == b"IEND" {
            return None;
        }
        if typ == b"iCCP" {
            if len > MAX_ICC_CHUNK {
                return None;
            }
            let mut data = vec![0u8; len as usize];
            file.read_exact(&mut data).ok()?;
            let nul = data.iter().position(|&b| b == 0)?;
            let name = String::from_utf8_lossy(&data[..nul]).into_owned();
            // 이름 NUL(1) + 압축 방식(1) 뒤가 압축 프로파일.
            let profile_len = len.saturating_sub(nul as u64 + 2);
            return Some((Some(name), profile_len));
        }
        // 청크 데이터 + CRC(4)를 건너뛴다.
        file.seek(SeekFrom::Current(len as i64 + 4)).ok()?;
    }
}
