//! DLL registration delegates to the same HKCU implementation as Settings.
//! This module only resolves the loaded DLL's absolute path.

use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use windows::core::PCWSTR;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
    GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
};

pub use araview_core::thumb_shell::{
    CLSID_DEV as CLSID_DEV_STR, CLSID_RELEASE as CLSID_RELEASE_STR,
    IID_THUMBNAIL as IID_ITHUMBNAILPROVIDER,
};

/// Resolve this DLL, rather than the Explorer/surrogate executable.
pub fn own_dll_path() -> io::Result<PathBuf> {
    // SAFETY: FROM_ADDRESS accepts an address inside this module. The
    // unchanged reference count means the handle must not be freed.
    unsafe {
        let mut module = HMODULE::default();
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(own_dll_path as *const () as *const u16),
            &mut module,
        )
        .map_err(io::Error::other)?;
        let mut buf = vec![0u16; 32768];
        let len = GetModuleFileNameW(Some(module), &mut buf) as usize;
        if len == 0 || len >= buf.len() {
            return Err(io::Error::other("Failed to resolve thumbnail DLL path"));
        }
        Ok(PathBuf::from(OsString::from_wide(&buf[..len])))
    }
}

pub fn register() -> io::Result<()> {
    register_with_dll_path(&own_dll_path()?)
}

pub fn register_with_dll_path(path: &Path) -> io::Result<()> {
    araview_core::thumb_shell::register_with_dll_path(path).map_err(io::Error::other)
}

pub fn unregister() -> io::Result<()> {
    araview_core::thumb_shell::unregister()
        .map(|_| ())
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clsids_are_stable_and_channel_separated() {
        assert_eq!(CLSID_RELEASE_STR, "{FD6BD976-2DF4-4656-94F2-1D166163EC59}");
        assert_eq!(CLSID_DEV_STR, "{BD277595-1702-4AC5-AA7C-A67965C3D570}");
        assert_ne!(CLSID_RELEASE_STR, CLSID_DEV_STR);
    }

    #[test]
    fn iid_is_the_thumbnail_provider_slot() {
        assert_eq!(
            IID_ITHUMBNAILPROVIDER,
            "{E357FCCD-A995-4576-B01F-234630154E96}"
        );
    }

    #[test]
    fn register_with_a_missing_dll_path_is_an_error() {
        // 등록은 core가 담당하므로 여기서는 위임 경로의 실패 전달만 본다.
        let dir = std::env::temp_dir().join(format!(
            "araview-thumb-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let result = register_with_dll_path(&dir.join("araview_thumb.dll"));
        assert!(result.is_err());
    }
}
