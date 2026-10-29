//! 동영상 스트리밍 재생 엔진 (M2, §7.6).
//!
//! 디먹스·디코드는 rsmpeg(FFmpeg 8.x, MIT)이다. 워커 스레드가 디먹스와 비디오
//! 디코드를 소유하고, 핸들 쪽은 시계(재생 위치)와 명령만 쥔다. 전 프레임을
//! 미리 적재하지 않고, 큐(바이트 상한)에 든 프레임에서 현재 위치의 것을
//! `poll_frame`으로 뽑는다. 오디오는 `video_audio.rs`가 맡고, 있으면 오디오
//! 클럭이 마스터다.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

use crate::app_error::AppError;

/// 재생 가능한 속도. `[`/`]`는 이 목록을 오르내린다.
pub const SPEEDS: &[f32] = &[0.25, 0.5, 1.0, 1.5, 2.0];
/// 재생 프레임 큐 바이트 상한. 1080p BGRA면 12프레임 수준이다.
const QUEUE_BYTES_MAX: u64 = 96 * 1024 * 1024;
/// 재생 출력의 긴 변 상한. 4K 원본도 이 크기로 줄여 변환·업로드 비용을 줄인다.
/// 원본이 더 작으면 그대로 쓴다.
const MAX_OUTPUT_SIDE: u32 = 1920;
/// 디코드 허용 픽셀 수. 표시 경로(`display.rs`)와 같은 값이다.
const MAX_DECODE_PIXELS: u64 = 100_000_000;

/// BGRA 한 프레임. UI는 이를 `RenderImage` 한 장으로 그린다.
/// GPUI가 BGRA를 받으므로 변환 단계에서 채널을 바꿔 둔다.
#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
    pub pts_ms: u64,
}

/// `open` 시점에 확정한 스트림 정보.
#[derive(Debug, Clone)]
pub struct PlayerInfo {
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub fps: Option<f64>,
    pub has_audio: bool,
}

enum Command {
    Play(f32),
    Pause,
    Seek(u64),
    Step(u64),
    Stop,
}

struct Shared {
    queue: Mutex<VecDeque<VideoFrame>>,
    queue_bytes: AtomicU64,
    error: Mutex<Option<String>>,
    eof: AtomicBool,
    speed_bits: AtomicU32,
    /// 마지막으로 UI에 내준 프레임 pts + 1. 0이면 아직 없음.
    last_pts: AtomicU64,
    /// 진행 중인 탐색 목표 + 1. 0이면 탐색 중이 아니다.
    seek_pending: AtomicU64,
    /// 탐색이 끝나 UI가 가져갈 프레임의 목표 + 1. 0이면 없음.
    /// UI가 결과 프레임을 받을 때까지 유지해 정지 상태에서도 렌더를 이어 간다.
    seek_result: AtomicU64,
}

/// 동영상 재생 핸들. 워커 스레드를 소유하고, 시계는 이쪽에서 잰다.
pub struct VideoPlayer {
    cmd_tx: mpsc::Sender<Command>,
    worker: Option<std::thread::JoinHandle<()>>,
    shared: Arc<Shared>,
    clock: Mutex<Clock>,
    info: PlayerInfo,
    audio: Option<crate::video_audio::AudioOutput>,
}

struct Clock {
    base_ms: u64,
    base_at: Instant,
    playing: bool,
    speed: f32,
}

impl VideoPlayer {
    /// 디먹서를 열고 워커를 시작한다. 호출자가 blocking 풀에서 실행한다.
    pub fn open(path: &Path) -> Result<Self, AppError> {
        let probed = crate::video::probe(path)?;
        let (width, height) = match (probed.width, probed.height) {
            (Some(w), Some(h)) => (w, h),
            _ => return Err(AppError::corrupt("Invalid video dimensions")),
        };
        if u64::from(width) * u64::from(height) > MAX_DECODE_PIXELS {
            return Err(AppError::too_large("Video dimensions are too large"));
        }
        let info = PlayerInfo {
            width,
            height,
            duration_ms: probed.duration_ms.unwrap_or(0),
            fps: probed.fps,
            has_audio: probed.has_audio,
        };
        let shared = Arc::new(Shared {
            queue: Mutex::new(VecDeque::new()),
            queue_bytes: AtomicU64::new(0),
            error: Mutex::new(None),
            eof: AtomicBool::new(false),
            speed_bits: AtomicU32::new(1.0f32.to_bits()),
            last_pts: AtomicU64::new(0),
            seek_pending: AtomicU64::new(0),
            seek_result: AtomicU64::new(0),
        });
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let worker_shared = Arc::clone(&shared);
        let source = path.to_path_buf();
        let worker = std::thread::Builder::new()
            .name("araview-video".to_string())
            .spawn(move || worker_main(source, cmd_rx, worker_shared))
            .map_err(|e| AppError::unknown(format!("Failed to start video thread: {e}")))?;
        // 오디오 장치를 열 수 없으면 영상만 재생한다.
        let audio = if probed.has_audio {
            match crate::video_audio::AudioOutput::open(path) {
                Ok(audio) => Some(audio),
                Err(e) => {
                    log::warn!("audio unavailable, playing video only: {e:?}");
                    None
                }
            }
        } else {
            None
        };
        Ok(Self {
            cmd_tx,
            worker: Some(worker),
            shared,
            clock: Mutex::new(Clock {
                base_ms: 0,
                base_at: Instant::now(),
                playing: false,
                speed: 1.0,
            }),
            info,
            audio,
        })
    }

    pub fn info(&self) -> &PlayerInfo {
        &self.info
    }

    pub fn position_ms(&self) -> u64 {
        if self.info.duration_ms == 0 {
            return match &self.audio {
                Some(audio) => audio.played_ms(),
                None => {
                    let clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
                    if clock.playing {
                        let elapsed = clock.base_at.elapsed().as_secs_f64();
                        clock.base_ms.saturating_add(
                            (elapsed * 1000.0 * f64::from(clock.speed)).round() as u64,
                        )
                    } else {
                        clock.base_ms
                    }
                }
            };
        }
        // 오디오가 있으면 오디오 클럭이 마스터, 없으면 단조 클럭이 마스터다.
        let mut pos = match &self.audio {
            Some(audio) => audio.played_ms(),
            None => {
                let clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
                if clock.playing {
                    let elapsed = clock.base_at.elapsed().as_secs_f64();
                    clock
                        .base_ms
                        .saturating_add((elapsed * 1000.0 * f64::from(clock.speed)).round() as u64)
                } else {
                    clock.base_ms
                }
            }
        };
        pos = pos.min(self.info.duration_ms);
        // 끝까지 가면 멈춘다. 유한 반복 GIF와 같은 규칙이다.
        if pos >= self.info.duration_ms {
            if self.is_playing() {
                self.pause();
            }
            return self.info.duration_ms;
        }
        pos
    }

    pub fn is_playing(&self) -> bool {
        self.clock.lock().unwrap_or_else(|e| e.into_inner()).playing
    }

    pub fn speed(&self) -> f32 {
        self.clock.lock().unwrap_or_else(|e| e.into_inner()).speed
    }

    /// 재생을 시작하지 않고 속도만 바꾼다. 정지 상태에서 속도 버튼용이다.
    pub fn set_speed(&self, speed: f32) {
        if !speed.is_finite() || speed <= 0.0 {
            return;
        }
        {
            let mut clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
            if clock.playing {
                clock.base_ms = self.clamped_position(&clock);
                clock.base_at = Instant::now();
            }
            clock.speed = speed;
        }
        self.shared
            .speed_bits
            .store(speed.to_bits(), Ordering::SeqCst);
        if let Some(audio) = &self.audio {
            audio.set_speed(speed);
        }
    }

    pub fn play(&self, speed: f32) {
        if !speed.is_finite() || speed <= 0.0 {
            return;
        }
        // 끝에서 다시 재생하면 처음부터 시작한다. 그대로 두면 위치가 이미 끝이라
        // `position_ms`가 곧바로 다시 멈춘다.
        if self.info.duration_ms > 0 && self.position_ms() >= self.info.duration_ms {
            self.seek(0);
        }
        {
            let mut clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
            clock.base_ms = self.clamped_position(&clock);
            clock.base_at = Instant::now();
            clock.playing = true;
            clock.speed = speed;
        }
        self.shared
            .speed_bits
            .store(speed.to_bits(), Ordering::SeqCst);
        if let Some(audio) = &self.audio {
            audio.set_speed(speed);
            audio.set_playing(true);
        }
        let _ = self.cmd_tx.send(Command::Play(speed));
    }

    pub fn pause(&self) {
        {
            let mut clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
            clock.base_ms = self.clamped_position(&clock);
            clock.playing = false;
        }
        if let Some(audio) = &self.audio {
            audio.set_playing(false);
        }
        let _ = self.cmd_tx.send(Command::Pause);
    }

    pub fn seek(&self, position_ms: u64) {
        let target = if self.info.duration_ms > 0 {
            position_ms.min(self.info.duration_ms)
        } else {
            position_ms
        };
        {
            let mut clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
            clock.base_ms = target;
            clock.base_at = Instant::now();
        }
        self.shared.eof.store(false, Ordering::SeqCst);
        self.shared.last_pts.store(0, Ordering::SeqCst);
        self.shared
            .seek_pending
            .store(target.saturating_add(1), Ordering::SeqCst);
        self.shared.seek_result.store(0, Ordering::SeqCst);
        if let Some(audio) = &self.audio {
            audio.seek(target);
        }
        let _ = self.cmd_tx.send(Command::Seek(target));
    }

    /// 정지한 채 1프레임 이동한다. 재생 중이면 먼저 멈춘다.
    pub fn step(&self, direction: i32) {
        if direction == 0 {
            return;
        }
        let frame_ms = self.frame_ms();
        let pos = self.position_ms();
        self.pause();
        let target = if direction > 0 {
            pos.saturating_add(frame_ms)
        } else {
            pos.saturating_sub(frame_ms)
        };
        let target = if self.info.duration_ms > 0 {
            target.min(self.info.duration_ms)
        } else {
            target
        };
        {
            let mut clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
            clock.base_ms = target;
            clock.base_at = Instant::now();
        }
        self.shared.eof.store(false, Ordering::SeqCst);
        self.shared.last_pts.store(0, Ordering::SeqCst);
        self.shared
            .seek_pending
            .store(target.saturating_add(1), Ordering::SeqCst);
        self.shared.seek_result.store(0, Ordering::SeqCst);
        if let Some(audio) = &self.audio {
            audio.seek(target);
        }
        let _ = self.cmd_tx.send(Command::Step(target));
    }

    /// 오디오 출력이 살아 있는가. 장치를 못 열면 영상만 재생한다.
    pub fn audio_active(&self) -> bool {
        self.audio.is_some()
    }

    /// 오디오 콜백이 선행 버퍼 부족으로 소리를 0으로 메운 횟수. 진단용이다.
    pub fn audio_underruns(&self) -> u64 {
        self.audio
            .as_ref()
            .map(|audio| audio.underruns())
            .unwrap_or(0)
    }

    /// 탐색·스텝 결과 프레임이 아직 오지 않았는가. UI는 이 동안 프레임을
    /// 계속 요청해 정지 상태에서도 결과를 화면에 올린다.
    pub fn is_seeking(&self) -> bool {
        self.shared.seek_pending.load(Ordering::SeqCst) != 0
            || self.shared.seek_result.load(Ordering::SeqCst) != 0
    }

    /// cpal 출력 스트림을 전용 스레드에서 열어 돌려준다.
    /// `AudioStream`을 들고 있는 동안 소리가 난다. 어느 스레드에서든 호출할 수 있다.
    pub fn attach_audio_output(&self) -> Result<crate::video_audio::AudioStream, AppError> {
        let audio = self
            .audio
            .as_ref()
            .ok_or_else(|| AppError::unknown("No audio stream"))?;
        audio.open_output_stream()
    }

    /// 오디오를 끄고 영상만 재생한다. 출력 장치를 열 수 없을 때 쓴다.
    pub fn disable_audio(&mut self) {
        self.audio.take();
    }

    /// 0.0~1.0. UI는 0~100 설정을 `/100`으로 넘긴다.
    pub fn set_volume(&self, volume: f32) {
        if let Some(audio) = &self.audio {
            audio.set_volume(volume);
        }
    }

    pub fn volume(&self) -> f32 {
        self.audio
            .as_ref()
            .map(|audio| audio.volume())
            .unwrap_or(crate::video_audio::DEFAULT_VOLUME)
    }

    pub fn set_muted(&self, muted: bool) {
        if let Some(audio) = &self.audio {
            audio.set_muted(muted);
        }
    }

    pub fn is_muted(&self) -> bool {
        self.audio.as_ref().is_some_and(|audio| audio.is_muted())
    }

    /// 현재 위치에 해당하는 프레임을 꺼내 돌려준다. 지난 프레임은 버리고
    /// 한 번 돌려준 프레임은 다시 돌려주지 않는다(UI가 마지막 그림을 들고 있다).
    pub fn poll_frame(&self) -> Result<Option<VideoFrame>, AppError> {
        if let Some(err) = self
            .shared
            .error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            return Err(AppError::corrupt(err));
        }
        if let Some(audio) = &self.audio {
            if let Some(err) = audio.has_error() {
                return Err(AppError::corrupt(err));
            }
        }
        let pos = self.position_ms();
        let mut queue = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        let seek_result = self.shared.seek_result.load(Ordering::SeqCst);
        let mut chosen = None;
        while let Some(front) = queue.front() {
            if front.pts_ms > pos {
                break;
            }
            if let Some(frame) = queue.pop_front() {
                self.shared
                    .queue_bytes
                    .fetch_sub(frame.bgra.len() as u64, Ordering::SeqCst);
                chosen = Some(frame);
            }
        }
        if chosen.is_none() && seek_result != 0 {
            // 탐색 결과는 목표 시각보다 조금 뒤 프레임일 수 있다. 정지 상태에서도
            // 목표 프레임을 바로 화면에 올리도록 시각을 앞질러 꺼낸다.
            if let Some(frame) = queue.pop_front() {
                self.shared
                    .queue_bytes
                    .fetch_sub(frame.bgra.len() as u64, Ordering::SeqCst);
                chosen = Some(frame);
            }
        } else if chosen.is_none() && self.shared.last_pts.load(Ordering::SeqCst) == 0 {
            // 재생 시작 직후에는 첫 프레임의 표시 시각이 아직 오지 않았어도
            // 바로 보여 준다(포스터에서 재생 화면으로 넘어가는 지연을 없앤다).
            if let Some(frame) = queue.pop_front() {
                self.shared
                    .queue_bytes
                    .fetch_sub(frame.bgra.len() as u64, Ordering::SeqCst);
                chosen = Some(frame);
            }
        }
        if let Some(frame) = &chosen {
            self.shared
                .last_pts
                .store(frame.pts_ms.saturating_add(1), Ordering::SeqCst);
            if seek_result != 0 {
                let target = seek_result - 1;
                if frame.pts_ms >= target || queue.is_empty() {
                    let _ = self.shared.seek_result.compare_exchange(
                        seek_result,
                        0,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    );
                }
            }
        } else if seek_result != 0 && queue.is_empty() {
            let _ = self.shared.seek_result.compare_exchange(
                seek_result,
                0,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
        }
        Ok(chosen)
    }

    pub fn has_error(&self) -> Option<String> {
        if let Some(err) = self
            .shared
            .error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            return Some(err);
        }
        self.audio.as_ref().and_then(|audio| audio.has_error())
    }

    fn frame_ms(&self) -> u64 {
        match self.info.fps {
            Some(fps) if fps.is_finite() && fps > 0.0 => (1000.0 / fps) as u64,
            _ => 40,
        }
        .max(1)
    }

    fn clamped_position(&self, clock: &Clock) -> u64 {
        let pos = if clock.playing {
            let elapsed = clock.base_at.elapsed().as_secs_f64();
            clock
                .base_ms
                .saturating_add((elapsed * 1000.0 * f64::from(clock.speed)).round() as u64)
        } else {
            clock.base_ms
        };
        if self.info.duration_ms == 0 {
            pos
        } else {
            pos.min(self.info.duration_ms)
        }
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(Command::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn worker_main(source: PathBuf, cmd_rx: mpsc::Receiver<Command>, shared: Arc<Shared>) {
    if let Err(e) = worker_loop(&source, &cmd_rx, &shared) {
        *shared.error.lock().unwrap_or_else(|e| e.into_inner()) = Some(e);
    }
}

struct Decoder {
    ictx: rsmpeg::avformat::AVFormatContextInput,
    ctx: rsmpeg::avcodec::AVCodecContext,
    scaler: Option<rsmpeg::swscale::SwsContext>,
    scaler_key: Option<(u32, u32, u32, u32, i32)>,
    stream_index: usize,
    time_base: (i32, i32),
    /// 재생 위치 0에 해당하는 컨테이너 시작 시각(ms). 오디오와 같은 기준이다.
    origin_ms: i64,
}

fn open_decoder(source: &Path) -> Result<Decoder, String> {
    use rsmpeg::ffi;
    let cpath = std::ffi::CString::new(source.as_os_str().as_encoded_bytes())
        .map_err(|_| "Invalid file path".to_string())?;
    let ictx = rsmpeg::avformat::AVFormatContextInput::open(&cpath)
        .map_err(|e| format!("Failed to open video: {e:?}"))?;
    let (stream_index, decoder) = ictx
        .find_best_stream(ffi::AVMEDIA_TYPE_VIDEO)
        .map_err(|e| format!("Failed to find video stream: {e:?}"))?
        .ok_or_else(|| "No video stream".to_string())?;
    let stream = &ictx.streams()[stream_index];
    let (width, height) = (stream.codecpar().width, stream.codecpar().height);
    if width <= 0 || height <= 0 {
        return Err("Invalid video dimensions".to_string());
    }
    let time_base = {
        let tb = stream.time_base;
        (tb.num, tb.den)
    };
    let decoder_name = decoder.name().to_str().unwrap_or("unknown").to_string();
    let cname = std::ffi::CString::new(decoder_name.as_str())
        .map_err(|_| "Invalid codec name".to_string())?;
    let decoder = rsmpeg::avcodec::AVCodec::find_decoder_by_name(&cname)
        .ok_or_else(|| format!("Unsupported video codec: {decoder_name}"))?;
    let mut ctx = rsmpeg::avcodec::AVCodecContext::new(&decoder);
    {
        let stream = &ictx.streams()[stream_index];
        ctx.apply_codecpar(&stream.codecpar())
            .map_err(|e| format!("Failed to open decoder: {e:?}"))?;
    }
    // SAFETY: `avcodec_open2` 전에 공개 필드를 바꾸는 것은 FFmpeg가 허용한다.
    // 0은 CPU 수에 맞춘 자동 스레드 수다. 기본값 1이면 4K 디코드가 실시간에 못 미친다.
    unsafe {
        rsmpeg::UnsafeDerefMut::deref_mut(&mut ctx).thread_count = 0;
    }
    ctx.open(None)
        .map_err(|e| format!("Failed to open decoder: {e:?}"))?;
    let origin_ms = container_origin_ms(&ictx);
    Ok(Decoder {
        ictx,
        ctx,
        scaler: None,
        scaler_key: None,
        stream_index,
        time_base,
        origin_ms,
    })
}

/// 컨테이너 시작 시각(ms). MPEG-TS나 편집 목록이 있는 MP4는 첫 pts가 0이
/// 아니다. 재생 위치는 이 시각을 0으로 잰다. 오디오도 같은 값을 써서 동기를 맞춘다.
pub(crate) fn container_origin_ms(ictx: &rsmpeg::avformat::AVFormatContextInput) -> i64 {
    const NOPTS: i64 = i64::MIN;
    let start = ictx.start_time;
    if start == NOPTS || start <= 0 {
        0
    } else {
        start / 1000
    }
}

/// 프레임 시각(ms, 컨테이너 시작 기준). pts가 없으면 디코더 추정값을 쓴다.
pub(crate) fn frame_time_ms(
    frame: &rsmpeg::avutil::AVFrame,
    time_base: (i32, i32),
    origin_ms: i64,
) -> Option<u64> {
    const NOPTS: i64 = i64::MIN;
    let pts: i64 = if frame.pts != NOPTS {
        frame.pts
    } else {
        frame.best_effort_timestamp
    };
    if pts == NOPTS || time_base.1 <= 0 {
        return None;
    }
    let ms = pts as f64 * 1000.0 * time_base.0 as f64 / time_base.1 as f64 - origin_ms as f64;
    if ms.is_finite() && ms >= 0.0 {
        Some(ms as u64)
    } else {
        None
    }
}

/// 재생 위치(ms)를 스트림 time_base 시각으로 바꾼다. 탐색 대상이다.
pub(crate) fn seek_timestamp(target_ms: u64, time_base: (i32, i32), origin_ms: i64) -> i64 {
    let (num, den) = time_base;
    if num <= 0 || den <= 0 {
        return 0;
    }
    let ms = target_ms as f64 + origin_ms as f64;
    ((ms * den as f64 / (1000.0 * num as f64)) as i64).max(0)
}

/// 패킷을 디코더에 넣는다. 손상된 패킷 하나로 재생 전체를 끝내지 않도록
/// 디코드 오류는 그 패킷만 건너뛴다.
pub(crate) fn send_packet_lenient(
    ctx: &mut rsmpeg::avcodec::AVCodecContext,
    pkt: &rsmpeg::avcodec::AVPacket,
) {
    if let Err(e) = ctx.send_packet(Some(pkt)) {
        log::debug!("skipping undecodable packet: {e:?}");
    }
}

/// 재생 출력 크기. 긴 변이 `MAX_OUTPUT_SIDE`를 넘으면 비율을 유지해 줄인다.
fn output_dimensions(width: u32, height: u32) -> (u32, u32) {
    let long = width.max(height);
    if long <= MAX_OUTPUT_SIDE {
        return (width, height);
    }
    let scale = f64::from(MAX_OUTPUT_SIDE) / f64::from(long);
    (
        ((f64::from(width) * scale).round() as u32).max(1),
        ((f64::from(height) * scale).round() as u32).max(1),
    )
}

impl Decoder {
    fn scale_to_bgra(
        &mut self,
        frame: &rsmpeg::avutil::AVFrame,
    ) -> Result<(Vec<u8>, u32, u32), String> {
        use rsmpeg::ffi;
        let width = frame.width;
        let height = frame.height;
        if width <= 0 || height <= 0 {
            return Err("Invalid video frame".to_string());
        }
        let (out_width, out_height) = output_dimensions(width as u32, height as u32);
        let format = frame.format;
        let key = (width as u32, height as u32, out_width, out_height, format);
        if self.scaler.is_none() || self.scaler_key != Some(key) {
            self.scaler = rsmpeg::swscale::SwsContext::get_context(
                width,
                height,
                format,
                out_width as i32,
                out_height as i32,
                ffi::AV_PIX_FMT_BGRA,
                ffi::SWS_BILINEAR,
                None,
                None,
                None,
            );
            self.scaler_key = Some(key);
        }
        let scaler = self
            .scaler
            .as_mut()
            .ok_or_else(|| "Failed to create scaler".to_string())?;
        // swscale이 UI로 넘길 버퍼에 바로 쓰게 한다. AVFrame 버퍼를 만들어
        // 복사하면 프레임마다 8MB를 한 번 더 훑게 된다(1080p 기준 2ms).
        let stride = out_width as usize * 4;
        let mut buf = vec![0u8; stride * out_height as usize];
        let mut data = [std::ptr::null_mut::<u8>(); 4];
        data[0] = buf.as_mut_ptr();
        let linesize = [stride as i32, 0, 0, 0];
        // SAFETY: `buf`가 `stride * out_height` 바이트를 소유하고 `data`·`linesize`가
        // 그 범위의 첫 평면을 가리킨다. BGRA는 한 평면이라 swscale이 이 범위 밖에
        // 쓰지 않는다.
        unsafe {
            scaler
                .scale(
                    frame.data.as_ptr() as *const *const u8,
                    frame.linesize.as_ptr(),
                    0,
                    height,
                    data.as_ptr(),
                    linesize.as_ptr(),
                )
                .map_err(|e| format!("Failed to scale frame: {e:?}"))?;
        }
        Ok((buf, out_width, out_height))
    }
}

fn worker_loop(
    source: &Path,
    cmd_rx: &mpsc::Receiver<Command>,
    shared: &Arc<Shared>,
) -> Result<(), String> {
    let mut dec = open_decoder(source)?;
    let mut playing = false;
    // 끝까지 디코드했는가. 재생 상태와 따로 둔다. 디코드는 화면보다 앞서
    // 끝나므로, 여기서 `playing`을 끄면 재생 중 뒤로 탐색했을 때 목표
    // 프레임 하나만 넣고 디코드가 멈춰 화면이 굳는다.
    let mut at_eof = false;
    let mut pending = VecDeque::new();
    loop {
        let cmd = if let Some(cmd) = pending.pop_front() {
            Some(cmd)
        } else if playing && !at_eof {
            match cmd_rx.try_recv() {
                Ok(cmd) => Some(cmd),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        } else {
            match cmd_rx.recv() {
                Ok(cmd) => Some(cmd),
                Err(_) => return Ok(()),
            }
        };
        let Some(mut cmd) = cmd else {
            // 재생 중인데 명령이 없으면 디코드한다. 정지 중이면 위에서 recv로 기다린다.
            if queue_full(shared) {
                std::thread::sleep(std::time::Duration::from_millis(5));
                continue;
            }
            if pump(&mut dec, shared)? {
                shared.eof.store(true, Ordering::SeqCst);
                at_eof = true;
            }
            continue;
        };
        // 타임라인을 빠르게 끌면 탐색 명령이 밀린다. 연속된 탐색·스텝은
        // 마지막 목표만 적용해 낡은 탐색을 줄줄이 디코드하지 않는다.
        if matches!(cmd, Command::Seek(_) | Command::Step(_)) {
            loop {
                match cmd_rx.try_recv() {
                    Ok(next) if matches!(next, Command::Seek(_) | Command::Step(_)) => {
                        cmd = next;
                    }
                    Ok(next) => {
                        pending.push_back(next);
                        break;
                    }
                    Err(_) => break,
                }
            }
        }
        if matches!(cmd, Command::Seek(_) | Command::Step(_)) {
            at_eof = false;
        }
        if apply_command(&mut dec, cmd, &mut playing, shared)? {
            return Ok(());
        }
    }
}

/// `true`면 워커를 끝낸다.
fn apply_command(
    dec: &mut Decoder,
    cmd: Command,
    playing: &mut bool,
    shared: &Arc<Shared>,
) -> Result<bool, String> {
    match cmd {
        Command::Play(speed) => {
            *playing = true;
            shared.speed_bits.store(speed.to_bits(), Ordering::SeqCst);
        }
        Command::Pause => {
            *playing = false;
        }
        Command::Seek(target_ms) | Command::Step(target_ms) => {
            let preview = matches!(cmd, Command::Seek(_));
            seek_to_push(dec, shared, target_ms, preview)?;
            shared.eof.store(false, Ordering::SeqCst);
            let pending = target_ms.saturating_add(1);
            if shared
                .seek_pending
                .compare_exchange(pending, 0, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                shared.seek_result.store(pending, Ordering::SeqCst);
            }
            // 스텝은 목표 1프레임을 넣는 즉시 멈춘다.
            if matches!(cmd, Command::Step(_)) {
                *playing = false;
            }
        }
        Command::Stop => return Ok(true),
    }
    Ok(false)
}

fn clear_queue(shared: &Arc<Shared>) {
    let mut queue = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
    queue.clear();
    shared.queue_bytes.store(0, Ordering::SeqCst);
}

fn queue_full(shared: &Arc<Shared>) -> bool {
    shared.queue_bytes.load(Ordering::SeqCst) >= QUEUE_BYTES_MAX
}

/// 패킷을 읽어 프레임을 큐에 넣는다. EOF면 `true`다.
fn pump(dec: &mut Decoder, shared: &Arc<Shared>) -> Result<bool, String> {
    loop {
        let packet = dec
            .ictx
            .read_packet()
            .map_err(|e| format!("Failed to read packet: {e:?}"))?;
        let Some(mut pkt) = packet else {
            dec.ctx.send_packet(None).ok();
            return flush_decoder(dec, shared);
        };
        if pkt.stream_index != dec.stream_index as i32 {
            continue;
        }
        send_packet_lenient(&mut dec.ctx, &pkt);
        pkt.set_stream_index(-1);
        while let Ok(frame) = dec.ctx.receive_frame() {
            push_frame(dec, &frame, shared)?;
        }
        if queue_full(shared) {
            return Ok(false);
        }
    }
}

fn flush_decoder(dec: &mut Decoder, shared: &Arc<Shared>) -> Result<bool, String> {
    loop {
        match dec.ctx.receive_frame() {
            Ok(frame) => push_frame(dec, &frame, shared)?,
            Err(_) => return Ok(true),
        }
    }
}

fn push_frame(
    dec: &mut Decoder,
    frame: &rsmpeg::avutil::AVFrame,
    shared: &Arc<Shared>,
) -> Result<(), String> {
    let Some(pts_ms) = frame_time_ms(frame, dec.time_base, dec.origin_ms) else {
        return Ok(());
    };
    let (buf, width, height) = dec.scale_to_bgra(frame)?;
    let mut queue = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
    // 같은 위치 탐색 반복으로 큐가 무한히 불지 않게 상한을 둔다.
    while shared.queue_bytes.load(Ordering::SeqCst) + buf.len() as u64 > QUEUE_BYTES_MAX * 2 {
        if let Some(old) = queue.pop_front() {
            shared
                .queue_bytes
                .fetch_sub(old.bgra.len() as u64, Ordering::SeqCst);
        } else {
            break;
        }
    }
    shared
        .queue_bytes
        .fetch_add(buf.len() as u64, Ordering::SeqCst);
    queue.push_back(VideoFrame {
        width,
        height,
        bgra: buf,
        pts_ms,
    });
    Ok(())
}

/// 키프레임 기준 탐색 후 목표 지점까지 순방향 디코드해 목표 프레임을 큐에 넣는다.
/// `preview`면 첫 프레임(보통 키프레임)을 먼저 넣어 긴 GOP에서도 화면이 바로 반응한다.
fn seek_to_push(
    dec: &mut Decoder,
    shared: &Arc<Shared>,
    target_ms: u64,
    preview: bool,
) -> Result<(), String> {
    use rsmpeg::ffi;
    clear_queue(shared);
    if dec.time_base.1 <= 0 {
        return Err("Invalid time base".to_string());
    }
    let ts = seek_timestamp(target_ms, dec.time_base, dec.origin_ms);
    dec.ictx
        .seek(
            dec.stream_index as i32,
            ts,
            ffi::AVSEEK_FLAG_BACKWARD as i32,
        )
        .map_err(|e| format!("Failed to seek: {e:?}"))?;
    dec.ctx.flush_buffers();
    let mut previewed = false;
    loop {
        let packet = dec
            .ictx
            .read_packet()
            .map_err(|e| format!("Failed to read packet: {e:?}"))?;
        let Some(mut pkt) = packet else {
            dec.ctx.send_packet(None).ok();
            let _ = flush_decoder(dec, shared);
            return Ok(());
        };
        if pkt.stream_index != dec.stream_index as i32 {
            continue;
        }
        send_packet_lenient(&mut dec.ctx, &pkt);
        pkt.set_stream_index(-1);
        while let Ok(frame) = dec.ctx.receive_frame() {
            let pts = frame_time_ms(&frame, dec.time_base, dec.origin_ms).unwrap_or(u64::MAX);
            if pts >= target_ms {
                push_frame(dec, &frame, shared)?;
                return Ok(());
            }
            if preview && !previewed {
                push_frame(dec, &frame, shared)?;
                previewed = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn sample(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples")
            .join(name)
    }

    #[test]
    fn player_opens_and_reports_info() {
        let player = VideoPlayer::open(&sample("sample.mp4")).unwrap();
        let info = player.info();
        assert_eq!((info.width, info.height), (64, 48));
        assert!(info.duration_ms > 0);
        assert!(!info.has_audio);
        assert!(!player.is_playing());
    }

    #[test]
    fn samples_without_audio_have_no_audio_output() {
        let player = VideoPlayer::open(&sample("sample.mp4")).unwrap();
        assert!(!player.audio_active());
        // 없으면 조용히 무시된다.
        player.set_volume(0.5);
        assert_eq!(player.volume(), crate::video_audio::DEFAULT_VOLUME);
        player.set_muted(true);
        assert!(!player.is_muted());
    }

    #[test]
    fn player_decodes_first_frame() {
        let player = VideoPlayer::open(&sample("sample.mp4")).unwrap();
        player.play(1.0);
        assert!(player.is_playing());
        let mut frame = None;
        for _ in 0..500 {
            if let Ok(Some(f)) = player.poll_frame() {
                frame = Some(f);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        player.pause();
        let frame = frame.expect("no frame decoded");
        assert_eq!((frame.width, frame.height), (64, 48));
        assert_eq!(frame.bgra.len(), 64 * 48 * 4);
    }

    #[test]
    fn seek_clamps_to_duration() {
        let player = VideoPlayer::open(&sample("sample.mp4")).unwrap();
        let duration = player.info().duration_ms;
        player.seek(duration + 10_000);
        assert_eq!(player.position_ms(), duration);
        assert!(!player.is_playing());
    }

    #[test]
    fn seek_finishes_with_a_frame() {
        let player = VideoPlayer::open(&sample("sample.mp4")).unwrap();
        player.pause();
        player.seek(300);
        let mut frame = None;
        for _ in 0..200 {
            if let Ok(Some(next)) = player.poll_frame() {
                if next.pts_ms >= 300 {
                    frame = Some(next);
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!player.is_seeking());
        let frame = frame.expect("no frame after seek");
        assert!(frame.pts_ms >= 300, "pts {}", frame.pts_ms);
    }

    #[test]
    fn playback_clock_advances_in_milliseconds() {
        let player = VideoPlayer::open(&sample("sample.mp4")).unwrap();
        player.play(1.0);
        std::thread::sleep(std::time::Duration::from_millis(100));
        let position = player.position_ms();
        player.pause();
        assert!(
            (50..=600).contains(&position),
            "position should be milliseconds, got {position}"
        );
    }

    #[test]
    fn poll_frame_returns_new_frames_only() {
        let player = VideoPlayer::open(&sample("sample.mp4")).unwrap();
        player.play(1.0);
        let mut pts = Vec::new();
        for _ in 0..200 {
            if let Ok(Some(frame)) = player.poll_frame() {
                if pts.last() != Some(&frame.pts_ms) {
                    pts.push(frame.pts_ms);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        player.pause();
        assert!(pts.len() > 1, "expected several frames, got {pts:?}");
        assert!(pts.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn seek_back_after_decode_eof_keeps_playing() {
        // 디코드는 화면보다 앞서 끝난다. 그 뒤 재생 중에 뒤로 탐색해도
        // 목표 프레임 하나로 굳지 않고 계속 디코드해야 한다.
        let player = VideoPlayer::open(&sample("sample.mp4")).unwrap();
        player.play(1.0);
        for _ in 0..500 {
            if player.shared.eof.load(Ordering::SeqCst) {
                break;
            }
            let _ = player.poll_frame();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(
            player.shared.eof.load(Ordering::SeqCst),
            "never reached eof"
        );
        player.seek(0);
        player.play(1.0);
        let mut pts = Vec::new();
        for _ in 0..60 {
            if let Ok(Some(frame)) = player.poll_frame() {
                pts.push(frame.pts_ms);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        player.pause();
        assert!(pts.len() > 2, "decode stalled after seek: {pts:?}");
    }

    #[test]
    fn play_at_end_restarts_from_start() {
        let player = VideoPlayer::open(&sample("sample.mp4")).unwrap();
        let duration = player.info().duration_ms;
        player.seek(duration);
        assert_eq!(player.position_ms(), duration);
        player.play(1.0);
        assert!(player.is_playing());
        assert!(player.position_ms() < duration);
    }

    #[test]
    fn seek_timestamp_adds_container_origin() {
        // 1/90000 time_base(MPEG-TS), 시작 1400ms.
        assert_eq!(seek_timestamp(0, (1, 90_000), 1_400), 126_000);
        assert_eq!(seek_timestamp(1_000, (1, 1_000), 0), 1_000);
        assert_eq!(seek_timestamp(1_000, (0, 1_000), 0), 0);
    }

    #[test]
    fn output_is_capped_to_playback_size() {
        assert_eq!(output_dimensions(3840, 2160), (1920, 1080));
        assert_eq!(output_dimensions(2560, 1440), (1920, 1080));
        assert_eq!(output_dimensions(1920, 1080), (1920, 1080));
        assert_eq!(output_dimensions(640, 480), (640, 480));
        // 세로 영상도 긴 변 기준이다.
        assert_eq!(output_dimensions(1080, 1920), (1080, 1920));
        assert_eq!(output_dimensions(2160, 3840), (1080, 1920));
    }

    #[test]
    fn seek_and_step_move_position() {
        let player = VideoPlayer::open(&sample("sample.mp4")).unwrap();
        let duration = player.info().duration_ms;
        assert!(duration > 0);
        player.seek(duration / 2);
        assert_eq!(player.position_ms(), duration / 2);
        player.step(1);
        assert!(player.position_ms() >= duration / 2);
    }

    #[test]
    fn downscaled_playback_fills_bottom_rows() {
        // 2560x1440은 재생 출력이 1920x1080으로 축소된다. `src_slice_h`에
        // 목적지 높이를 넘기면 하단 절반이 검게 잘린다.
        let player = VideoPlayer::open(&sample("sample-1440p.mp4")).unwrap();
        assert_eq!((player.info().width, player.info().height), (2560, 1440));
        player.play(1.0);
        let mut frame = None;
        for _ in 0..500 {
            if let Ok(Some(f)) = player.poll_frame() {
                frame = Some(f);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        player.pause();
        let frame = frame.expect("no frame decoded");
        assert_eq!((frame.width, frame.height), (1920, 1080));
        assert_eq!(frame.bgra.len(), 1920 * 1080 * 4);
        let (w, h) = (frame.width as usize, frame.height as usize);
        let bottom = &frame.bgra[(h - 1) * w * 4..h * w * 4];
        assert!(
            bottom.iter().any(|&b| b != 0),
            "bottom row is all zero (swscale src_slice_h)"
        );
    }
}
