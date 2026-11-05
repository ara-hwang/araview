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
    /// 핸들이 버려지는 중이다. 워커는 탐색·디코드 루프 안에서도 이 값을 보고 곧바로
    /// 끝낸다. 명령 채널만 보면 긴 GOP 탐색이 끝날 때까지 `Drop`의 `join`이 막힌다.
    stop: AtomicBool,
}

/// 동영상 재생 핸들. 워커 스레드를 소유하고, 시계는 이쪽에서 잰다.
pub struct VideoPlayer {
    cmd_tx: mpsc::Sender<Command>,
    worker: Option<std::thread::JoinHandle<()>>,
    shared: Arc<Shared>,
    clock: Mutex<Clock>,
    info: PlayerInfo,
    audio: Option<crate::video_audio::AudioOutput>,
    /// 오디오가 오류로 멈춰 단조 클럭으로 넘어갔는가. 한 번 넘어가면 돌아오지 않는다.
    audio_failed: AtomicBool,
    /// 길이를 모르는 파일(`duration_ms == 0`)에서 끝까지 재생해 알아낸 길이. 0이면 아직 모른다.
    discovered_duration_ms: AtomicU64,
}

/// 읽기 오류가 이만큼 이어지면 파일 끝으로 본다. 손상 구간 하나는 건너뛰되,
/// 더 읽을 수 없는 파일(잘린 파일, 끊긴 네트워크 드라이브)에서 무한히 돌지 않게 한다.
const MAX_CONSECUTIVE_READ_ERRORS: u32 = 64;

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
            stop: AtomicBool::new(false),
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
            audio_failed: AtomicBool::new(false),
            discovered_duration_ms: AtomicU64::new(0),
        })
    }

    pub fn info(&self) -> &PlayerInfo {
        &self.info
    }

    /// 재생 길이(ms). 컨테이너에 길이가 없으면(MediaRecorder WebM 등) 끝까지 재생해
    /// 알아낸 길이를, 아직 모르면 0을 돌려준다.
    pub fn duration_ms(&self) -> u64 {
        if self.info.duration_ms > 0 {
            self.info.duration_ms
        } else {
            self.discovered_duration_ms.load(Ordering::SeqCst)
        }
    }

    /// 살아 있는 오디오 출력. 오디오 디코드가 오류로 멈췄으면 경고를 남기고 그 시점의
    /// 오디오 위치에서 단조 클럭으로 넘어간 뒤 `None`을 돌려준다(§7.6: 오디오를 쓸 수
    /// 없으면 영상만 재생한다). 호출자는 `clock` 잠금을 잡은 채 부르지 않는다.
    fn live_audio(&self) -> Option<&crate::video_audio::AudioOutput> {
        let audio = self.audio.as_ref()?;
        if self.audio_failed.load(Ordering::SeqCst) {
            return None;
        }
        let Some(error) = audio.has_error() else {
            return Some(audio);
        };
        if !self.audio_failed.swap(true, Ordering::SeqCst) {
            log::warn!("audio stopped, continuing video only: {error}");
            let played = audio.played_ms();
            audio.set_playing(false);
            let mut clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
            clock.base_ms = played;
            clock.base_at = Instant::now();
        }
        None
    }

    /// 마스터 클럭 위치(ms). 오디오가 살아 있으면 오디오 클럭, 아니면 단조 클럭이다.
    fn master_ms(&self) -> u64 {
        if let Some(audio) = self.live_audio() {
            return audio.played_ms();
        }
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

    pub fn position_ms(&self) -> u64 {
        let pos = self.master_ms();
        let mut duration = self.duration_ms();
        if duration == 0 {
            // 길이를 모르면 마지막 프레임을 지난 시점을 끝으로 본다. 그대로 두면 디코드가
            // 끝나도 클럭이 끝없이 늘고, 재생을 눌러도 처음으로 돌아가지 않는다.
            match self.decoded_end_ms() {
                Some(end) if pos >= end => {
                    self.discovered_duration_ms.store(end, Ordering::SeqCst);
                    duration = end;
                }
                _ => return pos,
            }
        }
        // 끝까지 가면 멈춘다. 유한 반복 GIF와 같은 규칙이다.
        if pos >= duration {
            if self.is_playing() {
                self.pause();
            }
            return duration;
        }
        pos
    }

    /// 디코드가 파일 끝에 닿고 큐가 비었으면 마지막 프레임이 끝나는 시각(ms).
    /// 길이를 모르는 파일의 끝 판정에 쓴다.
    fn decoded_end_ms(&self) -> Option<u64> {
        if !self.shared.eof.load(Ordering::SeqCst) || self.is_seeking() {
            return None;
        }
        let queue = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        if !queue.is_empty() {
            return None;
        }
        let last = self.shared.last_pts.load(Ordering::SeqCst);
        if last == 0 {
            return None;
        }
        Some((last - 1).saturating_add(self.frame_ms()))
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
        if let Some(audio) = self.live_audio() {
            audio.set_speed(speed);
        }
    }

    pub fn play(&self, speed: f32) {
        if !speed.is_finite() || speed <= 0.0 {
            return;
        }
        // 끝에서 다시 재생하면 처음부터 시작한다. 그대로 두면 위치가 이미 끝이라
        // `position_ms`가 곧바로 다시 멈춘다.
        let pos = self.position_ms();
        let duration = self.duration_ms();
        if duration > 0 && pos >= duration {
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
        if let Some(audio) = self.live_audio() {
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
        if let Some(audio) = self.live_audio() {
            audio.set_playing(false);
        }
        let _ = self.cmd_tx.send(Command::Pause);
    }

    pub fn seek(&self, position_ms: u64) {
        let duration = self.duration_ms();
        let target = if duration > 0 {
            position_ms.min(duration)
        } else {
            position_ms
        };
        {
            let mut clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
            clock.base_ms = target;
            clock.base_at = Instant::now();
        }
        {
            // 프레임 큐 읽기·쓰기와 탐색 장벽을 같은 잠금으로 직렬화한다. 새 목표가
            // 대기 중인 뒤에는 이전 위치 프레임이 큐에 남거나 소비되지 않는다.
            let mut queue = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
            queue.clear();
            self.shared.queue_bytes.store(0, Ordering::SeqCst);
            self.shared.eof.store(false, Ordering::SeqCst);
            self.shared.last_pts.store(0, Ordering::SeqCst);
            self.shared
                .seek_pending
                .store(target.saturating_add(1), Ordering::SeqCst);
            self.shared.seek_result.store(0, Ordering::SeqCst);
        }
        if let Some(audio) = self.live_audio() {
            audio.seek(target);
        }
        let _ = self.cmd_tx.send(Command::Seek(target));
    }

    /// 정지한 채 1프레임 이동한다. 재생 중이면 먼저 멈춘다.
    pub fn step(&self, direction: i32) {
        if direction == 0 {
            return;
        }
        let pos = self.position_ms();
        self.pause();
        let target = step_target(pos, self.info.fps, direction);
        let duration = self.duration_ms();
        let target = if duration > 0 {
            target.min(duration)
        } else {
            target
        };
        {
            let mut clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
            clock.base_ms = target;
            clock.base_at = Instant::now();
        }
        {
            let mut queue = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
            queue.clear();
            self.shared.queue_bytes.store(0, Ordering::SeqCst);
            self.shared.eof.store(false, Ordering::SeqCst);
            self.shared.last_pts.store(0, Ordering::SeqCst);
            self.shared
                .seek_pending
                .store(target.saturating_add(1), Ordering::SeqCst);
            self.shared.seek_result.store(0, Ordering::SeqCst);
        }
        if let Some(audio) = self.live_audio() {
            audio.seek(target);
        }
        let _ = self.cmd_tx.send(Command::Step(target));
    }

    /// 오디오 출력이 살아 있는가. 장치를 못 열었거나 오디오 디코드가 오류로 멈췄으면
    /// 영상만 재생한다.
    pub fn audio_active(&self) -> bool {
        self.live_audio().is_some()
    }

    /// 오디오 콜백이 선행 버퍼 부족으로 소리를 0으로 메운 횟수. 진단용이다.
    pub fn audio_underruns(&self) -> u64 {
        self.audio
            .as_ref()
            .map(|audio| audio.underruns())
            .unwrap_or(0)
    }

    /// 마지막 오디오 언더런 시점의 (지역 버퍼, 공유 큐) 깊이(샘플 수). 진단용이다.
    pub fn audio_last_underrun(&self) -> (u64, u64) {
        self.audio
            .as_ref()
            .map(|audio| audio.last_underrun_depths())
            .unwrap_or((0, 0))
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
        // 오디오 오류는 여기서 올리지 않는다. `position_ms`가 단조 클럭으로 넘어가고
        // 영상은 계속 재생된다.
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

    /// 영상 디코드 오류. 오디오 오류는 영상을 닫지 않으므로 포함하지 않는다.
    pub fn has_error(&self) -> Option<String> {
        self.shared
            .error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// 한 프레임 길이(ms, 올림). 길이를 모르는 파일의 끝 판정에 쓴다.
    fn frame_ms(&self) -> u64 {
        match self.info.fps {
            Some(fps) if fps.is_finite() && fps > 0.0 => (1000.0 / fps).ceil() as u64,
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
        let duration = self.duration_ms();
        if duration == 0 {
            pos
        } else {
            pos.min(duration)
        }
    }
}

/// 1프레임 스텝의 목표 위치(ms). 현재 위치가 든 프레임의 다음(앞) 프레임 시작 시각을
/// 올림해 돌려준다. 프레임 길이를 정수 ms로 잘라 더하면 33.33ms 같은 길이에서 목표가
/// 다음 프레임에 못 미쳐 같은 프레임이 반복된다.
fn step_target(pos_ms: u64, fps: Option<f64>, direction: i32) -> u64 {
    let fps = match fps {
        Some(fps) if fps.is_finite() && fps > 0.0 => fps,
        _ => 25.0,
    };
    let frame = 1000.0 / fps;
    // 부동소수 오차로 프레임 시작 시각이 바로 앞 프레임으로 읽히지 않게 조금 더한다.
    let index = ((pos_ms as f64 + 1e-6) / frame).floor();
    let next = if direction > 0 {
        index + 1.0
    } else {
        index - 1.0
    };
    if next <= 0.0 {
        return 0;
    }
    // 1000/30*15 = 500.0000001처럼 정수 시각이 올림으로 1ms 밀리지 않게 조금 뺀다.
    (next * frame - 1e-6).ceil() as u64
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        // 탐색·디코드 루프가 먼저 멈추도록 플래그를 세운 뒤 명령을 보낸다.
        self.shared.stop.store(true, Ordering::SeqCst);
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
    let ms = frame_time_ms_signed(frame, time_base, origin_ms)?;
    if ms >= 0.0 {
        Some(ms as u64)
    } else {
        None
    }
}

/// 부호 있는 프레임 시각(ms, 컨테이너 시작 기준). 시작보다 이른 프레임은 음수다.
/// 오디오가 탐색 목표에 걸친 프레임의 앞부분을 잘라낼 때 쓴다.
pub(crate) fn frame_time_ms_signed(
    frame: &rsmpeg::avutil::AVFrame,
    time_base: (i32, i32),
    origin_ms: i64,
) -> Option<f64> {
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
    ms.is_finite().then_some(ms)
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

/// 패킷 하나를 읽는다. 파일 끝이면 `None`이다. 손상된 구간에서 나는 읽기 오류는
/// 그 패킷만 건너뛰고 다음 패킷을 읽는다(§7.6). 오류가 `MAX_CONSECUTIVE_READ_ERRORS`번
/// 이어지면 더 읽을 수 없는 것으로 보고 파일 끝처럼 `None`을 돌려준다. 그러면
/// 이미 디코드한 데까지는 재생되고, 재생 전체가 오류로 닫히지 않는다.
pub(crate) fn read_packet_lenient(
    ictx: &mut rsmpeg::avformat::AVFormatContextInput,
    cancelled: impl Fn() -> bool,
) -> Option<rsmpeg::avcodec::AVPacket> {
    let mut errors = 0u32;
    loop {
        match ictx.read_packet() {
            Ok(packet) => return packet,
            Err(error) => {
                if cancelled() {
                    return None;
                }
                if error.raw_error() == Some(rsmpeg::ffi::AVERROR(rsmpeg::ffi::EAGAIN)) {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    continue;
                }
                errors += 1;
                if errors >= MAX_CONSECUTIVE_READ_ERRORS {
                    log::warn!("stopping at unreadable data after {errors} read errors: {error:?}");
                    return None;
                }
                log::debug!("skipping unreadable packet: {error:?}");
            }
        }
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
            let completed = complete_seek(shared, target_ms);
            // 스텝이 최신 목표로 완료된 경우에만 멈춘다. 더 새 탐색이 시작됐다면
            // 취소된 스텝이 재생 상태를 덮지 않게 한다.
            if completed && matches!(cmd, Command::Step(_)) {
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
        if decode_interrupted(shared) {
            return Ok(false);
        }
        let packet = read_packet_lenient(&mut dec.ictx, || decode_interrupted(shared));
        if decode_interrupted(shared) {
            return Ok(false);
        }
        let Some(mut pkt) = packet else {
            dec.ctx.send_packet(None).ok();
            return flush_decoder(dec, shared, None);
        };
        if pkt.stream_index != dec.stream_index as i32 {
            continue;
        }
        send_packet_lenient(&mut dec.ctx, &pkt);
        pkt.set_stream_index(-1);
        while let Ok(frame) = dec.ctx.receive_frame() {
            if decode_interrupted(shared) {
                return Ok(false);
            }
            push_frame(dec, &frame, shared, None)?;
        }
        if queue_full(shared) {
            return Ok(false);
        }
    }
}

fn flush_decoder(
    dec: &mut Decoder,
    shared: &Arc<Shared>,
    seek_target: Option<u64>,
) -> Result<bool, String> {
    loop {
        let current = match seek_target {
            Some(target) => seek_is_current(shared, target),
            None => !decode_interrupted(shared),
        };
        if !current {
            return Ok(false);
        }
        match dec.ctx.receive_frame() {
            Ok(frame) => push_frame(dec, &frame, shared, seek_target)?,
            Err(_) => return Ok(true),
        }
    }
}

fn push_frame(
    dec: &mut Decoder,
    frame: &rsmpeg::avutil::AVFrame,
    shared: &Arc<Shared>,
    seek_target: Option<u64>,
) -> Result<(), String> {
    let Some(pts_ms) = frame_time_ms(frame, dec.time_base, dec.origin_ms) else {
        return Ok(());
    };
    let (buf, width, height) = dec.scale_to_bgra(frame)?;
    queue_video_frame(
        shared,
        VideoFrame {
            width,
            height,
            bgra: buf,
            pts_ms,
        },
        seek_target,
    );
    Ok(())
}

/// 현재 탐색의 프리뷰/결과 프레임이거나 탐색 중이 아닐 때만 프레임을 큐에 넣는다.
fn queue_video_frame(shared: &Shared, frame: VideoFrame, seek_target: Option<u64>) -> bool {
    let mut queue = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
    let current = match seek_target {
        Some(target) => seek_is_current(shared, target),
        None => !decode_interrupted(shared),
    };
    if !current {
        return false;
    }
    // 같은 위치 탐색 반복으로 큐가 무한히 불지 않게 상한을 둔다.
    while shared.queue_bytes.load(Ordering::SeqCst) + frame.bgra.len() as u64 > QUEUE_BYTES_MAX * 2
    {
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
        .fetch_add(frame.bgra.len() as u64, Ordering::SeqCst);
    queue.push_back(frame);
    true
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
    if !seek_is_current(shared, target_ms) {
        return Ok(());
    }
    clear_queue(shared);
    if dec.time_base.1 <= 0 {
        return Err("Invalid time base".to_string());
    }
    let ts = seek_timestamp(target_ms, dec.time_base, dec.origin_ms);
    if let Err(error) = dec.ictx.seek(
        dec.stream_index as i32,
        ts,
        ffi::AVSEEK_FLAG_BACKWARD as i32,
    ) {
        // 탐색할 수 없는 구간이면 지금 위치에서 이어 디코드한다. 재생 전체를 닫지 않는다.
        log::warn!(
            "video seek to {target_ms}ms failed, decoding from the current position: {error:?}"
        );
    }
    dec.ctx.flush_buffers();
    let mut previewed = false;
    loop {
        if !seek_is_current(shared, target_ms) {
            return Ok(());
        }
        let packet = read_packet_lenient(&mut dec.ictx, || !seek_is_current(shared, target_ms));
        if !seek_is_current(shared, target_ms) {
            return Ok(());
        }
        let Some(mut pkt) = packet else {
            dec.ctx.send_packet(None).ok();
            let _ = flush_decoder(dec, shared, Some(target_ms));
            return Ok(());
        };
        if pkt.stream_index != dec.stream_index as i32 {
            continue;
        }
        send_packet_lenient(&mut dec.ctx, &pkt);
        pkt.set_stream_index(-1);
        while let Ok(frame) = dec.ctx.receive_frame() {
            if !seek_is_current(shared, target_ms) {
                return Ok(());
            }
            let pts = frame_time_ms(&frame, dec.time_base, dec.origin_ms).unwrap_or(u64::MAX);
            if pts >= target_ms {
                push_frame(dec, &frame, shared, Some(target_ms))?;
                return Ok(());
            }
            if preview && !previewed {
                push_frame(dec, &frame, shared, Some(target_ms))?;
                previewed = true;
            }
        }
    }
}

fn seek_is_current(shared: &Shared, target_ms: u64) -> bool {
    !shared.stop.load(Ordering::SeqCst)
        && shared.seek_pending.load(Ordering::SeqCst) == target_ms.saturating_add(1)
}

/// 일반 디코드(탐색 아님)를 멈춰야 하는가. 새 탐색이 걸렸거나 핸들이 버려지는 중이다.
fn decode_interrupted(shared: &Shared) -> bool {
    shared.stop.load(Ordering::SeqCst) || shared.seek_pending.load(Ordering::SeqCst) != 0
}

/// 탐색 완료와 결과 기록을 큐 잠금으로 직렬화해 새 탐색이 이전 결과를 덮지 않게 한다.
fn complete_seek(shared: &Shared, target_ms: u64) -> bool {
    let _queue = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
    let pending = target_ms.saturating_add(1);
    if shared
        .seek_pending
        .compare_exchange(pending, 0, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        shared.seek_result.store(pending, Ordering::SeqCst);
        true
    } else {
        false
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
    fn pending_seek_rejects_stale_frames_but_allows_its_preview() {
        let player = VideoPlayer::open(&sample("sample.mp4")).unwrap();
        player.shared.seek_pending.store(501, Ordering::SeqCst);
        let frame = |pts_ms| VideoFrame {
            width: 1,
            height: 1,
            bgra: vec![0; 4],
            pts_ms,
        };
        assert!(!queue_video_frame(&player.shared, frame(100), None));
        assert!(!queue_video_frame(&player.shared, frame(100), Some(300)));
        assert!(queue_video_frame(&player.shared, frame(100), Some(500)));
        let preview = player.poll_frame().unwrap().expect("current seek preview");
        assert_eq!(preview.pts_ms, 100);
    }

    #[test]
    fn seek_cancellation_uses_latest_pending_target() {
        let player = VideoPlayer::open(&sample("sample.mp4")).unwrap();
        player.shared.seek_pending.store(801, Ordering::SeqCst);
        assert!(!seek_is_current(&player.shared, 500));
        assert!(seek_is_current(&player.shared, 800));
        assert!(!complete_seek(&player.shared, 500));
        assert_eq!(player.shared.seek_pending.load(Ordering::SeqCst), 801);
        assert_eq!(player.shared.seek_result.load(Ordering::SeqCst), 0);
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

    #[test]
    fn audio_error_falls_back_to_the_monotonic_clock() {
        // 오디오 디코드가 오류로 멈춰도 영상은 닫히지 않고, 오디오가 낸 위치에서
        // 단조 클럭으로 이어 간다.
        let mut player = VideoPlayer::open(&sample("sample.mp4")).unwrap();
        let audio = crate::video_audio::AudioOutput::open_with(
            &sample("sample-audio.mp4"),
            crate::video_audio::PREFERRED_RATE,
            crate::video_audio::AUDIO_CHANNELS as u16,
            cpal::SampleFormat::F32,
        )
        .unwrap();
        player.audio = Some(audio);
        assert!(
            player.audio_active(),
            "healthy audio should stay the master clock"
        );
        if let Some(audio) = &player.audio {
            audio.inject_error_for_test(200, "Failed to resample audio: test");
        }
        assert!(player.poll_frame().is_ok());
        assert_eq!(player.has_error(), None);
        assert!(!player.audio_active());
        assert_eq!(player.position_ms(), 200);
        player.play(1.0);
        std::thread::sleep(std::time::Duration::from_millis(100));
        let position = player.position_ms();
        player.pause();
        assert!(
            (250..=player.duration_ms()).contains(&position),
            "monotonic clock should continue from 200ms, got {position}"
        );
    }

    #[test]
    fn unknown_duration_ends_after_the_last_frame() {
        // MediaRecorder WebM처럼 길이가 없는 파일. 디코드가 끝나면 마지막 프레임 뒤를
        // 끝으로 보고 멈추며, 다시 재생하면 처음부터 시작한다.
        let player = VideoPlayer::open(&sample("sample-no-duration.webm")).unwrap();
        assert_eq!(player.info().duration_ms, 0);
        assert_eq!(player.duration_ms(), 0);
        player.play(2.0);
        for _ in 0..600 {
            let _ = player.poll_frame();
            if !player.is_playing() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!player.is_playing(), "playback never ended");
        let duration = player.duration_ms();
        assert!((1_900..=2_100).contains(&duration), "duration {duration}");
        assert_eq!(player.position_ms(), duration);
        player.play(1.0);
        assert!(player.position_ms() < duration);
        player.pause();
    }

    #[test]
    fn stepping_reaches_every_frame_at_fractional_frame_lengths() {
        // 30fps는 프레임 길이가 33.33ms다. 매 스텝이 다음 프레임 시작을 지나야 한다.
        let fps = Some(30.0);
        let mut pos = 0;
        for index in 1..=90u64 {
            pos = step_target(pos, fps, 1);
            let frame_start = (index * 1000).div_ceil(30);
            assert_eq!(pos, frame_start, "step {index}");
        }
        for index in (0..90u64).rev() {
            pos = step_target(pos, fps, -1);
            let frame_start = (index * 1000).div_ceil(30);
            assert_eq!(pos, frame_start, "back step to {index}");
        }
        assert_eq!(step_target(0, fps, -1), 0);
        assert_eq!(step_target(0, Some(25.0), 1), 40);
    }

    #[test]
    fn stopping_cancels_seek_and_decode_loops() {
        let player = VideoPlayer::open(&sample("sample.mp4")).unwrap();
        player.shared.seek_pending.store(801, Ordering::SeqCst);
        assert!(seek_is_current(&player.shared, 800));
        player.shared.stop.store(true, Ordering::SeqCst);
        assert!(!seek_is_current(&player.shared, 800));
        player.shared.seek_pending.store(0, Ordering::SeqCst);
        assert!(decode_interrupted(&player.shared));
    }

    /// 메모리의 파일을 읽되, 절반 지점에서 `fail`번 읽기 오류(EIO)를 내는 입력.
    fn flaky_input(name: &str, fail: u32) -> rsmpeg::avformat::AVFormatContextInput {
        use rsmpeg::avformat::{AVIOContextContainer, AVIOContextCustom};
        let data = std::fs::read(sample(name)).unwrap();
        let total = data.len();
        let mut pos = 0usize;
        let mut failed = 0u32;
        let read: rsmpeg::avformat::ReadPacketCallback = Box::new(move |_, buf| {
            if pos >= total / 2 && failed < fail {
                failed += 1;
                return rsmpeg::ffi::AVERROR(rsmpeg::ffi::EIO);
            }
            if pos >= total {
                return rsmpeg::ffi::AVERROR_EOF;
            }
            let n = buf.len().min(total - pos).min(512);
            buf[..n].copy_from_slice(&data[pos..pos + n]);
            pos += n;
            n as i32
        });
        let io = AVIOContextCustom::alloc_context(
            rsmpeg::avutil::AVMem::new(512),
            false,
            Vec::new(),
            Some(read),
            None,
            None,
        );
        rsmpeg::avformat::AVFormatContextInput::from_io_context(AVIOContextContainer::Custom(io))
            .unwrap()
    }

    #[test]
    fn read_errors_do_not_fail_the_stream() {
        // 읽기 오류(EIO)는 예전에는 `?`로 워커를 끝내 영상 전체를 닫았다. 이제는 건너뛰고,
        // 더 읽을 수 없으면 그때까지 읽은 데서 파일 끝처럼 끝난다.
        let name = "sample-late-audio.mkv";
        let lenient = |mut ictx: rsmpeg::avformat::AVFormatContextInput| {
            let mut packets = 0;
            while read_packet_lenient(&mut ictx, || false).is_some() {
                packets += 1;
            }
            packets
        };
        let intact = lenient(flaky_input(name, 0));
        let flaky = lenient(flaky_input(name, 1));
        let (strict, strict_error) = {
            let mut ictx = flaky_input(name, 1);
            let mut packets = 0;
            let error = loop {
                match ictx.read_packet() {
                    Ok(Some(_)) => packets += 1,
                    Ok(None) => break false,
                    Err(_) => break true,
                }
            };
            (packets, error)
        };
        assert!(strict_error, "the flaky input should surface a read error");
        assert!(strict < intact);
        assert!(
            flaky >= strict,
            "lenient read lost packets: {flaky} < {strict}"
        );
    }
}
