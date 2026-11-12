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
use std::io::Write as _;
use std::path::{Path, PathBuf};

use windows::core::{implement, IUnknown, Interface, Ref, Result, BOOL, GUID, PCWSTR};
use windows::Win32::Foundation::{CLASS_E_NOAGGREGATION, E_FAIL, E_INVALIDARG, E_POINTER};
use windows::Win32::Graphics::Gdi::HBITMAP;
use windows::Win32::System::Com::{
    CoTaskMemFree, IClassFactory, IClassFactory_Impl, IStream, STATFLAG_DEFAULT, STATSTG,
    STREAM_SEEK_SET,
};
use windows::Win32::UI::Shell::PropertiesSystem::{
    IInitializeWithFile, IInitializeWithFile_Impl, IInitializeWithStream,
    IInitializeWithStream_Impl,
};
use windows::Win32::UI::Shell::{
    IThumbnailProvider, IThumbnailProvider_Impl, WTSAT_RGB, WTS_ALPHATYPE,
};

use crate::bitmap;

/// Release channel CLSID. **Never change** (see `registry.rs`).
pub const CLSID_RELEASE: GUID = GUID::from_u128(0xfd6bd976_2df4_4656_94f2_1d166163ec59);
/// Dev channel CLSID. **Never change** (see `registry.rs`).
pub const CLSID_DEV: GUID = GUID::from_u128(0xbd277595_1702_4ac5_aa7c_a67965c3d570);

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

fn stream_file(stream: &IStream) -> Result<tempfile::NamedTempFile> {
    // SAFETY: `stream` is a live COM interface handed to us by the caller;
    // `Stat`/`Read` only touch the local out-params below. A hostile stream
    // can lie about sizes, so every length it reports is bounded before use.
    unsafe {
        let mut stat = STATSTG::default();
        stream.Stat(&mut stat, STATFLAG_DEFAULT)?;
        let name = if stat.pwcsName.is_null() {
            None
        } else {
            let name = stat.pwcsName.to_string().ok();
            CoTaskMemFree(Some(stat.pwcsName.0.cast()));
            name
        };
        let size: u64 = stat.cbSize;
        if size > MAX_STREAM_BYTES {
            return Err(E_FAIL.into());
        }
        stream.Seek(0, STREAM_SEEK_SET, None)?;
        let mut head = [0u8; 32];
        let mut filled = 0;
        while filled < head.len() {
            let mut read = 0;
            stream
                .Read(
                    head[filled..].as_mut_ptr().cast(),
                    (head.len() - filled) as u32,
                    Some(&mut read),
                )
                .ok()?;
            if read as usize > head.len() - filled {
                return Err(fail());
            }
            if read == 0 {
                break;
            }
            filled += read as usize;
        }
        stream.Seek(0, STREAM_SEEK_SET, None)?;
        // Only a whitelisted suffix is retained, never the stream's path.
        let suffix = stream_extension(name.as_deref(), &head[..filled]);
        let mut out = tempfile::Builder::new()
            .suffix(&format!(".{suffix}"))
            .tempfile()
            .map_err(|_| fail())?;
        let mut buf = vec![0u8; 65536];
        let mut total = 0u64;
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
            let read = read as usize;
            if read > buf.len() {
                return Err(fail());
            }
            if read == 0 {
                break;
            }
            total += read as u64;
            if total > MAX_STREAM_BYTES {
                return Err(E_FAIL.into());
            }
            out.write_all(&buf[..read]).map_err(|_| fail())?;
        }
        out.flush().map_err(|_| fail())?;
        Ok(out)
    }
}

fn fail() -> windows::core::Error {
    windows::core::Error::from_hresult(E_FAIL)
}

fn stream_extension(name: Option<&str>, head: &[u8]) -> String {
    if let Some(ext) = name
        .and_then(|name| Path::new(name).extension())
        .and_then(|ext| ext.to_str())
    {
        if araview_core::thumb_shell::thumbnail_extensions()
            .any(|known| known.eq_ignore_ascii_case(ext))
        {
            return ext.to_ascii_lowercase();
        }
    }
    if head.starts_with(b"PK") {
        return "cbz".into();
    }
    match araview_core::sniff::sniff_mime(head) {
        Some("image/vnd.adobe.photoshop") => "psd",
        Some("image/x-exr") => "exr",
        Some("image/heic" | "image/heif" | "image/avif") => "heic",
        _ if head.get(4..8) == Some(b"ftyp") && head.get(8..12) != Some(b"crx ") => "heic",
        // TIFF RAW and vendor containers are identified by the RAW decoder.
        _ => "dng",
    }
    .into()
}

fn thumbnail_for_path(path: &Path, cx: u32) -> Result<HBITMAP> {
    let rgb = araview_core::ops::decode_shell_thumbnail(path, cx).map_err(|_| fail())?;
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
            if !matches!(*self.this.state.borrow(), InitState::Empty) {
                return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                    0x800704df_u32 as i32,
                )));
            }
            let stream = pstream
                .cloned()
                .ok_or_else(|| windows::core::Error::from_hresult(E_INVALIDARG))?;
            *self.this.state.borrow_mut() = InitState::Stream(stream);
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
            if !matches!(*self.this.state.borrow(), InitState::Empty) {
                return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                    0x800704df_u32 as i32,
                )));
            }
            if pszfilepath.is_null() {
                return Err(windows::core::Error::from_hresult(E_INVALIDARG));
            }
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
        // SAFETY: the COM caller supplies valid output storage, checked non-null above.
        unsafe {
            *phbmp = HBITMAP::default();
            *pdwalpha = WTSAT_RGB;
        }
        let cx = cx.clamp(MIN_CX, MAX_CX);

        let state = self.this.state.borrow().duplicate();
        let hbmp = match &state {
            InitState::Stream(stream) => {
                let file = stream_file(stream)?;
                thumbnail_for_path(file.path(), cx)?
            }
            InitState::Path(path) => thumbnail_for_path(path, cx)?,
            InitState::Empty => return Err(E_FAIL.into()),
        };

        // Wrong content, PSB, truncated file, oversize: fall back to
        // Explorer's default icon instead of a broken bitmap.

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
