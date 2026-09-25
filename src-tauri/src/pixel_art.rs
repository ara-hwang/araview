//! Lightweight pixel-art detection used for display-only interpolation hints.
//!
//! The detector is intentionally conservative. It combines a small quantized
//! palette, local flatness, repeated color runs, and periodic edge profiles.
//! These ideas are also used by the MIT-licensed `unfake.js` project (reference
//! revision `b2bee10c1c3b211a2532baca9088857b19480dca`); AraView implements
//! its own bounded version here and does not modify source images.

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::time::UNIX_EPOCH;

use image::imageops::FilterType;
use image::{DynamicImage, ImageReader};
use serde::Serialize;

const ANALYSIS_MAX_SIDE: u32 = 1024;
const ANALYSIS_MAX_PIXELS: u64 = 40_000_000;
const ANALYSIS_MAX_ALLOC: u64 = 256 * 1024 * 1024;
const MIN_SCALE: u32 = 2;
const MAX_SCALE: u32 = 32;
const MIN_PERIOD: usize = 2;
const MAX_PERIOD: usize = 128;
const MIN_RUNS: usize = 8;
const PIXEL_ART_SCORE: f32 = 0.72;
const CONTINUOUS_SCORE: f32 = 0.42;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelArtClassification {
    PixelArt,
    Continuous,
    Uncertain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelArtMethod {
    Runs,
    Edges,
    Hybrid,
    Unsupported,
}

#[derive(Clone, Debug, Serialize)]
pub struct PixelArtDetection {
    pub classification: PixelArtClassification,
    pub confidence: f32,
    pub pixel_scale: Option<u32>,
    pub method: PixelArtMethod,
}

impl PixelArtDetection {
    fn unsupported() -> Self {
        Self {
            classification: PixelArtClassification::Uncertain,
            confidence: 0.0,
            pixel_scale: None,
            method: PixelArtMethod::Unsupported,
        }
    }
}

const DETECTION_CACHE_ENTRIES: usize = 128;
type DetectionCache = Mutex<VecDeque<(String, PixelArtDetection)>>;
static DETECTION_CACHE: LazyLock<DetectionCache> =
    LazyLock::new(|| Mutex::new(VecDeque::with_capacity(DETECTION_CACHE_ENTRIES)));

fn detection_cache_key(path: &Path) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    Some(format!(
        "{}:{}:{}",
        path.to_string_lossy(),
        metadata.len(),
        modified
    ))
}

fn cached_detection(path: &Path) -> Option<PixelArtDetection> {
    let key = detection_cache_key(path)?;
    let mut cache = DETECTION_CACHE.lock().ok()?;
    let index = cache
        .iter()
        .position(|(cached_key, _)| cached_key == &key)?;
    let entry = cache.remove(index)?;
    let result = entry.1.clone();
    cache.push_back(entry);
    Some(result)
}

/// 생성된 sidecar의 분석 결과를 같은 프로세스의 후속 IPC가 재사용할 수 있다.
pub fn cache_detection(path: &Path, detection: &PixelArtDetection) {
    let Some(key) = detection_cache_key(path) else {
        return;
    };
    let Ok(mut cache) = DETECTION_CACHE.lock() else {
        return;
    };
    if let Some(index) = cache.iter().position(|(cached_key, _)| cached_key == &key) {
        cache.remove(index);
    }
    cache.push_back((key, detection.clone()));
    while cache.len() > DETECTION_CACHE_ENTRIES {
        cache.pop_front();
    }
}

#[derive(Clone, Copy, Debug)]
struct PaletteStats {
    score: f32,
}

#[derive(Clone, Copy, Debug)]
struct GridStats {
    score: f32,
    scale: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
struct EdgeStats {
    score: f32,
    scale: Option<u32>,
}

/// Analyze a local file without making detection a prerequisite for opening it.
///
/// Unsupported or very large inputs return `uncertain`, which the frontend
/// treats as a safe smooth-rendering fallback.
pub fn detect_file(path: &Path) -> PixelArtDetection {
    if let Some(cached) = cached_detection(path) {
        return cached;
    }
    let result = detect_file_uncached(path);
    cache_detection(path, &result);
    result
}

fn detect_file_uncached(path: &Path) -> PixelArtDetection {
    let Some((width, height)) = image::image_dimensions(path).ok() else {
        return PixelArtDetection::unsupported();
    };
    if width == 0 || height == 0 {
        return PixelArtDetection::unsupported();
    }
    if u64::from(width) * u64::from(height) > ANALYSIS_MAX_PIXELS {
        return PixelArtDetection::unsupported();
    }

    let Ok(reader) = ImageReader::open(path) else {
        return PixelArtDetection::unsupported();
    };
    let mut reader = reader;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(ANALYSIS_MAX_ALLOC);
    reader.limits(limits);
    let Ok(reader) = reader.with_guessed_format() else {
        return PixelArtDetection::unsupported();
    };
    let Ok(decoded) = reader.decode() else {
        return PixelArtDetection::unsupported();
    };

    detect_decoded_image(&decoded)
}

/// Analyze an already-decoded image. The scaled-sidecar path uses this to
/// choose a matching downscale filter without decoding the source twice.
pub fn detect_decoded_image(image: &DynamicImage) -> PixelArtDetection {
    let rgba = if image.width().max(image.height()) > ANALYSIS_MAX_SIDE {
        image
            .resize(ANALYSIS_MAX_SIDE, ANALYSIS_MAX_SIDE, FilterType::Nearest)
            .to_rgba8()
    } else {
        image.to_rgba8()
    };
    analyze_rgba(rgba.as_raw(), rgba.width(), rgba.height())
}

fn analyze_rgba(rgba: &[u8], width: u32, height: u32) -> PixelArtDetection {
    if width < 3 || height < 3 || rgba.len() < (width as usize) * (height as usize) * 4 {
        return PixelArtDetection::unsupported();
    }

    let opaque_pixels = count_opaque(rgba, width, height);
    if opaque_pixels < 16 {
        return PixelArtDetection::unsupported();
    }

    let palette = palette_stats(rgba, width, height);
    let flatness = flatness_score(rgba, width, height);
    let grid = grid_stats(rgba, width, height);
    let edge = edge_stats(rgba, width, height);
    let detail = strong_edge_ratio(rgba, width, height);

    // Palette and flatness identify native 1x pixel art. Grid/edge signals
    // identify an already-upscaled sprite. A completely flat image has no
    // evidence of artistic intent, so it remains uncertain rather than being
    // treated as pixel art.
    let score = 0.35 * palette.score + 0.25 * flatness + 0.25 * grid.score + 0.15 * edge.score;
    let periodic_score = grid.score.max(edge.score);
    let classification = if detail < 0.005 {
        if flatness < 0.98 {
            PixelArtClassification::Continuous
        } else {
            PixelArtClassification::Uncertain
        }
    } else if score >= PIXEL_ART_SCORE
        && (periodic_score >= 0.45 || (palette.score >= 0.72 && flatness >= 0.65))
    {
        PixelArtClassification::PixelArt
    } else if palette.score >= 0.70 && detail >= 0.20 && flatness <= 0.25 {
        // 1x checkerboards and other hard-edged patterns have no long runs.
        PixelArtClassification::PixelArt
    } else if palette.score >= 0.70 && periodic_score >= 0.72 && detail >= 0.04 {
        // A regular hard-edged pattern can have few equal-color neighbors.
        PixelArtClassification::PixelArt
    } else if score <= CONTINUOUS_SCORE || palette.score < 0.35 {
        PixelArtClassification::Continuous
    } else {
        PixelArtClassification::Uncertain
    };

    let confidence = match classification {
        PixelArtClassification::PixelArt => (0.65 + score * 0.35).clamp(0.0, 1.0),
        PixelArtClassification::Continuous => (0.65 + (1.0 - score) * 0.35).clamp(0.0, 1.0),
        PixelArtClassification::Uncertain => {
            (1.0 - (score - CONTINUOUS_SCORE).abs() / 0.58).clamp(0.0, 1.0)
        }
    };
    let pixel_scale = if classification == PixelArtClassification::PixelArt {
        if grid.score >= edge.score {
            grid.scale
        } else {
            edge.scale
        }
    } else {
        None
    };
    let method = if grid.score >= 0.45 && edge.score >= 0.45 {
        PixelArtMethod::Hybrid
    } else if grid.score >= edge.score && grid.score > 0.0 {
        PixelArtMethod::Runs
    } else if edge.score > 0.0 {
        PixelArtMethod::Edges
    } else {
        PixelArtMethod::Unsupported
    };

    PixelArtDetection {
        classification,
        confidence,
        pixel_scale,
        method,
    }
}

fn count_opaque(rgba: &[u8], width: u32, height: u32) -> usize {
    let mut count = 0;
    for y in 0..height {
        for x in 0..width {
            let index = pixel_index(x, y, width);
            if rgba[index + 3] >= 128 {
                count += 1;
            }
        }
    }
    count
}

fn palette_stats(rgba: &[u8], width: u32, height: u32) -> PaletteStats {
    let mut counts: HashMap<u32, usize> = HashMap::new();
    let mut total = 0usize;
    for y in 0..height {
        for x in 0..width {
            let index = pixel_index(x, y, width);
            if rgba[index + 3] < 128 {
                continue;
            }
            *counts.entry(quantized_color(rgba, index)).or_insert(0) += 1;
            total += 1;
        }
    }
    if total == 0 || counts.is_empty() {
        return PaletteStats { score: 0.0 };
    }

    let color_count = counts.len();
    let score = 1.0 - ((color_count.saturating_sub(4) as f32) / 252.0).clamp(0.0, 1.0);
    PaletteStats {
        score: score.clamp(0.0, 1.0),
    }
}

fn flatness_score(rgba: &[u8], width: u32, height: u32) -> f32 {
    let mut same = 0usize;
    let mut total = 0usize;

    for y in 0..height {
        for x in 1..width {
            let a = pixel_index(x - 1, y, width);
            let b = pixel_index(x, y, width);
            if rgba[a + 3] >= 128 && rgba[b + 3] >= 128 {
                total += 1;
                if same_opaque_color(rgba, a, b) {
                    same += 1;
                }
            }
        }
    }
    for y in 1..height {
        for x in 0..width {
            let a = pixel_index(x, y - 1, width);
            let b = pixel_index(x, y, width);
            if rgba[a + 3] >= 128 && rgba[b + 3] >= 128 {
                total += 1;
                if same_opaque_color(rgba, a, b) {
                    same += 1;
                }
            }
        }
    }

    if total == 0 {
        0.0
    } else {
        same as f32 / total as f32
    }
}

fn grid_stats(rgba: &[u8], width: u32, height: u32) -> GridStats {
    let mut horizontal = Vec::new();
    let mut vertical = Vec::new();
    collect_runs(rgba, width, height, true, &mut horizontal);
    collect_runs(rgba, width, height, false, &mut vertical);

    let h = run_scale_score(&horizontal);
    let v = run_scale_score(&vertical);
    let score = match (h.1, v.1) {
        (Some(_), Some(_)) => (h.0 + v.0) * 0.5,
        (Some(_), None) => h.0 * 0.8,
        (None, Some(_)) => v.0 * 0.8,
        (None, None) => 0.0,
    };
    let scale = reconcile_scales(h.1, v.1);
    GridStats {
        score: score.clamp(0.0, 1.0),
        scale,
    }
}

fn collect_runs(rgba: &[u8], width: u32, height: u32, horizontal: bool, output: &mut Vec<u32>) {
    if horizontal {
        for y in 0..height {
            let mut length = 1u32;
            for x in 1..width {
                let previous = pixel_index(x - 1, y, width);
                let current = pixel_index(x, y, width);
                if same_opaque_color(rgba, previous, current) {
                    length += 1;
                } else {
                    if length > 1 {
                        output.push(length);
                    }
                    length = 1;
                }
            }
            if length > 1 {
                output.push(length);
            }
        }
    } else {
        for x in 0..width {
            let mut length = 1u32;
            for y in 1..height {
                let previous = pixel_index(x, y - 1, width);
                let current = pixel_index(x, y, width);
                if same_opaque_color(rgba, previous, current) {
                    length += 1;
                } else {
                    if length > 1 {
                        output.push(length);
                    }
                    length = 1;
                }
            }
            if length > 1 {
                output.push(length);
            }
        }
    }
}

fn run_scale_score(runs: &[u32]) -> (f32, Option<u32>) {
    if runs.len() < MIN_RUNS {
        return (0.0, None);
    }

    let mut best_score = 0.0;
    let mut best_scale = None;
    let mut scores = Vec::new();
    for scale in MIN_SCALE..=MAX_SCALE {
        let matching = runs.iter().filter(|length| **length % scale == 0).count();
        let support = (matching as f32 / MIN_RUNS as f32).min(1.0);
        let ratio = matching as f32 / runs.len() as f32;
        let score = ratio * (0.55 + support * 0.45);
        scores.push((scale, score));
        if score > best_score + 0.0001 {
            best_score = score;
            best_scale = Some(scale);
        }
    }

    // Candidates 2 and 4 often tie for a 4x sprite. Prefer the larger scale
    // when it is effectively tied, which better describes the source grid.
    for (scale, score) in scores {
        if score >= best_score - 0.025 && best_scale.is_some_and(|current| scale > current) {
            best_scale = Some(scale);
        }
    }
    (best_score.clamp(0.0, 1.0), best_scale)
}

fn edge_stats(rgba: &[u8], width: u32, height: u32) -> EdgeStats {
    let gray = grayscale(rgba, width, height);
    let (horizontal, vertical) = edge_profiles(&gray, width, height);
    let h = peak_lag(&horizontal);
    let v = peak_lag(&vertical);
    let score = h.1.max(v.1).clamp(0.0, 1.0);
    EdgeStats {
        score,
        scale: reconcile_scales((h.0 > 1).then_some(h.0), (v.0 > 1).then_some(v.0)),
    }
}

fn grayscale(rgba: &[u8], width: u32, height: u32) -> Vec<f32> {
    let mut gray = vec![0.0; (width as usize) * (height as usize)];
    for y in 0..height {
        for x in 0..width {
            let index = pixel_index(x, y, width);
            gray[(y as usize) * (width as usize) + x as usize] = 0.299 * rgba[index] as f32
                + 0.587 * rgba[index + 1] as f32
                + 0.114 * rgba[index + 2] as f32;
        }
    }
    gray
}

fn edge_profiles(gray: &[f32], width: u32, height: u32) -> (Vec<f32>, Vec<f32>) {
    let mut horizontal = vec![0.0; width as usize];
    let mut vertical = vec![0.0; height as usize];
    for y in 0..height {
        for x in 0..width {
            let gx = -sample_gray(gray, width, height, x as i32 - 1, y as i32 - 1)
                + sample_gray(gray, width, height, x as i32 + 1, y as i32 - 1)
                - 2.0 * sample_gray(gray, width, height, x as i32 - 1, y as i32)
                + 2.0 * sample_gray(gray, width, height, x as i32 + 1, y as i32)
                - sample_gray(gray, width, height, x as i32 - 1, y as i32 + 1)
                + sample_gray(gray, width, height, x as i32 + 1, y as i32 + 1);
            let gy = -sample_gray(gray, width, height, x as i32 - 1, y as i32 - 1)
                - 2.0 * sample_gray(gray, width, height, x as i32, y as i32 - 1)
                - sample_gray(gray, width, height, x as i32 + 1, y as i32 - 1)
                + sample_gray(gray, width, height, x as i32 - 1, y as i32 + 1)
                + 2.0 * sample_gray(gray, width, height, x as i32, y as i32 + 1)
                + sample_gray(gray, width, height, x as i32 + 1, y as i32 + 1);
            horizontal[x as usize] += gx.abs();
            vertical[y as usize] += gy.abs();
        }
    }
    (horizontal, vertical)
}

fn sample_gray(gray: &[f32], width: u32, height: u32, x: i32, y: i32) -> f32 {
    let x = x.clamp(0, width.saturating_sub(1) as i32) as usize;
    let y = y.clamp(0, height.saturating_sub(1) as i32) as usize;
    gray[y * width as usize + x]
}

fn peak_lag(profile: &[f32]) -> (u32, f32) {
    if profile.len() < 8 {
        return (1, 0.0);
    }
    let mean = profile.iter().sum::<f32>() / profile.len() as f32;
    let centered: Vec<f32> = profile.iter().map(|value| value - mean).collect();
    let norm = centered
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .max(1e-9);
    let max_lag = (profile.len() / 8).clamp(MIN_PERIOD, MAX_PERIOD);
    let mut correlations = vec![0.0; max_lag + 1];
    let mut best_lag = 1usize;
    let mut best_value = 0.0;
    for (lag, correlation) in correlations
        .iter_mut()
        .enumerate()
        .skip(MIN_PERIOD)
        .take(max_lag + 1)
    {
        let value = centered
            .iter()
            .zip(centered.iter().skip(lag))
            .map(|(left, right)| left * right)
            .sum::<f32>()
            / norm;
        *correlation = value;
        if value > best_value {
            best_value = value;
            best_lag = lag;
        }
    }
    if best_value < 0.25 {
        return (1, 0.0);
    }

    let threshold = best_value * 0.6;
    for lag in MIN_PERIOD..=max_lag {
        if correlations[lag] >= threshold
            && correlations[lag] >= correlations[lag.saturating_sub(1)]
            && correlations
                .get(lag + 1)
                .is_none_or(|next| correlations[lag] >= *next)
        {
            return (lag as u32, best_value.clamp(0.0, 1.0));
        }
    }
    (best_lag as u32, best_value.clamp(0.0, 1.0))
}

fn strong_edge_ratio(rgba: &[u8], width: u32, height: u32) -> f32 {
    let mut strong = 0usize;
    let mut total = 0usize;
    for y in 0..height {
        for x in 1..width {
            let a = pixel_index(x - 1, y, width);
            let b = pixel_index(x, y, width);
            if rgba[a + 3] >= 128 && rgba[b + 3] >= 128 {
                total += 1;
                if color_distance(rgba, a, b) >= 32 {
                    strong += 1;
                }
            }
        }
    }
    for y in 1..height {
        for x in 0..width {
            let a = pixel_index(x, y - 1, width);
            let b = pixel_index(x, y, width);
            if rgba[a + 3] >= 128 && rgba[b + 3] >= 128 {
                total += 1;
                if color_distance(rgba, a, b) >= 32 {
                    strong += 1;
                }
            }
        }
    }
    if total == 0 {
        0.0
    } else {
        strong as f32 / total as f32
    }
}

fn reconcile_scales(horizontal: Option<u32>, vertical: Option<u32>) -> Option<u32> {
    match (horizontal, vertical) {
        (Some(a), Some(b)) if a.abs_diff(b) <= 2 => Some((a + b) / 2),
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) | (None, Some(a)) => Some(a),
        (None, None) => None,
    }
}

fn pixel_index(x: u32, y: u32, width: u32) -> usize {
    (y as usize * width as usize + x as usize) * 4
}

fn same_opaque_color(rgba: &[u8], a: usize, b: usize) -> bool {
    rgba[a + 3] >= 128 && rgba[b + 3] >= 128 && quantized_color(rgba, a) == quantized_color(rgba, b)
}

fn quantized_color(rgba: &[u8], index: usize) -> u32 {
    let r = (rgba[index] >> 3) as u32;
    let g = (rgba[index + 1] >> 3) as u32;
    let b = (rgba[index + 2] >> 3) as u32;
    (r << 10) | (g << 5) | b
}

fn color_distance(rgba: &[u8], a: usize, b: usize) -> u32 {
    rgba[a..a + 3]
        .iter()
        .zip(&rgba[b..b + 3])
        .map(|(left, right)| (*left as i32 - *right as i32).unsigned_abs())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn from_fn(width: u32, height: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            let [r, g, b, a] = f(x, y);
            Rgba([r, g, b, a])
        })
    }

    #[test]
    fn detects_upscaled_block_art() {
        let image = from_fn(64, 64, |x, y| {
            if (x / 4 + y / 4) % 2 == 0 {
                [220, 40, 40, 255]
            } else {
                [30, 70, 220, 255]
            }
        });
        let result = analyze_rgba(image.as_raw(), image.width(), image.height());
        assert_eq!(result.classification, PixelArtClassification::PixelArt);
        assert!(result.pixel_scale.is_some());
    }

    #[test]
    fn detects_hard_edge_pattern() {
        let image = from_fn(64, 64, |x, y| {
            if (x + y) % 2 == 0 {
                [250, 250, 250, 255]
            } else {
                [10, 10, 10, 255]
            }
        });
        let result = analyze_rgba(image.as_raw(), image.width(), image.height());
        assert_eq!(result.classification, PixelArtClassification::PixelArt);
    }

    #[test]
    fn rejects_smooth_gradient() {
        let image = from_fn(96, 96, |x, y| {
            let value = ((x + y) * 255 / 190) as u8;
            [value, value / 2, 255 - value, 255]
        });
        let result = analyze_rgba(image.as_raw(), image.width(), image.height());
        assert_eq!(result.classification, PixelArtClassification::Continuous);
    }

    #[test]
    fn solid_image_is_uncertain() {
        let image = RgbaImage::from_pixel(32, 32, Rgba([12, 34, 56, 255]));
        let result = analyze_rgba(image.as_raw(), image.width(), image.height());
        assert_eq!(result.classification, PixelArtClassification::Uncertain);
    }

    #[test]
    fn detects_a_decoded_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sprite.png");
        let image = from_fn(32, 32, |x, y| {
            if (x / 4 + y / 4) % 2 == 0 {
                [240, 30, 30, 255]
            } else {
                [20, 40, 220, 255]
            }
        });
        image.save(&path).unwrap();
        let result = detect_file(&path);
        assert_eq!(result.classification, PixelArtClassification::PixelArt);
    }

    #[test]
    fn reuses_cached_detection_for_the_same_file_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cached.png");
        RgbaImage::from_pixel(16, 16, Rgba([10, 20, 30, 255]))
            .save(&path)
            .unwrap();
        let cached = PixelArtDetection {
            classification: PixelArtClassification::PixelArt,
            confidence: 0.77,
            pixel_scale: Some(4),
            method: PixelArtMethod::Runs,
        };
        cache_detection(&path, &cached);
        let result = detect_file(&path);
        assert_eq!(result.classification, cached.classification);
        assert_eq!(result.confidence, cached.confidence);
        assert_eq!(result.pixel_scale, cached.pixel_scale);
    }

    #[test]
    fn rejects_antialiased_sample_fixture_when_present() {
        let path = Path::new("../samples/sample.gif");
        if !path.is_file() {
            return;
        }
        let result = detect_file(path);
        assert_ne!(result.classification, PixelArtClassification::PixelArt);
    }

    #[test]
    fn transparent_image_is_uncertain() {
        let image = RgbaImage::from_pixel(32, 32, Rgba([255, 0, 0, 0]));
        let result = analyze_rgba(image.as_raw(), image.width(), image.height());
        assert_eq!(result.classification, PixelArtClassification::Uncertain);
    }
}
