//! Windows Explorer thumbnail handler registration shared by the app and DLL.
//!
//! The handler itself is `araview_thumb.dll` (`crates/araview-thumb`,
//! `IThumbnailProvider` + `IInitializeWithStream`/`IInitializeWithFile`).
//! This module owns the per-user (HKCU) registry glue so Settings can
//! enable/disable Explorer thumbnails without admin rights.
//!
//! CLSID strings are duplicated from the DLL crate, which deliberately
//! avoids pulling the `windows` crate into the app. Keep both in sync.
//! **Never change a published CLSID**; it is baked into users' registries.

use std::ffi::c_void;
use std::path::{Path, PathBuf};

use serde::Serialize;
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

use crate::app_error::AppError;

/// Thumbnail DLL file name, next to the exe (dev) or under
/// `resources/` (installed bundle).
pub const DLL_FILE_NAME: &str = "araview_thumb.dll";
/// Release channel CLSID. **Never change.**
pub const CLSID_RELEASE: &str = "{FD6BD976-2DF4-4656-94F2-1D166163EC59}";
/// Dev channel CLSID. **Never change.**
pub const CLSID_DEV: &str = "{BD277595-1702-4AC5-AA7C-A67965C3D570}";
/// `IThumbnailProvider` Shell extension slot.
pub const IID_THUMBNAIL: &str = "{E357FCCD-A995-4576-B01F-234630154E96}";

/// Dev builds register a separate handler so they never steal the
/// installed build's Explorer thumbnails.
const IS_DEV_BUILD: bool = cfg!(debug_assertions);

pub fn active_clsid() -> &'static str {
    if IS_DEV_BUILD {
        CLSID_DEV
    } else {
        CLSID_RELEASE
    }
}

pub fn display_name() -> &'static str {
    if IS_DEV_BUILD {
        "AraView (Dev) Thumbnail Provider"
    } else {
        "AraView Thumbnail Provider"
    }
}

fn classes_path(suffix: &str) -> String {
    format!(r"Software\Classes\{suffix}")
}

pub fn clsid_root_key() -> String {
    classes_path(&format!(r"CLSID\{}", active_clsid()))
}

/// Keep the RAW/archive set tied to the viewer's supported formats.
pub fn thumbnail_extensions() -> impl Iterator<Item = &'static str> {
    crate::image::SUPPORTED_EXTENSIONS
        .iter()
        .copied()
        .filter(|ext| {
            matches!(*ext, "psd" | "heic" | "heif" | "avif" | "exr")
                || matches!(
                    crate::image::get_mime_type(Path::new(&format!("file.{ext}"))),
                    Some(
                        crate::raw::RAW_MIME | "application/zip" | "application/vnd.comicbook+zip"
                    )
                )
        })
}

pub fn ext_shellex_key(ext: &str) -> String {
    classes_path(&format!(r".{ext}\ShellEx\{IID_THUMBNAIL}"))
}

pub fn prog_id_shellex_key(ext: &str) -> String {
    classes_path(&format!(
        r"{}\ShellEx\{IID_THUMBNAIL}",
        crate::file_assoc::prog_id_for(ext)
    ))
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct ThumbStatus {
    pub registered: bool,
    pub fully_registered: bool,
    pub clsid: String,
    pub dll_path: String,
    pub dll_exists: bool,
}

/// Locate the thumbnail DLL: next to the exe (dev workspace target dir)
/// or under `resources/` (installed NSIS bundle).
pub fn thumb_dll_path() -> Result<PathBuf, AppError> {
    let exe = std::env::current_exe()
        .map_err(|e| AppError::unknown(format!("Failed to get executable path: {e}")))?;
    let dir = exe
        .parent()
        .ok_or_else(|| AppError::unknown("Failed to resolve executable directory".to_string()))?;
    let direct = dir.join(DLL_FILE_NAME);
    if direct.is_file() {
        return Ok(direct);
    }
    Ok(dir.join("resources").join(DLL_FILE_NAME))
}

fn shellex_points_to_us(key_path: &str) -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(key) = hkcu.open_subkey(key_path) else {
        return false;
    };
    let Ok(value): Result<String, _> = key.get_value("") else {
        return false;
    };
    value.eq_ignore_ascii_case(active_clsid())
}

fn clsid_key_present() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(format!(r"{}\InprocServer32", clsid_root_key()))
        .is_ok()
}

pub fn status() -> Result<ThumbStatus, AppError> {
    let dll_path = thumb_dll_path()?;
    let slots: Vec<bool> = thumbnail_extensions()
        .flat_map(|ext| [ext_shellex_key(ext), prog_id_shellex_key(ext)])
        .map(|key| shellex_points_to_us(&key))
        .collect();
    Ok(ThumbStatus {
        registered: slots.iter().any(|&registered| registered),
        fully_registered: slots.iter().all(|&registered| registered) && clsid_key_present(),
        clsid: active_clsid().to_string(),
        dll_path: dll_path.to_string_lossy().to_string(),
        dll_exists: dll_path.is_file(),
    })
}

fn reg_err(error: impl std::fmt::Display) -> AppError {
    AppError::unknown(format!("Registry error: {error}"))
}

fn register_clsid(dll_path: &Path) -> Result<(), AppError> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (clsid_key, _) = hkcu.create_subkey(clsid_root_key()).map_err(reg_err)?;
    clsid_key.set_value("", &display_name()).map_err(reg_err)?;
    let (inproc, _) = clsid_key.create_subkey("InprocServer32").map_err(reg_err)?;
    inproc
        .set_value("", &dll_path.to_string_lossy().into_owned())
        .map_err(reg_err)?;
    inproc
        .set_value("ThreadingModel", &"Apartment")
        .map_err(reg_err)?;
    Ok(())
}

fn register_shellex(key_path: &str) -> Result<(), AppError> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(key_path).map_err(reg_err)?;
    match key.get_value::<String, _>("") {
        Ok(current) if !current.eq_ignore_ascii_case(active_clsid()) => {
            key.set_value(previous_value_name(), &current)
                .map_err(reg_err)?;
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // A removed/replaced slot must not restore a stale backup from
            // an earlier registration when the user later disables us.
            match key.delete_value(previous_value_name()) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(reg_err(e)),
            }
        }
        Err(e) => return Err(reg_err(e)),
    }
    key.set_value("", &active_clsid()).map_err(reg_err)?;
    Ok(())
}

/// Fill in image hints if missing. Never touches UserChoice or ZIP's kind.
fn ensure_perceived_type(ext: &str) -> Result<(), AppError> {
    let mime = crate::image::get_mime_type(Path::new(&format!("file.{ext}")))
        .ok_or_else(|| AppError::unsupported("Unsupported thumbnail extension"))?;
    if !mime.starts_with("image/") {
        return Ok(());
    }
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (ext_key, _) = hkcu
        .create_subkey(classes_path(&format!(".{ext}")))
        .map_err(reg_err)?;
    let perceived: Result<String, _> = ext_key.get_value("PerceivedType");
    if perceived.map(|v| v.is_empty()).unwrap_or(true) {
        ext_key
            .set_value("PerceivedType", &"image")
            .map_err(reg_err)?;
    }
    let content_type: Result<String, _> = ext_key.get_value("Content Type");
    if content_type.map(|v| v.is_empty()).unwrap_or(true) {
        ext_key.set_value("Content Type", &mime).map_err(reg_err)?;
    }
    Ok(())
}

fn delete_key_best_effort(key_path: &str) {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Err(e) = hkcu.delete_subkey_all(key_path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            log::warn!("[thumb-shell] failed to delete {key_path}: {e}");
        }
    }
}

/// 셸에 연결 프로그램/아이콘 변경을 알린다. `file_assoc.rs`도 같은 알림을
/// 쓰므로 `pub(crate)`다 (등록 후 Explorer가 즉시 다시 읽게 한다).
pub(crate) fn notify_shell() {
    const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
    const SHCNF_IDLIST: u32 = 0;
    // SAFETY: notification-only call with null item pointers, matching
    // the documented `SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, NULL, NULL)` usage.
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
}

//   SHChangeNotify(LONG wEventId, UINT uFlags, LPCVOID dwItem1, LPCVOID dwItem2)
// 앱은 의도적으로 `windows` 크레이트를 쓰지 않으므로(위 모듈 주석) 직접
// 선언한다. 시그니처를 바꿀 때는 MSDN 원본과 대조할 것.
#[link(name = "shell32")]
extern "system" {
    fn SHChangeNotify(
        w_event_id: i32,
        u_flags: u32,
        dw_item1: *const c_void,
        dw_item2: *const c_void,
    );
}

pub fn register() -> Result<ThumbStatus, AppError> {
    let dll_path = thumb_dll_path()?;
    register_with_dll_path(&dll_path)?;
    status()
}

pub fn register_with_dll_path(dll_path: &Path) -> Result<(), AppError> {
    if !dll_path.is_file() {
        return Err(AppError::not_found(format!(
            "Thumbnail DLL not found at {} (build it with `cargo build -p araview-thumb`)",
            dll_path.display()
        )));
    }
    register_clsid(dll_path)?;
    for ext in thumbnail_extensions() {
        register_shellex(&ext_shellex_key(ext))?;
        register_shellex(&prog_id_shellex_key(ext))?;
        ensure_perceived_type(ext)?;
    }
    notify_shell();
    Ok(())
}

fn previous_value_name() -> String {
    format!("AraView.Previous.{}", active_clsid())
}

fn unregister_shellex(path: &str) -> Result<(), AppError> {
    if !shellex_points_to_us(path) {
        return Ok(());
    }
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(path, winreg::enums::KEY_ALL_ACCESS)
        .map_err(reg_err)?;
    match key.get_value::<String, _>(&previous_value_name()) {
        Ok(previous) => key.set_value("", &previous).map_err(reg_err)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            key.delete_value("").map_err(reg_err)?
        }
        Err(e) => return Err(reg_err(e)),
    }
    let _ = key.delete_value(previous_value_name());
    // Leave unrelated named values and subkeys intact.
    if key.enum_values().next().is_none() && key.enum_keys().next().is_none() {
        drop(key);
        hkcu.delete_subkey(path).map_err(reg_err)?;
    }
    Ok(())
}

pub fn unregister() -> Result<ThumbStatus, AppError> {
    // 셸 슬롯은 우리 CLSID를 가리킬 때만 지운다. 등록 이후 다른 앱이 그
    // 슬롯을 덮어썼다면 AraView 해제가 그 앱의 등록까지 지워버린다.
    for ext in thumbnail_extensions() {
        unregister_shellex(&ext_shellex_key(ext))?;
        unregister_shellex(&prog_id_shellex_key(ext))?;
    }
    // CLSID 키는 채널별로 고유한 우리 값이라 소유 확인 없이 지워도 안전하다.
    delete_key_best_effort(&clsid_root_key());
    notify_shell();
    status()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_follow_supported_raw_and_archives() {
        let extensions: Vec<_> = thumbnail_extensions().collect();
        assert_eq!(extensions.len(), 26);
        for ext in crate::image::SUPPORTED_EXTENSIONS {
            let mime = crate::image::get_mime_type(Path::new(&format!("test.{ext}"))).unwrap();
            let expected = matches!(*ext, "psd" | "heic" | "heif" | "avif" | "exr")
                || mime == crate::raw::RAW_MIME
                || mime.starts_with("application/");
            assert_eq!(extensions.contains(ext), expected, "{ext}");
        }
    }

    #[test]
    fn registration_restores_previous_and_preserves_new_owner() {
        // These are private test keys, never Explorer's real association slots.
        let unique = tempfile::tempdir().unwrap();
        let path = format!(
            r"Software\AraView-Thumbnail-Test\{}",
            unique.path().file_name().unwrap().to_string_lossy()
        );
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(&path).unwrap();
        key.set_value("", &"{previous-provider}").unwrap();
        key.set_value("unrelated", &"keep").unwrap();
        register_shellex(&path).unwrap();
        register_shellex(&path).unwrap();
        assert!(shellex_points_to_us(&path));
        unregister_shellex(&path).unwrap();
        assert_eq!(
            key.get_value::<String, _>("").unwrap(),
            "{previous-provider}"
        );
        assert_eq!(key.get_value::<String, _>("unrelated").unwrap(), "keep");
        register_shellex(&path).unwrap();
        key.set_value("", &"{new-provider}").unwrap();
        unregister_shellex(&path).unwrap();
        assert_eq!(key.get_value::<String, _>("").unwrap(), "{new-provider}");
        key.delete_value("").unwrap();
        register_shellex(&path).unwrap();
        unregister_shellex(&path).unwrap();
        assert!(key.get_value::<String, _>("").is_err());
        assert_eq!(key.get_value::<String, _>("unrelated").unwrap(), "keep");
        drop(key);
        hkcu.delete_subkey_all(&path).unwrap();
    }
}
