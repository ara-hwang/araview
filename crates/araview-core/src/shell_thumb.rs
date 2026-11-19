//! 클라우드 전용(온디맨드) 파일의 썸네일을 Windows 셸에서 받아 온다(SPEC §9.6).
//!
//! `IShellItemImageFactory::GetImage`에 `SIIGBF_THUMBNAILONLY`를 주면 셸은 썸네일
//! 캐시나 동기화 공급자(Cloud Files)가 가진 미리보기를 돌려주고, 파일 내용을
//! 직접 읽어 placeholder를 내려받게 하지 않는다. 아이콘으로 대체하지 않으므로
//! 썸네일이 없으면 실패로 돌아온다. 결과는 일반 썸네일과 같은 캐시 폴더에
//! JPEG로 남긴다.

use std::ffi::c_void;
use std::path::Path;

use windows::core::HSTRING;
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows::Win32::System::Com::{
    CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_THUMBNAILONLY,
};

use crate::app_error::AppError;

/// 워커 스레드의 COM 초기화. 성공했을 때만 같은 스레드에서 해제한다.
pub(crate) struct ComApartment(bool);

impl ComApartment {
    pub(crate) fn enter() -> Self {
        // SAFETY: 예약 인자는 None이고, 성공(S_OK/S_FALSE)한 경우에만 Drop에서
        // 같은 스레드가 CoUninitialize로 짝을 맞춘다.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        Self(hr.is_ok())
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: `enter`가 이 스레드에서 초기화에 성공했을 때만 호출된다.
            unsafe { CoUninitialize() };
        }
    }
}

/// HBITMAP을 쥐고 있다가 지운다.
struct OwnedBitmap(HBITMAP);

impl Drop for OwnedBitmap {
    fn drop(&mut self) {
        // SAFETY: `GetImage`가 호출자에게 넘긴 핸들이고 이후 다른 곳에서 쓰지 않는다.
        unsafe {
            let _ = DeleteObject(HGDIOBJ(self.0 .0));
        }
    }
}

/// 셸이 이미 가진 썸네일만 읽어 `max_side` 안에 맞춘 RGB 이미지로 돌려준다.
/// 호출 스레드는 [`ComApartment`]로 COM이 초기화되어 있어야 한다.
pub(crate) fn shell_thumbnail(source: &Path, max_side: u32) -> Result<image::RgbImage, AppError> {
    let side = i32::try_from(max_side).unwrap_or(256);
    // SAFETY: 경로는 소유한 HSTRING이고 바인드 컨텍스트는 쓰지 않는다.
    let factory: IShellItemImageFactory =
        unsafe { SHCreateItemFromParsingName(&HSTRING::from(source.as_os_str()), None) }
            .map_err(|e| AppError::not_found(format!("Shell item unavailable: {e}")))?;
    // SAFETY: `factory`는 살아 있는 COM 인터페이스다. 성공하면 비트맵 소유권이
    // 호출자에게 넘어오므로 바로 `OwnedBitmap`에 담는다.
    let bitmap = unsafe { factory.GetImage(SIZE { cx: side, cy: side }, SIIGBF_THUMBNAILONLY) }
        .map_err(|e| AppError::unsupported(format!("No shell thumbnail: {e}")))?;
    let bitmap = OwnedBitmap(bitmap);
    let (width, height, bgra) = read_bgra(bitmap.0)?;
    let rgb = flatten_on_white(width, height, &bgra)
        .ok_or_else(|| AppError::corrupt("Shell thumbnail has no pixels"))?;
    // 셸은 요청보다 크게 줄 수 있으므로 다시 맞춘다.
    if width > max_side || height > max_side {
        return Ok(image::DynamicImage::ImageRgb8(rgb)
            .thumbnail(max_side, max_side)
            .to_rgb8());
    }
    Ok(rgb)
}

/// 비트맵을 위에서 아래 순서의 32bpp BGRA로 복사한다.
fn read_bgra(bitmap: HBITMAP) -> Result<(u32, u32, Vec<u8>), AppError> {
    let mut info = BITMAP::default();
    // SAFETY: `info`는 BITMAP 크기만큼 쓸 수 있는 출력 버퍼다.
    let written = unsafe {
        GetObjectW(
            HGDIOBJ(bitmap.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(std::ptr::from_mut(&mut info).cast::<c_void>()),
        )
    };
    let (width, height) = (info.bmWidth.unsigned_abs(), info.bmHeight.unsigned_abs());
    if written == 0 || width == 0 || height == 0 {
        return Err(AppError::corrupt("Shell thumbnail bitmap is empty"));
    }
    let mut header = BITMAPINFO::default();
    header.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    header.bmiHeader.biWidth = width as i32;
    header.bmiHeader.biHeight = -(height as i32); // top-down
    header.bmiHeader.biPlanes = 1;
    header.bmiHeader.biBitCount = 32;
    header.bmiHeader.biCompression = BI_RGB.0;
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    // SAFETY: 화면 DC를 빌려 GetDIBits에만 쓰고 바로 돌려준다. 버퍼는 32bpp
    // top-down DIB의 크기(`width * height * 4`, 행 패딩 없음)와 같다.
    let lines = unsafe {
        let dc = GetDC(None);
        let lines = GetDIBits(
            dc,
            bitmap,
            0,
            height,
            Some(pixels.as_mut_ptr().cast::<c_void>()),
            &mut header,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, dc);
        lines
    };
    if lines <= 0 {
        return Err(AppError::corrupt("Failed to read shell thumbnail"));
    }
    Ok((width, height, pixels))
}

/// 셸 비트맵(BGRA, 알파는 premultiplied)을 흰 바탕에 합성한다. 알파가 전부 0이면
/// 알파 없는 비트맵이라 그대로 불투명으로 본다.
fn flatten_on_white(width: u32, height: u32, bgra: &[u8]) -> Option<image::RgbImage> {
    let has_alpha = bgra.as_chunks::<4>().0.iter().any(|px| px[3] != 0);
    let rgb: Vec<u8> = bgra
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|px| {
            let [b, g, r, a] = *px;
            if has_alpha {
                let rest = 255 - a;
                [
                    r.saturating_add(rest),
                    g.saturating_add(rest),
                    b.saturating_add(rest),
                ]
            } else {
                [r, g, b]
            }
        })
        .collect();
    image::RgbImage::from_raw(width, height, rgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_bitmap_without_alpha_keeps_colors() {
        let rgb = flatten_on_white(1, 1, &[30, 20, 10, 0]).unwrap();
        assert_eq!(rgb.get_pixel(0, 0).0, [10, 20, 30]);
    }

    #[test]
    fn transparent_pixels_become_white() {
        let rgb = flatten_on_white(2, 1, &[0, 0, 0, 0, 30, 20, 10, 255]).unwrap();
        assert_eq!(rgb.get_pixel(0, 0).0, [255, 255, 255]);
        assert_eq!(rgb.get_pixel(1, 0).0, [10, 20, 30]);
    }

    #[test]
    fn missing_file_is_an_error() {
        let _com = ComApartment::enter();
        let dir = tempfile::tempdir().unwrap();
        assert!(shell_thumbnail(&dir.path().join("missing.png"), 64).is_err());
    }

    /// 로컬 PNG도 셸 썸네일 경로로 읽을 수 있다(셸이 기본 추출기를 쓴다).
    #[test]
    fn local_png_produces_bounded_thumbnail() {
        let _com = ComApartment::enter();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wide.png");
        image::RgbImage::from_pixel(300, 100, image::Rgb([200, 10, 10]))
            .save(&path)
            .unwrap();
        // 셸 썸네일 생성이 꺼진 환경도 있으므로 실패는 허용하고, 성공하면 크기를 본다.
        if let Ok(thumb) = shell_thumbnail(&path, 64) {
            assert!(thumb.width() <= 64 && thumb.height() <= 64);
        }
    }
}
