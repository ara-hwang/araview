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
        for (src_px, dst_px) in rgb.bytes.chunks_exact(3).zip(dst.chunks_exact_mut(4)) {
            dst_px[0] = src_px[2]; // B
            dst_px[1] = src_px[1]; // G
            dst_px[2] = src_px[0]; // R
            dst_px[3] = 255; // opaque
        }
    }
    Ok(hbmp)
}

#[cfg(test)]
// 테스트의 unsafe는 전부 GDI 핸들 정리/조회 같은 셋업 코드라
// SAFETY 주석을 요구하지 않는다. 프로덕션 코드에는 그대로 적용된다.
#[allow(clippy::undocumented_unsafe_blocks)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Gdi::{DeleteObject, GetObjectW, BITMAP, HGDIOBJ};

    struct OwnedHBitmap(HBITMAP);
    impl Drop for OwnedHBitmap {
        fn drop(&mut self) {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(self.0 .0));
            }
        }
    }

    fn solid(w: u32, h: u32, px: [u8; 3]) -> Rgb8 {
        Rgb8 {
            width: w,
            height: h,
            bytes: vec![px[0], px[1], px[2]]
                .into_iter()
                .cycle()
                .take(w as usize * h as usize * 3)
                .collect(),
        }
    }

    #[test]
    fn from_rgb8_returns_valid_handle_with_dimensions() {
        let rgb = solid(13, 7, [10, 20, 30]);
        let hbmp = from_rgb8(&rgb).expect("from_rgb8");
        let _guard = OwnedHBitmap(hbmp);
        assert!(!hbmp.is_invalid());

        let mut bm = BITMAP::default();
        let written = unsafe {
            GetObjectW(
                HGDIOBJ(hbmp.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bm as *mut _ as *mut _),
            )
        };
        assert!(written > 0, "GetObjectW failed");
        assert_eq!(bm.bmWidth, 13);
        assert_eq!(bm.bmHeight, 7);
        assert_eq!(bm.bmBitsPixel, 32);
    }

    #[test]
    fn from_rgb8_rejects_short_buffer() {
        let rgb = Rgb8 {
            width: 4,
            height: 4,
            bytes: vec![0u8; 10],
        };
        assert!(from_rgb8(&rgb).is_err());
    }
}
