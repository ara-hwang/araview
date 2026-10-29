//! Bounded, conservative raster previews and PNG thumbnail sampling.
//!
//! These paths intentionally reject inputs whose metadata or pixel layout is not
//! fully understood. Callers keep their existing full-decode path as fallback.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

pub(crate) const MIN_PREVIEW_PIXELS: u64 = 2_000_000;
const MAX_PREVIEW_PIXELS: u64 = 100_000_000;
const MAX_PREVIEW_SIDE: u32 = 2048;

/// Decode a display preview only for large, static JPEGs and conservative PNGs.
/// Unsupported or malformed input returns `None`, leaving the caller's normal
/// decoder responsible for the result.
pub(crate) fn decode_static_preview(source: &Path, max_side: u32) -> Option<image::RgbaImage> {
    let max_side = max_side.clamp(32, MAX_PREVIEW_SIDE);
    match crate::image::resolve_mime(source)? {
        "image/jpeg" => {
            let (width, height) = image::image_dimensions(source).ok()?;
            if !preview_size_allowed(width, height) {
                return None;
            }
            let image = crate::thumbnail::decode_jpeg_scaled(source, max_side)?;
            let image = crate::orientation::apply_to_image(
                image,
                crate::orientation::read_orientation(source),
            );
            Some(image.thumbnail(max_side, max_side).into_rgba8())
        }
        "image/png" => decode_png_sampled(source, max_side, MIN_PREVIEW_PIXELS),
        _ => None,
    }
}

/// Make a bounded RGB PNG thumbnail by streaming scanlines instead of allocating
/// the full decoded raster. Returns `None` for interlaced, animated, profiled,
/// alpha, non-RGB, non-8-bit, small, or otherwise unsupported inputs.
pub(crate) fn decode_png_sampled(
    source: &Path,
    max_side: u32,
    min_pixels: u64,
) -> Option<image::RgbaImage> {
    let file = File::open(source).ok()?;
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info().ok()?;
    let (width, height, color_type, bit_depth, interlaced, has_icc, animated) = {
        let info = reader.info();
        (
            info.width,
            info.height,
            info.color_type,
            info.bit_depth,
            info.interlaced,
            info.icc_profile.is_some(),
            info.animation_control.is_some(),
        )
    };
    if color_type != png::ColorType::Rgb
        || bit_depth != png::BitDepth::Eight
        || interlaced
        || has_icc
        || animated
        || !preview_size_allowed_with_min(width, height, min_pixels)
    {
        return None;
    }

    let max_side = max_side.clamp(32, MAX_PREVIEW_SIDE);
    let (out_width, out_height) = sampled_dimensions(width, height, max_side)?;
    let output_len = (out_width as usize)
        .checked_mul(out_height as usize)?
        .checked_mul(4)?;
    let mut output = vec![0; output_len];
    let expected_row_bytes = (width as usize).checked_mul(3)?;
    let mut output_y = 0u32;

    for source_y in 0..height {
        let row = reader.next_row().ok()??;
        let data = row.data();
        if data.len() != expected_row_bytes {
            return None;
        }
        if output_y < out_height
            && (u64::from(output_y) * u64::from(height) / u64::from(out_height))
                == u64::from(source_y)
        {
            let dest_row = output_y as usize * out_width as usize * 4;
            for output_x in 0..out_width {
                let source_x = (u64::from(output_x) * u64::from(width)
                    / u64::from(out_width)) as usize;
                let src = source_x * 3;
                let dst = dest_row + output_x as usize * 4;
                output[dst..dst + 3].copy_from_slice(&data[src..src + 3]);
                output[dst + 3] = 255;
            }
            output_y += 1;
        }
    }
    if output_y != out_height {
        return None;
    }
    image::RgbaImage::from_raw(out_width, out_height, output)
}

fn preview_size_allowed(width: u32, height: u32) -> bool {
    preview_size_allowed_with_min(width, height, MIN_PREVIEW_PIXELS)
}

fn preview_size_allowed_with_min(width: u32, height: u32, min_pixels: u64) -> bool {
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    width > 0 && height > 0 && pixels >= min_pixels && pixels <= MAX_PREVIEW_PIXELS
}

fn sampled_dimensions(width: u32, height: u32, max_side: u32) -> Option<(u32, u32)> {
    if width == 0 || height == 0 {
        return None;
    }
    let scale = (f64::from(max_side) / f64::from(width.max(height))).min(1.0);
    Some((
        (f64::from(width) * scale).round().max(1.0) as u32,
        (f64::from(height) * scale).round().max(1.0) as u32,
    ))
}

/// Parse JPEG marker segments only up to the scan data. A malformed or
/// unreadable header is treated as ineligible rather than guessed safe.
pub(crate) fn jpeg_has_icc_profile(source: &Path) -> Option<bool> {
    let mut reader = BufReader::new(File::open(source).ok()?);
    let mut soi = [0; 2];
    reader.read_exact(&mut soi).ok()?;
    if soi != [0xff, 0xd8] {
        return None;
    }
    loop {
        let mut byte = [0];
        reader.read_exact(&mut byte).ok()?;
        if byte[0] != 0xff {
            return None;
        }
        reader.read_exact(&mut byte).ok()?;
        while byte[0] == 0xff {
            reader.read_exact(&mut byte).ok()?;
        }
        let marker = byte[0];
        if marker == 0xd9 || marker == 0xda {
            return Some(false);
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        let mut len = [0; 2];
        reader.read_exact(&mut len).ok()?;
        let segment_len = usize::from(u16::from_be_bytes(len));
        let payload_len = segment_len.checked_sub(2)?;
        if marker == 0xe2 && payload_len >= 12 {
            let mut signature = [0; 12];
            reader.read_exact(&mut signature).ok()?;
            if &signature == b"ICC_PROFILE\0" {
                return Some(true);
            }
            reader.seek(SeekFrom::Current((payload_len - signature.len()) as i64)).ok()?;
        } else {
            reader.seek(SeekFrom::Current(payload_len as i64)).ok()?;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{BufWriter, Write as _};

    use super::*;

    fn large_rgb_jpeg(path: &Path) {
        let width = 10_000;
        let height = 5_000;
        let pixels = vec![128; width as usize * height as usize * 3];
        jpeg_encoder::Encoder::new(BufWriter::new(File::create(path).unwrap()), 80)
            .encode(
                &pixels,
                width as u16,
                height as u16,
                jpeg_encoder::ColorType::Rgb,
            )
            .unwrap();
    }

    fn rgb_png(path: &Path, width: u32, height: u32, interlaced: bool) {
        let file = File::create(path).unwrap();
        let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let row = (0..width)
            .flat_map(|x| [x as u8, 80, 160])
            .collect::<Vec<_>>();
        {
            let mut stream = writer.stream_writer().unwrap();
            for _ in 0..height {
                stream.write_all(&row).unwrap();
            }
            stream.finish().unwrap();
        }
        writer.finish().unwrap();
        if interlaced {
            let mut bytes = fs::read(path).unwrap();
            bytes[28] = 1;
            let crc = test_crc32(&bytes[12..29]);
            bytes[29..33].copy_from_slice(&crc.to_be_bytes());
            fs::write(path, bytes).unwrap();
        }
    }

    fn test_crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &byte in bytes {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
            }
        }
        !crc
    }

    #[::core::prelude::v1::test]
    fn samples_rgb_png_rows_without_changing_pixel_values() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("rgb.png");
        rgb_png(&path, 64, 32, false);
        let preview = decode_png_sampled(&path, 32, 1).unwrap();
        assert_eq!(preview.dimensions(), (32, 16));
        assert_eq!(preview.get_pixel(0, 0).0, [0, 80, 160, 255]);
        assert_eq!(preview.get_pixel(31, 15).0, [62, 80, 160, 255]);
    }

    #[::core::prelude::v1::test]
    fn samples_a_50_megapixel_jpeg_at_a_reduced_decoder_scale() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("large.jpg");
        large_rgb_jpeg(&path);

        let preview = decode_static_preview(&path, MAX_PREVIEW_SIDE).unwrap();
        assert_eq!(preview.dimensions(), (2_048, 1_024));

        let mut bytes = fs::read(&path).unwrap();
        let mut profile = vec![0xff, 0xe2, 0, 14];
        profile.extend_from_slice(b"ICC_PROFILE\0");
        bytes.splice(2..2, profile);
        fs::write(&path, bytes).unwrap();
        assert!(decode_static_preview(&path, MAX_PREVIEW_SIDE).is_none());
    }

    #[::core::prelude::v1::test]
    fn samples_a_50_megapixel_png_into_a_bounded_buffer() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("large.png");
        rgb_png(&path, 10_000, 5_000, false);

        let preview = decode_static_preview(&path, MAX_PREVIEW_SIDE).unwrap();
        assert_eq!(preview.dimensions(), (2_048, 1_024));
        let preview_bytes = preview.as_raw().len() as u64;
        let full_rgb_bytes = 10_000_u64 * 5_000 * 3;
        assert!(preview_bytes < full_rgb_bytes / 10);
    }

    #[::core::prelude::v1::test]
    fn falls_back_for_png_alpha_and_animation() {
        let temp = tempfile::tempdir().unwrap();
        let rgba = temp.path().join("alpha.png");
        let file = File::create(&rgba).unwrap();
        let mut encoder = png::Encoder::new(BufWriter::new(file), 64, 32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[255; 64 * 32 * 4]).unwrap();
        writer.finish().unwrap();
        assert!(decode_png_sampled(&rgba, 32, 1).is_none());

        let animated = temp.path().join("animated.png");
        fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/sample.apng"),
            &animated,
        )
        .unwrap();
        assert!(decode_png_sampled(&animated, 32, 1).is_none());
    }

    #[::core::prelude::v1::test]
    fn rejects_interlaced_png_and_inputs_below_the_size_limit() {
        let temp = tempfile::tempdir().unwrap();
        let interlaced = temp.path().join("interlaced.png");
        rgb_png(&interlaced, 64, 32, true);
        assert!(decode_png_sampled(&interlaced, 32, 1).is_none());

        let small = temp.path().join("small.png");
        rgb_png(&small, 64, 32, false);
        assert!(decode_png_sampled(&small, 32, MIN_PREVIEW_PIXELS).is_none());
    }

    #[::core::prelude::v1::test]
    fn jpeg_icc_marker_detection_is_conservative() {
        let original = fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../samples/sample.jpg"))
            .unwrap();
        let temp = tempfile::tempdir().unwrap();
        let plain = temp.path().join("plain.jpg");
        fs::write(&plain, &original).unwrap();
        assert_eq!(jpeg_has_icc_profile(&plain), Some(false));

        let profiled = temp.path().join("profiled.jpg");
        let mut bytes = original.clone();
        let mut segment = vec![0xff, 0xe2, 0, 14];
        segment.extend_from_slice(b"ICC_PROFILE\0");
        bytes.splice(2..2, segment);
        fs::write(&profiled, bytes).unwrap();
        assert_eq!(jpeg_has_icc_profile(&profiled), Some(true));
    }
}
