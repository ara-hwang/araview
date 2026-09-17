//! AraView PSD thumbnail shell extension (`araview_thumb.dll`).
//!
//! Implements `IThumbnailProvider` (+ `IInitializeWithStream` /
//! `IInitializeWithFile`) for `.psd` so Windows Explorer renders real
//! thumbnails instead of the generic icon. PSD composites are decoded
//! with the pure-Rust `psd` crate and flattened onto white, matching the
//! main app's JPEG sidecar rule. PSB (`8BPB`) is rejected.
//!
//! Registration is per-user (HKCU, no admin):
//! `regsvr32 araview_thumb.dll` / `regsvr32 /u araview_thumb.dll`.
//! The main app also drives the same keys from Settings
//! (`src/thumb_shell.rs`) so users never touch the command line.

#![allow(non_snake_case)]

mod bitmap;
mod com;
pub mod psd;
pub mod registry;

use std::panic::catch_unwind;

use windows::core::{Interface, GUID, HRESULT};
use windows::Win32::Foundation::{E_FAIL, E_POINTER, S_FALSE, S_OK};
use windows::Win32::System::Com::IClassFactory;

use com::{ThumbClassFactory, CLSID_DEV, CLSID_RELEASE};

/// COM error: "no class factory for the requested CLSID".
const CLASS_E_CLASSNOTAVAILABLE: HRESULT = HRESULT(0x80040111u32 as i32);

/// A panic crossing `extern "system"` is UB (and would crash Explorer),
/// so every DLL entry point funnels through this guard.
fn guard<F: FnOnce() -> HRESULT + std::panic::UnwindSafe>(f: F) -> HRESULT {
    match catch_unwind(f) {
        Ok(hr) => hr,
        Err(_) => E_FAIL,
    }
}

/// Called by COM when a client asks this DLL for a class factory.
///
/// # Safety
///
/// COM ABI surface called by the OLE runtime. `rclsid`, `riid` and `ppv`
/// must be null or point to valid, aligned objects for the call. On
/// success the caller owns one reference and must `Release` it.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    guard(|| {
        if rclsid.is_null() || riid.is_null() || ppv.is_null() {
            return E_POINTER;
        }
        // SAFETY: all three pointers were null-checked above and, per the
        // `DllGetClassObject` contract, point to valid aligned storage for the
        // duration of the call.
        unsafe {
            if *rclsid == CLSID_RELEASE || *rclsid == CLSID_DEV {
                let factory: IClassFactory = ThumbClassFactory.into();
                factory.query(&*riid, ppv)
            } else {
                CLASS_E_CLASSNOTAVAILABLE
            }
        }
    })
}

/// Called by COM to ask whether this DLL can be unloaded. `S_FALSE`
/// keeps us loaded; Explorer unloads us on shutdown.
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    guard(|| S_FALSE)
}

/// Called by `regsvr32 araview_thumb.dll` (per-user, no admin needed).
#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    guard(|| match registry::register() {
        Ok(()) => S_OK,
        Err(_) => E_FAIL,
    })
}

/// Called by `regsvr32 /u araview_thumb.dll`.
#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    guard(|| match registry::unregister() {
        Ok(()) => S_OK,
        Err(_) => E_FAIL,
    })
}
