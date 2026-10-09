//! Lightweight pixel-art detection used for display-only interpolation hints.
//!
//! The detector is intentionally conservative. It combines a small quantized
//! palette, local flatness, repeated color runs, and periodic edge profiles.
//! These ideas are also used by the MIT-licensed `unfake.js` project (reference
//! revision `b2bee10c1c3b211a2532baca9088857b19480dca`); AraView implements
//! its own bounded version here and does not modify source images.

use std::collections::VecDeque;
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
    /// 알파 128 이상인 픽셀 수.
    opaque: usize,
}

/// 가로/세로 이웃 픽셀 쌍을 한 번 순회해 얻는 신호들.
#[derive(Debug)]
struct NeighborStats {
    /// 둘 다 불투명한 이웃 쌍 중 양자화 색이 같은 비율.
    flatness: f32,
    /// 둘 다 불투명한 이웃 쌍 중 RGB 거리 32 이상인 비율.
    detail: f32,
    /// 같은 불투명 색이 2픽셀 이상 이어진 가로/세로 런 길이.
    horizontal_runs: Vec<u32>,
    vertical_runs: Vec<u32>,
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

    // 세 분석은 같은 픽셀을 읽기만 하고 서로 독립이라 동시에 돌린다. 각 분석
    // 내부의 누적 순서는 그대로라 결과는 순차 실행과 비트 단위로 같다.
    let (palette, (neighbors, edge)) = rayon::join(
        || palette_stats(rgba, width, height),
        || {
            rayon::join(
                || neighbor_stats(rgba, width, height),
                || edge_stats(rgba, width, height),
            )
        },
    );
    if palette.opaque < 16 {
        return PixelArtDetection::unsupported();
    }

    let flatness = neighbors.flatness;
    let detail = neighbors.detail;
    let grid = grid_stats(&neighbors.horizontal_runs, &neighbors.vertical_runs);

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

fn palette_stats(rgba: &[u8], width: u32, height: u32) -> PaletteStats {
    // 양자화 색은 15비트라 해시 대신 존재 표시 배열로 서로 다른 색 수를 센다.
    let mut seen = vec![false; 1 << 15];
    let mut color_count = 0usize;
    let mut total = 0usize;
    for y in 0..height {
        for x in 0..width {
            let index = pixel_index(x, y, width);
            if rgba[index + 3] < 128 {
                continue;
            }
            let slot = &mut seen[quantized_color(rgba, index) as usize];
            if !*slot {
                *slot = true;
                color_count += 1;
            }
            total += 1;
        }
    }
    if total == 0 || color_count == 0 {
        return PaletteStats {
            score: 0.0,
            opaque: total,
        };
    }

    let score = 1.0 - ((color_count.saturating_sub(4) as f32) / 252.0).clamp(0.0, 1.0);
    PaletteStats {
        score: score.clamp(0.0, 1.0),
        opaque: total,
    }
}

/// 평탄도, 강한 경계 비율, 가로/세로 런을 이웃 쌍 1회 순회로 함께 모은다.
/// 세로 런은 열마다 진행 중인 길이를 들고 행 단위로 내려간다.
fn neighbor_stats(rgba: &[u8], width: u32, height: u32) -> NeighborStats {
    let row_stride = width as usize * 4;
    let mut same = 0usize;
    let mut strong = 0usize;
    let mut total = 0usize;
    let mut horizontal_runs = Vec::new();
    let mut vertical_runs = Vec::new();
    let mut column_lengths = vec![1u32; width as usize];

    // 둘 다 불투명한 쌍만 통계에 넣고, 같은 불투명 색인지(런 연장 여부)를 돌려준다.
    let mut compare = |a: usize, b: usize| -> bool {
        if rgba[a + 3] < 128 || rgba[b + 3] < 128 {
            return false;
        }
        total += 1;
        if color_distance(rgba, a, b) >= 32 {
            strong += 1;
        }
        let same_color = quantized_color(rgba, a) == quantized_color(rgba, b);
        if same_color {
            same += 1;
        }
        same_color
    };

    for y in 0..height {
        let mut row_length = 1u32;
        for x in 0..width {
            let current = pixel_index(x, y, width);
            if x > 0 {
                if compare(current - 4, current) {
                    row_length += 1;
                } else {
                    if row_length > 1 {
                        horizontal_runs.push(row_length);
                    }
                    row_length = 1;
                }
            }
            if y > 0 {
                let column = &mut column_lengths[x as usize];
                if compare(current - row_stride, current) {
                    *column += 1;
                } else {
                    if *column > 1 {
                        vertical_runs.push(*column);
                    }
                    *column = 1;
                }
            }
        }
        if row_length > 1 {
            horizontal_runs.push(row_length);
        }
    }
    vertical_runs.extend(column_lengths.into_iter().filter(|length| *length > 1));

    let ratio = |count: usize| {
        if total == 0 {
            0.0
        } else {
            count as f32 / total as f32
        }
    };
    NeighborStats {
        flatness: ratio(same),
        detail: ratio(strong),
        horizontal_runs,
        vertical_runs,
    }
}

fn grid_stats(horizontal: &[u32], vertical: &[u32]) -> GridStats {
    let h = run_scale_score(horizontal);
    let v = run_scale_score(vertical);
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

fn run_scale_score(runs: &[u32]) -> (f32, Option<u32>) {
    if runs.len() < MIN_RUNS {
        return (0.0, None);
    }

    // 길이별 개수를 한 번 세 두면 후보 배율마다 런 전체를 다시 훑지 않고
    // 배수 위치만 더하면 된다.
    let max_length = runs.iter().copied().max().unwrap_or(0) as usize;
    let mut histogram = vec![0usize; max_length + 1];
    for &length in runs {
        histogram[length as usize] += 1;
    }

    let mut best_score = 0.0;
    let mut best_scale = None;
    let mut scores = Vec::new();
    for scale in MIN_SCALE..=MAX_SCALE {
        let matching: usize = histogram
            .iter()
            .skip(scale as usize)
            .step_by(scale as usize)
            .sum();
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
    use rayon::prelude::*;

    let w = width as usize;
    let mut gray = vec![0.0; w * (height as usize)];
    // 픽셀마다 독립이라 행 단위로 나눠 채운다.
    gray.par_chunks_mut(w)
        .zip(rgba.par_chunks(w * 4))
        .for_each(|(gray_row, rgba_row)| {
            for (value, pixel) in gray_row.iter_mut().zip(rgba_row.as_chunks::<4>().0) {
                *value =
                    0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32;
            }
        });
    gray
}

fn edge_profiles(gray: &[f32], width: u32, height: u32) -> (Vec<f32>, Vec<f32>) {
    let w = width as usize;
    let h = height as usize;
    let mut horizontal = vec![0.0; w];
    let mut vertical = vec![0.0; h];
    // Sobel 3x3. 이미지 밖은 가장자리 픽셀을 반복한다(좌표 clamp). 이웃 열
    // 인덱스를 미리 구해 픽셀마다 clamp를 반복하지 않는다. 항 순서는 유지한다.
    let prev_col: Vec<usize> = (0..w).map(|x| x.saturating_sub(1)).collect();
    let next_col: Vec<usize> = (0..w).map(|x| (x + 1).min(w - 1)).collect();
    for y in 0..h {
        let above = &gray[y.saturating_sub(1) * w..][..w];
        let row = &gray[y * w..][..w];
        let below = &gray[(y + 1).min(h - 1) * w..][..w];
        for x in 0..w {
            let (l, r) = (prev_col[x], next_col[x]);
            let gx = -above[l] + above[r] - 2.0 * row[l] + 2.0 * row[r] - below[l] + below[r];
            let gy = -above[l] - 2.0 * above[x] - above[r] + below[l] + 2.0 * below[x] + below[r];
            horizontal[x] += gx.abs();
            vertical[y] += gy.abs();
        }
    }
    (horizontal, vertical)
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
