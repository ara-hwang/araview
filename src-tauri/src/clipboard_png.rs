//! 클립보드 복사용 PNG 파생 이미지(§11.3).
//!
//! Ctrl+C는 예전에 웹뷰 안에서 fetch → createImageBitmap → 풀사이즈 canvas →
//! canvas.toBlob(PNG)를 실행해 비트맵·캔버스·블롭(픽셀 데이터의 수 배)을
//! WebView 프로세스에 올렸다. 이제 백엔드가 웹뷰가 그리는 같은 바이트를
//! 디코드해 PNG로 인코딩하고, 프론트는 그 경로를 클립보드에 쓴다.
//!
//! 표시와의 정합(§18): JPEG EXIF orientation은 표시와 같은 기준으로 반영하고,
//! GIF/APNG/움직이는 WebP는 첫 프레임만 담는다. 알파는 유지한다. 픽셀 값은
//! 변환하지 않고, 원본의 ICC 프로파일을 PNG에 옮겨 실어 붙여 넣는 쪽이 표시와
//! 같은 색으로 해석하게 한다.

use std::fs;
use std::io::{BufWriter, Write as _};
use std::path::{Path, PathBuf};

use image::{ImageDecoder as _, ImageEncoder as _};

use serde::Serialize;

use crate::app_error::{AppError, ErrorCode};

/// 파생 이미지 캐시 아래 클립보드 PNG 전용 하위 디렉터리. 캐시 통계에서는
/// 기타(other) 범주로 분류된다([`crate::cache::category_for_path`]의 폴백).
const CLIPBOARD_SUBDIR: &str = "clipboard";
/// 상한(500MB). paint/·scaled/와 같은 규칙이며, 넘기면 오래된 것부터 제거한다.
const MAX_CLIPBOARD_BYTES: u64 = 500 * 1024 * 1024;
/// 정책이 바뀌면 이전 캐시를 재사용하지 않게 하는 식별자.
const FORMAT_REVISION: &[u8] = b"clipboard-png-v2";
/// SVG 래스터 상한(긴 변). 내재 치수가 이보다 크면 이 값으로 줄여 쓴다.
const MAX_SVG_SIDE: u32 = 8192;
/// 일반 래스터 디코드 상한. 백엔드가 풀사이즈를 디코드하므로 손상·악의적
/// 헤더가 메모리를 폭주시키지 못하게 막는다(raster_sidecar와 같은 기준).
const MAX_DECODE_PIXELS: u64 = 100_000_000;
const MAX_DECODE_ALLOC: u64 = 1024 * 1024 * 1024;

#[derive(Serialize, Debug)]
pub struct ClipboardPng {
    pub file_path: String,
}

/// 렌더 바이트(`file_path`)를 디코드해 PNG로 발행하고 경로를 돌려준다.
/// 같은 원본의 반복 복사는 캐시 히트로 끝난다.
pub fn export_clipboard_png(source: &Path) -> Result<ClipboardPng, AppError> {
    if !source.is_file() {
        return Err(AppError::not_found("File not found"));
    }
    let hash = crate::sidecar::file_identity_hash(source, FORMAT_REVISION)?;
    let dest = clipboard_dir()?.join(format!("{hash:016x}.png"));
    // 프론트가 곧 이 경로를 asset URL로 fetch하므로 발행 전후로 보호한다.
    // 디코드 중에 캐시 clear가 실행돼도 반환된 경로가 사라지지 않게 한다.
    crate::process_temp::mark_in_use(&dest);
    if dest.exists() {
        crate::process_temp::touch_cache_file(&dest);
        return Ok(ClipboardPng {
            file_path: dest.to_string_lossy().into_owned(),
        });
    }

    let lock_key = dest.to_string_lossy().into_owned();
    let dest = crate::sidecar::with_file_lock(&lock_key, "clipboard png lock", || {
        if dest.exists() {
            crate::process_temp::touch_cache_file(&dest);
            return Ok(dest.clone());
        }
        let decoded = decode_for_clipboard(source)?;
        let tmp = crate::sidecar::tmp_path_for(&dest);
        if let Err(e) = write_png(&tmp, decoded) {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
        crate::sidecar::publish_atomic(&tmp, &dest, "Failed to publish clipboard PNG")?;
        // Best effort: eviction failures must not fail the copy.
        crate::process_temp::note_published(&dest, MAX_CLIPBOARD_BYTES);
        Ok(dest.clone())
    })?;
    crate::process_temp::mark_in_use(&dest);
    Ok(ClipboardPng {
        file_path: dest.to_string_lossy().into_owned(),
    })
}

/// 디코드한 픽셀과, 그 픽셀 값을 해석할 원본 ICC 프로파일.
struct Decoded {
    image: image::DynamicImage,
    icc: Option<Vec<u8>>,
}

impl Decoded {
    fn untagged(image: image::DynamicImage) -> Self {
        Self { image, icc: None }
    }
}

/// PNG로 인코딩한다. 프로파일은 디코드 결과와 색 공간이 맞을 때만 싣는다.
/// CMYK JPEG처럼 디코더가 RGB로 바꿔 준 경우 원본 프로파일은 더는 맞지 않는다.
fn write_png(tmp: &Path, decoded: Decoded) -> Result<(), AppError> {
    let encode_error = |e: &dyn std::fmt::Display| {
        AppError::unknown(format!("Failed to encode clipboard PNG: {e}"))
    };
    let file = fs::File::create(tmp).map_err(|e| encode_error(&e))?;
    let mut writer = BufWriter::new(file);
    let mut encoder = image::codecs::png::PngEncoder::new(&mut writer);
    let color = decoded.image.color();
    if let Some(icc) = decoded.icc.filter(|icc| icc_matches_color(icc, color)) {
        // Best effort: 프로파일을 못 실어도 복사 자체는 계속한다.
        let _ = encoder.set_icc_profile(icc);
    }
    decoded
        .image
        .write_with_encoder(encoder)
        .map_err(|e| encode_error(&e))?;
    writer.flush().map_err(|e| encode_error(&e))
}

/// ICC 헤더의 데이터 색 공간(16..20바이트)이 디코드 결과와 같은 계열인지 본다.
fn icc_matches_color(icc: &[u8], color: image::ColorType) -> bool {
    let Some(space) = icc.get(16..20) else {
        return false;
    };
    if color.has_color() {
        space == b"RGB "
    } else {
        space == b"GRAY"
    }
}

/// 웹뷰가 그리는 렌더 바이트를 표시와 같은 결과로 디코드한다. 판별은 내용
/// 기준([`crate::image::resolve_mime`])이라 이름이 바뀐 파일도 올바르게 간다.
fn decode_for_clipboard(source: &Path) -> Result<Decoded, AppError> {
    match crate::image::resolve_mime(source) {
        // 원본 HEIF 계열. 실제로는 네이티브 렌더라 원본이 곧 렌더 바이트인 AVIF가
        // 온다(HEIC/HEIF의 렌더 경로는 JPEG sidecar다). libheif가 컨테이너 변환
        // (irot/imir)을 반영하므로 EXIF 회전은 적용하지 않는다.
        Some("image/avif" | "image/heic" | "image/heif") => {
            let (image, icc) = crate::heif::decode_primary_rgba(source)?;
            Ok(Decoded {
                image: image::DynamicImage::ImageRgba8(image),
                icc,
            })
        }
        Some("image/svg+xml") => Ok(Decoded::untagged(image::DynamicImage::ImageRgba8(
            crate::svg_raster::rasterize_rgba(source, MAX_SVG_SIDE)?,
        ))),
        // sidecar 디스패치 대상 원본이 곧장 오면(방어선) sidecar와 같은 디코더를
        // 쓴다. 실제 렌더 경로는 JPEG sidecar라 위 JPEG 분기로 간다.
        Some(mime)
            if mime == crate::psd_sidecar::PSD_MIME || crate::transcode::is_raster_mime(mime) =>
        {
            let decoder = crate::transcode::decoder_for(source)
                .ok_or_else(|| AppError::unsupported("Unsupported image format"))?;
            let rgb = crate::transcode::decode_rgb8(source, decoder)?;
            let img = image::RgbImage::from_raw(rgb.width, rgb.height, rgb.bytes)
                .ok_or_else(|| AppError::corrupt("Decoded image dimensions are invalid"))?;
            Ok(Decoded::untagged(image::DynamicImage::ImageRgb8(img)))
        }
        // JPEG(원본·sidecar), PNG/APNG, GIF, WebP, BMP, ICO. 확장자가 틀려도
        // 내용으로 판별하며, 애니메이션 포맷은 첫 프레임이다.
        mime => {
            let mut reader = image::ImageReader::open(source)
                .map_err(|e| AppError::io("Failed to open image", e, ErrorCode::Corrupt))?
                .with_guessed_format()
                .map_err(|e| AppError::io("Failed to guess image format", e, ErrorCode::Corrupt))?;
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(MAX_DECODE_ALLOC);
            reader.limits(limits);
            let mut decoder = reader
                .into_decoder()
                .map_err(|e| AppError::image_error("Failed to decode image", e))?;
            let (width, height) = decoder.dimensions();
            check_pixel_budget(width, height)?;
            let icc = decoder.icc_profile().ok().flatten();
            let image = image::DynamicImage::from_decoder(decoder)
                .map_err(|e| AppError::image_error("Failed to decode image", e))?;
            // 회전은 표시 치수와 같은 기준(JPEG)으로만 반영한다. PNG/WebP에 든
            // EXIF까지 돌리면 화면과 다른 방향으로 복사된다.
            let image = if mime.is_some_and(crate::image::is_exif_orientation_format) {
                crate::orientation::apply_to_image(
                    image,
                    crate::orientation::read_orientation(source),
                )
            } else {
                image
            };
            Ok(Decoded { image, icc })
        }
    }
}

fn check_pixel_budget(width: u32, height: u32) -> Result<(), AppError> {
    if u64::from(width).saturating_mul(u64::from(height)) > MAX_DECODE_PIXELS {
        return Err(AppError::too_large("Image dimensions are too large"));
    }
    Ok(())
}

fn clipboard_dir() -> Result<PathBuf, AppError> {
    let dir = crate::process_temp::process_temp_dir()?.join(CLIPBOARD_SUBDIR);
    fs::create_dir_all(&dir)
        .map_err(|e| AppError::io("Failed to create clipboard dir", e, ErrorCode::Unknown))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use super::*;

    #[test]
    fn png_source_round_trips_and_reuses_cache() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.png");
        image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(4, 2, |x, y| {
            image::Rgb([(x as u8) * 40, (y as u8) * 100, 7])
        }))
        .save(&source)
        .unwrap();

        let first = export_clipboard_png(&source).unwrap();
        let path = Path::new(&first.file_path);
        assert_eq!(
            path.parent().and_then(|p| p.file_name()),
            Some(std::ffi::OsStr::new("clipboard"))
        );
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("png"));

        let decoded = image::open(path).unwrap().to_rgb8();
        assert_eq!((decoded.width(), decoded.height()), (4, 2));
        assert_eq!(decoded.get_pixel(1, 1).0, [40, 100, 7]);

        // 같은 원본은 같은 캐시 파일로 수렴한다.
        let second = export_clipboard_png(&source).unwrap();
        assert_eq!(first.file_path, second.file_path);
    }

    #[test]
    fn alpha_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("logo.png");
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(4, 2, |x, _| {
            if x < 2 {
                image::Rgba([0, 0, 0, 0])
            } else {
                image::Rgba([10, 20, 30, 255])
            }
        }))
        .save(&source)
        .unwrap();

        let out = export_clipboard_png(&source).unwrap();
        let decoded = image::open(&out.file_path).unwrap().to_rgba8();
        assert_eq!(decoded.get_pixel(0, 0).0, [0, 0, 0, 0]);
        assert_eq!(decoded.get_pixel(3, 0).0, [10, 20, 30, 255]);
    }

    #[test]
    fn exif_orientation_is_applied_like_display() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("rotated.jpg");
        // 2x1(왼쪽 빨강, 오른쪽 파랑) + orientation 6(시계 90°) → 1x2, 위 빨강.
        crate::orientation::write_test_jpeg_with_orientation(&source, 2, 1, 6);

        let out = export_clipboard_png(&source).unwrap();
        let decoded = image::open(&out.file_path).unwrap().to_rgb8();
        assert_eq!((decoded.width(), decoded.height()), (1, 2));
        // JPEG 재양자화 오차를 허용해 채널 우세로 판정한다.
        let top = decoded.get_pixel(0, 0).0;
        let bottom = decoded.get_pixel(0, 1).0;
        assert!(top[0] > 200 && top[1] < 50 && top[2] < 50, "top {top:?}");
        assert!(
            bottom[2] > 200 && bottom[0] < 50 && bottom[1] < 50,
            "bottom {bottom:?}"
        );
    }

    #[test]
    fn gif_first_frame_is_copied() {
        use std::io::BufWriter;
        use std::time::Duration;

        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("anim.gif");
        let file = std::fs::File::create(&source).unwrap();
        let mut encoder = image::codecs::gif::GifEncoder::new(BufWriter::new(file));
        let delay = image::Delay::from_saturating_duration(Duration::from_millis(100));
        for color in [[255u8, 0, 0], [0, 0, 255]] {
            let frame = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(4, 4, |_, _| {
                image::Rgb(color)
            }))
            .into_rgba8();
            encoder
                .encode_frame(image::Frame::from_parts(frame, 0, 0, delay))
                .unwrap();
        }
        drop(encoder);

        let out = export_clipboard_png(&source).unwrap();
        let decoded = image::open(&out.file_path).unwrap().to_rgb8();
        assert_eq!((decoded.width(), decoded.height()), (4, 4));
        // createImageBitmap과 같이 첫 프레임(빨강)만 담는다.
        assert_eq!(decoded.get_pixel(0, 0).0, [255, 0, 0]);
        assert_eq!(decoded.get_pixel(3, 3).0, [255, 0, 0]);
    }

    #[test]
    fn svg_rasterizes_with_alpha() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("icon.svg");
        std::fs::write(
            &source,
            r##"<svg width="40" height="20" xmlns="http://www.w3.org/2000/svg"><rect width="20" height="20" fill="#ff0000"/></svg>"##,
        )
        .unwrap();

        let out = export_clipboard_png(&source).unwrap();
        let decoded = image::open(&out.file_path).unwrap().to_rgba8();
        assert_eq!((decoded.width(), decoded.height()), (40, 20));
        assert_eq!(decoded.get_pixel(10, 10).0, [255, 0, 0, 255]);
        // 그리지 않은 오른쪽은 흰 배경이 아니라 투명하다.
        assert_eq!(decoded.get_pixel(30, 10).0, [0, 0, 0, 0]);
    }

    /// Orientation 6만 가진 TIFF 블록(PNG eXIf 청크 내용).
    const EXIF_ORIENTATION_6: [u8; 26] = [
        0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x12, 0x01, 0x03, 0x00, 0x01,
        0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    fn fake_icc(space: &[u8; 4]) -> Vec<u8> {
        let mut icc = vec![0u8; 128];
        icc[16..20].copy_from_slice(space);
        icc
    }

    /// 2x1(왼쪽 빨강, 오른쪽 파랑) PNG를 메타데이터와 함께 쓴다.
    fn write_png_with_metadata(path: &Path, icc: Option<Vec<u8>>, exif: Option<Vec<u8>>) {
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                image::Rgb([255, 0, 0])
            } else {
                image::Rgb([0, 0, 255])
            }
        }));
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
        if let Some(icc) = icc {
            encoder.set_icc_profile(icc).unwrap();
        }
        if let Some(exif) = exif {
            encoder.set_exif_metadata(exif).unwrap();
        }
        img.write_with_encoder(encoder).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn icc_of(path: &str) -> Option<Vec<u8>> {
        let file = std::fs::File::open(path).unwrap();
        image::codecs::png::PngDecoder::new(BufReader::new(file))
            .unwrap()
            .icc_profile()
            .unwrap()
    }

    #[test]
    fn icc_profile_is_carried_over() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("wide.png");
        let icc = fake_icc(b"RGB ");
        write_png_with_metadata(&source, Some(icc.clone()), None);

        let out = export_clipboard_png(&source).unwrap();
        assert_eq!(icc_of(&out.file_path), Some(icc));
    }

    #[test]
    fn mismatched_icc_profile_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("cmyk-profile.png");
        write_png_with_metadata(&source, Some(fake_icc(b"CMYK")), None);

        let out = export_clipboard_png(&source).unwrap();
        assert_eq!(icc_of(&out.file_path), None);
    }

    #[test]
    fn png_exif_orientation_is_ignored_like_display() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("tagged.png");
        write_png_with_metadata(&source, None, Some(EXIF_ORIENTATION_6.to_vec()));
        // 픽스처가 실제로 회전 태그를 갖고 있어야 검증이 의미 있다.
        assert_eq!(crate::orientation::read_orientation(&source), 6);

        let out = export_clipboard_png(&source).unwrap();
        let decoded = image::open(&out.file_path).unwrap().to_rgb8();
        assert_eq!((decoded.width(), decoded.height()), (2, 1));
        assert_eq!(decoded.get_pixel(0, 0).0, [255, 0, 0]);
    }

    /// 단색 4x4 프레임들로 움직이는 WebP(VP8X + ANIM + ANMF)를 조립한다.
    fn write_animated_webp(path: &Path, colors: &[[u8; 3]]) {
        fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut out = tag.to_vec();
            out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            out.extend_from_slice(payload);
            if payload.len() % 2 == 1 {
                out.push(0);
            }
            out
        }
        const SIDE_MINUS_ONE: [u8; 3] = [3, 0, 0];

        let mut vp8x = vec![0x02, 0, 0, 0]; // 애니메이션 플래그
        vp8x.extend_from_slice(&SIDE_MINUS_ONE);
        vp8x.extend_from_slice(&SIDE_MINUS_ONE);
        let mut body = b"WEBP".to_vec();
        body.extend(chunk(b"VP8X", &vp8x));
        body.extend(chunk(b"ANIM", &[0, 0, 0, 0, 0, 0]));
        for color in colors {
            let frame = image::RgbImage::from_fn(4, 4, |_, _| image::Rgb(*color));
            let mut still = Vec::new();
            image::codecs::webp::WebPEncoder::new_lossless(&mut still)
                .encode(frame.as_raw(), 4, 4, image::ExtendedColorType::Rgb8)
                .unwrap();
            let mut anmf = vec![0u8; 6]; // 프레임 x, y
            anmf.extend_from_slice(&SIDE_MINUS_ONE);
            anmf.extend_from_slice(&SIDE_MINUS_ONE);
            anmf.extend_from_slice(&[100, 0, 0, 0]); // 지속 시간 + 플래그
            anmf.extend_from_slice(&still[12..]); // RIFF 헤더 뒤의 VP8L 청크
            body.extend(chunk(b"ANMF", &anmf));
        }
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
        bytes.extend(body);
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn animated_webp_first_frame_is_copied() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("anim.webp");
        write_animated_webp(&source, &[[255, 0, 0], [0, 0, 255]]);

        let out = export_clipboard_png(&source).unwrap();
        let decoded = image::open(&out.file_path).unwrap().to_rgb8();
        assert_eq!((decoded.width(), decoded.height()), (4, 4));
        // 프레임 합성의 반올림 오차를 허용해 채널 우세로 판정한다.
        for (x, y) in [(0, 0), (3, 3)] {
            let px = decoded.get_pixel(x, y).0;
            assert!(px[0] > 250 && px[1] < 5 && px[2] < 5, "pixel {px:?}");
        }
    }

    #[test]
    fn apng_first_frame_is_copied() {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples/sample.apng");
        assert_eq!(crate::image::resolve_mime(&sample), Some("image/apng"));

        let out = export_clipboard_png(&sample).unwrap();
        let decoded = image::open(&out.file_path).unwrap().to_rgba8();
        // 샘플의 첫 프레임은 기본 이미지(IDAT)다.
        let first = image::open(&sample).unwrap().to_rgba8();
        assert_eq!((decoded.width(), decoded.height()), (800, 600));
        assert_eq!(decoded.as_raw(), first.as_raw());
    }

    #[test]
    fn avif_decodes_to_rgba_via_heif() {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples/sample.avif");

        let out = export_clipboard_png(&sample).unwrap();
        let decoded = image::open(&out.file_path).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (800, 600));
        assert!(decoded.color().has_alpha());
    }

    #[test]
    fn oversized_dimensions_are_rejected() {
        assert!(check_pixel_budget(10_000, 10_000).is_ok());
        let err = check_pixel_budget(20_000, 20_000).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::TooLarge);
    }

    #[test]
    fn jpeg_icc_profile_is_carried_over() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("tagged.jpg");
        let icc = fake_icc(b"RGB ");
        let img = image::RgbImage::from_fn(8, 8, |_, _| image::Rgb([120, 80, 40]));
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 90);
        encoder.set_icc_profile(icc.clone()).unwrap();
        encoder
            .encode_image(&image::DynamicImage::ImageRgb8(img))
            .unwrap();
        std::fs::write(&source, bytes).unwrap();

        let out = export_clipboard_png(&source).unwrap();
        assert_eq!(icc_of(&out.file_path), Some(icc));
    }

    #[test]
    fn missing_file_is_not_found() {
        let err = export_clipboard_png(Path::new("Z:\\no-such-clipboard\\a.png")).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::NotFound);
    }

    #[test]
    fn undecodable_source_is_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("broken.png");
        std::fs::write(&source, b"not really an image").unwrap();
        let err = export_clipboard_png(&source).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::Corrupt);
    }

    #[test]
    fn different_sources_get_different_cache_files() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.png");
        let b = dir.path().join("b.png");
        image::DynamicImage::ImageRgb8(image::RgbImage::new(2, 2))
            .save(&a)
            .unwrap();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(6, 6))
            .save(&b)
            .unwrap();

        let pa = export_clipboard_png(&a).unwrap();
        let pb = export_clipboard_png(&b).unwrap();
        assert_ne!(pa.file_path, pb.file_path);
    }
}
