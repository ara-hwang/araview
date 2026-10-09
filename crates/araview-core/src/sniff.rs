//! Content-based format detection.
//!
//! The extension is the first guess everywhere (listings, associations), but a
//! renamed file (`photo.jpg` that is really HEIC, an extensionless download)
//! would otherwise be painted with the wrong strategy. The magic bytes decide
//! when they recognise the file; formats without a reliable signature (TGA,
//! SVG, ZIP-based archives) are left to the extension.

use std::io::Read;
use std::path::Path;

/// Bytes needed to recognise every signature below.
const HEAD_LEN: usize = 32;

/// MIME for a recognised signature, `None` when the bytes say nothing certain.
pub fn sniff_mime(head: &[u8]) -> Option<&'static str> {
    if head.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    // "BM" alone is too weak; the reserved fields at 6..10 are always zero.
    if head.len() >= 14 && head.starts_with(b"BM") && head[6..10] == [0, 0, 0, 0] {
        return Some("image/bmp");
    }
    if head.starts_with(&[0, 0, 1, 0]) {
        return Some("image/x-icon");
    }
    if head.starts_with(b"8BPS") {
        return Some(crate::psd_sidecar::PSD_MIME);
    }
    if head.starts_with(b"DDS ") {
        return Some("image/vnd.ms-dds");
    }
    if head.starts_with(&[0x76, 0x2F, 0x31, 0x01]) {
        return Some("image/x-exr");
    }
    if head.starts_with(b"qoif") {
        return Some("image/qoi");
    }
    sniff_bmff(head)
}

/// ISO-BMFF `ftyp` major brand. `mif1`/`msf1` are shared by HEIF and AVIF, so
/// they stay undecided and the extension breaks the tie.
fn sniff_bmff(head: &[u8]) -> Option<&'static str> {
    if head.len() < 12 || &head[4..8] != b"ftyp" {
        return None;
    }
    match &head[8..12] {
        b"avif" | b"avis" => Some("image/avif"),
        b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" | b"hevm" | b"hevs" => {
            Some("image/heic")
        }
        _ => None,
    }
}

/// Read the first bytes of `path` and sniff them. I/O failures are `None` so
/// callers fall back to the extension.
pub fn sniff_file(path: &Path) -> Option<&'static str> {
    let mut head = [0u8; HEAD_LEN];
    let mut file = std::fs::File::open(path).ok()?;
    let mut filled = 0;
    while filled < HEAD_LEN {
        match file.read(&mut head[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(_) => return None,
        }
    }
    sniff_mime(&head[..filled])
}

/// Sniff `path` and, for a PNG, report whether it is animated, opening the
/// file once. The image load path asks both questions for every PNG.
pub fn sniff_file_with_animation(path: &Path) -> (Option<&'static str>, bool) {
    let Ok(file) = std::fs::File::open(path) else {
        return (None, false);
    };
    let mut file = std::io::BufReader::new(file);
    let mut head = [0u8; HEAD_LEN];
    let mut filled = 0;
    while filled < HEAD_LEN {
        match file.read(&mut head[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(_) => return (None, false),
        }
    }
    let mime = sniff_mime(&head[..filled]);
    let animated = mime == Some("image/png")
        // Rewind to just past the 8-byte signature; this stays inside the buffer.
        && file.seek_relative(8 - filled as i64).is_ok()
        && png_chunks_animated(&mut file) == Some(true);
    (mime, animated)
}

/// Chunks walked before giving up on a PNG that never reaches `IDAT`.
const PNG_MAX_CHUNKS: usize = 256;

/// Whether a PNG is animated (APNG). `acTL` must precede the first `IDAT`, so
/// only the leading chunk headers are read. `None` when the file is not a PNG
/// or cannot be read.
pub fn png_is_animated(path: &Path) -> Option<bool> {
    let mut file = std::io::BufReader::new(std::fs::File::open(path).ok()?);
    let mut signature = [0u8; 8];
    file.read_exact(&mut signature).ok()?;
    if signature != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return None;
    }
    png_chunks_animated(&mut file)
}

/// Walk chunk headers from just past the PNG signature.
fn png_chunks_animated(file: &mut std::io::BufReader<std::fs::File>) -> Option<bool> {
    for _ in 0..PNG_MAX_CHUNKS {
        let mut header = [0u8; 8];
        file.read_exact(&mut header).ok()?;
        match &header[4..] {
            b"acTL" => return Some(true),
            b"IDAT" | b"IEND" => return Some(false),
            _ => {}
        }
        let len = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        // Chunk data plus its CRC.
        file.seek_relative(i64::from(len) + 4).ok()?;
    }
    None
}
