//! JPEG 메타데이터 세그먼트 보존 (저장 시 EXIF/ICC/XMP 유지).
//!
//! `save.rs`는 `image` 크레이트로 디코드→재인코딩하므로 원본의
//! APP1(EXIF, XMP), APP2(ICC), APP13(IPTC/Photoshop), COM 세그먼트가
//! 사라진다. JPEG→JPEG 저장에 한해 원본 세그먼트를 인코딩 결과에 이식한다.
//!
//! 회전/반전은 픽셀에 이미 반영되므로 EXIF Orientation은 1로 되돌리고,
//! PixelXDimension/PixelYDimension도 새 크기로 맞춘다. 크기를 바꾸지 않는
//! in-place 패치만 하므로 IFD 오프셋 재계산이 필요 없다. 구조가 예상 밖인
//! EXIF는 건드리지 않고 그대로 이식한다(메타데이터 유실보다 안전).

use std::fs;
use std::io::Read as _;
use std::path::Path;

/// 이식할 메타데이터 세그먼트 총량 상한.
const MAX_METADATA_BYTES: u64 = 16 * 1024 * 1024;

/// JPEG 세그먼트 길이 필드 상한 (u16).
const MAX_SEGMENT_BYTES: u64 = 65_535;

/// `encoded`(새로 인코딩한 JPEG)에 `source`의 메타데이터 세그먼트를 이식한다.
/// `size`는 인코딩된 픽셀의 (width, height)다. 원본에서 세그먼트를 못 찾거나
/// 구조가 깨져 있으면 `encoded`를 그대로 돌려준다.
pub(crate) fn with_metadata(source: &Path, encoded: Vec<u8>, size: (u32, u32)) -> Vec<u8> {
    let Some(segments) = collect_segments(source, size) else {
        return encoded;
    };
    if segments.is_empty() {
        return encoded;
    }
    insert_after_app0(&encoded, &segments)
}

/// 원본 JPEG의 SOS 앞 세그먼트 중 메타데이터성 세그먼트를 복사한다.
/// 전체를 다 읽지 않고 메타데이터 상한 + 최대 세그먼트 길이만 읽는다.
fn collect_segments(source: &Path, size: (u32, u32)) -> Option<Vec<Vec<u8>>> {
    collect_segments_bounded(source, size, MAX_METADATA_BYTES)
}

fn collect_segments_bounded(
    source: &Path,
    size: (u32, u32),
    max_bytes: u64,
) -> Option<Vec<Vec<u8>>> {
    let file = fs::File::open(source).ok()?;
    let mut bytes = Vec::new();
    file.take(max_bytes + MAX_SEGMENT_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() < 4 || bytes[0..2] != [0xFF, 0xD8] {
        return None;
    }

    let mut segments: Vec<Vec<u8>> = Vec::new();
    let mut total: u64 = 0;
    let mut pos = 2usize;
    let mut reached_sos = false;
    while pos + 4 <= bytes.len() {
        if bytes[pos] != 0xFF {
            pos += 1;
            continue;
        }
        let marker = bytes[pos + 1];
        // 연속 FF 패딩과 단독 마커는 길이 없이 건너뛴다.
        if marker == 0xFF || marker == 0x00 {
            pos += 1;
            continue;
        }
        if marker == 0xDA {
            reached_sos = true;
            break;
        }
        if marker == 0xD8 || marker == 0xD9 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            pos += 2;
            continue;
        }
        let len = u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]) as usize;
        let end = pos.checked_add(2)?.checked_add(len)?;
        if len < 2 || end > bytes.len() {
            return None;
        }
        // APP1(EXIF/XMP), APP2(ICC), APP13(IPTC), COM만 보존한다.
        // APP0(JFIF)/APP14(Adobe)는 인코더가 새로 쓰는 색 공간 기준과
        // 충돌할 수 있어 제외한다.
        if matches!(marker, 0xE1 | 0xE2 | 0xED | 0xFE) {
            let mut segment = bytes[pos..end].to_vec();
            if marker == 0xE1 {
                patch_exif_segment(&mut segment, size);
            }
            total = total.saturating_add(segment.len() as u64);
            if total > max_bytes {
                log::warn!("[jpeg-meta] metadata too large, not preserved");
                return None;
            }
            segments.push(segment);
        }
        pos = end;
    }
    // SOS를 못 찾으면 잘린 파일이다. 일부만 이식하지 않는다.
    reached_sos.then_some(segments)
}

/// SOI 다음, 첫 APP0(JFIF) 뒤에 세그먼트를 끼워 넣는다. APP0가 없으면
/// SOI 바로 뒤가 JFIF/EXIF/ICC 관례상 맞다.
fn insert_after_app0(encoded: &[u8], segments: &[Vec<u8>]) -> Vec<u8> {
    if encoded.len() < 2 || encoded[0..2] != [0xFF, 0xD8] {
        return encoded.to_vec();
    }
    let mut insert_at = 2usize;
    if encoded.len() >= 6 && encoded[2..4] == [0xFF, 0xE0] {
        let len = u16::from_be_bytes([encoded[4], encoded[5]]) as usize;
        let end = 2usize.checked_add(2).and_then(|n| n.checked_add(len));
        if len >= 2 {
            if let Some(end) = end {
                if end <= encoded.len() {
                    insert_at = end;
                }
            }
        }
    }
    let extra: usize = segments.iter().map(Vec::len).sum();
    let mut out = Vec::with_capacity(encoded.len() + extra);
    out.extend_from_slice(&encoded[..insert_at]);
    for segment in segments {
        out.extend_from_slice(segment);
    }
    out.extend_from_slice(&encoded[insert_at..]);
    out
}

/// APP1 세그먼트 안의 EXIF Orientation을 1로, 픽셀 치수를 `size`로 패치한다.
/// EXIF가 아니거나 구조가 예상 밖이면 세그먼트를 그대로 둔다.
fn patch_exif_segment(segment: &mut [u8], size: (u32, u32)) {
    if segment.len() < 10 || segment[0..2] != [0xFF, 0xE1] {
        return;
    }
    let payload = &mut segment[4..];
    if !payload.starts_with(b"Exif\0\0") {
        return;
    }
    let tiff = &mut payload[6..];
    let Some(endian) = Endian::from_header(tiff) else {
        return;
    };
    let Some(ifd0_offset) = endian.read_u32(tiff, 4).map(|offset| offset as usize) else {
        return;
    };

    // IFD0: Orientation(0x0112) = 1, ExifIFD(0x8769) 포인터 확보.
    if let Some(entry) = find_ifd_entry(tiff, ifd0_offset, &endian, 0x0112) {
        patch_inline_value(tiff, &entry, &endian, 1);
    }
    let Some(exif_ifd) = find_ifd_entry(tiff, ifd0_offset, &endian, 0x8769)
        .and_then(|entry| read_inline_value(tiff, &entry, &endian))
        .map(|offset| offset as usize)
    else {
        return;
    };
    for (tag, value) in [(0xA002u16, size.0), (0xA003, size.1)] {
        if let Some(entry) = find_ifd_entry(tiff, exif_ifd, &endian, tag) {
            patch_inline_value(tiff, &entry, &endian, value);
        }
    }
}

#[derive(Clone, Copy)]
struct Endian {
    little: bool,
}

impl Endian {
    /// TIFF 헤더("II*\0" / "MM\0*")에서 바이트 순서를 읽는다.
    fn from_header(tiff: &[u8]) -> Option<Self> {
        match tiff.get(0..4)? {
            [0x49, 0x49, 0x2A, 0x00] => Some(Self { little: true }),
            [0x4D, 0x4D, 0x00, 0x2A] => Some(Self { little: false }),
            _ => None,
        }
    }

    fn read_u16(self, bytes: &[u8], offset: usize) -> Option<u16> {
        let raw: [u8; 2] = bytes.get(offset..offset + 2)?.try_into().ok()?;
        Some(if self.little {
            u16::from_le_bytes(raw)
        } else {
            u16::from_be_bytes(raw)
        })
    }

    fn read_u32(self, bytes: &[u8], offset: usize) -> Option<u32> {
        let raw: [u8; 4] = bytes.get(offset..offset + 4)?.try_into().ok()?;
        Some(if self.little {
            u32::from_le_bytes(raw)
        } else {
            u32::from_be_bytes(raw)
        })
    }

    fn write_u16(self, bytes: &mut [u8], offset: usize, value: u16) -> bool {
        let raw = if self.little {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        };
        let Some(slot) = bytes.get_mut(offset..offset + 2) else {
            return false;
        };
        slot.copy_from_slice(&raw);
        true
    }

    fn write_u32(self, bytes: &mut [u8], offset: usize, value: u32) -> bool {
        let raw = if self.little {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        };
        let Some(slot) = bytes.get_mut(offset..offset + 4) else {
            return false;
        };
        slot.copy_from_slice(&raw);
        true
    }
}

struct IfdEntry {
    offset: usize,
    typ: u16,
    count: u32,
}

/// IFD를 훑어 `tag` 엔트리를 찾는다. 엔트리 수가 비정상이거나 IFD가
/// 데이터 경계를 넘으면 None.
fn find_ifd_entry(tiff: &[u8], ifd_offset: usize, endian: &Endian, tag: u16) -> Option<IfdEntry> {
    let count = endian.read_u16(tiff, ifd_offset)? as usize;
    if count == 0 || count > 4096 {
        return None;
    }
    let entries_start = ifd_offset.checked_add(2)?;
    let entries_end = entries_start.checked_add(count.checked_mul(12)?)?;
    if entries_end.checked_add(4)? > tiff.len() {
        return None;
    }
    for i in 0..count {
        let offset = entries_start + i * 12;
        if endian.read_u16(tiff, offset)? != tag {
            continue;
        }
        return Some(IfdEntry {
            offset,
            typ: endian.read_u16(tiff, offset + 2)?,
            count: endian.read_u32(tiff, offset + 4)?,
        });
    }
    None
}

/// SHORT/LONG 하나짜리 값은 엔트리 안에 인라인되므로 그 자리에서 바꾼다.
fn patch_inline_value(tiff: &mut [u8], entry: &IfdEntry, endian: &Endian, value: u32) -> bool {
    match (entry.typ, entry.count) {
        // SHORT(2바이트) + 패딩(2바이트).
        (3, 1) => {
            endian.write_u16(tiff, entry.offset + 8, value as u16)
                && endian.write_u16(tiff, entry.offset + 10, 0)
        }
        (4, 1) => endian.write_u32(tiff, entry.offset + 8, value),
        _ => false,
    }
}

/// 포인터 태그(ExifIFD 등)처럼 SHORT/LONG 하나짜리 인라인 값을 읽는다.
fn read_inline_value(tiff: &[u8], entry: &IfdEntry, endian: &Endian) -> Option<u32> {
    match (entry.typ, entry.count) {
        (3, 1) => endian.read_u16(tiff, entry.offset + 8).map(u32::from),
        (4, 1) => endian.read_u32(tiff, entry.offset + 8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 최소 TIFF: IFD0(Orientation=orientation, ExifIFD 포인터) + ExifIFD(픽셀 치수).
    fn exif_app1(orientation: u16, pixel: (u32, u32), little: bool) -> Vec<u8> {
        let mut tiff = Vec::new();
        if little {
            tiff.extend_from_slice(&[0x49, 0x49, 0x2A, 0x00]);
        } else {
            tiff.extend_from_slice(&[0x4D, 0x4D, 0x00, 0x2A]);
        }
        let write_u16 = |out: &mut Vec<u8>, v: u16| {
            let raw = if little {
                v.to_le_bytes()
            } else {
                v.to_be_bytes()
            };
            out.extend_from_slice(&raw);
        };
        let write_u32 = |out: &mut Vec<u8>, v: u32| {
            let raw = if little {
                v.to_le_bytes()
            } else {
                v.to_be_bytes()
            };
            out.extend_from_slice(&raw);
        };
        write_u32(&mut tiff, 8); // IFD0 offset
                                 // IFD0: 항목 2개.
        write_u16(&mut tiff, 2);
        // Orientation(0x0112) SHORT count=1 value=orientation
        write_u16(&mut tiff, 0x0112);
        write_u16(&mut tiff, 3);
        write_u32(&mut tiff, 1);
        write_u32(&mut tiff, u32::from(orientation));
        // ExifIFD pointer(0x8769) LONG count=1 value=offset
        let exif_ifd_offset = 8 + 2 + 2 * 12 + 4;
        write_u16(&mut tiff, 0x8769);
        write_u16(&mut tiff, 4);
        write_u32(&mut tiff, 1);
        write_u32(&mut tiff, exif_ifd_offset as u32);
        write_u32(&mut tiff, 0); // next IFD
                                 // ExifIFD: 항목 2개 (PixelXDimension, PixelYDimension).
        write_u16(&mut tiff, 2);
        for (tag, value) in [(0xA002u16, pixel.0), (0xA003, pixel.1)] {
            write_u16(&mut tiff, tag);
            write_u16(&mut tiff, 4); // LONG
            write_u32(&mut tiff, 1);
            write_u32(&mut tiff, value);
        }
        write_u32(&mut tiff, 0); // next IFD

        let mut segment = vec![0xFF, 0xE1];
        let payload_len = 6 + tiff.len() + 2;
        segment.extend_from_slice(&(payload_len as u16).to_be_bytes());
        segment.extend_from_slice(b"Exif\0\0");
        segment.extend_from_slice(&tiff);
        segment
    }

    #[test]
    fn patches_orientation_and_pixel_dimensions_in_place() {
        for little in [true, false] {
            let mut segment = exif_app1(6, (400, 200), little);
            let before_len = segment.len();
            patch_exif_segment(&mut segment, (200, 400));
            assert_eq!(segment.len(), before_len, "in-place patch must not resize");

            let tiff = &segment[10..];
            let endian = Endian::from_header(tiff).unwrap();
            let orientation = find_ifd_entry(tiff, 8, &endian, 0x0112).unwrap();
            assert_eq!(read_inline_value(tiff, &orientation, &endian), Some(1));
            let exif_ifd = find_ifd_entry(tiff, 8, &endian, 0x8769).unwrap();
            let exif_ifd = read_inline_value(tiff, &exif_ifd, &endian).unwrap() as usize;
            let x = find_ifd_entry(tiff, exif_ifd, &endian, 0xA002).unwrap();
            assert_eq!(read_inline_value(tiff, &x, &endian), Some(200));
            let y = find_ifd_entry(tiff, exif_ifd, &endian, 0xA003).unwrap();
            assert_eq!(read_inline_value(tiff, &y, &endian), Some(400));
        }
    }

    #[test]
    fn non_exif_app1_is_left_untouched() {
        let mut segment = vec![0xFF, 0xE1, 0x00, 0x08];
        segment.extend_from_slice(b"XMP\0pad");
        let before = segment.clone();
        patch_exif_segment(&mut segment, (10, 10));
        assert_eq!(segment, before);
    }

    #[test]
    fn truncated_exif_does_not_panic() {
        let mut segment = exif_app1(6, (400, 200), true);
        segment.truncate(segment.len() / 2);
        patch_exif_segment(&mut segment, (1, 1));
    }

    #[test]
    fn inserts_after_app0_keeping_soi_first() {
        let encoded = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xD9];
        let segments = vec![vec![0xFF, 0xE1, 0x00, 0x04, 0xAA, 0xBB]];
        let out = insert_after_app0(&encoded, &segments);
        assert_eq!(
            out,
            vec![
                0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, // SOI + APP0
                0xFF, 0xE1, 0x00, 0x04, 0xAA, 0xBB, // inserted
                0xFF, 0xD9,
            ]
        );
    }

    #[test]
    fn missing_app0_inserts_right_after_soi() {
        let encoded = vec![0xFF, 0xD8, 0xFF, 0xDB, 0x00, 0x02];
        let segments = vec![vec![0xFF, 0xE2, 0x00, 0x02]];
        let out = insert_after_app0(&encoded, &segments);
        assert_eq!(out[..2], [0xFF, 0xD8]);
        assert_eq!(out[2..6], [0xFF, 0xE2, 0x00, 0x02]);
    }

    #[test]
    fn metadata_larger_than_cap_is_skipped() {
        // 64KB APP1 두 개(총 128KB)를 가진 소스를 작은 상한(100KB)으로 검사한다.
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("big.jpg");
        let mut bytes = vec![0xFF, 0xD8];
        let seg_len = 65_535usize;
        for _ in 0..2 {
            bytes.extend_from_slice(&[0xFF, 0xE1]);
            bytes.extend_from_slice(&((seg_len - 2) as u16).to_be_bytes());
            bytes.extend(std::iter::repeat_n(0u8, seg_len - 4));
        }
        bytes.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x02]);
        fs::write(&source, &bytes).unwrap();

        assert!(collect_segments_bounded(&source, (1, 1), 100_000).is_none());
        // 상한 안이면 그대로 이식된다.
        let copied = collect_segments_bounded(&source, (1, 1), 200_000).unwrap();
        assert_eq!(copied.len(), 2);
    }
}
