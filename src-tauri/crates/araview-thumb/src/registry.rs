//! Windows Explorer PSD thumbnail handler registration (per-user, HKCU).
//!
//! Layout after a successful install:
//!
//! ```text
//! HKCU\Software\Classes\CLSID\{AraView PSD CLSID}
//!     (Default)            = "AraView PSD Thumbnail Provider"
//!     InprocServer32\
//!         (Default)        = "C:\path\to\araview_thumb.dll"
//!         ThreadingModel   = "Apartment"
//!
//! HKCU\Software\Classes\.psd\ShellEx\{E357FCCD-A995-4576-B01F-234630154E96}
//!     (Default)            = "{AraView PSD CLSID}"
//!
//! HKCU\Software\Classes\<ProgID>\ShellEx\{E357FCCD-...}
//!     (Default)            = "{AraView PSD CLSID}"
//! ```
//!
//! `IInitializeWithStream` is implemented, so the handler runs in the
//! isolated surrogate by default. No `DisableProcessIsolation` is set.
//!
//! CLSID strings are duplicated in the main app (`src/thumb_shell.rs`, which
//! deliberately avoids the `windows` crate dependency). Keep both in sync.
//! **Never change a published CLSID**; it is baked into users' registries.

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
use winreg::enums::*;
use winreg::RegKey;

/// Release channel thumbnail provider CLSID. **Never change.**
pub const CLSID_RELEASE_STR: &str = "{FD6BD976-2DF4-4656-94F2-1D166163EC59}";
/// Dev channel thumbnail provider CLSID. **Never change.**
pub const CLSID_DEV_STR: &str = "{BD277595-1702-4AC5-AA7C-A67965C3D570}";

/// Active channel CLSID: dev builds register a separate handler so they
/// never steal the installed build's Explorer thumbnails.
pub const CLSID_STR: &str = if cfg!(debug_assertions) {
    CLSID_DEV_STR
} else {
    CLSID_RELEASE_STR
};

pub const DISPLAY_NAME: &str = if cfg!(debug_assertions) {
    "AraView (Dev) PSD Thumbnail Provider"
} else {
    "AraView PSD Thumbnail Provider"
};

/// ProgID owned by the active channel (`file_assoc::prog_id_for("psd")`).
pub const PROG_ID: &str = if cfg!(debug_assertions) {
    "com.araview.viewer.dev.psd"
} else {
    "com.araview.viewer.psd"
};

/// Standard IID of `IThumbnailProvider`. Explorer looks under
/// `.<ext>\ShellEx\<this IID>` to find the thumbnail handler.
pub const IID_ITHUMBNAILPROVIDER: &str = "{E357FCCD-A995-4576-B01F-234630154E96}";

const CLASSES_BASE: &str = "Software\\Classes";
const PSD_MIME: &str = "image/vnd.adobe.photoshop";

fn clsid_root() -> String {
    format!("{CLASSES_BASE}\\CLSID\\{CLSID_STR}")
}

fn ext_shellex_path() -> String {
    format!("{CLASSES_BASE}\\.psd\\ShellEx\\{IID_ITHUMBNAILPROVIDER}")
}

fn prog_id_shellex_path() -> String {
    format!("{CLASSES_BASE}\\{PROG_ID}\\ShellEx\\{IID_ITHUMBNAILPROVIDER}")
}

fn hkcu() -> RegKey {
    RegKey::predef(HKEY_CURRENT_USER)
}

/// Resolve this DLL's own path. Only meaningful when running inside
/// `araview_thumb.dll` (i.e. from `DllRegisterServer`).
fn own_dll_path() -> io::Result<PathBuf> {
    // SAFETY: with `GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS` the second
    // argument is an address inside the module, not a string, so passing this
    // function's own address as a `PCWSTR` is the documented idiom.
    // `UNCHANGED_REFCOUNT` means the returned handle must not be freed.
    unsafe {
        let mut hmodule = HMODULE::default();
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(own_dll_path as *const () as *const u16),
            &mut hmodule,
        )
        .map_err(|e| io::Error::other(format!("GetModuleHandleExW failed: {e}")))?;
        let mut buf = vec![0u16; 32768];
        let len = GetModuleFileNameW(Some(hmodule), &mut buf) as usize;
        if len == 0 {
            return Err(io::Error::other("GetModuleFileNameW returned 0"));
        }
        Ok(PathBuf::from(OsString::from_wide(&buf[..len])))
    }
}

fn register_clsid(dll_path: &Path) -> io::Result<()> {
    let (clsid_key, _) = hkcu().create_subkey(clsid_root())?;
    clsid_key.set_value("", &DISPLAY_NAME)?;
    let (inproc, _) = clsid_key.create_subkey("InprocServer32")?;
    inproc.set_value("", &dll_path.to_string_lossy().into_owned())?;
    inproc.set_value("ThreadingModel", &"Apartment")?;
    Ok(())
}

fn unregister_clsid() -> io::Result<()> {
    match hkcu().delete_subkey_all(clsid_root()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn register_shellex(path: &str) -> io::Result<()> {
    let (key, _) = hkcu().create_subkey(path)?;
    key.set_value("", &CLSID_STR.to_string())?;
    Ok(())
}

fn unregister_shellex(path: &str) -> io::Result<()> {
    match hkcu().delete_subkey_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Mark `.psd` as an image so Explorer treats it like one (Kind,
/// Details pane image properties). Only fills in missing values.
fn ensure_psd_perceived_type() -> io::Result<()> {
    let (ext_key, _) = hkcu().create_subkey(format!("{CLASSES_BASE}\\.psd"))?;
    let perceived: Result<String, _> = ext_key.get_value("PerceivedType");
    if perceived.map(|v| v.is_empty()).unwrap_or(true) {
        ext_key.set_value("PerceivedType", &"image")?;
    }
    let content_type: Result<String, _> = ext_key.get_value("Content Type");
    if content_type.map(|v| v.is_empty()).unwrap_or(true) {
        ext_key.set_value("Content Type", &PSD_MIME)?;
    }
    Ok(())
}

fn notify_shell() {
    use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
    // SAFETY: notification-only call with null item pointers, matching the
    // documented `SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, NULL, NULL)`
    // usage.
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
    }
}

/// Register the handler into HKCU using this DLL's own path.
/// Called by `regsvr32 araview_thumb.dll`. No admin rights required.
pub fn register() -> io::Result<()> {
    let dll_path = own_dll_path()?;
    register_with_dll_path(&dll_path)
}

/// Shared core used by `register()` and tests.
pub fn register_with_dll_path(dll_path: &Path) -> io::Result<()> {
    register_clsid(dll_path)?;
    register_shellex(&ext_shellex_path())?;
    register_shellex(&prog_id_shellex_path())?;
    ensure_psd_perceived_type()?;
    notify_shell();
    Ok(())
}

/// Remove the handler from HKCU (best effort). Called by
/// `regsvr32 /u araview_thumb.dll`.
pub fn unregister() -> io::Result<()> {
    let _ = unregister_shellex(&ext_shellex_path());
    let _ = unregister_shellex(&prog_id_shellex_path());
    let _ = unregister_clsid();
    notify_shell();
    Ok(())
}

/// True iff the `.psd` ShellEx slot currently points at our CLSID.
pub fn is_registered() -> bool {
    for path in [ext_shellex_path(), prog_id_shellex_path()] {
        let Ok(key) = hkcu().open_subkey(&path) else {
            return false;
        };
        let Ok(value): Result<String, _> = key.get_value("") else {
            return false;
        };
        if !value.eq_ignore_ascii_case(CLSID_STR) {
            return false;
        }
    }
    hkcu()
        .open_subkey(format!("{}\\InprocServer32", clsid_root()))
        .is_ok()
}

/// Read back the registered DLL path, if any.
pub fn registered_dll_path() -> Option<PathBuf> {
    let key = hkcu()
        .open_subkey(format!("{}\\InprocServer32", clsid_root()))
        .ok()?;
    let path: String = key.get_value("").ok()?;
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clsids_are_stable_and_channel_separated() {
        assert_eq!(CLSID_RELEASE_STR, "{FD6BD976-2DF4-4656-94F2-1D166163EC59}");
        assert_eq!(CLSID_DEV_STR, "{BD277595-1702-4AC5-AA7C-A67965C3D570}");
        assert_ne!(CLSID_RELEASE_STR, CLSID_DEV_STR);
        // Active channel follows the build profile.
        assert_eq!(
            CLSID_STR,
            if cfg!(debug_assertions) {
                CLSID_DEV_STR
            } else {
                CLSID_RELEASE_STR
            }
        );
    }

    #[test]
    fn registry_paths_use_thumbnail_shellex_iid() {
        assert_eq!(
            ext_shellex_path(),
            "Software\\Classes\\.psd\\ShellEx\\{E357FCCD-A995-4576-B01F-234630154E96}"
        );
        assert!(clsid_root().starts_with("Software\\Classes\\CLSID\\{"));
        assert!(prog_id_shellex_path().contains(PROG_ID));
    }

    #[test]
    fn shellex_register_roundtrip_in_sandbox() {
        let sandbox = format!(
            "Software\\AraViewThumbTest_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let path = format!("{sandbox}\\.psd\\ShellEx\\{IID_ITHUMBNAILPROVIDER}");
        let root = hkcu();
        assert!(root.open_subkey(&path).is_err());
        let (key, _) = root.create_subkey(&path).expect("create sandbox key");
        key.set_value("", &CLSID_STR.to_string())
            .expect("set clsid");
        let back: String = root
            .open_subkey(&path)
            .expect("open")
            .get_value("")
            .expect("read");
        assert!(back.eq_ignore_ascii_case(CLSID_STR));
        root.delete_subkey_all(&sandbox).expect("cleanup");
    }
}
