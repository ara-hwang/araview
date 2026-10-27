//! 동영상 오디오 디코드·출력 (M2, §7.6).
//!
//! 디코드 워커가 오디오를 스테레오 f32로 리샘플링해 공유 큐에 넣고, cpal 출력
//! 콜백이 꺼내 재생한다. 오디오가 있으면 오디오 클럭이 마스터다. 장치를 열 수
//! 없으면 호출자가 영상만 재생하도록 `Err`를 돌려준다.
//!
//! 콜백은 실시간 스레드다. 공유 큐 잠금을 기다리면 데드라인을 놓쳐 소리가
//! 깨지므로, 콜백은 지역 버퍼(200ms)를 들고 재생하고 모자랄 때만 `try_lock`으로
//! 보충한다. 시작·탐색 뒤에는 선행 버퍼가 찰 때까지 소리를 내지 않고 클럭도
//! 세우지 않는다(끊김 대신 짧은 대기). 큐가 마르면 0으로 점프하지 않고 짧게
//! 페이드해야 클릭이 나지 않는다.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::app_error::AppError;

/// 출력 채널 수. 모노·5.1 등은 스테레오로 내린다.
pub const AUDIO_CHANNELS: usize = 2;
/// 우선 시도하는 출력 레이트.
pub const PREFERRED_RATE: u32 = 48_000;
/// 디코드 선행 버퍼. 스테레오 인터리브 f32 쌍 기준 4초다.
const QUEUE_PAIRS_MAX: usize = 48_000 * 4;
/// 정지 중에 미리 디코드해 둘 양. 재개가 즉시 시작될 만큼만 담는다(1초).
const PAUSED_PAIRS_MAX: usize = 48_000;
/// 콜백이 잠금 없이 들고 재생하는 지역 버퍼 깊이(ms).
const LOCAL_BUFFER_MS: u32 = 200;
/// 소리를 내기 전에 채워 둘 선행량(ms). 콜백 지터를 흡수한다.
const PREBUFFER_MS: u32 = 60;
/// 큐가 빈 채로 이만큼 기다리면 클럭을 세워 영상이 멈추지 않게 한다(ms).
const HOLD_LIMIT_MS: u64 = 400;
/// 큐가 마르거나 다시 찰 때 클릭을 막는 페이드(ms).
const FADE_MS: u32 = 5;
/// 출력 버퍼 프레임 수. 기본값보다 크게 잡아 CPU 스파이크를 흡수한다.
const OUTPUT_BUFFER_FRAMES: u32 = 1024;
/// 기본 음량. 설정이 덮어쓴다.
pub const DEFAULT_VOLUME: f32 = 0.8;

pub struct AudioShared {
    queue: Mutex<VecDeque<f32>>,
    playing: AtomicBool,
    muted: AtomicBool,
    volume_bits: AtomicU32,
    speed_bits: AtomicU32,
    /// 출력한 샘플 쌍 수. 오디오 마스터 클럭이다.
    played_pairs: AtomicU64,
    eof: AtomicBool,
    /// 탐색 세대. 콜백은 값이 바뀌면 들고 있던 소리를 버린다.
    seek_gen: AtomicU64,
    /// 누적 언더런 수. 진단용이다.
    underruns: AtomicU64,
    error: Mutex<Option<String>>,
}

enum AudioCommand {
    Seek(u64),
    Stop,
}

/// cpal 출력 스트림과 오디오 워커를 소유한다. 살아 있는 동안 소리를 낸다.
pub struct AudioOutput {
    shared: Arc<AudioShared>,
    cmd_tx: mpsc::Sender<AudioCommand>,
    worker: Option<std::thread::JoinHandle<()>>,
    sample_rate: u32,
    channels: u16,
    format: cpal::SampleFormat,
}

/// cpal 출력 스트림. `!Send`라 UI 스레드에서 만들고 들고 있는다.
/// 필드를 읽지 않지만, 값이 살아 있는 동안 소리가 난다.
pub struct AudioStream(#[allow(dead_code)] cpal::Stream);

impl AudioOutput {
    /// 디코드 워커를 시작한다. 출력 장치는 열지 않는다(어느 스레드에서든 호출 가능).
    pub fn open(path: &Path) -> Result<Self, AppError> {
        let (rate, channels, format) = Self::probe_output()?;
        let shared = Arc::new(AudioShared {
            queue: Mutex::new(VecDeque::new()),
            playing: AtomicBool::new(false),
            muted: AtomicBool::new(false),
            volume_bits: AtomicU32::new(DEFAULT_VOLUME.to_bits()),
            speed_bits: AtomicU32::new(1.0f32.to_bits()),
            played_pairs: AtomicU64::new(0),
            eof: AtomicBool::new(false),
            seek_gen: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            error: Mutex::new(None),
        });
        let out_rate = rate;
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let worker_shared = Arc::clone(&shared);
        let source = path.to_path_buf();
        let worker = std::thread::Builder::new()
            .name("araview-audio".to_string())
            .spawn(move || {
                if let Err(e) = audio_loop(&source, out_rate, &cmd_rx, &worker_shared) {
                    *worker_shared
                        .error
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = Some(e);
                }
            })
            .map_err(|e| AppError::unknown(format!("Failed to start audio thread: {e}")))?;
        Ok(Self {
            shared,
            cmd_tx,
            worker: Some(worker),
            sample_rate: rate,
            channels,
            format,
        })
    }

    /// 출력 장치와 설정을 고른다. 값만 돌려주므로 어느 스레드에서든 호출 가능하고,
    /// 실제 스트림은 UI 스레드에서 `open_output_stream`으로 연다.
    pub fn probe_output() -> Result<(u32, u16, cpal::SampleFormat), AppError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AppError::unknown("No audio output device"))?;
        let configs: Vec<_> = device
            .supported_output_configs()
            .map_err(|e| AppError::unknown(format!("No audio output configs: {e}")))?
            .collect();
        // F32 스테레오 48kHz 우선, 없으면 F32, 그래도 없으면 첫 설정이다.
        use cpal::SampleFormat;
        let picked = configs
            .iter()
            .filter(|c| c.sample_format() == SampleFormat::F32)
            .find(|c| {
                c.channels() == AUDIO_CHANNELS as u16
                    && c.min_sample_rate() <= PREFERRED_RATE
                    && PREFERRED_RATE <= c.max_sample_rate()
            })
            .or_else(|| {
                configs
                    .iter()
                    .find(|c| c.sample_format() == SampleFormat::F32)
            })
            .or_else(|| configs.first())
            .ok_or_else(|| AppError::unknown("No audio output configs"))?;
        let rate = if picked.min_sample_rate() <= PREFERRED_RATE
            && PREFERRED_RATE <= picked.max_sample_rate()
        {
            PREFERRED_RATE
        } else {
            picked.max_sample_rate()
        };
        Ok((rate, picked.channels(), picked.sample_format()))
    }

    /// cpal 출력 스트림을 연다. `!Send`라 UI 스레드에서 호출하고 들고 있는다.
    /// 버퍼를 크게 잡아 CPU 스파이크를 흡수하고, 백엔드가 고정 크기를 거부하면
    /// 기본값으로 다시 연다.
    pub fn open_output_stream(&self) -> Result<AudioStream, AppError> {
        let rate = self.sample_rate;
        let channels = self.channels;
        let format = self.format;
        match open_stream(
            &self.shared,
            rate,
            channels,
            format,
            Some(OUTPUT_BUFFER_FRAMES),
        ) {
            Ok(stream) => Ok(AudioStream(stream)),
            Err(first) => open_stream(&self.shared, rate, channels, format, None)
                .map(AudioStream)
                .map_err(|_| first),
        }
    }

    pub fn set_playing(&self, playing: bool) {
        self.shared.playing.store(playing, Ordering::SeqCst);
    }

    pub fn set_volume(&self, volume: f32) {
        let volume = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.shared
            .volume_bits
            .store(volume.to_bits(), Ordering::SeqCst);
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.shared.volume_bits.load(Ordering::SeqCst))
    }

    pub fn set_muted(&self, muted: bool) {
        self.shared.muted.store(muted, Ordering::SeqCst);
    }

    pub fn is_muted(&self) -> bool {
        self.shared.muted.load(Ordering::SeqCst)
    }

    pub fn set_speed(&self, speed: f32) {
        if speed.is_finite() && speed > 0.0 {
            self.shared
                .speed_bits
                .store(speed.to_bits(), Ordering::SeqCst);
        }
    }

    /// 재생 위치를 옮긴다. 큐를 비우고 카운터를 리셋한 뒤 워커에 탐색을 건다.
    /// 세대를 올려 콜백이 들고 있던 이전 위치의 소리를 버리게 한다.
    pub fn seek(&self, position_ms: u64) {
        {
            let mut queue = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
            queue.clear();
        }
        self.shared.eof.store(false, Ordering::SeqCst);
        self.shared.played_pairs.store(
            position_ms.saturating_mul(self.sample_rate as u64) / 1000,
            Ordering::SeqCst,
        );
        self.shared.seek_gen.fetch_add(1, Ordering::SeqCst);
        let _ = self.cmd_tx.send(AudioCommand::Seek(position_ms));
    }

    /// 출력한 샘플 기준 위치. 오디오 마스터 클럭이다.
    pub fn played_ms(&self) -> u64 {
        self.shared.played_pairs.load(Ordering::SeqCst) * 1000 / self.sample_rate as u64
    }

    /// 누적 언더런 수. 선행 버퍼가 모자라 소리를 0으로 메운 횟수다.
    pub fn underruns(&self) -> u64 {
        self.shared.underruns.load(Ordering::SeqCst)
    }

    pub fn has_error(&self) -> Option<String> {
        self.shared
            .error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(AudioCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn open_stream(
    shared: &Arc<AudioShared>,
    rate: u32,
    channels: u16,
    format: cpal::SampleFormat,
    buffer_frames: Option<u32>,
) -> Result<cpal::Stream, AppError> {
    use cpal::SampleFormat;
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| AppError::unknown("No audio output device"))?;
    let configs: Vec<_> = device
        .supported_output_configs()
        .map_err(|e| AppError::unknown(format!("No audio output configs: {e}")))?
        .collect();
    // `probe_output`이 고른 설정과 정확히 같은 것을 쓴다. 다른 설정으로 폴백하면
    // 리샘플러·클럭이 쓰는 레이트와 실제 출력 레이트가 어긋난다.
    let picked = configs
        .iter()
        .find(|c| {
            c.sample_format() == format
                && c.channels() == channels
                && c.min_sample_rate() <= rate
                && rate <= c.max_sample_rate()
        })
        .ok_or_else(|| AppError::unknown("No matching audio output config"))?;
    let mut config = (*picked).with_sample_rate(rate).config();
    if let Some(frames) = buffer_frames {
        if let cpal::SupportedBufferSize::Range { min, max } = picked.buffer_size() {
            if (*min..=*max).contains(&frames) {
                config.buffer_size = cpal::BufferSize::Fixed(frames);
            }
        }
    }
    let err_fn = |e| log::warn!("audio output error: {e}");
    let stream = match format {
        SampleFormat::F32 => {
            let worker = Arc::clone(shared);
            let mut mixer = Mixer::new(rate);
            device.build_output_stream(
                config,
                move |out: &mut [f32], _| fill_output(&worker, &mut mixer, channels, out, |s| s),
                err_fn,
                None,
            )
        }
        SampleFormat::I16 => {
            let worker = Arc::clone(shared);
            let mut mixer = Mixer::new(rate);
            device.build_output_stream(
                config,
                move |out: &mut [i16], _| {
                    fill_output(&worker, &mut mixer, channels, out, |s| {
                        (s.clamp(-1.0, 1.0) * 32767.0) as i16
                    })
                },
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let worker = Arc::clone(shared);
            let mut mixer = Mixer::new(rate);
            device.build_output_stream(
                config,
                move |out: &mut [u16], _| {
                    fill_output(&worker, &mut mixer, channels, out, |s| {
                        ((s.clamp(-1.0, 1.0) * 0.5 + 0.5) * 65535.0) as u16
                    })
                },
                err_fn,
                None,
            )
        }
        _ => return Err(AppError::unknown("Unsupported audio sample format")),
    }
    .map_err(|e| AppError::unknown(format!("Failed to open audio output: {e}")))?;
    stream
        .play()
        .map_err(|e| AppError::unknown(format!("Failed to start audio output: {e}")))?;
    Ok(stream)
}

/// 콜백이 들고 다니는 재생 상태. `cpal` 없이도 검증할 수 있게 분리한다.
/// 실시간 스레드이므로 잠금을 잡지 않고, 모자랄 때만 `try_lock`으로 보충한다.
struct Mixer {
    /// 출력 레이트(Hz). 선행량과 페이드 길이를 프레임으로 환산한다.
    rate: u32,
    /// 지역 버퍼(인터리브 스테레오 쌍). 콜백은 여기서만 꺼낸다.
    local: VecDeque<f32>,
    /// 보간 기준 쌍과 다음 쌍.
    cur: Option<(f32, f32)>,
    next: Option<(f32, f32)>,
    /// `cur`와 `next` 사이 진행도(0..1).
    phase: f32,
    /// 미디어 쌍 진행의 소수부. 속도가 소수여도 클럭이 어긋나지 않는다.
    carry: f32,
    /// 선행 버퍼가 차서 소리를 내는 중인가.
    primed: bool,
    /// 마지막으로 본 탐색 세대.
    generation: u64,
    /// 0..1 페이드 값. 큐가 마르면 줄고, 차면 늘어난다.
    fade: f32,
    /// 마지막으로 낸 표본(게인 전). 페이드 아웃에 쓴다.
    last: (f32, f32),
    /// 큐가 빈 채로 기다리기 시작한 시각.
    stalled: Option<Instant>,
}

impl Mixer {
    fn new(rate: u32) -> Self {
        Self {
            rate,
            local: VecDeque::new(),
            cur: None,
            next: None,
            phase: 0.0,
            carry: 0.0,
            primed: false,
            generation: 0,
            fade: 0.0,
            last: (0.0, 0.0),
            stalled: None,
        }
    }

    /// 지역 버퍼 목표(샘플 수).
    fn local_target(&self) -> usize {
        self.rate as usize * LOCAL_BUFFER_MS as usize / 1000 * AUDIO_CHANNELS
    }

    /// 재생을 시작하는 데 필요한 선행량(샘플 수).
    fn prebuffer(&self) -> usize {
        self.rate as usize * PREBUFFER_MS as usize / 1000 * AUDIO_CHANNELS
    }

    /// 출력 한 프레임에 적용하는 페이드 스텝.
    fn fade_step(&self) -> f32 {
        1.0 / (self.rate as f32 * FADE_MS as f32 / 1000.0).max(1.0)
    }

    /// 탐색 세대가 바뀌었으면 쌓아 둔 소리를 버린다.
    fn sync(&mut self, shared: &Arc<AudioShared>) {
        let generation = shared.seek_gen.load(Ordering::SeqCst);
        if generation != self.generation {
            self.generation = generation;
            self.local.clear();
            self.cur = None;
            self.next = None;
            self.phase = 0.0;
            self.primed = false;
            self.stalled = None;
        }
    }

    /// 공유 큐에서 지역 버퍼로 옮긴다. 잠금을 못 잡으면 가진 것으로 버틴다.
    fn refill(&mut self, shared: &Arc<AudioShared>) {
        let target = self.local_target();
        if self.local.len() >= target {
            return;
        }
        let Ok(mut queue) = shared.queue.try_lock() else {
            return;
        };
        let take = (target - self.local.len()).min(queue.len());
        // 좌우가 뒤섞이지 않도록 쌍 단위로만 가져온다.
        let take = take - take % AUDIO_CHANNELS;
        if take > 0 {
            self.local.extend(queue.drain(..take));
        }
    }

    /// 선행 버퍼가 찼는가.
    fn ready(&self) -> bool {
        self.local.len() >= self.prebuffer()
    }

    fn pop_pair(&mut self) -> Option<(f32, f32)> {
        if self.local.len() < AUDIO_CHANNELS {
            return None;
        }
        let left = self.local.pop_front()?;
        let right = self.local.pop_front()?;
        Some((left, right))
    }

    /// 기준 쌍을 다음 쌍으로 넘긴다. 남은 쌍이 없으면 `false`다.
    fn advance(&mut self) -> bool {
        self.cur = self.next.take();
        if self.cur.is_none() {
            self.cur = self.pop_pair();
        }
        self.next = self.pop_pair();
        self.cur.is_some()
    }

    /// 다음 출력 프레임의 스테레오 값. 속도만큼 소스를 선형 보간해 앞으로 간다.
    /// 큐가 비었으면 `None`이다.
    fn next_sample(&mut self, speed: f32) -> Option<(f32, f32)> {
        if self.cur.is_none() {
            self.cur = self.pop_pair();
            self.next = self.pop_pair();
        }
        let (left, right) = self.cur?;
        let (next_left, next_right) = self.next.unwrap_or((left, right));
        let phase = self.phase.clamp(0.0, 1.0);
        let sample = (
            left + (next_left - left) * phase,
            right + (next_right - right) * phase,
        );
        self.phase += speed;
        while self.phase >= 1.0 {
            self.phase -= 1.0;
            if !self.advance() {
                self.phase = 0.0;
                break;
            }
        }
        Some(sample)
    }

    /// 미디어 클럭(쌍)을 진행하고 정수부를 돌려준다.
    fn advance_media(&mut self, speed: f32) -> u64 {
        self.carry += speed;
        let whole = self.carry.floor();
        self.carry -= whole;
        whole as u64
    }

    /// 큐가 빈 채로 오래 기다렸는가.
    fn stalled_out(&self) -> bool {
        self.stalled
            .is_some_and(|since| since.elapsed() >= Duration::from_millis(HOLD_LIMIT_MS))
    }
}

/// 한 프레임을 장치 채널 수에 맞춰 쓴다. 모노는 좌우를 섞고, 남는 채널은 0이다.
fn write_frame<T>(frame: &mut [T], channels: usize, left: f32, right: f32, conv: fn(f32) -> T) {
    let left = if channels == 1 {
        (left + right) * 0.5
    } else {
        left
    };
    for (index, slot) in frame.iter_mut().enumerate() {
        let value = match index {
            0 => left,
            1 if channels > 1 => right,
            _ => 0.0,
        };
        *slot = conv(value);
    }
}

/// 출력 콜백. 실시간 스레드이므로 잠금을 기다리지 않고, 소리는 지역 버퍼에서 낸다.
/// 정지면 침묵만 내고, 음소거면 소모는 이어가고 0을 낸다.
fn fill_output<T>(
    shared: &Arc<AudioShared>,
    mixer: &mut Mixer,
    channels: u16,
    out: &mut [T],
    conv: fn(f32) -> T,
) {
    let channels = channels.max(1) as usize;
    let volume = if shared.muted.load(Ordering::SeqCst) {
        0.0
    } else {
        f32::from_bits(shared.volume_bits.load(Ordering::SeqCst))
    };
    let speed = {
        let speed = f32::from_bits(shared.speed_bits.load(Ordering::SeqCst));
        if speed.is_finite() && speed > 0.0 {
            speed
        } else {
            1.0
        }
    };
    let fade_step = mixer.fade_step();
    if !shared.playing.load(Ordering::SeqCst) {
        // 정지: 들고 있던 소리를 짧게 줄여 끈다. 0으로 점프하면 클릭이 난다.
        // 지역 버퍼는 버리지 않는다. 버리면 재개할 때 그만큼 건너뛴다.
        mixer.sync(shared);
        for frame in out.chunks_exact_mut(channels) {
            mixer.fade = (mixer.fade - fade_step).max(0.0);
            let gain = volume * mixer.fade;
            let (left, right) = mixer.last;
            write_frame(frame, channels, left * gain, right * gain, conv);
        }
        return;
    }
    mixer.sync(shared);
    mixer.refill(shared);
    if !mixer.primed {
        mixer.primed = mixer.ready();
    }
    let eof = shared.eof.load(Ordering::SeqCst);
    let mut played = 0u64;
    for frame in out.chunks_exact_mut(channels) {
        let sample = if mixer.primed {
            mixer.next_sample(speed)
        } else {
            None
        };
        match sample {
            Some((left, right)) => {
                mixer.stalled = None;
                mixer.fade = (mixer.fade + fade_step).min(1.0);
                mixer.last = (left, right);
                let gain = volume * mixer.fade;
                write_frame(frame, channels, left * gain, right * gain, conv);
                played += mixer.advance_media(speed);
            }
            None => {
                if mixer.primed && !eof {
                    // 선행 버퍼가 마르면 다시 찰 때까지 기다린다. 클럭도 세운다.
                    mixer.primed = false;
                    shared.underruns.fetch_add(1, Ordering::SeqCst);
                }
                mixer.stalled.get_or_insert_with(Instant::now);
                mixer.fade = (mixer.fade - fade_step).max(0.0);
                let gain = volume * mixer.fade;
                let (left, right) = mixer.last;
                write_frame(frame, channels, left * gain, right * gain, conv);
                // 스트림이 끝났거나 너무 오래 기다렸으면 클럭만 진행해 영상이
                // 무한정 멈추지 않게 한다.
                if eof || mixer.stalled_out() {
                    played += mixer.advance_media(speed);
                }
            }
        }
    }
    if played > 0 {
        shared.played_pairs.fetch_add(played, Ordering::SeqCst);
    }
}

struct AudioDecoder {
    ictx: rsmpeg::avformat::AVFormatContextInput,
    ctx: rsmpeg::avcodec::AVCodecContext,
    swr: rsmpeg::swresample::SwrContext,
    stream_index: usize,
    time_base: (i32, i32),
    out_rate: u32,
    in_rate: i32,
    in_fmt: i32,
    in_channels: i32,
}

fn open_audio_decoder(source: &Path, out_rate: u32) -> Result<AudioDecoder, String> {
    use rsmpeg::ffi;
    let cpath = std::ffi::CString::new(source.as_os_str().as_encoded_bytes())
        .map_err(|_| "Invalid file path".to_string())?;
    let ictx = rsmpeg::avformat::AVFormatContextInput::open(&cpath)
        .map_err(|e| format!("Failed to open audio: {e:?}"))?;
    let (stream_index, decoder) = ictx
        .find_best_stream(ffi::AVMEDIA_TYPE_AUDIO)
        .map_err(|e| format!("Failed to find audio stream: {e:?}"))?
        .ok_or_else(|| "No audio stream".to_string())?;
    let (in_rate, in_fmt, in_channels) = {
        let par = ictx.streams()[stream_index].codecpar();
        let in_rate = par.sample_rate;
        let in_fmt = par.format;
        let in_channels = par.ch_layout().nb_channels;
        (in_rate, in_fmt, in_channels)
    };
    if in_rate <= 0 {
        return Err("Invalid audio sample rate".to_string());
    }
    let decoder_name = decoder.name().to_str().unwrap_or("unknown").to_string();
    let cname = std::ffi::CString::new(decoder_name.as_str())
        .map_err(|_| "Invalid codec name".to_string())?;
    let decoder = rsmpeg::avcodec::AVCodec::find_decoder_by_name(&cname)
        .ok_or_else(|| format!("Unsupported audio codec: {decoder_name}"))?;
    let mut ctx = rsmpeg::avcodec::AVCodecContext::new(&decoder);
    {
        let stream = &ictx.streams()[stream_index];
        ctx.apply_codecpar(&stream.codecpar())
            .map_err(|e| format!("Failed to open decoder: {e:?}"))?;
    }
    ctx.open(None)
        .map_err(|e| format!("Failed to open decoder: {e:?}"))?;
    let time_base = {
        let tb = ictx.streams()[stream_index].time_base;
        (tb.num, tb.den)
    };
    Ok(AudioDecoder {
        ictx,
        ctx,
        swr: new_swr(out_rate, in_rate, in_fmt, in_channels)?,
        stream_index,
        time_base,
        out_rate,
        in_rate,
        in_fmt,
        in_channels,
    })
}

/// 입력·출력 레이아웃으로 리샘플러를 만들어 초기화한다. 탐색 뒤에는 이전 위치의
/// 지연 버퍼가 섞이지 않도록 다시 만들어야 한다.
fn new_swr(
    out_rate: u32,
    in_rate: i32,
    in_fmt: i32,
    in_channels: i32,
) -> Result<rsmpeg::swresample::SwrContext, String> {
    use rsmpeg::ffi;
    let out_layout = rsmpeg::avutil::AVChannelLayout::from_nb_channels(AUDIO_CHANNELS as i32);
    // 입력은 채널 수 기준 기본 레이아웃으로 본다. 특수 마스크는 기본값과
    // 같은 순서라고 가정한다(일반적인 스테레오·모노·5.1에 해당).
    let in_layout = rsmpeg::avutil::AVChannelLayout::from_nb_channels(in_channels);
    let mut swr = rsmpeg::swresample::SwrContext::new(
        &out_layout.into_inner(),
        ffi::AV_SAMPLE_FMT_FLT,
        out_rate as i32,
        &in_layout.into_inner(),
        in_fmt,
        in_rate,
    )
    .map_err(|e| format!("Failed to create resampler: {e:?}"))?;
    swr.init()
        .map_err(|e| format!("Failed to init resampler: {e:?}"))?;
    Ok(swr)
}

fn audio_loop(
    source: &Path,
    out_rate: u32,
    cmd_rx: &mpsc::Receiver<AudioCommand>,
    shared: &Arc<AudioShared>,
) -> Result<(), String> {
    let mut dec = open_audio_decoder(source, out_rate)?;
    let mut target_ms: Option<u64> = None;
    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                AudioCommand::Seek(ms) => {
                    seek_audio(&mut dec, ms)?;
                    clear_audio(shared);
                    target_ms = Some(ms);
                }
                AudioCommand::Stop => return Ok(()),
            }
        }
        // 정지 중에는 재개에 필요한 만큼만 미리 담아 둔다. 더 담으면 큐가 넘쳐
        // 오래 멈춘 뒤 재개할 때 재생 위치가 앞으로 건너뛴다.
        if !shared.playing.load(Ordering::SeqCst) && buffered_pairs(shared) >= PAUSED_PAIRS_MAX {
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        }
        if audio_queue_full(shared) {
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        }
        if pump_audio(&mut dec, shared, &mut target_ms)? {
            shared.eof.store(true, Ordering::SeqCst);
            // 끝까지 디코드했으면 명령만 기다린다.
            match cmd_rx.recv() {
                Ok(AudioCommand::Seek(ms)) => {
                    seek_audio(&mut dec, ms)?;
                    clear_audio(shared);
                    target_ms = Some(ms);
                    shared.eof.store(false, Ordering::SeqCst);
                }
                Ok(AudioCommand::Stop) | Err(_) => return Ok(()),
            }
        }
    }
}

fn clear_audio(shared: &Arc<AudioShared>) {
    let mut queue = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
    queue.clear();
}

/// 공유 큐에 쌓인 샘플 쌍 수.
fn buffered_pairs(shared: &Arc<AudioShared>) -> usize {
    shared.queue.lock().unwrap_or_else(|e| e.into_inner()).len() / AUDIO_CHANNELS
}

fn audio_queue_full(shared: &Arc<AudioShared>) -> bool {
    shared.queue.lock().unwrap_or_else(|e| e.into_inner()).len() >= QUEUE_PAIRS_MAX * 2
}

/// 패킷을 읽어 f32 스테레오로 큐에 넣는다. EOF면 `true`다.
fn pump_audio(
    dec: &mut AudioDecoder,
    shared: &Arc<AudioShared>,
    target_ms: &mut Option<u64>,
) -> Result<bool, String> {
    loop {
        let packet = dec
            .ictx
            .read_packet()
            .map_err(|e| format!("Failed to read packet: {e:?}"))?;
        let Some(mut pkt) = packet else {
            dec.ctx.send_packet(None).ok();
            return flush_audio(dec, shared, target_ms);
        };
        if pkt.stream_index != dec.stream_index as i32 {
            continue;
        }
        dec.ctx
            .send_packet(Some(&pkt))
            .map_err(|e| format!("Failed to decode audio: {e:?}"))?;
        pkt.set_stream_index(-1);
        while let Ok(frame) = dec.ctx.receive_frame() {
            push_audio(dec, &frame, shared, target_ms)?;
        }
        if audio_queue_full(shared) {
            return Ok(false);
        }
    }
}

fn flush_audio(
    dec: &mut AudioDecoder,
    shared: &Arc<AudioShared>,
    target_ms: &mut Option<u64>,
) -> Result<bool, String> {
    while let Ok(frame) = dec.ctx.receive_frame() {
        push_audio(dec, &frame, shared, target_ms)?;
    }
    Ok(true)
}

fn push_audio(
    dec: &mut AudioDecoder,
    frame: &rsmpeg::avutil::AVFrame,
    shared: &Arc<AudioShared>,
    target_ms: &mut Option<u64>,
) -> Result<(), String> {
    if frame.nb_samples <= 0 {
        return Ok(());
    }
    // 탐색 목표보다 이른 프레임은 버린다.
    if let Some(target) = target_ms.take() {
        let pts = frame_ms_of_audio(frame, dec.time_base).unwrap_or(0);
        if pts < target {
            *target_ms = Some(target);
            return Ok(());
        }
    }
    let mut out = rsmpeg::avutil::AVFrame::new();
    out.set_format(rsmpeg::ffi::AV_SAMPLE_FMT_FLT);
    let layout = rsmpeg::avutil::AVChannelLayout::from_nb_channels(AUDIO_CHANNELS as i32);
    out.set_ch_layout(layout.into_inner());
    out.set_sample_rate(dec.out_rate as i32);
    out.set_nb_samples(dec.swr.get_out_samples(frame.nb_samples).max(1));
    out.alloc_buffer()
        .map_err(|e| format!("Failed to allocate audio frame: {e:?}"))?;
    dec.swr
        .convert_frame(Some(frame), &mut out)
        .map_err(|e| format!("Failed to resample audio: {e:?}"))?;
    let samples = out.nb_samples.max(0) as usize * AUDIO_CHANNELS;
    if samples == 0 {
        return Ok(());
    }
    // SAFETY: `alloc_buffer`로 확보한 packed f32 버퍼를 `nb_samples` 범위에서만 읽는다.
    let bytes: &[f32] = unsafe {
        let ptr = out.data_mut()[0] as *const f32;
        if ptr.is_null() {
            return Err("Invalid audio buffer".to_string());
        }
        std::slice::from_raw_parts(ptr, samples)
    };
    let mut queue = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
    while queue.len() + bytes.len() > QUEUE_PAIRS_MAX * 2 {
        if queue.len() >= 2 {
            queue.pop_front();
            queue.pop_front();
        } else {
            break;
        }
    }
    queue.extend(bytes.iter().copied());
    Ok(())
}

fn frame_ms_of_audio(frame: &rsmpeg::avutil::AVFrame, time_base: (i32, i32)) -> Option<u64> {
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

fn seek_audio(dec: &mut AudioDecoder, target_ms: u64) -> Result<(), String> {
    use rsmpeg::ffi;
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
        .map_err(|e| format!("Failed to seek audio: {e:?}"))?;
    dec.ctx.flush_buffers();
    // 리샘플러 지연 버퍼에 이전 위치의 표본이 남아 있으면 탐색 지점에서
    // 잠깐 다른 소리가 섞인다. 새 위치는 새 리샘플러로 시작한다.
    dec.swr = new_swr(dec.out_rate, dec.in_rate, dec.in_fmt, dec.in_channels)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared_with(playing: bool) -> Arc<AudioShared> {
        Arc::new(AudioShared {
            queue: Mutex::new(VecDeque::new()),
            playing: AtomicBool::new(playing),
            muted: AtomicBool::new(false),
            volume_bits: AtomicU32::new(DEFAULT_VOLUME.to_bits()),
            speed_bits: AtomicU32::new(1.0f32.to_bits()),
            played_pairs: AtomicU64::new(0),
            eof: AtomicBool::new(false),
            seek_gen: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            error: Mutex::new(None),
        })
    }

    fn push(shared: &Arc<AudioShared>, pairs: &[(f32, f32)]) {
        let mut queue = shared.queue.lock().unwrap();
        for (left, right) in pairs {
            queue.push_back(*left);
            queue.push_back(*right);
        }
    }

    fn run(shared: &Arc<AudioShared>, mixer: &mut Mixer, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0f32; frames * AUDIO_CHANNELS];
        fill_output(shared, mixer, AUDIO_CHANNELS as u16, &mut out, |s| s);
        out
    }

    /// 미리 채운 뒤 재생을 시작할 만큼 넣는다.
    fn prime(shared: &Arc<AudioShared>, mixer: &mut Mixer, level: f32) {
        push(shared, &[(level, level); 48_000]);
        let _ = run(shared, mixer, 480);
    }

    #[test]
    fn holds_until_prebuffer_is_ready() {
        let shared = shared_with(true);
        let mut mixer = Mixer::new(PREFERRED_RATE);
        push(&shared, &[(1.0, 1.0); 480]);
        let out = run(&shared, &mut mixer, 480);
        assert!(out.iter().all(|sample| *sample == 0.0));
        assert_eq!(shared.played_pairs.load(Ordering::SeqCst), 0);
        assert_eq!(shared.underruns.load(Ordering::SeqCst), 0);
        push(&shared, &[(1.0, 1.0); 3600]);
        let out = run(&shared, &mut mixer, 480);
        assert!(out[479] > 0.7, "선행 버퍼가 차면 소리가 나야 한다");
        assert!(shared.played_pairs.load(Ordering::SeqCst) >= 400);
    }

    #[test]
    fn starving_fades_out_instead_of_clicking() {
        let shared = shared_with(true);
        let mut mixer = Mixer::new(PREFERRED_RATE);
        prime(&shared, &mut mixer, 1.0);
        let out = run(&shared, &mut mixer, 960);
        let last = out[out.len() - 1];
        assert!((last - DEFAULT_VOLUME).abs() < 0.01, "페이드 인 뒤 값");
        // 큐를 비우면 남은 소리가 0으로 부드럽게 줄어야 한다.
        shared.queue.lock().unwrap().clear();
        mixer.local.clear();
        mixer.cur = None;
        mixer.next = None;
        let out = run(&shared, &mut mixer, 960);
        let step = DEFAULT_VOLUME * mixer.fade_step();
        let mut previous = last;
        for sample in &out {
            assert!(
                (previous - sample).abs() <= step + 1e-4,
                "표본이 한 번에 튀면 클릭이 된다: {previous} {sample}"
            );
            previous = *sample;
        }
        assert!(out[out.len() - 1].abs() < 1e-6);
        assert!(shared.underruns.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn seeking_drops_buffered_audio() {
        let shared = shared_with(true);
        let mut mixer = Mixer::new(PREFERRED_RATE);
        prime(&shared, &mut mixer, 1.0);
        let out = run(&shared, &mut mixer, 480);
        assert!(out[479] > 0.7);
        shared.queue.lock().unwrap().clear();
        shared.seek_gen.fetch_add(1, Ordering::SeqCst);
        let out = run(&shared, &mut mixer, 480);
        assert!(
            out[479].abs() < 1e-3,
            "이전 위치의 소리를 이어 쓰면 안 된다"
        );
    }

    #[test]
    fn speed_consumes_source_faster_with_interpolation() {
        let shared = shared_with(true);
        shared.speed_bits.store(2.0f32.to_bits(), Ordering::SeqCst);
        let mut mixer = Mixer::new(PREFERRED_RATE);
        let ramp: Vec<(f32, f32)> = (0..48_000).map(|i| (i as f32, i as f32)).collect();
        push(&shared, &ramp);
        let out = run(&shared, &mut mixer, 960);
        // 페이드 인이 끝난 뒤에는 소스를 두 배로 건너뛴다.
        let index = 600;
        let expected = index as f32 * 2.0 * DEFAULT_VOLUME;
        assert!(
            (out[index * AUDIO_CHANNELS] - expected).abs() < 2.0,
            "{} vs {expected}",
            out[index * AUDIO_CHANNELS]
        );
        // 소수 속도에서도 클럭이 어긋나지 않아야 한다.
        let played = shared.played_pairs.load(Ordering::SeqCst);
        assert!((played as i64 - (960 * 2)).abs() <= 2, "played={played}");
    }

    #[test]
    fn eof_keeps_the_clock_running() {
        let shared = shared_with(true);
        shared.eof.store(true, Ordering::SeqCst);
        let mut mixer = Mixer::new(PREFERRED_RATE);
        let out = run(&shared, &mut mixer, 480);
        assert!(out.iter().all(|sample| *sample == 0.0));
        assert_eq!(shared.played_pairs.load(Ordering::SeqCst), 480);
    }

    #[test]
    fn pause_fades_out_and_stops_the_clock() {
        let shared = shared_with(true);
        let mut mixer = Mixer::new(PREFERRED_RATE);
        prime(&shared, &mut mixer, 1.0);
        let out = run(&shared, &mut mixer, 480);
        assert!(out[479] > 0.7);
        shared.playing.store(false, Ordering::SeqCst);
        let before = shared.played_pairs.load(Ordering::SeqCst);
        let out = run(&shared, &mut mixer, 480);
        assert!(out[479].abs() < 1e-6);
        assert_eq!(shared.played_pairs.load(Ordering::SeqCst), before);
    }

    #[test]
    fn mono_output_mixes_both_channels() {
        let shared = shared_with(true);
        let mut mixer = Mixer::new(PREFERRED_RATE);
        push(&shared, &[(0.4, 0.2); 48_000]);
        let mut out = vec![0.0f32; 480];
        fill_output(&shared, &mut mixer, 1, &mut out, |s| s);
        let expected = (0.4 + 0.2) * 0.5 * DEFAULT_VOLUME;
        assert!((out[479] - expected).abs() < 0.01, "{}", out[479]);
    }

    #[test]
    fn resume_continues_where_pause_stopped() {
        let shared = shared_with(true);
        let mut mixer = Mixer::new(PREFERRED_RATE);
        let ramp: Vec<(f32, f32)> = (0..200_000).map(|i| (i as f32, i as f32)).collect();
        push(&shared, &ramp);
        let _ = run(&shared, &mut mixer, 2_000);
        shared.playing.store(false, Ordering::SeqCst);
        let _ = run(&shared, &mut mixer, 2_000);
        let paused_at = shared.played_pairs.load(Ordering::SeqCst);
        assert!(paused_at > 1_500);
        shared.playing.store(true, Ordering::SeqCst);
        let out = run(&shared, &mut mixer, 960);
        let index = 400;
        let value = out[index * AUDIO_CHANNELS] / DEFAULT_VOLUME;
        let expected = (paused_at + index as u64) as f32;
        assert!(
            (value - expected).abs() < 3.0,
            "재개가 멈춘 지점에서 이어져야 한다: {value} vs {expected}"
        );
    }
}
