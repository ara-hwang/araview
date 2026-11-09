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
        let mut encoder = jpeg_encoder::Encoder::new_file(path, 90).unwrap();
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend_from_slice(&exif_tiff(orientation));
        encoder.add_app_segment(1, app1).unwrap();
        encoder
            .encode(
                &pixels(u32::from(width), u32::from(height)),
                width,
                height,
                jpeg_encoder::ColorType::Rgb,
            )
            .unwrap();
    }
}
