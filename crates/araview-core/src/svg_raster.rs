//! SVG 래스터화. `image` 크레이트는 SVG를 디코드하지 못하므로 썸네일과
//! 히스토그램처럼 픽셀이 필요한 경로에서만 resvg를 쓴다. 표시는 WebView가 맡는다.

use std::io::Read;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{fontdb, ImageHrefResolver, Options, Tree};

use crate::app_error::AppError;

/// 비정상적으로 큰 SVG 입력으로 인한 메모리/연산 폭주를 막는다.
const MAX_SVG_BYTES: u64 = 32 * 1024 * 1024;

fn system_fonts() -> Arc<fontdb::Database> {
    static FONTS: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

/// SVG를 읽어 파스 트리를 만든다. 입력 크기 상한을 적용한다.
fn parse_tree(path: &Path) -> Result<Tree, AppError> {
    let file = std::fs::File::open(path).map_err(|e| {
        AppError::io(
            "Failed to read SVG",
            e,
            crate::app_error::ErrorCode::Unknown,
        )
    })?;
    // 크기는 실제 읽기 경로에서 강제한다. 메타데이터 검사 후 파일이 커지는
    // TOCTOU를 피하려고 `take`로 상한을 건다.
    let mut data = Vec::new();
    file.take(MAX_SVG_BYTES + 1)
        .read_to_end(&mut data)
        .map_err(|e| {
            AppError::io(
                "Failed to read SVG",
                e,
                crate::app_error::ErrorCode::Unknown,
            )
        })?;
    if data.len() as u64 > MAX_SVG_BYTES {
        return Err(AppError::too_large("SVG file is too large"));
    }
    let options = Options {
        fontdb: system_fonts(),
        // 외부 href는 로컬 파일을 읽지 않는다. 악성 SVG가 임의 경로의 파일을
        // 통째로 메모리에 올리거나 다른 로컬 SVG를 렌더링하지 못하게 한다.
        // data URI는 입력 크기 상한 안에서 계속 허용한다.
        image_href_resolver: ImageHrefResolver {
            resolve_data: ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(|_, _| None),
        },
        ..Options::default()
    };
    Tree::from_data(&data, &options)
        .map_err(|e| AppError::corrupt(format!("Failed to parse SVG: {e}")))
}

/// 긴 변이 `max_side`가 되도록 렌더링한다. 투명 영역은 흰 배경으로 합성한다.
pub fn rasterize(path: &Path, max_side: u32) -> Result<image::RgbImage, AppError> {
    let tree = parse_tree(path)?;
    let size = tree.size();
    let (sw, sh) = (size.width(), size.height());
    if !(sw > 0.0 && sh > 0.0) {
        return Err(AppError::corrupt("SVG has no drawable size"));
    }
    let scale = max_side.max(1) as f32 / sw.max(sh);
    let width = ((sw * scale).round() as u32).max(1);
    let height = ((sh * scale).round() as u32).max(1);
    let mut pixmap = Pixmap::new(width, height)
        .ok_or_else(|| AppError::corrupt("SVG render size is invalid"))?;
    resvg::render(
        &tree,
        Transform::from_scale(width as f32 / sw, height as f32 / sh),
        &mut pixmap.as_mut(),
    );
    // tiny-skia는 premultiplied RGBA. 흰 배경 위로 합성한다.
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for px in pixmap.pixels() {
        let inv = 255 - u32::from(px.alpha());
        rgb.push((u32::from(px.red()) + inv).min(255) as u8);
        rgb.push((u32::from(px.green()) + inv).min(255) as u8);
        rgb.push((u32::from(px.blue()) + inv).min(255) as u8);
    }
    image::RgbImage::from_raw(width, height, rgb)
        .ok_or_else(|| AppError::corrupt("SVG render produced invalid buffer"))
}

/// 내재 치수로 렌더링하고 알파를 유지한다. 긴 변이 `max_side`를 넘으면 그 값으로
/// 줄인다. 프론트가 createImageBitmap으로 내재 치수 래스터를 얻던 클립보드 복사
/// 경로와 맞춘 것으로, 투명 영역을 흰 배경으로 합성하지 않는다.
pub fn rasterize_rgba(path: &Path, max_side: u32) -> Result<image::RgbaImage, AppError> {
    let tree = parse_tree(path)?;
    let size = tree.size();
    let (sw, sh) = (size.width(), size.height());
    if !(sw > 0.0 && sh > 0.0) {
        return Err(AppError::corrupt("SVG has no drawable size"));
    }
    let scale = if sw.max(sh) > max_side as f32 {
        max_side as f32 / sw.max(sh)
    } else {
        1.0
    };
    let width = ((sw * scale).round() as u32).max(1);
    let height = ((sh * scale).round() as u32).max(1);
    let mut pixmap = Pixmap::new(width, height)
        .ok_or_else(|| AppError::corrupt("SVG render size is invalid"))?;
    resvg::render(
        &tree,
        Transform::from_scale(width as f32 / sw, height as f32 / sh),
        &mut pixmap.as_mut(),
    );
    // tiny-skia는 premultiplied RGBA. 곱셈을 풀어 알파를 그대로 보존한다.
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for px in pixmap.pixels() {
        let color = px.demultiply();
        rgba.push(color.red());
        rgba.push(color.green());
        rgba.push(color.blue());
        rgba.push(color.alpha());
    }
    image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| AppError::corrupt("SVG render produced invalid buffer"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn external_svg_href_is_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside.svg");
        write(
            &outside,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" fill="#ff0000"/></svg>"##,
        );
        let page = dir.path().join("page.svg");
        let href = outside.display().to_string();
        write(
            &page,
            &format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><image href="{href}" width="64" height="64"/></svg>"#
            ),
        );

        let image = rasterize(&page, 64).unwrap();
        assert_eq!(
            image.get_pixel(32, 32).0,
            [255, 255, 255],
            "a local file referenced by href must not be rendered"
        );
    }

    #[test]
    fn missing_external_href_does_not_break_rendering() {
        let dir = tempfile::tempdir().unwrap();
        let page = dir.path().join("page.svg");
        write(
            &page,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="#00ff00"/><image href="missing.png" width="32" height="32"/></svg>"##,
        );

        let image = rasterize(&page, 32).unwrap();
        assert_eq!(image.get_pixel(16, 16).0, [0, 255, 0]);
    }
}
