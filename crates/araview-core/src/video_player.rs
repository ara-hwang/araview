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
/// 프레임 큐 바이트 상한. 4K면 2프레임, 1080p면 8프레임 수준이다.
const QUEUE_BYTES_MAX: u64 = 64 * 1024 * 1024;
/// 디코드 허용 픽셀 수. 표시 경로(`display.rs`)와 같은 값이다.
const MAX_DECODE_PIXELS: u64 = 100_000_000;
/// 자막 큐 상한. 비정상 파일을 막는다.
const MAX_CUES: usize = 20_000;
/// 자막 추출 패킷 상한. 비정상 파일을 막는다.
const MAX_SUBTITLE_PACKETS: usize = 200_000;

/// RGBA 한 프레임. UI는 이를 `RenderImage` 한 장으로 그린다.
#[derive(Debug, Clone)]
pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub pts_ms: u64,
}

/// 자막 한 구간. ASS/SRT 서식은 걷어내고 일반 텍스트만 둔다.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleCue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

/// `open` 시점에 확정한 스트림 정보.
#[derive(Debug, Clone)]
pub struct PlayerInfo {
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub fps: Option<f64>,
    pub has_audio: bool,
    pub has_subtitles: bool,
}

enum Command {
    Play(f32),
    Pause,
    Seek(u64),
    Step(u64),
    Stop,
}

struct Shared {
    queue: Mutex<VecDeque<RgbaFrame>>,
    queue_bytes: AtomicU64,
    error: Mutex<Option<String>>,
    eof: AtomicBool,
    speed_bits: AtomicU32,
}

/// 동영상 재생 핸들. 워커 스레드를 소유하고, 시계는 이쪽에서 잰다.
pub struct VideoPlayer {
    cmd_tx: mpsc::Sender<Command>,
    worker: Option<std::thread::JoinHandle<()>>,
    shared: Arc<Shared>,
    clock: Mutex<Clock>,
    info: PlayerInfo,
    subtitles: Vec<SubtitleCue>,
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
        let subtitles = list_subtitle_cues(path)?;
        let info = PlayerInfo {
            width,
            height,
            duration_ms: probed.duration_ms.unwrap_or(0),
            fps: probed.fps,
            has_audio: probed.has_audio,
            has_subtitles: !subtitles.is_empty(),
        };
        let shared = Arc::new(Shared {
            queue: Mutex::new(VecDeque::new()),
            queue_bytes: AtomicU64::new(0),
            error: Mutex::new(None),
            eof: AtomicBool::new(false),
            speed_bits: AtomicU32::new(1.0f32.to_bits()),
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
            subtitles,
            audio,
        })
    }

    pub fn info(&self) -> &PlayerInfo {
        &self.info
    }

    pub fn subtitles(&self) -> &[SubtitleCue] {
        &self.subtitles
    }

    /// 현재 위치의 자막 텍스트. 없으면 `None`이다.
    pub fn cue_at(&self, position_ms: u64) -> Option<&str> {
        self.subtitles
            .iter()
            .find(|c| c.start_ms <= position_ms && position_ms < c.end_ms)
            .map(|c| c.text.as_str())
    }

    pub fn position_ms(&self) -> u64 {
        if self.info.duration_ms == 0 {
            return match &self.audio {
                Some(audio) => audio.played_ms(),
                None => {
                    let clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
                    if clock.playing {
                        let elapsed = clock.base_at.elapsed().as_secs_f64();
                        clock
                            .base_ms
                            .saturating_add((elapsed * f64::from(clock.speed)) as u64)
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
                        .saturating_add((elapsed * f64::from(clock.speed)) as u64)
                } else {
                    clock.base_ms
                }
            }
        };
        pos = pos.min(self.info.duration_ms);
        // 끝까지 가면 멈춘다. 유한 반복 GIF와 같은 규칙이다.
        if pos >= self.info.duration_ms {
            self.pause();
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
        let target = position_ms.min(self.info.duration_ms.max(position_ms));
        {
            let mut clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
            clock.base_ms = target;
            clock.base_at = Instant::now();
        }
        self.shared.eof.store(false, Ordering::SeqCst);
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
        let target = target.min(self.info.duration_ms.max(target));
        {
            let mut clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
            clock.base_ms = target;
            clock.base_at = Instant::now();
        }
        self.shared.eof.store(false, Ordering::SeqCst);
        if let Some(audio) = &self.audio {
            audio.seek(target);
        }
        let _ = self.cmd_tx.send(Command::Step(target));
    }

    /// 오디오 출력이 살아 있는가. 장치를 못 열면 영상만 재생한다.
    pub fn audio_active(&self) -> bool {
        self.audio.is_some()
    }

    /// cpal 출력 스트림을 UI 스레드에서 열어 돌려준다.
    /// `AudioStream`을 들고 있는 동안 소리가 난다.
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

    /// 현재 위치의 프레임을 돌려준다. 오래된 큐 프레임은 버린다.
    pub fn poll_frame(&self) -> Result<Option<RgbaFrame>, AppError> {
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
        while queue.len() > 1 {
            let next_pts = queue[1].pts_ms;
            if next_pts <= pos {
                if let Some(old) = queue.pop_front() {
                    self.shared
                        .queue_bytes
                        .fetch_sub(old.rgba.len() as u64, Ordering::SeqCst);
                }
            } else {
                break;
            }
        }
        Ok(queue.front().cloned())
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
                .saturating_add((elapsed * f64::from(clock.speed)) as u64)
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
    stream_index: usize,
    time_base: (i32, i32),
    width: u32,
    height: u32,
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
    ctx.open(None)
        .map_err(|e| format!("Failed to open decoder: {e:?}"))?;
    Ok(Decoder {
        ictx,
        ctx,
        scaler: None,
        stream_index,
        time_base,
        width: width as u32,
        height: height as u32,
    })
}

impl Decoder {
    fn scale_to_rgba(&mut self, frame: &rsmpeg::avutil::AVFrame) -> Result<Vec<u8>, String> {
        use rsmpeg::ffi;
        let width = frame.width;
        let height = frame.height;
        if width <= 0 || height <= 0 {
            return Err("Invalid video frame".to_string());
        }
        let format = frame.format;
        if self.scaler.is_none() {
            self.scaler = rsmpeg::swscale::SwsContext::get_context(
                width,
                height,
                format,
                width,
                height,
                ffi::AV_PIX_FMT_RGBA,
                ffi::SWS_BILINEAR,
                None,
                None,
                None,
            );
        }
        let scaler = self
            .scaler
            .as_mut()
            .ok_or_else(|| "Failed to create scaler".to_string())?;
        let image = rsmpeg::avutil::AVImage::new(ffi::AV_PIX_FMT_RGBA, width, height, 1)
            .ok_or_else(|| "Failed to allocate frame".to_string())?;
        let mut framed = rsmpeg::avutil::AVFrameWithImage::new(image);
        scaler
            .scale_frame(frame, 0, height, &mut framed)
            .map_err(|e| format!("Failed to scale frame: {e:?}"))?;
        let mut buf = vec![0u8; framed.image_get_buffer_size(1).unwrap_or(0)];
        framed
            .image_copy_to_buffer(&mut buf, 1)
            .map_err(|e| format!("Failed to copy frame: {e:?}"))?;
        Ok(buf)
    }
}

fn worker_loop(
    source: &Path,
    cmd_rx: &mpsc::Receiver<Command>,
    shared: &Arc<Shared>,
) -> Result<(), String> {
    let mut dec = open_decoder(source)?;
    let mut playing = false;
    loop {
        // 정지 중에는 명령을 기다린다. 재생 중에는 큐가 차면 잠시 쉰다.
        if !playing {
            match cmd_rx.recv() {
                Ok(cmd) => {
                    if apply_command(&mut dec, cmd, &mut playing, shared)? {
                        return Ok(());
                    }
                    continue;
                }
                Err(_) => return Ok(()),
            }
        }
        while let Ok(cmd) = cmd_rx.try_recv() {
            if apply_command(&mut dec, cmd, &mut playing, shared)? {
                return Ok(());
            }
        }
        if queue_full(shared) {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }
        if pump(&mut dec, shared)? {
            shared.eof.store(true, Ordering::SeqCst);
            playing = false;
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
            seek_to_push(dec, shared, target_ms)?;
            shared.eof.store(false, Ordering::SeqCst);
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
        dec.ctx
            .send_packet(Some(&pkt))
            .map_err(|e| format!("Failed to decode video: {e:?}"))?;
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
    let Some(pts_ms) = frame_ms_of(frame, dec.time_base) else {
        return Ok(());
    };
    let buf = dec.scale_to_rgba(frame)?;
    let mut queue = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
    // 같은 위치 탐색 반복으로 큐가 무한히 불지 않게 상한을 둔다.
    while shared.queue_bytes.load(Ordering::SeqCst) + buf.len() as u64 > QUEUE_BYTES_MAX * 2 {
        if let Some(old) = queue.pop_front() {
            shared
                .queue_bytes
                .fetch_sub(old.rgba.len() as u64, Ordering::SeqCst);
        } else {
            break;
        }
    }
    shared
        .queue_bytes
        .fetch_add(buf.len() as u64, Ordering::SeqCst);
    queue.push_back(RgbaFrame {
        width: dec.width,
        height: dec.height,
        rgba: buf,
        pts_ms,
    });
    Ok(())
}

fn frame_ms_of(frame: &rsmpeg::avutil::AVFrame, time_base: (i32, i32)) -> Option<u64> {
    const NOPTS: i64 = i64::MIN;
    let pts: i64 = frame.pts;
    if pts == NOPTS || pts < 0 || time_base.1 <= 0 {
        return None;
    }
    let ms = pts as f64 * 1000.0 * time_base.0 as f64 / time_base.1 as f64;
    if ms.is_finite() && ms >= 0.0 {
        Some(ms as u64)
    } else {
        None
    }
}

/// 키프레임 기준 탐색 후 목표 지점까지 순방향 디코드해 목표 프레임을 큐에 넣는다.
fn seek_to_push(dec: &mut Decoder, shared: &Arc<Shared>, target_ms: u64) -> Result<(), String> {
    use rsmpeg::ffi;
    clear_queue(shared);
    let (num, den) = dec.time_base;
    if den <= 0 {
        return Err("Invalid time base".to_string());
    }
    let ts = (target_ms as f64 * den as f64 / (1000.0 * num as f64)) as i64;
    dec.ictx
        .seek(
            dec.stream_index as i32,
            ts.max(0),
            ffi::AVSEEK_FLAG_BACKWARD as i32,
        )
        .map_err(|e| format!("Failed to seek: {e:?}"))?;
    dec.ctx.flush_buffers();
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
        dec.ctx
            .send_packet(Some(&pkt))
            .map_err(|e| format!("Failed to decode video: {e:?}"))?;
        pkt.set_stream_index(-1);
        while let Ok(frame) = dec.ctx.receive_frame() {
            let pts = frame_ms_of(&frame, dec.time_base).unwrap_or(u64::MAX);
            if pts >= target_ms {
                push_frame(dec, &frame, shared)?;
                return Ok(());
            }
        }
    }
}

/// 내장 자막 트랙을 모두 읽어 구간 목록으로 돌려준다. 없으면 빈 목록이다.
pub fn list_subtitle_cues(path: &Path) -> Result<Vec<SubtitleCue>, AppError> {
    use rsmpeg::ffi;
    if crate::video::probe(path).is_err() {
        return Err(AppError::invalid_input("Not a video file"));
    }
    let cpath = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
        .map_err(|_| AppError::invalid_input("Invalid file path"))?;
    let mut ictx = rsmpeg::avformat::AVFormatContextInput::open(&cpath)
        .map_err(|e| AppError::corrupt(format!("Failed to open video: {e:?}")))?;
    let found = ictx
        .find_best_stream(ffi::AVMEDIA_TYPE_SUBTITLE)
        .map_err(|e| AppError::corrupt(format!("Failed to find subtitle stream: {e:?}")))?;
    let Some((stream_index, decoder)) = found else {
        return Ok(Vec::new());
    };
    let mut ctx = rsmpeg::avcodec::AVCodecContext::new(&decoder);
    {
        let stream = &ictx.streams()[stream_index];
        ctx.apply_codecpar(&stream.codecpar())
            .map_err(|e| AppError::corrupt(format!("Failed to open decoder: {e:?}")))?;
    }
    ctx.open(None)
        .map_err(|_| AppError::unsupported("Unsupported subtitle codec"))?;
    let mut cues = Vec::new();
    let mut packets = 0usize;
    while packets < MAX_SUBTITLE_PACKETS {
        let Ok(Some(mut pkt)) = ictx.read_packet() else {
            break;
        };
        packets += 1;
        if pkt.stream_index != stream_index as i32 {
            continue;
        }
        match ctx.decode_subtitle(Some(&mut pkt)) {
            Ok(Some(sub)) => {
                if let Some(cue) = cue_of(&sub) {
                    if !cue.text.is_empty() {
                        cues.push(cue);
                    }
                    if cues.len() >= MAX_CUES {
                        break;
                    }
                }
            }
            Ok(None) => {}
            Err(_) => break,
        }
    }
    cues.sort_by_key(|c| (c.start_ms, c.end_ms));
    cues.dedup();
    Ok(cues)
}

fn cue_of(sub: &rsmpeg::avcodec::AVSubtitle) -> Option<SubtitleCue> {
    // SAFETY: `decode_subtitle`이 채운 유효한 AVSubtitle이며, Drop이 해제한다.
    // `num_rects`/`rects` 범위 안에서만 읽고, `text`/`ass`는 NUL 종료 C 문자열이다.
    unsafe {
        let start_ms = sub.start_display_time as u64;
        let end_ms = sub.end_display_time as u64;
        if end_ms <= start_ms {
            return None;
        }
        let mut texts = Vec::new();
        let n = (sub.num_rects as usize).min(64);
        for i in 0..n {
            let rect = *sub.rects.add(i);
            if rect.is_null() {
                continue;
            }
            let r = &*rect;
            if !r.text.is_null() {
                let s = std::ffi::CStr::from_ptr(r.text)
                    .to_string_lossy()
                    .into_owned();
                let s = strip_ass(&s);
                if !s.trim().is_empty() {
                    texts.push(s);
                }
            } else if !r.ass.is_null() {
                let s = std::ffi::CStr::from_ptr(r.ass)
                    .to_string_lossy()
                    .into_owned();
                let s = ass_dialogue_text(&s);
                if !s.trim().is_empty() {
                    texts.push(s);
                }
            }
        }
        if texts.is_empty() {
            return None;
        }
        Some(SubtitleCue {
            start_ms,
            end_ms,
            text: texts.join("\n"),
        })
    }
}

/// ASS `Dialogue:` 행에서 실제 대사를 꺼내고 override 블록(`{...}`)을 걷는다.
fn ass_dialogue_text(ass: &str) -> String {
    let mut out = Vec::new();
    for line in ass.lines() {
        let line = line.trim();
        if !line.to_lowercase().starts_with("dialogue:") {
            continue;
        }
        // Dialogue: Layer,Start,End,Style,Name,ML,MR,ME,Effect,Text (콤마 9개 뒤가 본문)
        let mut parts = line.splitn(10, ',');
        let text = parts.nth(9).unwrap_or("").trim();
        if text.is_empty() {
            continue;
        }
        out.push(strip_ass(text));
    }
    out.join("\n")
}

/// `{...}` 블록을 걷고 `\N`·`\h`를 공백으로 푼다.
fn strip_ass(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0usize;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '\\' if depth == 0 => match chars.peek() {
                Some('N') | Some('n') => {
                    chars.next();
                    out.push(' ');
                }
                Some('h') => {
                    chars.next();
                    out.push(' ');
                }
                _ => out.push(c),
            },
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
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
        assert!(!info.has_subtitles);
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
        assert_eq!(frame.rgba.len(), 64 * 48 * 4);
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
    fn samples_have_no_subtitles() {
        for name in [
            "sample.mp4",
            "sample.mov",
            "sample.webm",
            "sample.mkv",
            "sample.avi",
        ] {
            let cues = list_subtitle_cues(&sample(name)).unwrap();
            assert!(cues.is_empty(), "{name}");
        }
    }

    #[test]
    fn ass_helpers_strip_formatting() {
        assert_eq!(strip_ass("{\\an8}Hello\\NWorld"), "Hello World");
        let ass = "Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,{\\i1}Hi\\Nthere";
        assert_eq!(ass_dialogue_text(ass), "Hi there");
    }
}
