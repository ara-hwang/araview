//! SVG 루트 태그의 width/height/viewBox에서 표시 픽셀 치수를 구하는 경량 파서.

use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

/// SVG `<svg>` 루트 태그의 width/height/viewBox에서 픽셀 치수를 구한다.
/// 의존성 없이 선두 64KB만 읽는다. `%`/폰트 단위 등 해석 불가분은 None.
pub fn svg_dimensions(path: &Path) -> Option<(u32, u32)> {
    use std::io::Read as _;

    const HEAD_LIMIT: u64 = 64 * 1024;
    let file = std::fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    file.take(HEAD_LIMIT).read_to_end(&mut buf).ok()?;
    if buf.is_empty() {
        return None;
    }
    let head = String::from_utf8_lossy(&buf).to_ascii_lowercase();
    let start = head.find("<svg")?;
    let after = &head[start..];
    let end = after.find('>')?;
    let tag = &after[..end];
    let (width_raw, height_raw, viewbox_raw) = svg_root_attrs(tag);
    let width = width_raw.as_deref().and_then(svg_length_px);
    let height = height_raw.as_deref().and_then(svg_length_px);
    let viewbox = viewbox_raw.as_deref().and_then(parse_viewbox);
    match (width, height, viewbox) {
        (Some(w), Some(h), _) => finite_pixels(w, h),
        (Some(w), None, Some((vb_w, vb_h))) => finite_pixels(w, w * vb_h / vb_w),
        (None, Some(h), Some((vb_w, vb_h))) => finite_pixels(h * vb_w / vb_h, h),
        (None, None, Some((vb_w, vb_h))) => finite_pixels(vb_w, vb_h),
        _ => None,
    }
}

/// `<svg ...>` 태그에서 width/height/viewBox 원시값을 뽑는다.
/// `stroke-width` 같은 합성 이름과 겹치지 않게 토큰 단위로 비교한다.
fn svg_root_attrs(tag: &str) -> (Option<String>, Option<String>, Option<String>) {
    let bytes = tag.as_bytes();
    let mut i = 0;
    // "<svg" 건너뛰기
    if tag.starts_with("<svg") {
        i = 4;
    }
    let (mut width, mut height, mut viewbox) = (None, None, None);
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b'>' {
            break;
        }
        let name_start = i;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric()
                || bytes[i] == b'-'
                || bytes[i] == b'_'
                || bytes[i] == b':'
                || bytes[i] == b'.')
        {
            i += 1;
        }
        if name_start == i {
            i += 1;
            continue;
        }
        let name = &tag[name_start..i];
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let value: String;
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            i += 1;
            let value_start = i;
            while i < bytes.len() && bytes[i] != quote {
                i += 1;
            }
            value = tag[value_start..i].to_string();
            if i < bytes.len() {
                i += 1;
            }
        } else {
            let value_start = i;
            while i < bytes.len()
                && !bytes[i].is_ascii_whitespace()
                && bytes[i] != b'>'
                && bytes[i] != b'/'
            {
                i += 1;
            }
            value = tag[value_start..i].to_string();
        }
        match name {
            "width" => width = Some(value),
            "height" => height = Some(value),
            "viewbox" => viewbox = Some(value),
            _ => {}
        }
    }
    (width, height, viewbox)
}

/// CSS 길이 → px. `%`/폰트·뷰포트 단위는 해석 불가라 None.
fn svg_length_px(raw: &str) -> Option<f32> {
    let s = raw.trim().to_ascii_lowercase();
    if s.is_empty() || s == "auto" {
        return None;
    }
    if s.ends_with('%') {
        return None;
    }
    const ABSOLUTE: &[(&str, f32)] = &[
        ("px", 1.0),
        ("pt", 96.0 / 72.0),
        ("pc", 16.0),
        ("mm", 96.0 / 25.4),
        ("cm", 96.0 / 2.54),
        ("in", 96.0),
        ("q", 96.0 / 25.4 / 4.0),
    ];
    let mut number = s.as_str();
    let mut factor = 1.0;
    for (suffix, scale) in ABSOLUTE {
        if let Some(prefix) = s.strip_suffix(suffix) {
            number = prefix;
            factor = *scale;
            break;
        }
    }
    // 숫자로 끝나지 않으면 폰트·뷰포트 등 미지 단위로 버린다.
    let value: f32 = number.trim().parse().ok()?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    let px = value * factor;
    if !px.is_finite() || px <= 0.0 {
        return None;
    }
    Some(px)
}

fn parse_viewbox(raw: &str) -> Option<(f32, f32)> {
    let nums: Vec<f32> = raw
        .replace(',', " ")
        .split_whitespace()
        .filter_map(|p| p.parse::<f32>().ok())
        .collect();
    if nums.len() != 4 {
        return None;
    }
    let (w, h) = (nums[2], nums[3]);
    if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
        return None;
    }
    Some((w, h))
}

fn finite_pixels(w: f32, h: f32) -> Option<(u32, u32)> {
    const MAX_SIDE: f32 = 1_000_000.0;
    if !w.is_finite() || !h.is_finite() {
        return None;
    }
    if w <= 0.0 || h <= 0.0 || w > MAX_SIDE || h > MAX_SIDE {
        return None;
    }
    let (w, h) = (w.round() as u32, h.round() as u32);
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

/// 테스트용 SVG 파일을 쓴다. `image` 모듈의 SVG 로드 테스트도 함께 쓴다.
#[cfg(test)]
pub(crate) fn write_svg(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).unwrap();
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_dimensions_explicit_width_height() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_svg(
            dir.path(),
            "icon.svg",
            r#"<svg width="800" height="600" xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        assert_eq!(svg_dimensions(&path), Some((800, 600)));
    }

    #[test]
    fn svg_dimensions_viewbox_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_svg(
            dir.path(),
            "icon.svg",
            r#"<svg viewBox="0 0 400 300" xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        assert_eq!(svg_dimensions(&path), Some((400, 300)));
    }

    #[test]
    fn svg_dimensions_width_plus_viewbox_ratio() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_svg(
            dir.path(),
            "icon.svg",
            r#"<svg width="200" viewBox="0 0 400 300" xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        assert_eq!(svg_dimensions(&path), Some((200, 150)));
    }

    #[test]
    fn svg_dimensions_percent_falls_back_to_viewbox() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_svg(
            dir.path(),
            "icon.svg",
            r#"<svg width="100%" height="100%" viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        assert_eq!(svg_dimensions(&path), Some((64, 64)));
    }

    #[test]
    fn svg_dimensions_absolute_units_convert_to_px() {
        let dir = tempfile::tempdir().unwrap();
        // 1in x 72pt = 96px x 96px
        let path = write_svg(
            dir.path(),
            "icon.svg",
            r#"<svg width="1in" height="72pt" xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        assert_eq!(svg_dimensions(&path), Some((96, 96)));
    }

    #[test]
    fn svg_dimensions_unresolvable_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        // 크기 정보 없음
        let bare = write_svg(
            dir.path(),
            "bare.svg",
            r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        assert_eq!(svg_dimensions(&bare), None);
        // 폰트 단위는 해석 불가
        let em = write_svg(
            dir.path(),
            "em.svg",
            r#"<svg width="10em" height="10em" xmlns="http://www.w3.org/2000/svg"></svg>"#,
        );
        assert_eq!(svg_dimensions(&em), None);
        // SVG가 아님
        let not_svg = write_svg(dir.path(), "icon.svg", r#"<html></html>"#);
        assert_eq!(svg_dimensions(&not_svg), None);
        // stroke-width의 width와 겹치지 않아야 함
        let stroke = write_svg(
            dir.path(),
            "stroke.svg",
            r#"<svg viewBox="0 0 20 10" xmlns="http://www.w3.org/2000/svg"><path stroke-width="5" d="M0 0h20"/></svg>"#,
        );
        assert_eq!(svg_dimensions(&stroke), Some((20, 10)));
    }
}
