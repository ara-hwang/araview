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
    pub has_subtitles: bool,
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
        has_subtitles: opened.has_subtitles,
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
    let hash = crate::sidecar::file_identity_hash(source, &max_side.to_le_bytes())?;
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
    has_subtitles: bool,
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
    let has_subtitles = ictx
        .find_best_stream(ffi::AVMEDIA_TYPE_SUBTITLE)
        .map(|r| r.is_some())
        .unwrap_or(false);
    Ok(OpenedVideo {
        stream_index,
        decoder_name,
        width,
        height,
        fps,
        duration_ms,
        has_audio,
        has_subtitles,
    })
}

fn stream_dims(opened: &OpenedVideo) -> Result<(Option<u32>, Option<u32>), AppError> {
    Ok((Some(opened.width as u32), Some(opened.height as u32)))
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
        let Ok(Some(mut pkt)) = ictx.read_packet() else {
            break;
        };
        packets += 1;
        if pkt.stream_index != opened.stream_index as i32 {
            continue;
        }
        ctx.send_packet(Some(&pkt))
            .map_err(|e| AppError::corrupt(format!("Failed to decode video: {e:?}")))?;
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
    rgb24_of(&frame, max_side)
}

/// 디코드 프레임을 RGB24 바이트로 바꾼다. 포스터에 필요한 크기로 바로
/// 줄여서 변환해, 전체 해상도 버퍼를 만든 뒤 다시 줄이는 비용을 없앤다.
fn rgb24_of(frame: &rsmpeg::avutil::AVFrame, max_side: u32) -> Result<Rgb8, AppError> {
    use rsmpeg::ffi;
    let (width, height) = (frame.width, frame.height);
    if width <= 0 || height <= 0 {
        return Err(AppError::corrupt("Invalid video frame"));
    }
    let longest = width.max(height) as u32;
    let scale = (f64::from(max_side.max(1)) / f64::from(longest)).min(1.0);
    let out_width = ((width as f64 * scale).round() as u32).max(1);
    let out_height = ((height as f64 * scale).round() as u32).max(1);
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
        .scale_frame(frame, 0, out_height as i32, &mut framed)
        .map_err(|e| AppError::corrupt(format!("Failed to scale frame: {e:?}")))?;
    let mut buf = vec![0u8; framed.image_get_buffer_size(1).unwrap_or(0)];
    framed
        .image_copy_to_buffer(&mut buf, 1)
        .map_err(|e| AppError::corrupt(format!("Failed to copy frame: {e:?}")))?;
    Ok(Rgb8 {
        width: out_width,
        height: out_height,
        bytes: buf,
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
            assert!(!probed.has_subtitles);
        }
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
}
