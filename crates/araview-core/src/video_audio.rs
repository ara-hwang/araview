//! 동영상 오디오 디코드·출력 (M2, §7.6).
//!
//! 워커 스레드가 오디오를 디코드해 스테레오 f32로 리샘플링하고, cpal 출력
//! 스트림이 콜백으로 가져간다. 오디오가 있으면 오디오 클럭이 마스터다.
//! 장치를 열 수 없으면 호출자가 영상만 재생하도록 `Err`를 돌려준다.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::app_error::AppError;

/// 출력 채널 수. 모노·5.1 등은 스테레오로 내린다.
pub const AUDIO_CHANNELS: usize = 2;
/// 우선 시도하는 출력 레이트.
pub const PREFERRED_RATE: u32 = 48_000;
/// 디코드 선행 버퍼. 스테레오 인터리브 f32 쌍 기준 4초다.
const QUEUE_PAIRS_MAX: usize = 48_000 * 4;
/// 기본 음량. 설정이 덮어쓴다.
pub const DEFAULT_VOLUME: f32 = 0.8;

pub struct AudioShared {
    queue: Mutex<VecDeque<f32>>,
    playing: AtomicBool,
    muted: AtomicBool,
    volume_bits: AtomicU32,
    speed_bits: AtomicU32,
    played_pairs: AtomicU64,
    eof: AtomicBool,
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
                    && c.min_sample_rate().0 <= PREFERRED_RATE
                    && PREFERRED_RATE <= c.max_sample_rate().0
            })
            .or_else(|| {
                configs
                    .iter()
                    .find(|c| c.sample_format() == SampleFormat::F32)
            })
            .or_else(|| configs.first())
            .ok_or_else(|| AppError::unknown("No audio output configs"))?;
        let rate = if picked.min_sample_rate().0 <= PREFERRED_RATE
            && PREFERRED_RATE <= picked.max_sample_rate().0
        {
            PREFERRED_RATE
        } else {
            picked.max_sample_rate().0
        };
        Ok((rate, picked.channels(), picked.sample_format()))
    }

    /// cpal 출력 스트림을 연다. `!Send`라 UI 스레드에서 호출하고 들고 있는다.
    pub fn open_output_stream(&self) -> Result<AudioStream, AppError> {
        open_stream(&self.shared, self.sample_rate, self.channels, self.format).map(AudioStream)
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
        let _ = self.cmd_tx.send(AudioCommand::Seek(position_ms));
    }

    /// 출력한 샘플 기준 위치. 오디오 마스터 클럭이다.
    pub fn played_ms(&self) -> u64 {
        self.shared.played_pairs.load(Ordering::SeqCst) * 1000 / self.sample_rate as u64
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
) -> Result<cpal::Stream, AppError> {
    use cpal::{SampleFormat, SampleRate};
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| AppError::unknown("No audio output device"))?;
    let configs: Vec<_> = device
        .supported_output_configs()
        .map_err(|e| AppError::unknown(format!("No audio output configs: {e}")))?
        .collect();
    // probe_output이 고른 설정과 같은 것을 찾는다. 없으면 첫 F32 설정을 쓴다.
    let picked = configs
        .iter()
        .find(|c| {
            c.sample_format() == format
                && c.channels() == channels
                && c.min_sample_rate().0 <= rate
                && rate <= c.max_sample_rate().0
        })
        .or_else(|| {
            configs
                .iter()
                .find(|c| c.sample_format() == SampleFormat::F32)
        })
        .or_else(|| configs.first())
        .ok_or_else(|| AppError::unknown("No audio output configs"))?;
    let rate = if picked.min_sample_rate().0 <= rate && rate <= picked.max_sample_rate().0 {
        rate
    } else {
        picked.max_sample_rate().0
    };
    let channels = picked.channels();
    let format = picked.sample_format();
    let config = (*picked).with_sample_rate(SampleRate(rate)).config();
    let err_fn = |e| log::warn!("audio output error: {e}");
    let stream = match format {
        SampleFormat::F32 => {
            let worker = Arc::clone(shared);
            device.build_output_stream(
                &config,
                move |out: &mut [f32], _| fill_output(&worker, channels, out, |s| s),
                err_fn,
                None,
            )
        }
        SampleFormat::I16 => {
            let worker = Arc::clone(shared);
            device.build_output_stream(
                &config,
                move |out: &mut [i16], _| {
                    fill_output(&worker, channels, out, |s| {
                        (s.clamp(-1.0, 1.0) * 32767.0) as i16
                    })
                },
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let worker = Arc::clone(shared);
            device.build_output_stream(
                &config,
                move |out: &mut [u16], _| {
                    fill_output(&worker, channels, out, |s| {
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

/// 콜백이 잠깐 쥐고 Worker가 채우는 구조라, 콜백 안에서 큐를 직접 꺼낸다.
/// 정지면 침묵만 내고, 음소거면 소모는 이어가고 0을 낸다.
fn fill_output<T>(shared: &Arc<AudioShared>, channels: u16, out: &mut [T], conv: fn(f32) -> T) {
    let channels = channels.max(1) as usize;
    if !shared.playing.load(Ordering::SeqCst) {
        for slot in out.iter_mut() {
            *slot = conv(0.0);
        }
        return;
    }
    let muted = shared.muted.load(Ordering::SeqCst);
    let volume = f32::from_bits(shared.volume_bits.load(Ordering::SeqCst));
    let speed = f32::from_bits(shared.speed_bits.load(Ordering::SeqCst));
    let speed = if speed.is_finite() && speed > 0.0 {
        speed
    } else {
        1.0
    };
    let frames = out.len() / channels;
    let need_pairs = (frames as f32 * speed).ceil() as usize;
    let mut queue = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
    let mut taken = 0usize;
    for frame in out.chunks_exact_mut(channels) {
        let (left, right) = if taken < need_pairs && queue.len() >= 2 {
            taken += 1;
            let left = queue.pop_front().unwrap_or(0.0);
            let right = queue.pop_front().unwrap_or(0.0);
            (left, right)
        } else {
            (0.0, 0.0)
        };
        for (i, slot) in frame.iter_mut().enumerate() {
            let sample = if channels == 1 {
                (left + right) * 0.5
            } else if i < 2 {
                if i == 0 {
                    left
                } else {
                    right
                }
            } else {
                0.0
            };
            let gain = if muted { 0.0 } else { volume };
            *slot = conv(sample * gain);
        }
    }
    drop(queue);
    // 모자라면 0으로 메웠지만 클럭은 실시간으로 간다.
    shared
        .played_pairs
        .fetch_add(need_pairs as u64, Ordering::SeqCst);
}

struct AudioDecoder {
    ictx: rsmpeg::avformat::AVFormatContextInput,
    ctx: rsmpeg::avcodec::AVCodecContext,
    swr: rsmpeg::swresample::SwrContext,
    stream_index: usize,
    time_base: (i32, i32),
    out_rate: u32,
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
    Ok(AudioDecoder {
        ictx,
        ctx,
        swr,
        stream_index,
        time_base,
        out_rate,
    })
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
    Ok(())
}
