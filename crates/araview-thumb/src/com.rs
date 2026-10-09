//! COM objects: `ThumbClassFactory` and `PsdThumbProvider`.
//!
//! - `ThumbClassFactory` implements `IClassFactory` and hands fresh
//!   providers to COM (`DllGetClassObject`).
//! - `PsdThumbProvider` implements `IInitializeWithStream` (primary path:
//!   Explorer's isolated surrogate only uses stream init),
//!   `IInitializeWithFile` (direct third-party requests) and
//!   `IThumbnailProvider` (the `GetThumbnail` bitmap request).
//!
//! Every cross-ABI entry point is wrapped in `catch_unwind`: a panic in
//! the decoder must surface as `E_FAIL`, never as UB inside Explorer.

use std::cell::RefCell;
use std::ffi::c_void;
use std::path::PathBuf;

use windows::core::{implement, IUnknown, Interface, Ref, Result, BOOL, GUID, PCWSTR};
use windows::Win32::Foundation::{CLASS_E_NOAGGREGATION, E_FAIL, E_POINTER};
use windows::Win32::Graphics::Gdi::HBITMAP;
use windows::Win32::System::Com::{
    IClassFactory, IClassFactory_Impl, IStream, STATFLAG_DEFAULT, STATSTG,
};
use windows::Win32::UI::Shell::PropertiesSystem::{
    IInitializeWithFile, IInitializeWithFile_Impl, IInitializeWithStream,
    IInitializeWithStream_Impl,
};
use windows::Win32::UI::Shell::{
    IThumbnailProvider, IThumbnailProvider_Impl, WTSAT_RGB, WTS_ALPHATYPE,
};

use crate::{bitmap, psd};

/// Release channel CLSID. **Never change** (see `registry.rs`).
pub const CLSID_RELEASE: GUID = GUID::from_u128(0xFD6BD976_2DF4_4656_94F2_1D166163EC59);
/// Dev channel CLSID. **Never change** (see `registry.rs`).
pub const CLSID_DEV: GUID = GUID::from_u128(0xBD277595_1702_4AC5_AA7C_A67965C3D570);

const MIN_CX: u32 = 16;
const MAX_CX: u32 = 2560;
/// Fail-fast stream cap (the decode budget guards the rest).
const MAX_STREAM_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Default)]
enum InitState {
    #[default]
    Empty,
    Path(PathBuf),
    Stream(IStream),
}

impl InitState {
    fn duplicate(&self) -> InitState {
        match self {
            InitState::Empty => InitState::Empty,
            InitState::Path(p) => InitState::Path(p.clone()),
            InitState::Stream(s) => InitState::Stream(s.clone()),
        }
    }
}

fn read_stream_all(stream: &IStream) -> Result<Vec<u8>> {
    // SAFETY: `stream` is a live COM interface handed to us by the caller;
    // `Stat`/`Read` only touch the local out-params below. A hostile stream
    // can lie about sizes, so every length it reports is bounded before use.
    unsafe {
        let mut stat = STATSTG::default();
        stream.Stat(&mut stat, STATFLAG_DEFAULT)?;
        let size: u64 = stat.cbSize;
        if size > MAX_STREAM_BYTES {
            return Err(E_FAIL.into());
        }
        let mut out = Vec::with_capacity(size.min(8 * 1024 * 1024) as usize);
        let mut buf = vec![0u8; 65536];
        loop {
            let mut read: u32 = 0;
            // `Read` returns `HRESULT` (S_FALSE at EOF is not an error),
            // so inspect it manually instead of `?`.
            let hr = stream.Read(
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as u32,
                Some(&mut read),
            );
            if hr.is_err() {
                return Err(windows::core::Error::from_hresult(hr));
            }
            // A buggy or hostile `IStream` may report more bytes than the
            // buffer holds; clamp instead of panicking on the slice index.
            let read = (read as usize).min(buf.len());
            if read == 0 {
                break;
            }
            out.extend_from_slice(&buf[..read]);
            if out.len() as u64 > MAX_STREAM_BYTES {
                return Err(E_FAIL.into());
            }
        }
        Ok(out)
    }
}

fn thumbnail_for_bytes(bytes: &[u8], cx: u32) -> Result<HBITMAP> {
    let rgb =
        psd::decode_psd_rgb8(bytes).map_err(|_| windows::core::Error::from_hresult(E_FAIL))?;
    let (nw, nh) = psd::fit_inside(rgb.width, rgb.height, cx);
    let rgb = if (nw, nh) == (rgb.width, rgb.height) {
        rgb
    } else {
        psd::resize_rgb8(&rgb, nw, nh)
    };
    bitmap::from_rgb8(&rgb)
}

// =============================================================================
// IClassFactory
// =============================================================================

#[implement(IClassFactory)]
pub struct ThumbClassFactory;

impl IClassFactory_Impl for ThumbClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Ref<'_, IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> Result<()> {
        if !punkouter.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        if ppvobject.is_null() || riid.is_null() {
            return Err(E_POINTER.into());
        }
        // SAFETY: both pointers were null-checked above and, per the
        // `IClassFactory` contract, point to valid aligned storage for the
        // duration of the call. `query` hands the caller one owned reference.
        unsafe {
            *ppvobject = std::ptr::null_mut();
            let provider = PsdThumbProvider::default();
            let unknown: IUnknown = provider.into();
            unknown.query(&*riid, ppvobject).ok()
        }
    }

    fn LockServer(&self, _flock: BOOL) -> Result<()> {
        Ok(())
    }
}

// =============================================================================
// PsdThumbProvider
// =============================================================================

#[implement(IThumbnailProvider, IInitializeWithStream, IInitializeWithFile)]
#[derive(Default)]
pub struct PsdThumbProvider {
    state: RefCell<InitState>,
}

impl IInitializeWithStream_Impl for PsdThumbProvider_Impl {
    fn Initialize(&self, pstream: Ref<'_, IStream>, _grfmode: u32) -> Result<()> {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            *self.this.state.borrow_mut() = pstream
                .cloned()
                .map(InitState::Stream)
                .unwrap_or(InitState::Empty);
            Ok(())
        }));
        match result {
            Ok(r) => r,
            Err(_) => Err(windows::core::Error::from_hresult(E_FAIL)),
        }
    }
}

impl IInitializeWithFile_Impl for PsdThumbProvider_Impl {
    fn Initialize(&self, pszfilepath: &PCWSTR, _grfmode: u32) -> Result<()> {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // SAFETY: per `IInitializeWithFile`, `pszfilepath` is a valid
            // NUL-terminated UTF-16 string owned by the caller for this call.
            let path = unsafe { pszfilepath.to_string() }?;
            *self.this.state.borrow_mut() = InitState::Path(PathBuf::from(path));
            Ok(())
        }));
        match result {
            Ok(r) => r,
            Err(_) => Err(windows::core::Error::from_hresult(E_FAIL)),
        }
    }
}

impl IThumbnailProvider_Impl for PsdThumbProvider_Impl {
    fn GetThumbnail(
        &self,
        cx: u32,
        phbmp: *mut HBITMAP,
        pdwalpha: *mut WTS_ALPHATYPE,
    ) -> Result<()> {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.get_thumbnail_inner(cx, phbmp, pdwalpha)
        }));
        match result {
            Ok(r) => r,
            Err(_) => Err(windows::core::Error::from_hresult(E_FAIL)),
        }
    }
}

impl PsdThumbProvider_Impl {
    fn get_thumbnail_inner(
        &self,
        cx: u32,
        phbmp: *mut HBITMAP,
        pdwalpha: *mut WTS_ALPHATYPE,
    ) -> Result<()> {
        if phbmp.is_null() || pdwalpha.is_null() {
            return Err(E_POINTER.into());
        }
        let cx = cx.clamp(MIN_CX, MAX_CX);

        let state = self.this.state.borrow().duplicate();
        let bytes: Vec<u8> = match &state {
            InitState::Stream(stream) => read_stream_all(stream)?,
            InitState::Path(path) => {
                std::fs::read(path).map_err(|_| windows::core::Error::from_hresult(E_FAIL))?
            }
            InitState::Empty => return Err(E_FAIL.into()),
        };

        // Wrong content, PSB, truncated file, oversize: fall back to
        // Explorer's default icon instead of a broken bitmap.
        let hbmp = thumbnail_for_bytes(&bytes, cx)?;

        // SAFETY: both out-params were null-checked at the top of this
        // function and, per `IThumbnailProvider`, point to valid aligned
        // storage. Ownership of `hbmp` transfers to the caller here.
        unsafe {
            *phbmp = hbmp;
            *pdwalpha = WTSAT_RGB;
        }
        Ok(())
    }
}
