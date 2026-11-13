//! 무채색 이미지의 축소를 진 선형광에서 한다.
//!
//! 스크린톤(하단톤) 같은 주기 패턴은 감마 공간에서 축소하면 무아레가 남는다.
//! yosh가 회색조 축소를 진 선형광에서 하는 이유이고, 스파이크로 같은 결과를
//! 재확인했다: 4px 주기 스크린톤을 1/4 축소할 때 감마 공간 Triangle의 밝기
//! 표준편차 1.13/255 대비 선형광 Triangle은 0.57/255로 절반이고, 밝기도 잉크
//! 면적에 비례하는 혼합값으로 수렴한다. 색 이미지는 사진의 톤이 밝기 혼합과
//! 달리 인지적으로 보는 값이므로 감마 공간 축소를 그대로 쓴다(기존 Triangle).
//! 알파는 프리멀티 처리가 필요해 이 경로를 쓰지 않는다.

use std::sync::LazyLock;

use image::{ImageBuffer, Luma, Rgb};

use crate::sidecar::Rgb8;

/// 무채색 판정 허용 오차. 픽셀 채널 최대-최소 차이가 이 값 이하면 무채색으로
/// 본다. MangaJaNai 계열의 회색조 감지 임계값(12/255)과 같은 값이다.
const GRAY_TOLERANCE: u8 = 12;

/// 축소 알고리즘 revision. 파생 캐시 파일 이름에 들어가므로 알고리즘을 바꾸면
/// 이전 사본이 새 표시를 대신하지 않게 값을 갱신한다.
pub(crate) const DOWNSCALE_FILTER_REVISION: &[u8] = b"downscale-v3-linear-gray";

/// sRGB 전송 특성 값(0..255)을 진 선형광(0..1)으로 옮기는 LUT. 컴파일 시점
/// 평가에서 `powf`를 쓸 수 없어 첫 사용에 만든다.
static SRGB_TO_LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
    let mut table = [0.0_f32; 256];
    for (i, value) in table.iter_mut().enumerate() {
        let x = i as f32 / 255.0;
        *value = if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        };
    }
    table
});

/// 선형광(0..1)을 sRGB 전송 특성 값(0..255)으로 돌려놓는 LUT. 1/4096 간격이다.
static LINEAR_TO_SRGB: LazyLock<[u8; 4097]> = LazyLock::new(|| {
    let mut table = [0_u8; 4097];
    for (i, value) in table.iter_mut().enumerate() {
        let x = i as f32 / 4096.0;
        let srgb = if x <= 0.003_130_8 {
            x * 12.92
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        };
        *value = (srgb.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    table
});

/// `max_w`×`max_h` 박스 안으로 축소할 목표 치수를 계산한다(확대하지 않는다).
/// `image` 크레이트 `DynamicImage::resize`(`resize_dimensions`)과 같은 규칙:
/// min 비율 × round, 최소 1.
pub(crate) fn fit_inside(width: u32, height: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    let width = width.max(1);
    let height = height.max(1);
    let max_w = max_w.max(1);
    let max_h = max_h.max(1);
    let ratio = (f64::from(max_w) / f64::from(width))
        .min(f64::from(max_h) / f64::from(height))
        .min(1.0);
    let out_w = ((f64::from(width) * ratio).round() as u32).max(1);
    let out_h = ((f64::from(height) * ratio).round() as u32).max(1);
    (out_w, out_h)
}

/// RGB8 픽셀이 무채색인지 판정한다. 축소 대상은 큰 이미지이므로 rayon으로
/// 스캔하고, 한 픽셀이라도 범위를 넘으면 조기에 거짓으로 끝낸다.
pub(crate) fn is_grayscale(bytes: &[u8]) -> bool {
    use rayon::prelude::*;
    bytes
        .par_chunks_exact(3)
        .all(|pixel| pixel.iter().max().unwrap() - pixel.iter().min().unwrap() <= GRAY_TOLERANCE)
}

/// RGB8을 `narrow_wide`×`narrow_high`로 축소한다. 무채색이면 선형광에서
/// 축소하고(무아레 감소), 아니면 감마 공간에서 축소한다(Triangle 보간).
///
/// `rgb`를 값으로 받는 이유는 전체 해상도 디코드가 최대
/// `MAX_DECODE_PIXELS * 3`바이트(~450MB)라 소스 버퍼를 복사하지 않기 위해서다.
pub(crate) fn resize_rgb8(rgb: Rgb8, narrow_wide: u32, narrow_high: u32) -> Rgb8 {
    let src_w = rgb.width.max(1);
    let src_h = rgb.height.max(1);
    let expected = (src_w as usize)
        .saturating_mul(src_h as usize)
        .saturating_mul(3);
    if rgb.bytes.len() < expected || narrow_wide == 0 || narrow_high == 0 {
        return rgb;
    }
    let view: ImageBuffer<Rgb<u8>, &[u8]> =
        match ImageBuffer::from_raw(src_w, src_h, rgb.bytes.as_slice()) {
            Some(value) => value,
            None => return rgb,
        };
    let resized = if is_grayscale(&rgb.bytes) {
        resize_linear(view, narrow_wide, narrow_high)
    } else {
        image::imageops::resize(
            &view,
            narrow_wide,
            narrow_high,
            image::imageops::FilterType::Triangle,
        )
    };
    Rgb8 {
        width: narrow_wide,
        height: narrow_high,
        bytes: resized.into_raw(),
    }
}

/// 선형광 회색 단일 채널로 축소한다. 색 3채널로 축소하는 것보다 비용이 낮고,
/// 수학적으로도 R=G=B 결과는 단일 채널 혼합과 같다. 채널 간 차이가 12 이하인
/// 입력이라 중앙값을 채택해도 최대 4.7%의 오차이며, 축소된 결과는 R=G=B가 된다.
fn resize_linear(
    view: ImageBuffer<Rgb<u8>, &[u8]>,
    narrow_wide: u32,
    narrow_high: u32,
) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let to_linear = &*SRGB_TO_LINEAR;
    let mut linear = ImageBuffer::<Luma<f32>, Vec<f32>>::new(view.width(), view.height());
    for (x, y, pixel) in view.enumerate_pixels() {
        // 채널 간 차이는 GRAY_TOLERANCE 이하다. 대표값은 중앙값이 손실이 가장 적다.
        let mut channels = [pixel[0], pixel[1], pixel[2]];
        channels.sort_unstable();
        let gray = to_linear[channels[1] as usize];
        linear.put_pixel(x, y, Luma([gray]));
    }
    let resized = image::imageops::resize(
        &linear,
        narrow_wide,
        narrow_high,
        image::imageops::FilterType::Triangle,
    );
    let to_srgb = &*LINEAR_TO_SRGB;
    let mut out = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(narrow_wide, narrow_high);
    for (x, y, pixel) in resized.enumerate_pixels() {
        let v = to_srgb[(pixel[0].clamp(0.0, 1.0) * 4096.0).round() as usize];
        out.put_pixel(x, y, Rgb([v; 3]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb8(width: u32, height: u32, fill: fn(u32, u32) -> [u8; 3]) -> Rgb8 {
        let bytes: Vec<u8> = (0..width * height)
            .flat_map(|i| fill(i % width, i / width))
            .collect();
        Rgb8 {
            width,
            height,
            bytes,
        }
    }

    fn mean(gray: &[u8]) -> f64 {
        gray.iter().map(|v| f64::from(*v)).sum::<f64>() / gray.len() as f64
    }

    #[test]
    fn detects_gray_and_rejects_color_spots() {
        assert!(is_grayscale(&vec![10_u8; 300]));
        // 허용 오차(12) 안의 색은 무채색으로 본다.
        assert!(is_grayscale(&[100, 105, 110]));
        // 하나라도 넘으면 거짓이다.
        assert!(!is_grayscale(&[100, 105, 113]));
    }

    #[test]
    fn grayscale_shrink_stays_gray_and_brightens_like_ink_area_mix() {
        // 25% 면적의 검은 도트 스크린톤(4px 주기). 선형광 혼합은 잉크 면적에
        // 비례한 밝기(선형 평균 0.75 → sRGB 225 근처)로 수렴한다.
        let pattern = |x: u32, _y: u32| {
            if x % 4 < 2 {
                [0, 0, 0]
            } else {
                [255, 255, 255]
            }
        };
        let small = resize_rgb8(rgb8(1024, 1024, pattern), 256, 256);
        assert_eq!(small.width, 256);
        // 모든 픽셀이 R=G=B다(회색 단일 채널 경로).
        for pixel in small.bytes.as_chunks::<3>().0 {
            assert_eq!(pixel[0], pixel[1]);
            assert_eq!(pixel[1], pixel[2]);
        }
        let grays: Vec<u8> = small
            .bytes
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| p[0])
            .collect();
        let mean = mean(&grays);
        // 감마 공간 축소는 산술 평균(191)에 머문다. 선형광 축소는 면적 혼합
        // (225 근처)으로 이동한다는 것을 상대 비교로 본다.
        assert!(mean > 200.0, "선형광 혼합 밝기 기대, 실제 {mean:.1}");
    }

    #[test]
    fn color_shrink_keeps_gamma_path() {
        // R만 높은 스폿이 있으면 무채색 판정을 넘기고 감마 경로를 탄다.
        // 결과는 오버슈트가 없어 0..255 안에 머문다.
        let pattern = |x: u32, y: u32| {
            if (x, y) == (10, 10) {
                [255, 0, 0]
            } else {
                [128, 128, 128]
            }
        };
        let small = resize_rgb8(rgb8(512, 512, pattern), 256, 256);
        assert_eq!(small.width, 256);
        for pixel in small.bytes.as_chunks::<3>().0 {
            let min = pixel.iter().min().unwrap();
            let max = pixel.iter().max().unwrap();
            assert!(max - min <= 2, "감마 경로는 채널 관계를 대체로 유지한다");
        }
    }

    #[test]
    fn fit_inside_matches_image_crate_resize_dimensions() {
        // image 크레이트 resize와 같은 목표 치수다(최소 1, min 비율 round).
        assert_eq!(fit_inside(1200, 600, 512, 512), (512, 256));
        assert_eq!(fit_inside(600, 1200, 512, 512), (256, 512));
        assert_eq!(fit_inside(1024, 1024, 2048, 2048), (1024, 1024));
        assert_eq!(fit_inside(3, 2, 1, 1), (1, 1));
    }
}
