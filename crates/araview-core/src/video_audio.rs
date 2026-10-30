//! 동영상 오디오 디코드·출력 (M2, §7.6).
//!
//! 디코드 워커가 오디오를 스테레오 f32로 리샘플링해 공유 큐에 넣고, cpal 출력
//! 콜백이 꺼내 재생한다. 오디오가 있으면 오디오 클럭이 마스터다. 장치를 열 수
//! 없으면 호출자가 영상만 재생하도록 `Err`를 돌려주고 이유를 경고로 남긴다.
//!
//! cpal 출력 스트림은 전용 스레드가 만들고 들고 있다. Windows에서 cpal의 기본 장치
//! 핸들은 오디오 클라이언트를 `ActivateAudioInterfaceAsync`로 만드는데, GPUI가 COM을
//! 먼저 초기화한 이 프로세스에서는 그 호출이 `RPC_E_CHANGED_MODE`로 항상 실패한다
//! (2.7.0-beta.10까지는 이 때문에 소리가 조용히 사라졌다). 그래서 Windows에서는 같은
//! 엔드포인트를 "특정 장치" 핸들로 먼저 열고, 그것도 실패할 때만 기본 장치 핸들로
//! 물러난다. 스트림 스레드는 `!Send`인 백엔드 스트림을 만든 스레드에서 그대로 버리기
//! 위해서도 필요하다.
//!
//! 스테레오는 열거가 아니라 직접 요청한다. cpal WASAPI 열거는 믹스 채널만
//! 돌려주므로(7.1 믹스면 전부 8ch), 열거에서 스테레오를 고르는 방식은 8ch 믹스에서
//! 그대로 8ch를 연다. 출력은 `AUTOCONVERTPCM` 공유 모드라 열거에 없어도 스테레오
//! 48kHz F32를 직접 열면 Windows가 변환해 준다. 그래서 프로브는 스테레오를 고정하고,
//! 스트림도 직접 요청을 먼저 시도한 뒤 열거 일치로 폴백한다.
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

/// 마지막으로 연 출력 설정(장치·레이트·채널·형식). 지원 요청의 진단 정보가 읽는다.
/// 스트림을 열지 못했으면 비어 있다.
static LAST_OUTPUT: Mutex<Option<String>> = Mutex::new(None);

/// 마지막으로 연 출력 설정. 진단 정보용이다. 아직 열지 않았으면 `None`이다.
pub fn last_output() -> Option<String> {
    LAST_OUTPUT.lock().ok().and_then(|last| last.clone())
}

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
    /// 마지막 언더런 시점의 지역 버퍼 깊이(샘플 수). 진단용이다.
    underrun_local: AtomicU64,
    /// 마지막 언더런 시점의 공유 큐 깊이(샘플 수). 그 순간 잠금을 못 잡았으면
    /// `u64::MAX`다. 디코드 경합과 디코드 부족을 구분하는 진단용이다.
    underrun_queue: AtomicU64,
    error: Mutex<Option<String>>,
}

enum AudioCommand {
    Seek { position_ms: u64, generation: u64 },
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

/// cpal 출력 스트림을 소유하는 전용 스레드 핸들. 이 값이 살아 있는 동안 소리가 난다.
///
/// 스트림은 이 전용 스레드에서 만들고 같은 스레드에서 버린다. 백엔드에 따라
/// `cpal::Stream`이 `!Send`라 만든 스레드에서 버려야 하고, 오디오 장치 활성화가
/// 호출 스레드의 COM 아파트먼트에 민감한 백엔드(Windows)도 같은 방식으로 피해 간다.
pub struct AudioStream {
    /// 신호를 보내거나 버리면 스트림 스레드가 스트림을 버리고 끝난다.
    stop: Option<mpsc::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for AudioStream {
    fn drop(&mut self) {
        // 송신자를 버리면 스트림 스레드의 `recv`가 풀리고, 그 스레드는 스트림을 버린 뒤 끝난다.
        self.stop.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

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
            underrun_local: AtomicU64::new(0),
            underrun_queue: AtomicU64::new(0),
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
    /// 실제 스트림은 `open_output_stream`이 전용 스레드에서 연다.
    ///
    /// 스테레오 48kHz F32를 고정한다. cpal WASAPI 열거는 믹스 채널만 돌려주므로
    /// 열거에서 채널을 고르면 7.1 믹스에서 8ch가 열린다. 출력은 공유 모드
    /// `AUTOCONVERTPCM`이라 열거에 없어도 직접 요청하면 Windows가 변환한다.
    /// 열거는 장치가 출력 가능함을 확인하고 레이트·형식을 고르는 데만 쓴다.
    /// 활성화에 실패하면(COM 등) 고정 스테레오로 넘어가 전용 스레드의 재시도에 맡긴다.
    pub fn probe_output() -> Result<(u32, u16, cpal::SampleFormat), AppError> {
        use cpal::SampleFormat;
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AppError::unknown("No audio output device"))?;
        let configs: Vec<_> = match device.supported_output_configs() {
            Ok(configs) => configs.collect(),
            Err(e) => {
                log::warn!("audio probe failed, requesting stereo directly: {e:?}");
                return Ok((PREFERRED_RATE, AUDIO_CHANNELS as u16, SampleFormat::F32));
            }
        };
        if configs.is_empty() {
            return Ok((PREFERRED_RATE, AUDIO_CHANNELS as u16, SampleFormat::F32));
        }
        // 장치 기본(믹스) 레이트를 함께 본다. 사용자가 제어판에서 고른 값이고 WASAPI가 항상
        // 받아들이는 형식이다. 채널은 여기서 고르지 않는다. 항상 스테레오를 직접 요청한다.
        let default_rate = device
            .default_output_config()
            .map(|config| config.sample_rate())
            .unwrap_or(PREFERRED_RATE);
        let has = |rate: u32| {
            configs.iter().any(|c| {
                c.sample_format() == SampleFormat::F32
                    && c.min_sample_rate() <= rate
                    && rate <= c.max_sample_rate()
            })
        };
        let rate = if has(PREFERRED_RATE) {
            PREFERRED_RATE
        } else if has(default_rate) {
            default_rate
        } else if let Some(first) = configs
            .iter()
            .find(|c| c.sample_format() == SampleFormat::F32)
        {
            first.max_sample_rate()
        } else if let Some(first) = configs.first() {
            // F32 자체가 없으면 첫 설정의 레이트·형식을 쓰고 채널만 스테레오로 요청한다.
            // 직접 요청이 실패하면 `build_stream`의 열거 폴백이 원 설정을 연다.
            let rate = if first.min_sample_rate() <= PREFERRED_RATE
                && PREFERRED_RATE <= first.max_sample_rate()
            {
                PREFERRED_RATE
            } else {
                first.max_sample_rate()
            };
            return Ok((rate, AUDIO_CHANNELS as u16, first.sample_format()));
        } else {
            return Ok((PREFERRED_RATE, AUDIO_CHANNELS as u16, SampleFormat::F32));
        };
        Ok((rate, AUDIO_CHANNELS as u16, SampleFormat::F32))
    }

    /// cpal 출력 스트림을 전용 스레드에서 열고, 그 스레드가 스트림을 들고 있게 한다.
    /// 버퍼를 크게 잡아 CPU 스파이크를 흡수하고, 백엔드가 고정 크기를 거부하면
    /// 기본값으로 다시 연다. 열지 못하면 이유를 경고로 남기고 `Err`를 돌려준다
    /// (호출자는 영상만 재생한다).
    pub fn open_output_stream(&self) -> Result<AudioStream, AppError> {
        let rate = self.sample_rate;
        let channels = self.channels;
        let format = self.format;
        let shared = Arc::clone(&self.shared);
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let thread = std::thread::Builder::new()
            .name("araview-audio-out".to_string())
            .spawn(move || {
                let opened =
                    open_stream(&shared, rate, channels, format, Some(OUTPUT_BUFFER_FRAMES))
                        .or_else(|first| {
                            log::warn!("audio output with a fixed buffer failed: {first:?}");
                            open_stream(&shared, rate, channels, format, None)
                        });
                let (stream, actual_channels, actual_format, device) = match opened {
                    Ok(opened) => opened,
                    Err(error) => {
                        log::warn!("audio output unavailable, playing video only: {error:?}");
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };
                // 고른 설정을 남긴다. 끊김 지원 요청에서 8ch 선택 같은 오선택을 가린다.
                // 폴백으로 열리면 요청과 다를 수 있어 실제 설정을 남긴다.
                let summary = format!("{device}, {rate}Hz {actual_channels}ch {actual_format:?}");
                log::info!("audio output: {summary}");
                if let Ok(mut last) = LAST_OUTPUT.lock() {
                    *last = Some(summary);
                }
                if ready_tx.send(Ok(())).is_err() {
                    return;
                }
                // 정지 신호가 올 때까지 스트림을 살려 둔다. 스트림은 만든 스레드에서 버린다.
                // 그동안 끊김 누적을 1초마다 확인해 경고로 남긴다. 오디오 콜백(실시간
                // 스레드)에서는 로그를 남기지 않고 여기서 남겨야 재생이 흔들리지 않는다.
                let mut reported = 0u64;
                loop {
                    match stop_rx.recv_timeout(Duration::from_secs(1)) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            let underruns = shared.underruns.load(Ordering::SeqCst);
                            if underruns > reported {
                                let (local, queue) = (
                                    shared.underrun_local.load(Ordering::SeqCst),
                                    shared.underrun_queue.load(Ordering::SeqCst),
                                );
                                log::warn!(
                                    "audio underruns: {} (+{}) on {device}, {rate}Hz {actual_channels}ch {actual_format:?}, local={local}, queue={queue}",
                                    underruns,
                                    underruns - reported
                                );
                                reported = underruns;
                            }
                        }
                    }
                }
                drop(stream);
            })
            .map_err(|e| AppError::unknown(format!("Failed to start audio thread: {e}")))?;
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(AudioStream {
                stop: Some(stop_tx),
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            // 시간 안에 열리지 않았다. 송신자를 버려 스레드를 정리한다.
            Err(_) => {
                drop(stop_tx);
                let _ = thread.join();
                Err(AppError::unknown("Audio output did not start in time"))
            }
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
        let generation = {
            // 큐 쓰기도 같은 잠금을 잡고 디코드 세대를 검사한다. 따라서 탐색과 동시에
            // 이전 패킷 디코드가 끝나더라도 이전 위치 표본이 새 큐에 들어오지 않는다.
            let mut queue = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
            queue.clear();
            self.shared.eof.store(false, Ordering::SeqCst);
            self.shared.played_pairs.store(
                position_ms.saturating_mul(self.sample_rate as u64) / 1000,
                Ordering::SeqCst,
            );
            self.shared.seek_gen.fetch_add(1, Ordering::SeqCst) + 1
        };
        let _ = self.cmd_tx.send(AudioCommand::Seek {
            position_ms,
            generation,
        });
    }

    /// 출력한 샘플 기준 위치. 오디오 마스터 클럭이다.
    pub fn played_ms(&self) -> u64 {
        self.shared.played_pairs.load(Ordering::SeqCst) * 1000 / self.sample_rate as u64
    }

    /// 누적 언더런 수. 선행 버퍼가 모자라 소리를 0으로 메운 횟수다.
    pub fn underruns(&self) -> u64 {
        self.shared.underruns.load(Ordering::SeqCst)
    }

    /// 마지막 언더런 시점의 (지역 버퍼, 공유 큐) 깊이(샘플 수). 큐 깊이가
    /// `u64::MAX`면 그 순간 공유 큐 잠금을 잡지 못했다(디코드 경합). 진단용이다.
    pub fn last_underrun_depths(&self) -> (u64, u64) {
        (
            self.shared.underrun_local.load(Ordering::SeqCst),
            self.shared.underrun_queue.load(Ordering::SeqCst),
        )
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

/// 출력 스트림을 연 장치와 실제 설정. 진단 로그에 쓴다.
type OpenedStream = (cpal::Stream, u16, cpal::SampleFormat, String);

fn open_stream(
    shared: &Arc<AudioShared>,
    rate: u32,
    channels: u16,
    format: cpal::SampleFormat,
    buffer_frames: Option<u32>,
) -> Result<OpenedStream, AppError> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| AppError::unknown("No audio output device"))?;
    // Windows에서 cpal의 기본 장치 핸들은 오디오 클라이언트를 `ActivateAudioInterfaceAsync`로
    // 만든다. COM 아파트먼트가 이미 설정된 프로세스(GPUI가 먼저 초기화한다)에서는 이 호출이
    // 매번 `RPC_E_CHANGED_MODE`로 실패하므로, 같은 엔드포인트를 "특정 장치" 핸들로 먼저 연다.
    // 실패했을 때만 기본 장치 핸들로 물러난다(둘 다 실패하면 그때가 진짜 실패다).
    #[cfg(windows)]
    {
        let endpoint = same_endpoint(&host, &device);
        let first = match &endpoint {
            Some(endpoint) => build_stream(endpoint, shared, rate, channels, format, buffer_frames),
            None => build_stream(&device, shared, rate, channels, format, buffer_frames),
        };
        return match first {
            Ok((stream, actual_channels, actual_format)) => {
                let name = match &endpoint {
                    Some(endpoint) => device_name(endpoint),
                    None => device_name(&device),
                };
                Ok((stream, actual_channels, actual_format, name))
            }
            Err(error) => {
                log::warn!(
                    "audio output via the endpoint failed, retrying on the default device: {error:?}"
                );
                match build_stream(&device, shared, rate, channels, format, buffer_frames) {
                    Ok((stream, actual_channels, actual_format)) => {
                        Ok((stream, actual_channels, actual_format, device_name(&device)))
                    }
                    Err(_) => Err(error),
                }
            }
        };
    }
    // 그 밖의 OS는 기본 장치 핸들이 기본 장치 변경을 따라가므로 그대로 연다.
    #[cfg(not(windows))]
    {
        let (stream, actual_channels, actual_format) =
            build_stream(&device, shared, rate, channels, format, buffer_frames)?;
        Ok((stream, actual_channels, actual_format, device_name(&device)))
    }
}

/// 장치 표시 이름. 이름을 읽지 못하면 `None` 없이 기본 문구를 쓴다.
fn device_name(device: &cpal::Device) -> String {
    device
        .description()
        .map(|description| description.name().to_string())
        .unwrap_or_else(|_| "unknown device".to_string())
}

/// 기본 장치와 같은 엔드포인트를 "특정 장치" 핸들로 돌려준다. ID를 읽지 못하면 `None`이다.
#[cfg(windows)]
fn same_endpoint(host: &cpal::Host, default: &cpal::Device) -> Option<cpal::Device> {
    let id = default.id().ok()?;
    host.device_by_id(&id)
}

/// 한 장치에 출력 스트림을 만든다. `probe_output`이 고른 스테레오 설정을 직접 먼저
/// 요청한다. WASAPI 공유 모드는 `AUTOCONVERTPCM`이라 열거에 없는 스테레오도
/// Windows가 변환해 준다. 직접 요청이 거부된 장치만 열거 일치로 폴백한다.
/// 폴백도 레이트는 유지한다. 레이트가 바뀌면 리샘플러·클럭과 실제 출력이 어긋난다.
/// 성공하면 `(스트림, 실제 채널, 실제 형식)`을 돌려준다.
fn build_stream(
    device: &cpal::Device,
    shared: &Arc<AudioShared>,
    rate: u32,
    channels: u16,
    format: cpal::SampleFormat,
    buffer_frames: Option<u32>,
) -> Result<(cpal::Stream, u16, cpal::SampleFormat), AppError> {
    let direct = cpal::StreamConfig {
        channels,
        sample_rate: rate,
        buffer_size: buffer_frames
            .map(cpal::BufferSize::Fixed)
            .unwrap_or(cpal::BufferSize::Default),
    };
    match build_with_config(device, shared, rate, channels, format, direct) {
        Ok(stream) => Ok((stream, channels, format)),
        Err(direct_err) => {
            let fallback = enum_fallback_config(device, rate, channels, format, buffer_frames);
            match fallback {
                Some((config, actual_channels, actual_format)) => {
                    match build_with_config(
                        device,
                        shared,
                        rate,
                        actual_channels,
                        actual_format,
                        config,
                    ) {
                        Ok(stream) => {
                            if actual_channels != channels || actual_format != format {
                                log::warn!(
                                    "audio output fell back to device config: {rate}Hz {actual_channels}ch {actual_format:?}"
                                );
                            }
                            Ok((stream, actual_channels, actual_format))
                        }
                        Err(_) => Err(direct_err),
                    }
                }
                None => Err(direct_err),
            }
        }
    }
}

/// 열거에서 같은 레이트의 설정을 찾는다. 직접 스테레오 요청이 거부된 장치의
/// 폴백이라 레이트는 바꾸지 않고 채널·형식만 맞춘다. `(설정, 채널, 형식)`이다.
fn enum_fallback_config(
    device: &cpal::Device,
    rate: u32,
    channels: u16,
    format: cpal::SampleFormat,
    buffer_frames: Option<u32>,
) -> Option<(cpal::StreamConfig, u16, cpal::SampleFormat)> {
    let configs: Vec<_> = device.supported_output_configs().ok()?.collect();
    let supported = |c: &cpal::SupportedStreamConfigRange| {
        matches!(
            c.sample_format(),
            cpal::SampleFormat::F32 | cpal::SampleFormat::I16 | cpal::SampleFormat::U16
        )
    };
    // 요청과 정확히 같은 것부터 찾는다. 버퍼 고정 크기는 범위 안일 때만 쓴다.
    let exact = configs.iter().find(|c| {
        supported(c)
            && c.sample_format() == format
            && c.channels() == channels
            && c.min_sample_rate() <= rate
            && rate <= c.max_sample_rate()
    });
    if let Some(picked) = exact {
        return Some((
            config_with_buffer(picked, rate, buffer_frames),
            channels,
            format,
        ));
    }
    // 같은 레이트면 채널·형식이 달라도 연다. 요청 형식·스테레오를 우선한다.
    let mut same_rate: Vec<_> = configs
        .iter()
        .filter(|c| supported(c) && c.min_sample_rate() <= rate && rate <= c.max_sample_rate())
        .collect();
    same_rate.sort_by_key(|c| {
        (
            c.sample_format() != format,
            c.channels() != channels,
            c.channels() != AUDIO_CHANNELS as u16,
        )
    });
    same_rate.first().map(|picked| {
        let actual_channels = picked.channels();
        let actual_format = picked.sample_format();
        (
            config_with_buffer(picked, rate, buffer_frames),
            actual_channels,
            actual_format,
        )
    })
}

/// 열거 설정에 레이트와 버퍼 크기를 입힌다. 고정 크기는 범위 안일 때만 쓴다.
fn config_with_buffer(
    picked: &cpal::SupportedStreamConfigRange,
    rate: u32,
    buffer_frames: Option<u32>,
) -> cpal::StreamConfig {
    let mut config = picked.with_sample_rate(rate).config();
    if let Some(frames) = buffer_frames {
        if let cpal::SupportedBufferSize::Range { min, max } = picked.buffer_size() {
            if (*min..=*max).contains(&frames) {
                config.buffer_size = cpal::BufferSize::Fixed(frames);
            }
        }
    }
    config
}

/// 주어진 `StreamConfig`로 출력 스트림을 열고 시작한다.
fn build_with_config(
    device: &cpal::Device,
    shared: &Arc<AudioShared>,
    rate: u32,
    channels: u16,
    format: cpal::SampleFormat,
    config: cpal::StreamConfig,
) -> Result<cpal::Stream, AppError> {
    use cpal::SampleFormat;
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
            self.carry = 0.0;
            self.primed = false;
            self.fade = 0.0;
            self.last = (0.0, 0.0);
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
            if shared.seek_gen.load(Ordering::SeqCst) != mixer.generation {
                mixer.sync(shared);
            }
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
    let mut eof = shared.eof.load(Ordering::SeqCst);
    let mut played = 0u64;
    for frame in out.chunks_exact_mut(channels) {
        if shared.seek_gen.load(Ordering::SeqCst) != mixer.generation {
            mixer.sync(shared);
            mixer.refill(shared);
            mixer.primed = mixer.ready();
            eof = shared.eof.load(Ordering::SeqCst);
        }
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
                    shared
                        .underrun_local
                        .store(mixer.local.len() as u64, Ordering::SeqCst);
                    shared.underrun_queue.store(
                        shared
                            .queue
                            .try_lock()
                            .map(|queue| queue.len() as u64)
                            .unwrap_or(u64::MAX),
                        Ordering::SeqCst,
                    );
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
    /// 재생 위치 0에 해당하는 컨테이너 시작 시각(ms). 영상과 같은 기준이다.
    origin_ms: i64,
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
    let origin_ms = crate::video_player::container_origin_ms(&ictx);
    Ok(AudioDecoder {
        ictx,
        ctx,
        swr: new_swr(out_rate, in_rate, in_fmt, in_channels)?,
        stream_index,
        time_base,
        origin_ms,
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
    let mut decode_generation = shared.seek_gen.load(Ordering::SeqCst);
    loop {
        // 타임라인을 끌며 탐색이 여러 번 쌓이면 마지막 위치만 적용한다. 이전 세대에서
        // 디코드가 끝난 표본은 출력 큐에 들어가지 않는다.
        let mut latest_seek = None;
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                AudioCommand::Seek {
                    position_ms,
                    generation,
                } => latest_seek = Some((position_ms, generation)),
                AudioCommand::Stop => return Ok(()),
            }
        }
        if let Some((position_ms, generation)) = latest_seek {
            if seek_audio_to(&mut dec, shared, position_ms, generation, &mut target_ms)? {
                decode_generation = generation;
            }
        }
        if shared.seek_gen.load(Ordering::SeqCst) != decode_generation {
            continue;
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
        if pump_audio(&mut dec, shared, &mut target_ms, decode_generation)?
            && shared.seek_gen.load(Ordering::SeqCst) == decode_generation
        {
            shared.eof.store(true, Ordering::SeqCst);
            // 끝까지 디코드했으면 명령만 기다린다.
            match cmd_rx.recv() {
                Ok(AudioCommand::Seek {
                    mut position_ms,
                    mut generation,
                }) => {
                    while let Ok(cmd) = cmd_rx.try_recv() {
                        match cmd {
                            AudioCommand::Seek {
                                position_ms: next_position,
                                generation: next_generation,
                            } => {
                                position_ms = next_position;
                                generation = next_generation;
                            }
                            AudioCommand::Stop => return Ok(()),
                        }
                    }
                    if seek_audio_to(&mut dec, shared, position_ms, generation, &mut target_ms)? {
                        decode_generation = generation;
                    }
                }
                Ok(AudioCommand::Stop) | Err(_) => return Ok(()),
            }
        }
    }
}

/// 현재 세대의 탐색만 적용하고, 이미 대체된 탐색은 건너뛴다.
fn seek_audio_to(
    dec: &mut AudioDecoder,
    shared: &Arc<AudioShared>,
    position_ms: u64,
    generation: u64,
    target_ms: &mut Option<u64>,
) -> Result<bool, String> {
    if shared.seek_gen.load(Ordering::SeqCst) != generation {
        return Ok(false);
    }
    if let Err(error) = seek_audio(dec, position_ms) {
        if shared.seek_gen.load(Ordering::SeqCst) != generation {
            return Ok(false);
        }
        return Err(error);
    }
    clear_audio(shared);
    *target_ms = Some(position_ms);
    shared.eof.store(false, Ordering::SeqCst);
    Ok(true)
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
    generation: u64,
) -> Result<bool, String> {
    loop {
        if shared.seek_gen.load(Ordering::SeqCst) != generation {
            return Ok(false);
        }
        let packet = dec
            .ictx
            .read_packet()
            .map_err(|e| format!("Failed to read packet: {e:?}"))?;
        let Some(mut pkt) = packet else {
            dec.ctx.send_packet(None).ok();
            return flush_audio(dec, shared, target_ms, generation);
        };
        if pkt.stream_index != dec.stream_index as i32 {
            continue;
        }
        crate::video_player::send_packet_lenient(&mut dec.ctx, &pkt);
        pkt.set_stream_index(-1);
        while let Ok(frame) = dec.ctx.receive_frame() {
            push_audio(dec, &frame, shared, target_ms, generation)?;
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
    generation: u64,
) -> Result<bool, String> {
    while let Ok(frame) = dec.ctx.receive_frame() {
        push_audio(dec, &frame, shared, target_ms, generation)?;
    }
    Ok(true)
}

fn push_audio(
    dec: &mut AudioDecoder,
    frame: &rsmpeg::avutil::AVFrame,
    shared: &Arc<AudioShared>,
    target_ms: &mut Option<u64>,
    generation: u64,
) -> Result<(), String> {
    if frame.nb_samples <= 0 {
        return Ok(());
    }
    // 탐색 목표보다 이른 프레임은 버린다.
    if let Some(target) = target_ms.take() {
        let pts =
            crate::video_player::frame_time_ms(frame, dec.time_base, dec.origin_ms).unwrap_or(0);
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
    queue_audio_samples(shared, generation, bytes);
    Ok(())
}

/// 디코드 중 탐색 세대가 바뀌지 않은 경우에만 표본을 출력 큐에 넣는다.
fn queue_audio_samples(shared: &Arc<AudioShared>, generation: u64, samples: &[f32]) -> bool {
    let mut queue = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
    if shared.seek_gen.load(Ordering::SeqCst) != generation {
        return false;
    }
    while queue.len() + samples.len() > QUEUE_PAIRS_MAX * 2 {
        if queue.len() >= 2 {
            queue.pop_front();
            queue.pop_front();
        } else {
            break;
        }
    }
    queue.extend(samples.iter().copied());
    true
}

fn seek_audio(dec: &mut AudioDecoder, target_ms: u64) -> Result<(), String> {
    use rsmpeg::ffi;
    if dec.time_base.1 <= 0 {
        return Err("Invalid time base".to_string());
    }
    let ts = crate::video_player::seek_timestamp(target_ms, dec.time_base, dec.origin_ms);
    dec.ictx
        .seek(
            dec.stream_index as i32,
            ts,
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
            underrun_local: AtomicU64::new(0),
            underrun_queue: AtomicU64::new(0),
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
        let stale_generation = shared.seek_gen.load(Ordering::SeqCst);
        {
            let mut queue = shared.queue.lock().unwrap();
            queue.clear();
            shared.seek_gen.fetch_add(1, Ordering::SeqCst);
        }
        // 탐색 전에 시작한 패킷 디코드가 늦게 끝나도 새 세대 큐에는 넣지 않는다.
        assert!(!queue_audio_samples(
            &shared,
            stale_generation,
            &[0.25; 7200]
        ));
        let out = run(&shared, &mut mixer, 480);
        assert!(
            out[479].abs() < 1e-3,
            "이전 위치의 소리를 이어 쓰면 안 된다"
        );
        let generation = shared.seek_gen.load(Ordering::SeqCst);
        assert!(queue_audio_samples(&shared, generation, &[-0.5; 7200]));
        let out = run(&shared, &mut mixer, 1);
        assert!(
            out[0].abs() < 0.01,
            "새 위치의 소리는 짧게 페이드 인해야 한다"
        );
    }

    #[test]
    fn decodes_audio_from_sample_with_audio() {
        // 출력 장치 없이 디코드 경로만 검증한다. 오디오 없는 샘플은 디코더가 없다.
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples")
            .join("sample-audio.mp4");
        assert!(path.is_file(), "missing sample: {}", path.to_string_lossy());
        let probed = crate::video::probe(&path).unwrap();
        assert!(probed.has_audio);
        let mut dec = open_audio_decoder(&path, PREFERRED_RATE).unwrap();
        let shared = shared_with(true);
        let mut target_ms = None;
        let eof = pump_audio(&mut dec, &shared, &mut target_ms, 0).unwrap();
        assert!(eof, "tiny sample should decode to EOF");
        assert!(
            buffered_pairs(&shared) > 0,
            "expected decoded samples in the queue"
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
