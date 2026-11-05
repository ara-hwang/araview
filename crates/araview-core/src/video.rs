//! 동영상 프로브와 포스터(첫 프레임 정지 이미지) 파이프라인 (M1-B).
//!
//! 디먹스·디코드는 rsmpeg(FFmpeg 8.x, MIT)이다. `link_system_ffmpeg`가
//! `PKG_CONFIG_PATH`의 FFmpeg를 쓰고, 없으면 `link_vcpkg_ffmpeg`가 vcpkg
//! 제공 FFmpeg로 폴백한다. WTFPL인 ffmpeg-next 계열은 쓰지 않는다.

use std::ffi::CString;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::app_error::AppError;
use crate::image::VIDEO_MIMES;
use crate::sidecar::Rgb8;

/// 포스터 JPEG 화질. 썸네일(80)보다 표시용에 가깝게 둔다.
const POSTER_JPEG_QUALITY: u8 = 85;
/// 포스터 캐시 파일명 접두. `paint/`에 `{prefix}{hash}.jpg`로 둔다.
const POSTER_PREFIX: &str = "video-";
/// 포스터 한 변 상한. 표시용(최대 4K)과 썸네일(최대 1024) 사이에서 호출자가 정한다.
const POSTER_SIDE_MIN: u32 = 32;
const POSTER_SIDE_MAX: u32 = 4096;
/// 첫 프레임을 찾기까지 읽는 패킷 상한. 넘기면 손상으로 본다.
const MAX_FIRST_FRAME_PACKETS: usize = 4000;
/// 디코드 허용 픽셀 수. 표시 경로(`display.rs`)와 같은 값이다.
const MAX_DECODE_PIXELS: u64 = 100_000_000;
/// 포스터 캐시 키에 섞는 형식 버전. 포스터 픽셀이 바뀌는 수정(회전·SAR 반영 등)마다
/// 올려 예전 포스터를 다시 쓰지 않게 한다.
const POSTER_KEY_VERSION: u8 = 2;

/// 표시 기하: 회전 메타데이터(DISPLAYMATRIX)와 픽셀 종횡비(SAR).
/// 디코드 프레임은 저장된 그대로라, 폰 세로 영상은 누워 있고 아나모픽 영상은 찌그러져
/// 있다. 포스터·썸네일·재생 프레임과 치수가 모두 이 값을 같은 방식으로 적용한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Geometry {
    /// 시계 방향 회전(0, 90, 180, 270).
    pub rotation: u32,
    /// 픽셀 종횡비(가로:세로). 모르면 1:1이다.
    pub sar: (i32, i32),
}

impl Geometry {
    /// 회전 전 표시 크기. SAR은 FFmpeg처럼 가로를 늘이거나 줄여 반영한다.
    pub(crate) fn unrotated(&self, width: u32, height: u32) -> (u32, u32) {
        let (num, den) = self.sar;
        if num <= 0 || den <= 0 || num == den {
            return (width, height);
        }
        let display = (f64::from(width) * f64::from(num) / f64::from(den)).round();
        ((display as u32).max(1), height)
    }

    /// 화면에 보이는 크기(SAR과 회전을 모두 반영).
    pub(crate) fn display(&self, width: u32, height: u32) -> (u32, u32) {
        let (w, h) = self.unrotated(width, height);
        if self.rotation % 180 == 90 {
            (h, w)
        } else {
            (w, h)
        }
    }
}

/// 스트림의 표시 기하를 읽는다.
pub(crate) fn stream_geometry(
    ictx: &rsmpeg::avformat::AVFormatContextInput,
    stream_index: usize,
) -> Geometry {
    use rsmpeg::ffi;
    let stream = &ictx.streams()[stream_index];
    // SAFETY: 열린 입력 컨텍스트와 그 안의 스트림 포인터다. 프레임은 넘기지 않는다(null).
    let sar = unsafe {
        ffi::av_guess_sample_aspect_ratio(
            ictx.as_ptr() as *mut _,
            stream.as_ptr() as *mut _,
            std::ptr::null_mut(),
        )
    };
    let sar = if sar.num > 0 && sar.den > 0 {
        (sar.num, sar.den)
    } else {
        (1, 1)
    };
    let par = stream.codecpar();
    // SAFETY: 코덱 파라미터의 coded_side_data 배열과 그 길이를 그대로 넘긴다.
    let side = unsafe {
        ffi::av_packet_side_data_get(
            par.coded_side_data,
            par.nb_coded_side_data,
            ffi::AV_PKT_DATA_DISPLAYMATRIX,
        )
    };
    let mut rotation = 0;
    // SAFETY: null이 아니면 FFmpeg가 소유한 유효한 side data 항목이다. 크기를 확인한 뒤
    // 3x3 int32 행렬로 읽는다.
    unsafe {
        if !side.is_null() && !(*side).data.is_null() && (*side).size >= 9 * 4 {
            let theta = ffi::av_display_rotation_get((*side).data as *const i32);
            rotation = clockwise_rotation(theta);
        }
    }
    Geometry { rotation, sar }
}

/// `av_display_rotation_get`(반시계 방향 각도)을 시계 방향 90도 단위로 바꾼다.
/// fftools의 `get_rotation`과 같은 규칙이다. 90도 단위가 아닌 각도는 가장 가까운 단위로 맞춘다.
fn clockwise_rotation(ccw_degrees: f64) -> u32 {
    if !ccw_degrees.is_finite() {
        return 0;
    }
    let theta = (-ccw_degrees).rem_euclid(360.0);
    (((theta / 90.0).round() as u32) % 4) * 90
}

/// 픽셀 버퍼를 시계 방향으로 돌린다(`bpp`는 픽셀당 바이트). 90/270이면 가로세로가 바뀐다.
/// 큰 프레임에서 캐시를 덜 흐트러뜨리도록 타일 단위로 옮긴다.
pub(crate) fn rotate_pixels(
    src: &[u8],
    width: u32,
    height: u32,
    bpp: usize,
    rotation: u32,
) -> (Vec<u8>, u32, u32) {
    let (w, h) = (width as usize, height as usize);
    if rotation.is_multiple_of(360) || w == 0 || h == 0 {
        return (src.to_vec(), width, height);
    }
    let mut dst = vec![0u8; src.len()];
    let (out_w, out_h) = if rotation % 180 == 90 { (h, w) } else { (w, h) };
    if bpp == 4 {
        // 재생 프레임(BGRA)은 픽셀을 u32 하나로 옮긴다. 바이트 단위 복사보다 몇 배 빠르다.
        // SAFETY: u8 버퍼를 u32로 보는 것은 정렬만 맞으면 안전하고, `align_to`가 정렬이
        // 맞는 가운데 부분만 돌려준다. 앞뒤가 남으면(정렬 안 됨) 아래 일반 경로로 간다.
        let (src_head, src_px, src_tail) = unsafe { src.align_to::<u32>() };
        // SAFETY: 위와 같다. `dst`는 0으로 채운 같은 길이의 버퍼다.
        let (dst_head, dst_px, dst_tail) = unsafe { dst.align_to_mut::<u32>() };
        if src_head.is_empty()
            && src_tail.is_empty()
            && dst_head.is_empty()
            && dst_tail.is_empty()
            && src_px.len() == w * h
        {
            rotate_tiles(src_px, dst_px, w, h, out_w, rotation);
            return (dst, out_w as u32, out_h as u32);
        }
    }
    const TILE: usize = 64;
    for ty in (0..h).step_by(TILE) {
        for tx in (0..w).step_by(TILE) {
            for y in ty..(ty + TILE).min(h) {
                for x in tx..(tx + TILE).min(w) {
                    let (dx, dy) = match rotation % 360 {
                        90 => (h - 1 - y, x),
                        180 => (w - 1 - x, h - 1 - y),
                        _ => (y, w - 1 - x),
                    };
                    let s = (y * w + x) * bpp;
                    let d = (dy * out_w + dx) * bpp;
                    dst[d..d + bpp].copy_from_slice(&src[s..s + bpp]);
                }
            }
        }
    }
    (dst, out_w as u32, out_h as u32)
}

/// 픽셀 하나가 원소 하나인 버퍼를 타일 단위로 돌린다.
fn rotate_tiles<T: Copy>(
    src: &[T],
    dst: &mut [T],
    w: usize,
    h: usize,
    out_w: usize,
    rotation: u32,
) {
    const TILE: usize = 32;
    for ty in (0..h).step_by(TILE) {
        for tx in (0..w).step_by(TILE) {
            for y in ty..(ty + TILE).min(h) {
                let row = &src[y * w..(y + 1) * w];
                let x_end = (tx + TILE).min(w);
                for (x, &pixel) in row.iter().enumerate().take(x_end).skip(tx) {
                    let (dx, dy) = match rotation % 360 {
                        90 => (h - 1 - y, x),
                        180 => (w - 1 - x, h - 1 - y),
                        _ => (y, w - 1 - x),
                    };
                    dst[dy * out_w + dx] = pixel;
                }
            }
        }
    }
}

/// 동영상 프로브 결과. 재생(M2) 전에는 메타데이터와 포스터만 쓴다.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct VideoProbe {
    pub mime_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<u64>,
    pub fps: Option<f64>,
    pub codec: Option<String>,
    pub has_audio: bool,
    /// 실제 프레임을 뽑을 수 있는지. rsmpeg 링크가 있으면 항상 true다.
    pub decodable: bool,
}

/// 동영상 프로브. 컨테이너 헤더와 첫 비디오 스트림만 읽는다.
pub fn probe(path: &Path) -> Result<VideoProbe, AppError> {
    let mime = crate::image::resolve_mime(path)
        .ok_or_else(|| AppError::unsupported("Unsupported file"))?;
    if !VIDEO_MIMES.contains(&mime) {
        return Err(AppError::invalid_input("Not a video file"));
    }
    if !path.is_file() {
        return Err(AppError::not_found("File not found"));
    }
    let opened = open_input(path)?;
    let (width, height) = stream_dims(&opened)?;
    Ok(VideoProbe {
        mime_type: mime.to_string(),
        width,
        height,
        duration_ms: stream_duration_ms(&opened),
        fps: stream_fps(&opened),
        codec: Some(opened.decoder_name),
        has_audio: opened.has_audio,
        decodable: true,
    })
}

/// 포스터(첫 프레임 JPEG sidecar)를 보장하고 경로와 치수를 돌려준다.
/// `paint/`에 원본 식별 해시 + 한 변 기준으로 캐시한다.
pub fn ensure_poster(source: &Path, max_side: u32) -> Result<(PathBuf, u32, u32), AppError> {
    let max_side = max_side.clamp(POSTER_SIDE_MIN, POSTER_SIDE_MAX);
    let dest = poster_path(source, max_side)?;
    let key = dest.to_string_lossy().into_owned();
    crate::sidecar::with_file_lock(&key, "video poster", || {
        crate::process_temp::mark_in_use(&dest);
        if dest.exists() {
            crate::process_temp::touch_cache_file(&dest);
            let (width, height) = jpeg_dims(&dest)?;
            return Ok((dest, width, height));
        }
        let rgb = decode_first_frame_rgb8(source, max_side)?;
        let (width, height) = (rgb.width, rgb.height);
        crate::sidecar::write_rgb8_jpeg_atomic(&dest, &rgb, POSTER_JPEG_QUALITY)?;
        crate::process_temp::note_published(&dest, crate::sidecar::MAX_PAINT_BYTES);
        Ok((dest, width, height))
    })
}

/// 이미 캐시된 포스터만 돌려준다. 없으면 생성하지 않는다.
pub fn cached_poster(source: &Path, max_side: u32) -> Option<(PathBuf, u32, u32)> {
    let dest = poster_path(source, max_side.clamp(POSTER_SIDE_MIN, POSTER_SIDE_MAX)).ok()?;
    if !dest.exists() {
        return None;
    }
    crate::process_temp::mark_in_use(&dest);
    crate::process_temp::touch_cache_file(&dest);
    let (width, height) = jpeg_dims(&dest).ok()?;
    Some((dest, width, height))
}

fn poster_path(source: &Path, max_side: u32) -> Result<PathBuf, AppError> {
    let mut salt = max_side.to_le_bytes().to_vec();
    salt.push(POSTER_KEY_VERSION);
    let hash = crate::sidecar::file_identity_hash(source, &salt)?;
    Ok(crate::sidecar::paint_dir()?.join(format!("{POSTER_PREFIX}{hash:016x}.jpg")))
}

fn jpeg_dims(path: &Path) -> Result<(u32, u32), AppError> {
    image::image_dimensions(path)
        .map_err(|e| AppError::corrupt(format!("Failed to read poster: {e}")))
}

struct OpenedVideo {
    stream_index: usize,
    decoder_name: String,
    width: i32,
    height: i32,
    fps: Option<f64>,
    duration_ms: Option<u64>,
    has_audio: bool,
    geometry: Geometry,
}

fn cstr(path: &Path) -> Result<CString, AppError> {
    CString::new(path.as_os_str().as_encoded_bytes())
        .map_err(|_| AppError::invalid_input("Invalid file path"))
}

fn open_input(path: &Path) -> Result<OpenedVideo, AppError> {
    use rsmpeg::ffi;
    let cpath = cstr(path)?;
    let ictx = rsmpeg::avformat::AVFormatContextInput::open(&cpath)
        .map_err(|e| AppError::corrupt(format!("Failed to open video: {e:?}")))?;
    let (stream_index, decoder) = ictx
        .find_best_stream(ffi::AVMEDIA_TYPE_VIDEO)
        .map_err(|e| AppError::corrupt(format!("Failed to find video stream: {e:?}")))?
        .ok_or_else(|| AppError::unsupported("No video stream"))?;
    let decoder_name = decoder.name().to_str().unwrap_or("unknown").to_string();
    let (width, height, fps, duration_ms) = {
        let stream = &ictx.streams()[stream_index];
        let codecpar = stream.codecpar();
        (
            codecpar.width,
            codecpar.height,
            avg_fps(stream),
            stream_duration(stream, &ictx),
        )
    };
    if width <= 0 || height <= 0 {
        return Err(AppError::corrupt("Invalid video dimensions"));
    }
    if u64::from(width as u32) * u64::from(height as u32) > MAX_DECODE_PIXELS {
        return Err(AppError::too_large("Video dimensions are too large"));
    }
    let has_audio = ictx
        .find_best_stream(ffi::AVMEDIA_TYPE_AUDIO)
        .map(|r| r.is_some())
        .unwrap_or(false);
    let geometry = stream_geometry(&ictx, stream_index);
    Ok(OpenedVideo {
        stream_index,
        decoder_name,
        width,
        height,
        fps,
        duration_ms,
        has_audio,
        geometry,
    })
}

/// 표시 치수. 회전 메타데이터와 SAR을 반영해 화면에 보이는 크기를 돌려준다.
fn stream_dims(opened: &OpenedVideo) -> Result<(Option<u32>, Option<u32>), AppError> {
    let (width, height) = opened
        .geometry
        .display(opened.width as u32, opened.height as u32);
    Ok((Some(width), Some(height)))
}

fn stream_fps(opened: &OpenedVideo) -> Option<f64> {
    opened.fps
}

fn stream_duration_ms(opened: &OpenedVideo) -> Option<u64> {
    opened.duration_ms
}

fn avg_fps(stream: &rsmpeg::avformat::AVStreamRef<'_>) -> Option<f64> {
    let rate = stream.avg_frame_rate;
    if rate.den > 0 && rate.num > 0 {
        Some(rate.num as f64 / rate.den as f64)
    } else {
        None
    }
}

/// 스트림 duration(스트림 time_base 단위) 우선, 없으면 포맷 duration(AV_TIME_BASE).
fn stream_duration(
    stream: &rsmpeg::avformat::AVStreamRef<'_>,
    ictx: &rsmpeg::avformat::AVFormatContextInput,
) -> Option<u64> {
    const NOPTS: i64 = i64::MIN;
    if stream.duration != NOPTS && stream.duration > 0 && stream.time_base.den > 0 {
        let ms = stream.duration as f64 * 1000.0 * stream.time_base.num as f64
            / stream.time_base.den as f64;
        if ms.is_finite() && ms > 0.0 {
            return Some(ms as u64);
        }
    }
    let total = ictx.duration;
    if total != NOPTS && total > 0 {
        return Some((total / 1000) as u64);
    }
    None
}

/// 첫 비디오 프레임을 RGB24로 디코드한다.
fn decode_first_frame_rgb8(source: &Path, max_side: u32) -> Result<Rgb8, AppError> {
    let opened = open_input(source)?;
    let cpath = cstr(source)?;
    let mut ictx = rsmpeg::avformat::AVFormatContextInput::open(&cpath)
        .map_err(|e| AppError::corrupt(format!("Failed to open video: {e:?}")))?;
    let decoder = rsmpeg::avcodec::AVCodec::find_decoder_by_name(
        &CString::new(opened.decoder_name.as_str())
            .map_err(|_| AppError::corrupt("Invalid codec name"))?,
    )
    .ok_or_else(|| AppError::unsupported("Unsupported video codec"))?;
    let mut ctx = rsmpeg::avcodec::AVCodecContext::new(&decoder);
    {
        let stream = &ictx.streams()[opened.stream_index];
        ctx.apply_codecpar(&stream.codecpar())
            .map_err(|e| AppError::corrupt(format!("Failed to open decoder: {e:?}")))?;
    }
    // SAFETY: `avcodec_open2` 전에 공개 필드를 바꾸는 것은 FFmpeg가 허용한다.
    // 0은 CPU 수에 맞춘 자동 스레드 수다.
    unsafe {
        rsmpeg::UnsafeDerefMut::deref_mut(&mut ctx).thread_count = 0;
    }
    ctx.open(None)
        .map_err(|e| AppError::corrupt(format!("Failed to open decoder: {e:?}")))?;

    let mut packets = 0usize;
    let mut frame: Option<rsmpeg::avutil::AVFrame> = None;
    while packets < MAX_FIRST_FRAME_PACKETS {
        // 손상된 패킷의 읽기 오류는 건너뛴다(재생과 같은 규칙, §7.6).
        let Some(mut pkt) = crate::video_player::read_packet_lenient(&mut ictx, || false) else {
            break;
        };
        packets += 1;
        if pkt.stream_index != opened.stream_index as i32 {
            continue;
        }
        crate::video_player::send_packet_lenient(&mut ctx, &pkt);
        pkt.set_stream_index(-1);
        match ctx.receive_frame() {
            Ok(f) => {
                frame = Some(f);
                break;
            }
            Err(_) => continue,
        }
    }
    if frame.is_none() {
        ctx.send_packet(None).ok();
        while frame.is_none() {
            match ctx.receive_frame() {
                Ok(f) => frame = Some(f),
                Err(_) => break,
            }
        }
    }
    let frame = frame.ok_or_else(|| AppError::corrupt("No decodable video frame"))?;
    rgb24_of(&frame, max_side, opened.geometry)
}

/// 디코드 프레임을 RGB24 바이트로 바꾼다. 포스터에 필요한 크기로 바로
/// 줄여서 변환해, 전체 해상도 버퍼를 만든 뒤 다시 줄이는 비용을 없앤다.
/// SAR은 변환 크기에, 회전은 변환 뒤 픽셀에 반영한다.
fn rgb24_of(
    frame: &rsmpeg::avutil::AVFrame,
    max_side: u32,
    geometry: Geometry,
) -> Result<Rgb8, AppError> {
    use rsmpeg::ffi;
    let (width, height) = (frame.width, frame.height);
    if width <= 0 || height <= 0 {
        return Err(AppError::corrupt("Invalid video frame"));
    }
    let (display_width, display_height) = geometry.unrotated(width as u32, height as u32);
    let longest = display_width.max(display_height);
    let scale = (f64::from(max_side.max(1)) / f64::from(longest)).min(1.0);
    let out_width = ((f64::from(display_width) * scale).round() as u32).max(1);
    let out_height = ((f64::from(display_height) * scale).round() as u32).max(1);
    let mut scaler = rsmpeg::swscale::SwsContext::get_context(
        width,
        height,
        frame.format,
        out_width as i32,
        out_height as i32,
        ffi::AV_PIX_FMT_RGB24,
        ffi::SWS_BILINEAR,
        None,
        None,
        None,
    )
    .ok_or_else(|| AppError::corrupt("Failed to create scaler"))?;
    let image = rsmpeg::avutil::AVImage::new(
        ffi::AV_PIX_FMT_RGB24,
        out_width as i32,
        out_height as i32,
        1,
    )
    .ok_or_else(|| AppError::corrupt("Failed to allocate frame"))?;
    let mut framed = rsmpeg::avutil::AVFrameWithImage::new(image);
    scaler
        .scale_frame(frame, 0, height, &mut framed)
        .map_err(|e| AppError::corrupt(format!("Failed to scale frame: {e:?}")))?;
    let mut buf = vec![0u8; framed.image_get_buffer_size(1).unwrap_or(0)];
    framed
        .image_copy_to_buffer(&mut buf, 1)
        .map_err(|e| AppError::corrupt(format!("Failed to copy frame: {e:?}")))?;
    let (bytes, width, height) = rotate_pixels(&buf, out_width, out_height, 3, geometry.rotation);
    Ok(Rgb8 {
        width,
        height,
        bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples")
            .join(name)
    }

    #[test]
    fn extensions_map_to_video_mimes() {
        for (ext, mime) in [
            ("mp4", "video/mp4"),
            ("mov", "video/quicktime"),
            ("webm", "video/webm"),
            ("mkv", "video/x-matroska"),
            ("avi", "video/x-msvideo"),
        ] {
            assert_eq!(
                crate::image::get_mime_type(Path::new(&format!("a.{ext}"))),
                Some(mime)
            );
            assert!(VIDEO_MIMES.contains(&mime));
            assert!(crate::image::is_video_file(Path::new(&format!("a.{ext}"))));
            assert!(!crate::image::is_image_file(Path::new(&format!("a.{ext}"))));
            assert!(crate::image::is_supported_file(Path::new(&format!(
                "a.{ext}"
            ))));
        }
    }

    #[test]
    fn resolve_samples_to_video() {
        for (name, mime) in [
            ("sample.mp4", "video/mp4"),
            ("sample.mov", "video/quicktime"),
            ("sample.webm", "video/webm"),
            ("sample.mkv", "video/x-matroska"),
            ("sample.avi", "video/x-msvideo"),
        ] {
            let path = sample(name);
            assert!(path.is_file(), "missing sample: {}", path.to_string_lossy());
            assert_eq!(crate::image::resolve_mime(&path), Some(mime), "{name}");
            assert_eq!(crate::sniff::sniff_file(&path).unwrap()[..5], *"video");
        }
    }

    #[test]
    fn probe_rejects_images() {
        assert!(probe(&sample("sample.jpg")).is_err());
    }

    #[test]
    fn probe_reads_stream_metadata() {
        for (name, codec) in [
            ("sample.mp4", "mpeg4"),
            ("sample.mov", "mpeg4"),
            ("sample.webm", "vp9"),
            ("sample.mkv", "mpeg4"),
            ("sample.avi", "mpeg4"),
        ] {
            let probed = probe(&sample(name)).unwrap();
            assert_eq!(probed.width, Some(64));
            assert_eq!(probed.height, Some(48));
            assert_eq!(probed.codec.as_deref(), Some(codec));
            assert!(probed.decodable);
            assert!(probed.duration_ms.is_some());
        }
    }

    #[test]
    fn probe_detects_audio_stream() {
        let probed = probe(&sample("sample-audio.mp4")).unwrap();
        assert!(probed.has_audio);
        assert!(!probe(&sample("sample.mp4")).unwrap().has_audio);
    }

    #[test]
    fn poster_decodes_first_frame() {
        for name in [
            "sample.mp4",
            "sample.mov",
            "sample.webm",
            "sample.mkv",
            "sample.avi",
        ] {
            let (path, width, height) = ensure_poster(&sample(name), 64).unwrap();
            assert_eq!((width, height), (64, 48));
            assert!(path.is_file());
            let cached = cached_poster(&sample(name), 64).unwrap();
            assert_eq!(cached.0, path);
        }
    }

    #[test]
    fn poster_downscale_fills_bottom_rows() {
        // `sws_scale`의 `src_slice_h`에 목적지 높이를 넘기면 출력 하단이
        // 검게 잘린다. 축소가 일어나는 입력으로 하단 행을 검증한다.
        let rgb = decode_first_frame_rgb8(&sample("sample.mp4"), 32).unwrap();
        assert_eq!((rgb.width, rgb.height), (32, 24));
        let (w, h) = (rgb.width as usize, rgb.height as usize);
        assert_eq!(rgb.bytes.len(), w * h * 3);
        let bottom = &rgb.bytes[(h - 1) * w * 3..h * w * 3];
        assert!(
            bottom.iter().any(|&b| b != 0),
            "bottom row is all zero (swscale src_slice_h)"
        );

        let rgb = decode_first_frame_rgb8(&sample("sample-1440p.mp4"), 256).unwrap();
        assert_eq!((rgb.width, rgb.height), (256, 144));
        let (w, h) = (rgb.width as usize, rgb.height as usize);
        assert_eq!(rgb.bytes.len(), w * h * 3);
        let bottom = &rgb.bytes[(h - 1) * w * 3..h * w * 3];
        assert!(
            bottom.iter().any(|&b| b != 0),
            "720p+ bottom row is all zero (swscale src_slice_h)"
        );
    }

    /// RGB24 버퍼에서 빨간 칸(왼쪽 위 16x12로 그린 표식)의 경계 상자.
    fn red_box(
        rgb: &[u8],
        width: u32,
        height: u32,
        bpp: usize,
        red_index: usize,
    ) -> (u32, u32, u32, u32) {
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        for y in 0..height {
            for x in 0..width {
                let p = ((y * width + x) as usize) * bpp;
                let red = rgb[p + red_index];
                let green = rgb[p + 1];
                if red > 180 && green < 80 {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        (x0, y0, x1, y1)
    }

    #[test]
    fn rotation_metadata_turns_dimensions_and_poster() {
        // 64x48로 저장되고 시계 방향 90도로 표시해야 하는 영상(폰 세로 영상과 같은 메타데이터).
        // 왼쪽 위 빨간 칸은 화면에서 오른쪽 위에 와야 한다(ffmpeg 자동 회전과 같음).
        let path = sample("sample-rotated.mp4");
        let probed = probe(&path).unwrap();
        assert_eq!((probed.width, probed.height), (Some(48), Some(64)));
        let rgb = decode_first_frame_rgb8(&path, 64).unwrap();
        assert_eq!((rgb.width, rgb.height), (48, 64));
        let (x0, y0, x1, y1) = red_box(&rgb.bytes, rgb.width, rgb.height, 3, 0);
        assert!(
            x0 >= 32 && x1 == 47 && y0 == 0 && y1 <= 16,
            "red at {x0},{y0}-{x1},{y1}"
        );
        let (poster, width, height) = ensure_poster(&path, 64).unwrap();
        assert_eq!((width, height), (48, 64));
        assert!(poster.is_file());
    }

    #[test]
    fn sample_aspect_ratio_widens_dimensions_and_poster() {
        // 64x48, SAR 2:1. 화면 비율 8:3으로 보여야 한다.
        let path = sample("sample-anamorphic.mp4");
        let probed = probe(&path).unwrap();
        assert_eq!((probed.width, probed.height), (Some(128), Some(48)));
        let rgb = decode_first_frame_rgb8(&path, 128).unwrap();
        assert_eq!((rgb.width, rgb.height), (128, 48));
        let (x0, y0, x1, y1) = red_box(&rgb.bytes, rgb.width, rgb.height, 3, 0);
        // 16x12 칸이 가로로 두 배가 된다.
        assert!(
            x0 == 0 && (29..=33).contains(&x1) && y0 == 0 && y1 <= 12,
            "red at {x0},{y0}-{x1},{y1}"
        );
    }

    #[test]
    fn clockwise_rotation_follows_fftools() {
        assert_eq!(clockwise_rotation(0.0), 0);
        // ffprobe의 rotation=-90(폰 세로 영상)은 시계 방향 90도다.
        assert_eq!(clockwise_rotation(-90.0), 90);
        assert_eq!(clockwise_rotation(90.0), 270);
        assert_eq!(clockwise_rotation(180.0), 180);
        assert_eq!(clockwise_rotation(-180.0), 180);
        assert_eq!(clockwise_rotation(-89.6), 90);
        assert_eq!(clockwise_rotation(f64::NAN), 0);
    }

    #[test]
    fn rotate_pixels_moves_corners() {
        // 3x2, 픽셀 값은 인덱스. 0 1 2 / 3 4 5
        let src: Vec<u8> = (0..6).collect();
        let (cw, w, h) = rotate_pixels(&src, 3, 2, 1, 90);
        assert_eq!((w, h), (2, 3));
        assert_eq!(cw, vec![3, 0, 4, 1, 5, 2]);
        let (flip, w, h) = rotate_pixels(&src, 3, 2, 1, 180);
        assert_eq!((w, h), (3, 2));
        assert_eq!(flip, vec![5, 4, 3, 2, 1, 0]);
        let (ccw, w, h) = rotate_pixels(&src, 3, 2, 1, 270);
        assert_eq!((w, h), (2, 3));
        assert_eq!(ccw, vec![2, 5, 1, 4, 0, 3]);
    }

    #[test]
    fn rotate_pixels_moves_four_byte_pixels_whole() {
        // 3x2 BGRA. 픽셀 i는 [i, i, i, 255].
        let src: Vec<u8> = (0..6u8).flat_map(|i| [i, i, i, 255]).collect();
        for (rotation, order) in [
            (90, vec![3u8, 0, 4, 1, 5, 2]),
            (180, vec![5, 4, 3, 2, 1, 0]),
            (270, vec![2, 5, 1, 4, 0, 3]),
        ] {
            let (out, _, _) = rotate_pixels(&src, 3, 2, 4, rotation);
            let expected: Vec<u8> = order.iter().flat_map(|&i| [i, i, i, 255]).collect();
            assert_eq!(out, expected, "rotation {rotation}");
        }
    }

    #[test]
    fn geometry_applies_sar_before_rotation() {
        let geometry = Geometry {
            rotation: 90,
            sar: (4, 3),
        };
        assert_eq!(geometry.unrotated(1440, 1080), (1920, 1080));
        assert_eq!(geometry.display(1440, 1080), (1080, 1920));
        let identity = Geometry {
            rotation: 0,
            sar: (1, 1),
        };
        assert_eq!(identity.display(64, 48), (64, 48));
    }
}
#[cfg(test)]
mod rot_bench {
    #[test]
    fn bench() {
        let src = vec![7u8; 1920 * 1080 * 4];
        let t = std::time::Instant::now();
        for _ in 0..10 {
            let _ = super::rotate_pixels(&src, 1920, 1080, 4, 90);
        }
        println!(
            "ROT 1080p bgra avg {:.2}ms",
            t.elapsed().as_secs_f64() * 100.0
        );
    }
}
