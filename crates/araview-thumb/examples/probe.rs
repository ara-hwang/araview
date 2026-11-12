//! Exercise the shipped DLL without registry changes or physical input.
//! cargo run -p araview-thumb --example probe -- <absolute DLL> <sample folder>

use std::ffi::c_void;
use std::path::Path;
use windows::core::{Interface as _, GUID, HRESULT, PCSTR, PCWSTR};
use windows::Win32::Graphics::Gdi::{DeleteObject, GetObjectW, BITMAP, HBITMAP, HGDIOBJ};
use windows::Win32::System::Com::{CoInitializeEx, IClassFactory, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_WITH_ALTERED_SEARCH_PATH,
};
use windows::Win32::UI::Shell::PropertiesSystem::{IInitializeWithFile, IInitializeWithStream};
use windows::Win32::UI::Shell::{IThumbnailProvider, SHCreateMemStream, WTSAT_RGB};

fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt as _;
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    let dll = Path::new(args.get(1).ok_or("DLL path required")?);
    let samples = Path::new(args.get(2).ok_or("Sample directory required")?);
    type GetClass =
        unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut c_void) -> HRESULT;
    // SAFETY: the absolute DLL and its export are built from this workspace.
    // The module remains loaded until process exit, after all COM references.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let module = LoadLibraryExW(
            PCWSTR(wide(dll).as_ptr()),
            None,
            LOAD_WITH_ALTERED_SEARCH_PATH,
        )?;
        let export = GetProcAddress(module, PCSTR(c"DllGetClassObject".as_ptr().cast()))
            .ok_or("DllGetClassObject missing")?;
        let get_class: GetClass = std::mem::transmute(export);
        let clsid = GUID::from_u128(0xbd277595_1702_4ac5_aa7c_a67965c3d570);
        let mut ptr = std::ptr::null_mut();
        get_class(&clsid, &IClassFactory::IID, &mut ptr).ok()?;
        let factory = IClassFactory::from_raw(ptr);
        for name in [
            "sample.psd",
            "sample.cbz",
            "sample.zip",
            "sample-comicinfo.cbz",
            "sample.heic",
            "sample.heif",
            "sample.exr",
            "sample.dng",
            "sample.cr3",
            "sample.raf",
            "sample.mrw",
            "sample.3fr",
            "sample.iiq",
            "sample.rwl",
            "sample.kdc",
            "sample.dcr",
            "sample.sr2",
            "sample.srf",
        ] {
            let path = samples.join(name);
            let bytes = std::fs::read(&path)?;
            for mode in ["file", "unnamed stream"] {
                let provider: IThumbnailProvider = factory.CreateInstance(None)?;
                if mode == "file" {
                    let init: IInitializeWithFile = provider.cast()?;
                    init.Initialize(PCWSTR(wide(&path).as_ptr()), 0)?;
                    assert!(init.Initialize(PCWSTR(wide(&path).as_ptr()), 0).is_err());
                } else {
                    let stream = SHCreateMemStream(Some(&bytes)).ok_or("Memory stream failed")?;
                    let init: IInitializeWithStream = provider.cast()?;
                    init.Initialize(&stream, 0)?;
                    assert!(init.Initialize(&stream, 0).is_err());
                }
                for cx in [16, 128, 2560, 128] {
                    let mut bitmap = HBITMAP::default();
                    let mut alpha = WTSAT_RGB;
                    provider.GetThumbnail(cx, &mut bitmap, &mut alpha)?;
                    let mut info = BITMAP::default();
                    let read = GetObjectW(
                        HGDIOBJ(bitmap.0),
                        std::mem::size_of::<BITMAP>() as i32,
                        Some((&mut info as *mut BITMAP).cast()),
                    );
                    let deleted = DeleteObject(HGDIOBJ(bitmap.0)).as_bool();
                    assert!(deleted && read > 0);
                    assert!(info.bmWidth > 0 && info.bmWidth <= cx as i32);
                    assert!(info.bmHeight > 0 && info.bmHeight <= cx as i32);
                    assert_eq!(alpha, WTSAT_RGB);
                }
                println!(
                    "PASS {name}: {mode}, 16/128/2560px, repeated request, initialization guard"
                );
            }
        }
        for bytes in [&b"broken"[..], &b"8BPS\0\x02"[..]] {
            let provider: IThumbnailProvider = factory.CreateInstance(None)?;
            let mut bitmap = HBITMAP::default();
            let mut alpha = WTSAT_RGB;
            assert!(provider.GetThumbnail(128, &mut bitmap, &mut alpha).is_err());
            let init: IInitializeWithStream = provider.cast()?;
            let stream = SHCreateMemStream(Some(bytes)).ok_or("Memory stream failed")?;
            init.Initialize(&stream, 0)?;
            assert!(provider.GetThumbnail(128, &mut bitmap, &mut alpha).is_err());
            assert!(bitmap.is_invalid());
        }
        println!("PASS uninitialized, corrupt, and PSB requests return errors without bitmaps");
    }
    Ok(())
}
