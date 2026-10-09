//! SVG 루트 태그의 width/height/viewBox에서 표시 픽셀 치수를 구하는 경량 파서.

use std::path::Path;

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
