//! HBITMAP helpers for `IThumbnailProvider::GetThumbnail`.
//!
//! Converts flattened RGB8 (opaque, white background) into a top-down
//! 32bpp DIB section with `WTSAT_RGB` semantics. The caller (Explorer)
//! owns the returned handle.

use std::ffi::c_void;

use windows::core::{Error, Result};
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Graphics::Gdi::{
    CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP,
    HGDIOBJ,
};

use crate::psd::Rgb8;

/// Build an opaque 32bpp DIB section from RGB8 bytes.
pub fn from_rgb8(rgb: &Rgb8) -> Result<HBITMAP> {
    let (width, height) = (rgb.width.max(1), rgb.height.max(1));
    if rgb.bytes.len() < width as usize * height as usize * 3 {
        return Err(Error::from_hresult(E_FAIL));
    }

    let mut bi = BITMAPINFO::default();
    bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bi.bmiHeader.biWidth = width as i32;
    bi.bmiHeader.biHeight = -(height as i32); // top-down
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = 32;
    bi.bmiHeader.biCompression = BI_RGB.0;

    let mut bits: *mut c_void = std::ptr::null_mut();
    // SAFETY: `bi` is a fully initialised BITMAPINFOHEADER describing a 32bpp
    // top-down DIB, and `bits` is a live out-param. On success GDI owns the
    // section until the handle is deleted.
    let hbmp = unsafe { CreateDIBSection(None, &bi, DIB_RGB_COLORS, &mut bits, None, 0)? };
    if bits.is_null() {
        // GDI handed back a handle but no backing store: delete it rather
        // than leaking the object.
        // SAFETY: `hbmp` is a live handle returned by `CreateDIBSection` and
        // is not referenced again after this point.
        unsafe {
            let _ = DeleteObject(HGDIOBJ(hbmp.0));
        }
        return Err(Error::from_hresult(E_FAIL));
    }

    // SAFETY: for a 32bpp top-down DIB the stride is exactly `width * 4` (no
    // row padding), so the section is `width * height * 4` bytes and `bits` is
    // non-null and uniquely owned here. The source length was checked above.
    unsafe {
        let dst =
            std::slice::from_raw_parts_mut(bits as *mut u8, width as usize * height as usize * 4);
        for (src_px, dst_px) in rgb
            .bytes
            .as_chunks::<3>()
            .0
            .iter()
            .zip(dst.as_chunks_mut::<4>().0.iter_mut())
        {
            dst_px[0] = src_px[2]; // B
            dst_px[1] = src_px[1]; // G
            dst_px[2] = src_px[0]; // R
            dst_px[3] = 255; // opaque
        }
    }
    Ok(hbmp)
}
