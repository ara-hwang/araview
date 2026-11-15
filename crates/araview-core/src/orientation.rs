//! EXIF Orientation 처리.
//!
//! WebView2는 `<img>` 렌더 시 EXIF orientation을 자동 적용하므로 화면 표시는
//! 이미 upright다. 반면 `image` 크레이트를 쓰는 경로(썸네일, 저장, 치수 계산,
//! 파일 정보)는 회전을 모르기 때문에, 여기서 명시적으로 적용해 표시와 맞춘다.
//!
//! 적용 범위는 JPEG/TIFF다. HEIC/HEIF는 libheif sidecar가 컨테이너 변환
//! (irot/imir)을 반영하고, sidecar JPEG에는 EXIF가 없어 이 파이프라인을 타지
//! 않는다. WebP/PNG의 orientation은 실사용 빈도가 낮아 다루지 않는다.

use std::io::BufReader;
use std::path::Path;

use image::metadata::Orientation;

/// EXIF Orientation 값을 읽는다. 태그가 없거나 비정상이면 1(정상)을 돌려준다.
pub fn read_orientation(path: &Path) -> u8 {
    let Ok(file) = std::fs::File::open(path) else {
        return 1;
    };
    let mut reader = BufReader::new(file);
    let Ok(exif) = exif::Reader::new().read_from_container(&mut reader) else {
        return 1;
    };
    match exif
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|field| field.value.get_uint(0))
    {
        Some(value) if (1..=8).contains(&value) => value as u8,
        _ => 1,
    }
}

/// 표시 경로와 같은 기준으로 EXIF Orientation을 픽셀에 반영할 원본인지(JPEG/TIFF).
/// 내용으로 판별한다(`resolve_mime`). 썸네일·축소 sidecar처럼 표시를 대신하는
/// 사본이 이 기준과 다르면 PNG/WebP에 든 Orientation 태그 때문에 사본만 돌아간다.
pub(crate) fn applies_to(source: &Path) -> bool {
    crate::image::resolve_mime(source).is_some_and(crate::image::is_exif_orientation_format)
}

/// 썸네일·축소 sidecar 캐시 키에 더하는 구분값. 예전 빌드는 PNG/WebP의
/// Orientation도 반영한 사본을 남겼으므로, JPEG/TIFF가 아닌 원본은 다른 키를 써
/// 그 사본을 다시 쓰지 않는다. 캐시 hit마다 원본을 읽지 않도록 확장자로 판단한다
/// (JPEG/TIFF 확장자의 사본은 반영 기준이 그대로라 기존 캐시를 계속 쓴다).
pub(crate) fn cache_scope_tag(source: &Path) -> &'static [u8] {
    if crate::image::get_mime_type(source).is_some_and(crate::image::is_exif_orientation_format) {
        b""
    } else {
        b"orientation-jpeg-tiff-only"
    }
}

/// 5..=8은 90도 회전 계열이라 가로/세로가 뒤바뀐다.
pub fn swaps_axes(orientation: u8) -> bool {
    (5..=8).contains(&orientation)
}

/// orientation을 반영해 픽셀을 돌린다. 1이면 그대로 돌려준다.
pub fn apply_to_image(mut img: image::DynamicImage, orientation: u8) -> image::DynamicImage {
    if orientation == 1 {
        return img;
    }
    if let Some(parsed) = Orientation::from_exif(orientation) {
        img.apply_orientation(parsed);
    }
    img
}

/// 테스트용: EXIF Orientation 태그가 든 PNG(`eXIf` 청크)와 JPEG(APP1)를 만든다.
#[cfg(test)]
pub(crate) mod test_images {
    use std::path::Path;

    /// Orientation 하나만 담은 빅엔디언 TIFF 블록.
    fn exif_tiff(orientation: u16) -> Vec<u8> {
        let mut tiff = b"MM\0\x2a\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01".to_vec();
        tiff.extend_from_slice(&orientation.to_be_bytes());
        tiff.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
        tiff
    }

    fn pixels(width: u32, height: u32) -> Vec<u8> {
        (0..width * height)
            .flat_map(|i| [(i % 251) as u8, 90, 160])
            .collect()
    }

    pub(crate) fn write_png(path: &Path, width: u32, height: u32, orientation: u16) {
        let file = std::fs::File::create(path).unwrap();
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_chunk(png::chunk::ChunkType(*b"eXIf"), &exif_tiff(orientation))
            .unwrap();
        writer.write_image_data(&pixels(width, height)).unwrap();
    }

    pub(crate) fn write_jpeg(path: &Path, width: u16, height: u16, orientation: u16) {
        write_jpeg_with_pixels(
            path,
            width,
            height,
            orientation,
            &pixels(u32::from(width), u32::from(height)),
        );
    }

    /// 왼쪽 절반 빨강, 오른쪽 절반 파랑인 JPEG. 회전 방향을 픽셀 값으로 확인한다.
    pub(crate) fn write_jpeg_halves(path: &Path, width: u16, height: u16, orientation: u16) {
        let mut rgb = Vec::with_capacity(usize::from(width) * usize::from(height) * 3);
        for x in 0..width {
            for _ in 0..height {
                rgb.extend_from_slice(if x < width / 2 {
                    &[255, 0, 0]
                } else {
                    &[0, 0, 255]
                });
            }
        }
        write_jpeg_with_pixels(path, width, height, orientation, &rgb);
    }

    fn write_jpeg_with_pixels(path: &Path, width: u16, height: u16, orientation: u16, rgb: &[u8]) {
        let mut encoder = jpeg_encoder::Encoder::new_file(path, 90).unwrap();
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend_from_slice(&exif_tiff(orientation));
        encoder.add_app_segment(1, app1).unwrap();
        encoder
            .encode(rgb, width, height, jpeg_encoder::ColorType::Rgb)
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_orientation_tag_from_jpeg() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rotated.jpg");
        test_images::write_jpeg(&path, 2, 1, 6);
        assert_eq!(read_orientation(&path), 6);
    }

    #[test]
    fn missing_or_broken_exif_defaults_to_one() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("plain.png");
        image::RgbImage::new(2, 2).save(&png).unwrap();
        assert_eq!(read_orientation(&png), 1);
        assert_eq!(read_orientation(&dir.path().join("missing.jpg")), 1);

        let broken = dir.path().join("broken.jpg");
        std::fs::write(&broken, b"not an image").unwrap();
        assert_eq!(read_orientation(&broken), 1);
    }

    #[test]
    fn reads_orientation_from_archive_tar_bytes() {
        // 아카이브 엔트리 추출물은 확장자가 없을 수 있어 파일명이 아니라 내용으로 판별한다.
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("entry.jpg");
        test_images::write_jpeg(&source, 2, 1, 8);
        let bytes = std::fs::read(&source).unwrap();
        let extracted = dir.path().join("extracted-no-ext");
        std::fs::write(&extracted, bytes).unwrap();
        assert_eq!(read_orientation(&extracted), 8);
    }

    #[test]
    fn swaps_axes_only_for_rotated_orientations() {
        for orientation in 1..=8u8 {
            assert_eq!(swaps_axes(orientation), (5..=8).contains(&orientation));
        }
    }

    #[test]
    fn apply_rotates_pixels_clockwise() {
        // 2x1: R | B → orientation 6(시계 90°) → 1x2: R 위, B 아래
        let img = image::DynamicImage::ImageRgb8(
            image::RgbImage::from_raw(2, 1, vec![255, 0, 0, 0, 0, 255]).unwrap(),
        );
        let out = apply_to_image(img, 6).to_rgb8();
        assert_eq!((out.width(), out.height()), (1, 2));
        assert_eq!(out.get_pixel(0, 0), &image::Rgb([255, 0, 0]));
        assert_eq!(out.get_pixel(0, 1), &image::Rgb([0, 0, 255]));
    }

    #[test]
    fn apply_keeps_pixels_for_orientation_one() {
        let img = image::DynamicImage::ImageRgb8(
            image::RgbImage::from_raw(2, 1, vec![1, 2, 3, 4, 5, 6]).unwrap(),
        );
        let out = apply_to_image(img, 1).to_rgb8();
        assert_eq!((out.width(), out.height()), (2, 1));
        assert_eq!(out.get_pixel(0, 0), &image::Rgb([1, 2, 3]));
    }
}
