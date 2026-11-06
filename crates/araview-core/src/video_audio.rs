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
    /// 핸들이 버려지는 중이다. 디코드 루프가 명령을 기다리지 않고 곧바로 끝낸다.
    stop: AtomicBool,
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
        Self::open_with(path, rate, channels, format)
    }

    /// 정해진 출력 설정으로 디코드 워커를 시작한다. 장치를 조회하지 않으므로 출력
    /// 장치가 없는 환경의 테스트도 디코드·정렬 경로를 그대로 검증할 수 있다.
    pub(crate) fn open_with(
        path: &Path,
        rate: u32,
        channels: u16,
        format: cpal::SampleFormat,
    ) -> Result<Self, AppError> {
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
            stop: AtomicBool::new(false),
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
            // 시간 안에 열리지 않았다. 스레드는 장치 열기(`open_stream`)에서 막혀 있을 수
            // 있으므로 기다리지 않고 떼어 낸다. 늦게 열리더라도 `ready_tx` 전송이 실패해
            // 스트림을 버리고 스스로 끝난다. 여기서 `join`하면 타임아웃이 무의미해지고
            // 호출 스레드가 장치가 풀릴 때까지 멈춘다.
            Err(_) => {
                drop(stop_tx);
                drop(thread);
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
                ms_to_pairs_ceil(position_ms, self.sample_rate),
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
    /// 테스트용: 오디오 워커가 오류로 멈춘 상태를 흉내 낸다.
    #[cfg(test)]
    pub(crate) fn inject_error_for_test(&self, played_ms: u64, error: &str) {
        self.shared.played_pairs.store(
            ms_to_pairs_ceil(played_ms, self.sample_rate),
            Ordering::SeqCst,
        );
        *self.shared.error.lock().unwrap() = Some(error.to_string());
    }

    /// 테스트용: 공유 큐에 쌓인 표본(인터리브 스테레오)의 사본.
    #[cfg(test)]
    pub(crate) fn queued_for_test(&self) -> Vec<f32> {
        self.shared.queue.lock().unwrap().iter().copied().collect()
    }

    /// 테스트용: 디코드가 파일 끝에 닿았는가.
    #[cfg(test)]
    pub(crate) fn eof_for_test(&self) -> bool {
        self.shared.eof.load(Ordering::SeqCst)
    }
}

/// 위치(ms)를 샘플 쌍 수로 올림해 바꾼다. 내림하면 `played_ms`가 1ms 작게 돌아와
/// 44.1kHz에서 끝으로 탐색해도 끝에 닿지 않는다.
fn ms_to_pairs_ceil(position_ms: u64, rate: u32) -> u64 {
    position_ms.saturating_mul(u64::from(rate)).div_ceil(1000)
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
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
        let target = endpoint.as_ref().unwrap_or(&device);
        match build_stream(target, shared, rate, channels, format, buffer_frames) {
            Ok((stream, actual_channels, actual_format)) => {
                Ok((stream, actual_channels, actual_format, device_name(target)))
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
        }
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
    /// 공급 상태: 선행 버퍼 대기 / 재생 / 파일 끝에서 소진.
    feed: Feed,
    /// 마지막으로 본 탐색 세대.
    generation: u64,
    /// 0..1 페이드 값. 표본을 내면 늘고, 비거나 정지하면 준다. 공급 상태와 별개로
    /// 출력 표본마다 조금씩 움직이는 연속 값이라 `feed`에 넣지 않는다.
    fade: f32,
    /// 마지막으로 낸 표본(게인 전). 페이드 아웃에 쓴다.
    last: (f32, f32),
}

/// 콜백의 공급 상태. 예전에는 `primed: bool`과 `stalled: Option<Instant>` 두 필드였다.
///
/// 전이:
/// - 탐색(세대 변경): 무엇이든 → `Buffering { since: None }`
/// - 선행 버퍼가 참: `Buffering` → `Running`
/// - 표본을 냄: `Running`/`Drained` → `Running`
/// - 비었는데 파일 끝이 아님(언더런): `Running`/`Drained` → `Buffering { since: Some(..) }`
/// - 비었고 파일 끝: `Running` → `Drained`
/// - 선행 대기 중 비었음: `Buffering` → 빈 채로 기다린 시각을 기록
#[derive(Debug, Clone, Copy, PartialEq)]
enum Feed {
    /// 선행 버퍼가 차기를 기다리며 소리를 내지 않는다(탐색 직후, 언더런 뒤).
    /// `since`는 큐가 빈 채로 기다리기 시작한 시각이고, 아직 비어 본 적이 없으면 `None`이다.
    Buffering { since: Option<Instant> },
    /// 선행 버퍼가 차서 소리를 내는 중이다.
    Running,
    /// 소리를 내다가 디코드가 파일 끝에 닿아 표본이 떨어졌다. 클럭만 진행한다.
    Drained { since: Instant },
}

impl Feed {
    const fn new() -> Self {
        Feed::Buffering { since: None }
    }

    /// 표본을 꺼내 소리를 낼 수 있는 상태인가(선행 버퍼를 한 번 채웠다).
    fn is_primed(&self) -> bool {
        matches!(self, Feed::Running | Feed::Drained { .. })
    }

    /// 선행 대기 중이고 선행 버퍼가 찼으면 재생으로 넘긴다.
    fn prime_if(&mut self, ready: bool) {
        if ready && !self.is_primed() {
            *self = Feed::Running;
        }
    }

    /// 표본을 냈다.
    fn played(&mut self) {
        *self = Feed::Running;
    }

    /// 낼 표본이 없다. 선행이 찬 상태에서 파일 끝이 아닌데 마르면 언더런이라 `true`를
    /// 돌려주고 다시 선행 대기로 간다. 빈 채로 기다린 시각은 처음 빈 시각을 유지하고,
    /// `now`는 새로 기록할 때만 부른다(실시간 콜백에서 표본마다 시계를 읽지 않는다).
    fn starve(&mut self, eof: bool, now: impl FnOnce() -> Instant) -> bool {
        match *self {
            Feed::Running if !eof => {
                *self = Feed::Buffering { since: Some(now()) };
                true
            }
            Feed::Drained { since } if !eof => {
                *self = Feed::Buffering { since: Some(since) };
                true
            }
            Feed::Running => {
                *self = Feed::Drained { since: now() };
                false
            }
            Feed::Buffering { since: None } => {
                *self = Feed::Buffering { since: Some(now()) };
                false
            }
            Feed::Buffering { since: Some(_) } | Feed::Drained { .. } => false,
        }
    }

    /// 큐가 빈 채로 기다리기 시작한 시각.
    fn stalled_since(&self) -> Option<Instant> {
        match *self {
            Feed::Buffering { since } => since,
            Feed::Drained { since } => Some(since),
            Feed::Running => None,
        }
    }
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
            feed: Feed::new(),
            generation: 0,
            fade: 0.0,
            last: (0.0, 0.0),
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
            self.feed = Feed::new();
            self.fade = 0.0;
            self.last = (0.0, 0.0);
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
        self.feed
            .stalled_since()
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
    mixer.feed.prime_if(mixer.ready());
    let mut eof = shared.eof.load(Ordering::SeqCst);
    let mut played = 0u64;
    for frame in out.chunks_exact_mut(channels) {
        if shared.seek_gen.load(Ordering::SeqCst) != mixer.generation {
            mixer.sync(shared);
            mixer.refill(shared);
            mixer.feed.prime_if(mixer.ready());
            eof = shared.eof.load(Ordering::SeqCst);
        }
        let sample = if mixer.feed.is_primed() {
            mixer.next_sample(speed)
        } else {
            None
        };
        match sample {
            Some((left, right)) => {
                mixer.feed.played();
                mixer.fade = (mixer.fade + fade_step).min(1.0);
                mixer.last = (left, right);
                let gain = volume * mixer.fade;
                write_frame(frame, channels, left * gain, right * gain, conv);
                played += mixer.advance_media(speed);
            }
            None => {
                // 선행 버퍼가 마르면 다시 찰 때까지 기다린다. 클럭도 세운다.
                if mixer.feed.starve(eof, Instant::now) {
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
    /// 리샘플러. 첫 프레임의 실제 입력 설정(레이트·형식·채널 배치)으로 만들고,
    /// 스트림 중간에 설정이 바뀌면 다시 만든다. 탐색 뒤에는 비운다.
    swr: Option<Resampler>,
    stream_index: usize,
    time_base: (i32, i32),
    /// 재생 위치 0에 해당하는 컨테이너 시작 시각(ms). 영상과 같은 기준이다.
    origin_ms: i64,
    out_rate: u32,
    /// 탐색 목표(또는 시작점) 앞을 메울 무음(샘플 쌍). 출력 큐에 자리가 날 때마다 넣는다.
    pending_silence: usize,
    /// 무음 뒤에 이어 넣을 표본. 무음이 남아 있는 동안 디코드한 소리를 순서대로 담는다.
    pending_samples: VecDeque<f32>,
}

impl AudioDecoder {
    fn has_pending(&self) -> bool {
        self.pending_silence > 0 || !self.pending_samples.is_empty()
    }

    fn clear_pending(&mut self) {
        self.pending_silence = 0;
        self.pending_samples.clear();
    }
}

/// 리샘플러와 그것을 만든 입력 설정.
struct Resampler {
    ctx: rsmpeg::swresample::SwrContext,
    rate: i32,
    format: i32,
    layout: rsmpeg::avutil::AVChannelLayout,
}

impl Resampler {
    /// 프레임의 입력 설정이 이 리샘플러와 같은가.
    fn matches(&self, frame: &rsmpeg::avutil::AVFrame) -> bool {
        if self.rate != frame.sample_rate || self.format != frame.format {
            return false;
        }
        let input = input_layout(frame);
        // SAFETY: 두 포인터 모두 살아 있는 초기화된 채널 배치를 가리킨다.
        unsafe { rsmpeg::ffi::av_channel_layout_compare(&*self.layout, &*input) == 0 }
    }
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
        swr: None,
        stream_index,
        time_base,
        origin_ms,
        out_rate,
        pending_silence: 0,
        pending_samples: VecDeque::new(),
    })
}

/// 프레임의 입력 채널 배치. 순서가 지정되지 않았거나 잘못된 배치는 채널 수 기준
/// 기본 배치로 본다(swresample은 지정되지 않은 배치를 받지 않는다).
fn input_layout(frame: &rsmpeg::avutil::AVFrame) -> rsmpeg::avutil::AVChannelLayout {
    let raw = &frame.ch_layout;
    let channels = raw.nb_channels.max(1);
    // SAFETY: `raw`는 디코더가 채운 프레임의 채널 배치다. 검사만 하고 바꾸지 않는다.
    let valid = unsafe { rsmpeg::ffi::av_channel_layout_check(raw) } == 1;
    if valid && raw.order != rsmpeg::ffi::AV_CHANNEL_ORDER_UNSPEC {
        let mut copy = std::mem::MaybeUninit::<rsmpeg::ffi::AVChannelLayout>::zeroed();
        // SAFETY: 복사 대상은 0으로 채운 저장소이고, 성공하면 초기화된 배치를 소유한다.
        if unsafe { rsmpeg::ffi::av_channel_layout_copy(copy.as_mut_ptr(), raw) } == 0 {
            // SAFETY: 위 복사가 성공해 `copy`가 올바른 배치를 담고 있다.
            return unsafe { rsmpeg::avutil::AVChannelLayout::new(copy.assume_init()) };
        }
    }
    rsmpeg::avutil::AVChannelLayout::from_nb_channels(channels)
}

/// 프레임의 실제 입력 설정으로 리샘플러를 만들어 초기화한다. 채널 수만 보고 기본
/// 배치를 가정하면 AAC 5.1(back)·AC-3 5.1(side)처럼 기본값과 다른 배치에서
/// `swr_convert_frame`이 입력이 바뀌었다는 오류(`AVERROR_INPUT_CHANGED`)를 낸다.
fn new_swr(out_rate: u32, frame: &rsmpeg::avutil::AVFrame) -> Result<Resampler, String> {
    use rsmpeg::ffi;
    let out_layout = rsmpeg::avutil::AVChannelLayout::from_nb_channels(AUDIO_CHANNELS as i32);
    let in_layout = input_layout(frame);
    let mut ctx = rsmpeg::swresample::SwrContext::new(
        &out_layout,
        ffi::AV_SAMPLE_FMT_FLT,
        out_rate as i32,
        &in_layout,
        frame.format,
        frame.sample_rate,
    )
    .map_err(|e| format!("Failed to create resampler: {e:?}"))?;
    ctx.init()
        .map_err(|e| format!("Failed to init resampler: {e:?}"))?;
    Ok(Resampler {
        ctx,
        rate: frame.sample_rate,
        format: frame.format,
        layout: in_layout,
    })
}

/// 탐색 목표(또는 재생 시작점)에 맞춰 첫 프레임을 놓는 방법.
#[derive(Debug, PartialEq, Eq)]
enum Placement {
    /// 프레임 전체가 목표보다 앞이다. 버린다.
    Drop,
    /// 앞에 무음 `silence` 쌍을 넣고, 프레임의 앞 `skip` 쌍을 잘라 넣는다.
    Place { silence: usize, skip: usize },
}

/// 첫 오디오 프레임이 늦게 시작하면 이 길이까지만 무음으로 메운다. 그보다 크면
/// 타임스탬프가 끊긴 것으로 보고 메우지 않는다.
const MAX_LEAD_SILENCE_MS: f64 = 10_000.0;

/// 오디오 클럭(출력한 샘플 쌍)은 목표 위치에서 시작한다. 첫 프레임의 pts가 목표보다
/// 늦으면 그 차이만큼 무음을 먼저 내고, 목표에 걸친 프레임은 목표 이전 샘플만 잘라
/// 낸다. 그래야 오디오가 늦게 시작하는 파일(MPEG-TS, MKV, MP4)과 탐색 직후에도 소리가
/// 영상과 같은 시각에 난다. `frame_pairs`는 출력 레이트 기준 프레임 길이다.
fn place_first_frame(start_ms: f64, frame_pairs: usize, target_ms: u64, rate: u32) -> Placement {
    let per_ms = f64::from(rate) / 1000.0;
    let start = (start_ms * per_ms).round() as i64;
    let target = (target_ms as f64 * per_ms).round() as i64;
    let end = start.saturating_add(frame_pairs as i64);
    if end <= target {
        return Placement::Drop;
    }
    if start >= target {
        let gap_ms = (start - target) as f64 / per_ms;
        let silence = if gap_ms <= MAX_LEAD_SILENCE_MS {
            (start - target) as usize
        } else {
            log::debug!("audio starts {gap_ms:.0}ms after the target; not padding");
            0
        };
        return Placement::Place { silence, skip: 0 };
    }
    Placement::Place {
        silence: 0,
        skip: (target - start) as usize,
    }
}

fn audio_loop(
    source: &Path,
    out_rate: u32,
    cmd_rx: &mpsc::Receiver<AudioCommand>,
    shared: &Arc<AudioShared>,
) -> Result<(), String> {
    let mut dec = open_audio_decoder(source, out_rate)?;
    // 재생 시작도 위치 0으로의 탐색처럼 정렬한다. 오디오가 늦게 시작하면 앞을 무음으로 메운다.
    let mut target_ms: Option<u64> = Some(0);
    let mut decode_generation = shared.seek_gen.load(Ordering::SeqCst);
    loop {
        if shared.stop.load(Ordering::SeqCst) {
            return Ok(());
        }
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
        if dec.has_pending() {
            push_pending(&mut dec, shared, decode_generation);
            continue;
        }
        let at_eof = pump_audio(&mut dec, shared, &mut target_ms, decode_generation)?;
        if at_eof
            && !dec.has_pending()
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
    dec.clear_pending();
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

/// 디코드를 쉴 만큼 찼는가. 버림 상한(`queue_audio_samples`)보다 충분히 낮아야
/// 재생 중 새 패킷이 곧 재생될 소리를 밀어내지 않는다.
fn audio_queue_full(shared: &Arc<AudioShared>) -> bool {
    shared.queue.lock().unwrap_or_else(|e| e.into_inner()).len() >= QUEUE_PAIRS_MAX * AUDIO_CHANNELS
}

/// 대기 중인 무음과 표본을 출력 큐의 빈자리만큼 넣는다. 무음이 먼저, 표본이 뒤다.
/// 긴 무음도 큐 상한을 넘겨 앞쪽 소리를 버리지 않도록 나눠 넣는다.
fn push_pending(dec: &mut AudioDecoder, shared: &Arc<AudioShared>, generation: u64) {
    let room = {
        let queue = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        (QUEUE_PAIRS_MAX * AUDIO_CHANNELS).saturating_sub(queue.len())
    };
    let room = room - room % AUDIO_CHANNELS;
    if room == 0 {
        return;
    }
    if dec.pending_silence > 0 {
        let pairs = dec.pending_silence.min(room / AUDIO_CHANNELS);
        if queue_audio_samples(shared, generation, &vec![0.0; pairs * AUDIO_CHANNELS]) {
            dec.pending_silence -= pairs;
        }
        return;
    }
    let take = dec.pending_samples.len().min(room);
    let chunk: Vec<f32> = dec.pending_samples.drain(..take).collect();
    queue_audio_samples(shared, generation, &chunk);
}

/// 디코드를 멈춰야 하는가. 새 탐색이 걸렸거나 핸들이 버려지는 중이다.
fn audio_interrupted(shared: &Arc<AudioShared>, generation: u64) -> bool {
    shared.stop.load(Ordering::SeqCst) || shared.seek_gen.load(Ordering::SeqCst) != generation
}

/// 패킷을 읽어 f32 스테레오로 큐에 넣는다. EOF면 `true`다.
fn pump_audio(
    dec: &mut AudioDecoder,
    shared: &Arc<AudioShared>,
    target_ms: &mut Option<u64>,
    generation: u64,
) -> Result<bool, String> {
    loop {
        if audio_interrupted(shared, generation) {
            return Ok(false);
        }
        let packet = crate::video_player::read_packet_lenient(&mut dec.ictx, || {
            audio_interrupted(shared, generation)
        });
        if audio_interrupted(shared, generation) {
            return Ok(false);
        }
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
        // 앞에 메울 무음이 생겼으면 큐 자리를 보며 나눠 넣도록 루프로 돌아간다.
        if dec.has_pending() || audio_queue_full(shared) {
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
    if frame.nb_samples <= 0 || frame.sample_rate <= 0 {
        return Ok(());
    }
    // 탐색 목표(또는 시작점) 뒤 첫 프레임의 시각. pts를 모르면 정렬하지 않는다.
    let first_start = match *target_ms {
        Some(target) => {
            match crate::video_player::frame_time_ms_signed(frame, dec.time_base, dec.origin_ms) {
                Some(start) => {
                    let length_ms =
                        f64::from(frame.nb_samples) * 1000.0 / f64::from(frame.sample_rate);
                    // 목표보다 완전히 앞선 프레임은 리샘플러에 넣지 않고 버린다.
                    if start + length_ms <= target as f64 {
                        return Ok(());
                    }
                    Some((start, target))
                }
                None => {
                    *target_ms = None;
                    None
                }
            }
        }
        None => None,
    };
    // 입력 설정이 바뀌면(스트림 중간 레이트·채널 변경) 리샘플러를 다시 만든다.
    if !dec.swr.as_ref().is_some_and(|swr| swr.matches(frame)) {
        if dec.swr.is_some() {
            log::debug!(
                "audio input changed to {}Hz {}ch, rebuilding the resampler",
                frame.sample_rate,
                frame.ch_layout.nb_channels
            );
        }
        dec.swr = Some(new_swr(dec.out_rate, frame)?);
    }
    let Some(swr) = dec.swr.as_ref() else {
        return Ok(());
    };
    let mut out = rsmpeg::avutil::AVFrame::new();
    out.set_format(rsmpeg::ffi::AV_SAMPLE_FMT_FLT);
    let layout = rsmpeg::avutil::AVChannelLayout::from_nb_channels(AUDIO_CHANNELS as i32);
    out.set_ch_layout(layout.into_inner());
    out.set_sample_rate(dec.out_rate as i32);
    out.set_nb_samples(swr.ctx.get_out_samples(frame.nb_samples).max(1));
    out.alloc_buffer()
        .map_err(|e| format!("Failed to allocate audio frame: {e:?}"))?;
    swr.ctx
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
    let bytes = match first_start {
        Some((start, target)) => {
            *target_ms = None;
            match place_first_frame(start, samples / AUDIO_CHANNELS, target, dec.out_rate) {
                Placement::Drop => return Ok(()),
                Placement::Place { silence, skip } => {
                    dec.pending_silence = silence;
                    &bytes[(skip * AUDIO_CHANNELS).min(bytes.len())..]
                }
            }
        }
        None => bytes,
    };
    if dec.has_pending() {
        // 앞에 넣을 무음이 남아 있으면 순서를 지키도록 뒤에 모아 둔다.
        dec.pending_samples.extend(bytes.iter().copied());
    } else {
        queue_audio_samples(shared, generation, bytes);
    }
    Ok(())
}

/// 디코드 중 탐색 세대가 바뀌지 않은 경우에만 표본을 출력 큐에 넣는다.
fn queue_audio_samples(shared: &Arc<AudioShared>, generation: u64, samples: &[f32]) -> bool {
    let mut queue = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
    if shared.seek_gen.load(Ordering::SeqCst) != generation {
        return false;
    }
    // 디코더는 `audio_queue_full`에서 멈추므로 정상 재생에서는 여기에 닿지 않는다.
    // 그 기준과 같으면 재생 중 패킷마다 앞쪽 소리를 버려 소리가 건너뛴다. 영상
    // 큐처럼 두 배를 넘을 때만 버려 큐가 끝없이 붇는 것만 막는다.
    while queue.len() + samples.len() > QUEUE_PAIRS_MAX * AUDIO_CHANNELS * 2 {
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
    dec.swr = None;
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
            stop: AtomicBool::new(false),
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
    fn steady_decode_never_discards_audio_about_to_play() {
        // 재생 중에는 큐가 늘 상한 근처다. 콜백이 조금 꺼내 가면 디코더가 패킷
        // 하나를 더 넣는데, 이때 곧 재생될 앞쪽 소리를 버리면 소리가 건너뛰어
        // 끊기고 영상보다 앞서 간다.
        let shared = shared_with(true);
        let generation = shared.seek_gen.load(Ordering::SeqCst);
        let packet: Vec<f32> = (0..2048).map(|i| i as f32).collect();
        while !audio_queue_full(&shared) {
            assert!(queue_audio_samples(&shared, generation, &packet));
        }
        let front = shared.queue.lock().unwrap()[0];
        // 콜백이 10ms(480쌍)씩 꺼내 가 상한 아래로 막 내려온 상태.
        while audio_queue_full(&shared) {
            shared.queue.lock().unwrap().drain(..480 * AUDIO_CHANNELS);
        }
        let before = shared.queue.lock().unwrap().len();
        let next_front = shared.queue.lock().unwrap()[0];
        assert!(queue_audio_samples(&shared, generation, &packet));
        let queue = shared.queue.lock().unwrap();
        assert_eq!(
            queue.len(),
            before + packet.len(),
            "디코드한 소리를 버리면 안 된다"
        );
        assert_eq!(
            queue[0], next_front,
            "곧 재생될 소리가 그대로 맨 앞이어야 한다"
        );
        assert_eq!(front, 0.0);
    }

    #[test]
    fn feed_transitions_match_the_old_primed_and_stalled_flags() {
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_millis(10);
        let mut feed = Feed::new();
        assert_eq!(feed, Feed::Buffering { since: None });
        assert!(!feed.is_primed());
        assert_eq!(feed.stalled_since(), None);

        // 선행 대기 중 비면 처음 빈 시각만 남긴다. 언더런은 아니다.
        assert!(!feed.starve(false, || t0));
        assert!(!feed.starve(false, || t1));
        assert_eq!(feed, Feed::Buffering { since: Some(t0) });
        // 선행이 안 찼으면 그대로다.
        feed.prime_if(false);
        assert!(!feed.is_primed());
        feed.prime_if(true);
        assert_eq!(feed, Feed::Running);
        assert_eq!(feed.stalled_since(), None);

        // 재생 중 마르면 언더런이고 선행 대기로 돌아간다.
        assert!(feed.starve(false, || t1));
        assert_eq!(feed, Feed::Buffering { since: Some(t1) });
        feed.prime_if(true);
        feed.played();

        // 파일 끝에서 마르면 언더런이 아니고 소진 상태로 클럭만 진행한다.
        assert!(!feed.starve(true, || t0));
        assert_eq!(feed, Feed::Drained { since: t0 });
        assert!(feed.is_primed());
        assert!(!feed.starve(true, || t1));
        assert_eq!(feed.stalled_since(), Some(t0));
        // 소진 뒤 남은 표본이 나오면 다시 재생이다.
        feed.played();
        assert_eq!(feed, Feed::Running);
        // 소진 상태에서 파일 끝이 아니게 마르면 언더런이며 처음 빈 시각을 유지한다.
        let _ = feed.starve(true, || t0);
        assert!(feed.starve(false, || t1));
        assert_eq!(feed, Feed::Buffering { since: Some(t0) });
        // 선행 대기에서는 `prime_if`만 재생으로 넘긴다.
        feed.prime_if(true);
        assert!(feed.is_primed());
    }

    #[test]
    fn seek_resets_the_feed_to_buffering() {
        let shared = shared_with(true);
        let mut mixer = Mixer::new(PREFERRED_RATE);
        prime(&shared, &mut mixer, 1.0);
        assert_eq!(mixer.feed, Feed::Running);
        shared.seek_gen.fetch_add(1, Ordering::SeqCst);
        mixer.sync(&shared);
        assert_eq!(mixer.feed, Feed::Buffering { since: None });
    }

    #[test]
    fn eof_after_playing_drains_and_keeps_the_clock_running() {
        let shared = shared_with(true);
        let mut mixer = Mixer::new(PREFERRED_RATE);
        push(&shared, &[(0.5, 0.5); 4_800]);
        let _ = run(&shared, &mut mixer, 480);
        assert_eq!(mixer.feed, Feed::Running);
        shared.eof.store(true, Ordering::SeqCst);
        let _ = run(&shared, &mut mixer, 9_600);
        assert!(matches!(mixer.feed, Feed::Drained { .. }));
        assert_eq!(shared.underruns.load(Ordering::SeqCst), 0);
        let before = shared.played_pairs.load(Ordering::SeqCst);
        let _ = run(&shared, &mut mixer, 480);
        assert_eq!(shared.played_pairs.load(Ordering::SeqCst), before + 480);
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

    fn sample(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples")
            .join(name)
    }

    /// 출력 장치 없이 디코드 워커를 돌려 파일 끝까지 큐에 담는다.
    fn decode_all(name: &str) -> (AudioOutput, Vec<f32>) {
        let audio = AudioOutput::open_with(
            &sample(name),
            PREFERRED_RATE,
            AUDIO_CHANNELS as u16,
            cpal::SampleFormat::F32,
        )
        .unwrap();
        audio.set_playing(true);
        for _ in 0..500 {
            if audio.eof_for_test() || audio.has_error().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(audio.has_error(), None, "{name}: audio worker failed");
        assert!(audio.eof_for_test(), "{name}: never reached eof");
        let queued = audio.queued_for_test();
        (audio, queued)
    }

    /// 큐 앞쪽 무음 길이(ms).
    fn leading_silence_ms(samples: &[f32]) -> u64 {
        let pairs = samples
            .as_chunks::<AUDIO_CHANNELS>()
            .0
            .iter()
            .take_while(|pair| pair.iter().all(|s| s.abs() < 1e-4))
            .count();
        pairs as u64 * 1000 / u64::from(PREFERRED_RATE)
    }

    #[test]
    fn surround_layouts_other_than_the_default_decode() {
        // AC-3 5.1(side)는 채널 수 기준 기본 배치와 다르다. 기본 배치로 리샘플러를
        // 만들면 첫 프레임에서 `AVERROR_INPUT_CHANGED`로 워커가 멈췄다.
        let (_audio, queued) = decode_all("sample-audio-51side.mkv");
        assert!(queued.len() > PREFERRED_RATE as usize / 4);
        assert!(
            queued.iter().any(|s| s.abs() > 0.01),
            "decoded only silence"
        );
    }

    #[test]
    fn late_audio_start_is_padded_with_silence() {
        // 오디오가 컨테이너 시작보다 979ms 늦게 시작한다. 앞을 무음으로 메우지 않으면
        // 소리가 영상보다 그만큼 앞선다.
        let (_audio, queued) = decode_all("sample-late-audio.mkv");
        let lead = leading_silence_ms(&queued);
        assert!((930..=1030).contains(&lead), "leading silence {lead}ms");
    }

    #[test]
    fn audio_starting_at_zero_is_not_padded() {
        let (_audio, queued) = decode_all("sample-audio.mp4");
        assert!(leading_silence_ms(&queued) < 60, "unexpected padding");
    }

    #[test]
    fn first_frame_is_placed_on_the_target() {
        // 48kHz, 1024쌍(21.3ms) 프레임.
        assert_eq!(
            place_first_frame(500.0, 1024, 0, 48_000),
            Placement::Place {
                silence: 24_000,
                skip: 0
            }
        );
        // 목표에 걸친 프레임은 목표 이전 샘플만 잘라 낸다.
        assert_eq!(
            place_first_frame(990.0, 1024, 1000, 48_000),
            Placement::Place {
                silence: 0,
                skip: 480
            }
        );
        assert_eq!(
            place_first_frame(900.0, 1024, 1000, 48_000),
            Placement::Drop
        );
        assert_eq!(
            place_first_frame(1000.0, 1024, 1000, 48_000),
            Placement::Place {
                silence: 0,
                skip: 0
            }
        );
        // 시작보다 이른 프레임(음수 pts)도 같은 규칙이다.
        assert_eq!(
            place_first_frame(-10.0, 1024, 0, 48_000),
            Placement::Place {
                silence: 0,
                skip: 480
            }
        );
        // 끊긴 타임스탬프로 보이는 큰 간격은 메우지 않는다.
        assert_eq!(
            place_first_frame(60_000.0, 1024, 0, 48_000),
            Placement::Place {
                silence: 0,
                skip: 0
            }
        );
    }

    #[test]
    fn seek_position_round_trips_at_44_1khz() {
        for position in [0u64, 1, 999, 1_000, 12_345, 3_600_001] {
            let pairs = ms_to_pairs_ceil(position, 44_100);
            assert_eq!(pairs * 1000 / 44_100, position, "{position}ms");
        }
    }
}
