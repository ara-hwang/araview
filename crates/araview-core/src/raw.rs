//! 카메라 RAW(CR2/CR3/NEF/NRW/ARW/DNG/ORF/RW2/PEF/SRW/RAF/MRW/SR2/SRF/RWL/3FR/KDC/DCR/IIQ) 미리보기.
//!
//! 센서 데이터를 현상(디모자이크)하지 않고, RAW 안에 들어 있는 카메라 제작
//! JPEG 미리보기를 꺼내 HEIC/PSD와 같은 JPEG sidecar 규칙(`SPEC`)에 얹는다.
//! - 컨테이너는 TIFF다. IFD0에서 시작해 다음 IFD와 SubIFD(`0x014A`)를 폭 우선으로
//!   돌며 JPEG 후보(`JPEGInterchangeFormat`, 단일 스트립 JPEG, RW2의 `JpgFromRaw`)를
//!   모은다. 후보를 못 찾으면 파일을 앞에서부터 훑어 JPEG 시작 표지를 찾는다.
//!   TIFF 기반이 아닌 CR3(ISO-BMFF)와 RAF는 이 스캔으로만 미리보기를 찾으므로
//!   Orientation을 반영하지 않는다.
//! - 후보는 JPEG 세그먼트를 직접 읽어 검증한다. CR2/DNG의 센서 데이터는 무손실
//!   JPEG(SOF3)로 압축돼 있어 디코드할 수 없으므로 SOF0~2만 받고, 그중 픽셀 수가
//!   가장 큰 것을 쓴다.
//! - 방향은 미리보기 JPEG가 아니라 IFD0의 Orientation 태그를 따른다.
//! - 미리보기가 없는 파일은 `unsupported`로 돌려준다.
//! - 이 설계로는 안 열리는 파일(내장 일반 JPEG가 없는 CR3의 Digic X 세대, MRW,
//!   IIQ, 3FR, DCR, KDC)은 rawler(dnglab의 순수 Rust RAW 라이브러리, LGPL-2.1)로
//!   폴백한다: 카메라 제작 미리보기를 꺼내고, 없으면 센서 데이터를 현상한다.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use crate::app_error::{AppError, ErrorCode};
use crate::sidecar::{Rgb8, SidecarSpec};

pub const RAW_MIME: &str = "image/x-raw";

/// 이 디코더의 JPEG sidecar 규칙. `transcode`가 디코더별로 골라 쓴다.
pub(crate) static SPEC: SidecarSpec = SidecarSpec {
    label: "RAW sidecar lock",
    paint_prefix: "raw-",
    thumb_prefix: "raw-thumb-",
    decode: decode_rgb8,
};

/// 디코드 허용 픽셀 상한. 표시·클립보드 경로와 같은 값이다.
const MAX_DECODE_PIXELS: u64 = 100_000_000;

/// 미리보기 JPEG 하나를 읽는 바이트 상한.
const MAX_PREVIEW_BYTES: u64 = 128 * 1024 * 1024;

/// 순회할 IFD 수 상한. 순환하거나 부풀려진 파일에서 멈추게 한다.
const MAX_IFDS: usize = 32;

/// IFD 하나에서 읽는 엔트리 수 상한.
const MAX_IFD_ENTRIES: u16 = 512;

/// IFD로 찾지 못했을 때 JPEG 표지를 훑는 범위와 단위, 받아들일 후보 수.
const MAX_SCAN_BYTES: u64 = 256 * 1024 * 1024;
const SCAN_CHUNK: usize = 1024 * 1024;
const MAX_SCAN_HITS: usize = 64;

const TAG_COMPRESSION: u16 = 0x0103;
const TAG_STRIP_OFFSETS: u16 = 0x0111;
const TAG_ORIENTATION: u16 = 0x0112;
const TAG_SUB_IFDS: u16 = 0x014A;
const TAG_JPEG_INTERCHANGE_FORMAT: u16 = 0x0201;
/// 파나소닉 RW2가 IFD0에 싣는 JPEG 미리보기 오프셋.
const TAG_RW2_JPG_FROM_RAW: u16 = 0x002E;

const TYPE_SHORT: u16 = 3;
const TYPE_LONG: u16 = 4;
const TYPE_IFD: u16 = 13;

#[derive(Clone, Copy)]
struct ByteOrder {
    little: bool,
}

impl ByteOrder {
    fn u16(self, b: [u8; 2]) -> u16 {
        if self.little {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        }
    }

    fn u32(self, b: [u8; 4]) -> u32 {
        if self.little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        }
    }
}

struct Entry {
    tag: u16,
    kind: u16,
    count: u32,
    /// 값이 4바이트 이하면 값 자체, 아니면 오프셋이 들어 있는 필드.
    value: [u8; 4],
}

impl Entry {
    fn first_uint(&self, order: ByteOrder) -> Option<u32> {
        if self.count == 0 {
            return None;
        }
        match self.kind {
            TYPE_SHORT => Some(u32::from(order.u16([self.value[0], self.value[1]]))),
            TYPE_LONG => Some(order.u32(self.value)),
            _ => None,
        }
    }
}

/// 미리보기 JPEG 하나의 위치와 크기.
struct Jpeg {
    offset: u64,
    len: u64,
    width: u32,
    height: u32,
}

impl Jpeg {
    fn pixels(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

fn read_entries(file: &mut File, order: ByteOrder, offset: u64) -> Option<(Vec<Entry>, u32)> {
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut count = [0u8; 2];
    file.read_exact(&mut count).ok()?;
    let count = order.u16(count);
    if count > MAX_IFD_ENTRIES {
        return None;
    }
    // 엔트리 12바이트씩과 뒤따르는 다음 IFD 오프셋 4바이트.
    let mut raw = vec![0u8; usize::from(count) * 12 + 4];
    file.read_exact(&mut raw).ok()?;
    let (body, next) = raw.split_at(usize::from(count) * 12);
    let entries = body
        .as_chunks::<12>()
        .0
        .iter()
        .map(|e| Entry {
            tag: order.u16([e[0], e[1]]),
            kind: order.u16([e[2], e[3]]),
            count: order.u32([e[4], e[5], e[6], e[7]]),
            value: [e[8], e[9], e[10], e[11]],
        })
        .collect();
    Some((entries, order.u32([next[0], next[1], next[2], next[3]])))
}

/// SubIFD 오프셋 목록. 하나면 값 필드에, 여럿이면 가리키는 곳에 배열로 있다.
fn sub_ifd_offsets(file: &mut File, order: ByteOrder, entry: &Entry) -> Vec<u32> {
    if !matches!(entry.kind, TYPE_LONG | TYPE_IFD) || entry.count == 0 {
        return Vec::new();
    }
    if entry.count == 1 {
        return vec![order.u32(entry.value)];
    }
    let count = entry.count.min(16) as usize;
    let mut raw = vec![0u8; count * 4];
    let read = file
        .seek(SeekFrom::Start(u64::from(order.u32(entry.value))))
        .and_then(|_| file.read_exact(&mut raw));
    if read.is_err() {
        return Vec::new();
    }
    raw.as_chunks::<4>()
        .0
        .iter()
        .map(|c| order.u32(*c))
        .collect()
}

/// IFD가 가리키는 JPEG 후보 오프셋.
fn jpeg_offsets(entries: &[Entry], order: ByteOrder) -> Vec<u64> {
    let find = |tag: u16| entries.iter().find(|e| e.tag == tag);
    let mut offsets = Vec::new();
    if let Some(offset) = find(TAG_JPEG_INTERCHANGE_FORMAT).and_then(|e| e.first_uint(order)) {
        offsets.push(u64::from(offset));
    }
    // 단일 스트립에 담긴 JPEG(압축 6, 7). 여러 스트립이면 이어 붙일 수 없어 건너뛴다.
    let jpeg_compressed = find(TAG_COMPRESSION)
        .and_then(|e| e.first_uint(order))
        .is_some_and(|c| c == 6 || c == 7);
    if let Some(strip) = find(TAG_STRIP_OFFSETS).filter(|e| jpeg_compressed && e.count == 1) {
        if let Some(offset) = strip.first_uint(order) {
            offsets.push(u64::from(offset));
        }
    }
    // RW2의 JpgFromRaw는 UNDEFINED(7) 바이트열이고 값 필드가 오프셋이다.
    if let Some(entry) = find(TAG_RW2_JPG_FROM_RAW).filter(|e| e.kind == 7 && e.count > 4) {
        offsets.push(u64::from(order.u32(entry.value)));
    }
    offsets
}

/// TIFF 구조를 돌며 JPEG 후보 오프셋과 IFD0의 Orientation(1~8)을 모은다.
/// TIFF 머리글이 아니면 `None`이다.
fn walk_tiff(file: &mut File) -> Option<(Vec<u64>, u8)> {
    let mut head = [0u8; 8];
    file.seek(SeekFrom::Start(0)).ok()?;
    file.read_exact(&mut head).ok()?;
    let order = match &head[..2] {
        b"II" => ByteOrder { little: true },
        b"MM" => ByteOrder { little: false },
        _ => return None,
    };
    // 일반 TIFF는 42, 올림푸스(ORF)는 "OR"/"RS", 파나소닉(RW2)은 0x55를 쓴다.
    if !matches!(
        order.u16([head[2], head[3]]),
        0x002A | 0x4F52 | 0x5352 | 0x0055
    ) {
        return None;
    }
    let mut queue = VecDeque::from([order.u32([head[4], head[5], head[6], head[7]])]);
    let mut seen = Vec::new();
    let mut offsets = Vec::new();
    let mut orientation = 1;
    while let Some(ifd) = queue.pop_front() {
        if seen.len() >= MAX_IFDS {
            break;
        }
        if ifd == 0 || seen.contains(&ifd) {
            continue;
        }
        seen.push(ifd);
        let Some((entries, next)) = read_entries(file, order, u64::from(ifd)) else {
            continue;
        };
        if seen.len() == 1 {
            orientation = entries
                .iter()
                .find(|e| e.tag == TAG_ORIENTATION)
                .and_then(|e| e.first_uint(order))
                .filter(|v| (1..=8).contains(v))
                .map_or(1, |v| v as u8);
        }
        offsets.extend(jpeg_offsets(&entries, order));
        for entry in entries.iter().filter(|e| e.tag == TAG_SUB_IFDS) {
            queue.extend(sub_ifd_offsets(file, order, entry));
        }
        queue.push_back(next);
    }
    Some((offsets, orientation))
}

fn read_u8(reader: &mut impl BufRead) -> Option<u8> {
    let mut byte = [0u8; 1];
    reader.read_exact(&mut byte).ok()?;
    Some(byte[0])
}

fn read_u16_be(reader: &mut impl BufRead) -> Option<u16> {
    let mut bytes = [0u8; 2];
    reader.read_exact(&mut bytes).ok()?;
    Some(u16::from_be_bytes(bytes))
}

/// 스캔 데이터를 건너뛰어 다음 표지의 종류 바이트를 돌려준다. 바이트 채움(`FF 00`)과
/// 재시작 표지(`FF D0`~`FF D7`)는 스캔의 일부다.
fn skip_entropy(reader: &mut impl BufRead) -> Option<u8> {
    loop {
        let buf = reader.fill_buf().ok()?;
        if buf.is_empty() {
            return None;
        }
        let Some(at) = buf.iter().position(|&b| b == 0xFF) else {
            let len = buf.len();
            reader.consume(len);
            continue;
        };
        reader.consume(at + 1);
        loop {
            match read_u8(reader)? {
                0x00 | 0xD0..=0xD7 => break,
                0xFF => {}
                marker => return Some(marker),
            }
        }
    }
}

/// `offset`에서 시작하는 JPEG의 크기와 치수를 세그먼트를 따라 읽는다. 순차 허프만
/// 방식(SOF0~2)이 아니면, 그리고 EOI까지 닿지 않으면 `None`이다.
fn read_jpeg(file: &mut File, offset: u64) -> Option<Jpeg> {
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut soi = [0u8; 2];
    reader.read_exact(&mut soi).ok()?;
    if soi != [0xFF, 0xD8] {
        return None;
    }
    let mut size = None;
    let mut pending: Option<u8> = None;
    loop {
        let marker = match pending.take() {
            Some(marker) => marker,
            None => {
                if read_u8(&mut reader)? != 0xFF {
                    return None;
                }
                let mut marker = read_u8(&mut reader)?;
                while marker == 0xFF {
                    marker = read_u8(&mut reader)?;
                }
                marker
            }
        };
        match marker {
            0x01 | 0xD0..=0xD7 => {}
            0xD9 => {
                let (width, height) = size?;
                let end = reader.stream_position().ok()?;
                return Some(Jpeg {
                    offset,
                    len: end - offset,
                    width,
                    height,
                });
            }
            // 무손실(SOF3), 계층형, 산술 부호화는 디코드할 수 없다. CR2/DNG의 센서 데이터가
            // 대표적인 무손실 JPEG다.
            0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => return None,
            _ => {
                let segment = usize::from(read_u16_be(&mut reader)?.checked_sub(2)?);
                if matches!(marker, 0xC0..=0xC2) {
                    let mut frame = [0u8; 5];
                    reader.read_exact(&mut frame).ok()?;
                    let height = u32::from(u16::from_be_bytes([frame[1], frame[2]]));
                    let width = u32::from(u16::from_be_bytes([frame[3], frame[4]]));
                    size = Some((width, height)).filter(|&(w, h)| w > 0 && h > 0);
                    reader
                        .seek_relative(i64::try_from(segment.checked_sub(5)?).ok()?)
                        .ok()?;
                } else {
                    reader.seek_relative(i64::try_from(segment).ok()?).ok()?;
                }
                if marker == 0xDA {
                    size?;
                    pending = Some(skip_entropy(&mut reader)?);
                }
            }
        }
        if reader.stream_position().ok()? - offset > MAX_PREVIEW_BYTES {
            return None;
        }
    }
}

/// IFD에서 후보를 못 찾은 파일용: 앞에서부터 `FF D8 FF`를 찾아 유효한 JPEG를 모은다.
/// 찾은 JPEG는 통째로 건너뛰어 그 안의 썸네일이 따로 잡히지 않게 한다.
fn scan_for_jpegs(file: &mut File, file_len: u64) -> Vec<Jpeg> {
    let limit = file_len.min(MAX_SCAN_BYTES);
    let mut found = Vec::new();
    let mut buf = vec![0u8; SCAN_CHUNK];
    let mut pos = 0u64;
    while pos + 3 <= limit && found.len() < MAX_SCAN_HITS {
        let want = usize::try_from(limit - pos).map_or(SCAN_CHUNK, |n| n.min(SCAN_CHUNK));
        let chunk = &mut buf[..want];
        if file.seek(SeekFrom::Start(pos)).is_err() || file.read_exact(chunk).is_err() {
            break;
        }
        match chunk.windows(3).position(|w| w == [0xFF, 0xD8, 0xFF]) {
            Some(at) => {
                let start = pos + at as u64;
                pos = match read_jpeg(file, start) {
                    Some(jpeg) => {
                        let end = start + jpeg.len;
                        found.push(jpeg);
                        end
                    }
                    None => start + 1,
                };
            }
            // 청크 경계에 걸친 표지를 놓치지 않게 2바이트를 겹친다.
            None => pos += (want as u64).saturating_sub(2).max(1),
        }
    }
    found
}

fn largest(jpegs: impl IntoIterator<Item = Jpeg>) -> Option<Jpeg> {
    jpegs.into_iter().max_by_key(Jpeg::pixels)
}

/// 썸네일·히스토그램·표시용 RGB8 디코드. 미리보기 JPEG를 풀고 IFD0의 방향을 적용한다.
pub(crate) fn decode_rgb8(source: &Path) -> Result<Rgb8, AppError> {
    let own = decode_rgb8_tiff(source);
    match own {
        Ok(rgb) => Ok(rgb),
        // 내장 일반 JPEG가 없는 파일(CR3의 Digic X 세대, MRW, IIQ, 3FR, DCR, KDC)은
        // rawler 폴백으로 다시 시도한다. 나머지 오류는 그대로 전달한다.
        Err(e) if e.code == ErrorCode::Unsupported => decode_rgb8_rawler(source),
        Err(e) => Err(e),
    }
}

fn decode_rgb8_tiff(source: &Path) -> Result<Rgb8, AppError> {
    let mut file = File::open(source)
        .map_err(|e| AppError::io("Failed to open image", e, ErrorCode::Corrupt))?;
    let file_len = file
        .metadata()
        .map_err(|e| AppError::io("Failed to read metadata", e, ErrorCode::Corrupt))?
        .len();
    let (offsets, orientation) = walk_tiff(&mut file).unwrap_or((Vec::new(), 1));
    let mut preview = largest(
        offsets
            .into_iter()
            .filter_map(|offset| read_jpeg(&mut file, offset)),
    );
    if preview.is_none() {
        preview = largest(scan_for_jpegs(&mut file, file_len));
    }
    let preview =
        preview.ok_or_else(|| AppError::unsupported("RAW file has no embedded preview"))?;
    if preview.pixels() > MAX_DECODE_PIXELS {
        return Err(AppError::too_large("Image dimensions are too large"));
    }
    let len = usize::try_from(preview.len)
        .map_err(|_| AppError::too_large("RAW preview is too large"))?;
    let mut bytes = vec![0u8; len];
    file.seek(SeekFrom::Start(preview.offset))
        .and_then(|_| file.read_exact(&mut bytes))
        .map_err(|e| AppError::io("Failed to read file", e, ErrorCode::Corrupt))?;
    let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg)
        .map_err(|e| AppError::image_error("Failed to decode image", e))?;
    let rgb = crate::orientation::apply_to_image(decoded, orientation).into_rgb8();
    Ok(Rgb8 {
        width: rgb.width(),
        height: rgb.height(),
        bytes: rgb.into_raw(),
    })
}

/// 내장 일반 JPEG가 없는 RAW용 rawler(dnglab의 순수 Rust RAW 라이브러리, LGPL-2.1) 폴백.
/// 카메라 제작 미리보기 → 썸네일 → 풀 이미지 순서로 꺼내고, 전부 없으면
/// `extract_full_pixels`가 센서 데이터를 현상해 sRGB로 내려준다. 방향은 rawler
/// 메타데이터의 EXIF Orientation을 픽셀에 반영한다.
fn decode_rgb8_rawler(source: &Path) -> Result<Rgb8, AppError> {
    let rawfile = rawler::rawsource::RawSource::new(source)
        .map_err(|e| AppError::new(ErrorCode::Corrupt, format!("Failed to open image: {e}")))?;
    let decoder = rawler::get_decoder(&rawfile)
        .map_err(|e| AppError::unsupported(format!("Unsupported RAW file: {e}")))?;
    let params = rawler::decoders::RawDecodeParams::default();
    let img = [
        decoder.preview_image(&rawfile, &params),
        decoder.thumbnail_image(&rawfile, &params),
        decoder.full_image(&rawfile, &params),
    ]
    .into_iter()
    .find_map(|attempt| attempt.ok().flatten())
    // 카메라 제작 미리보기가 전혀 없는 파일은 센서 데이터를 현상한다(sRGB).
    .or_else(|| rawler::analyze::extract_full_pixels(source, &params).ok())
    .ok_or_else(|| AppError::unsupported("RAW file has no embedded preview"))?;
    if u64::from(img.width()) * u64::from(img.height()) > MAX_DECODE_PIXELS {
        return Err(AppError::too_large("Image dimensions are too large"));
    }
    let orientation = decoder
        .raw_metadata(&rawfile, &params)
        .ok()
        .and_then(|meta| meta.exif.orientation)
        .filter(|v| (1..=8).contains(v))
        .unwrap_or(1) as u8;
    Ok(crate::raster_sidecar::flatten_to_rgb8(
        crate::orientation::apply_to_image(img, orientation),
    ))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    fn sample(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples")
            .join(name)
    }

    #[test]
    fn tiff_container_picks_the_largest_preview_with_ifd0_orientation() {
        // sample.dng: 후보 중 가장 큰 미리보기 JPEG는 640x480이고 IFD0의
        // Orientation 6(축 교환)를 반영해 480x640으로 나온다.
        let decoded = decode_rgb8(&sample("sample.dng")).unwrap();
        assert_eq!((decoded.width, decoded.height), (480, 640));
    }

    #[test]
    fn tiff_based_raw_extensions_decode_the_preview() {
        for name in ["sample.sr2", "sample.srf"] {
            let decoded = decode_rgb8(&sample(name)).unwrap();
            assert_eq!((decoded.width, decoded.height), (64, 48), "{name}");
        }
    }

    #[test]
    fn non_tiff_raw_extensions_find_the_preview_by_scanning() {
        for name in ["sample.cr3", "sample.raf"] {
            let decoded = decode_rgb8(&sample(name)).unwrap();
            assert_eq!((decoded.width, decoded.height), (64, 48), "{name}");
        }
    }

    #[test]
    fn raw_without_preview_reports_unsupported() {
        // TIFF 머리글만 있고 JPEG 후보가 없는 파일은 unsupported로 돌아간다.
        let mut path = std::env::temp_dir();
        path.push(format!(
            "araview-raw-no-preview-{}.tiff",
            std::process::id()
        ));
        let mut bytes = b"II".to_vec();
        bytes.extend_from_slice(&42u16.to_le_bytes());
        bytes.extend_from_slice(&8u32.to_le_bytes());
        std::fs::write(&path, bytes).unwrap();
        let result = decode_rgb8(&path);
        std::fs::remove_file(&path).ok();
        assert_eq!(result.unwrap_err().code, ErrorCode::Unsupported);
    }
}
