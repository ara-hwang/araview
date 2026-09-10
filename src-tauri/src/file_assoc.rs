use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;
use std::time::Duration;

use serde::Serialize;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::{RegKey, HKEY};

use crate::app_error::AppError;
use crate::image::SUPPORTED_EXTENSIONS;

const BUNDLE_ID: &str = "com.tauri-image-viewer.app";
const APP_NAME: &str = "Image Viewer";
const APP_DESCRIPTION: &str = "Image Viewer";
const CAPABILITIES_PATH: &str = r"Software\Image Viewer\Capabilities";

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct FileAssociation {
    pub extension: String,
    pub associated: bool,
    pub current_prog_id: Option<String>,
    pub needs_os_confirmation: bool,
}

pub fn list_associations() -> Result<Vec<FileAssociation>, AppError> {
    let exe = current_exe()?;
    ensure_application_registration(&exe)?;
    SUPPORTED_EXTENSIONS
        .iter()
        .map(|ext| status_for(ext, &exe))
        .collect()
}

pub fn set_association(
    extension: &str,
    _associate: bool,
    parent_hwnd: *mut c_void,
) -> Result<FileAssociation, AppError> {
    let ext = parse_extension(extension)?;
    let exe = current_exe()?;
    ensure_application_registration(&exe)?;
    if open_extension_picker(&ext, parent_hwnd).is_err() {
        open_settings_uri(&file_extension_settings_uri(&ext))?;
    }
    status_after_picker(&ext, &exe)
}

pub fn set_all_associations(_associate: bool) -> Result<Vec<FileAssociation>, AppError> {
    let exe = current_exe()?;
    ensure_application_registration(&exe)?;
    open_settings_uri(&default_apps_settings_uri())?;
    SUPPORTED_EXTENSIONS
        .iter()
        .map(|ext| status_for(ext, &exe))
        .collect()
}

pub fn open_default_apps_settings() -> Result<(), AppError> {
    let exe = current_exe()?;
    ensure_application_registration(&exe)?;
    open_settings_uri(&default_apps_settings_uri())
}

pub fn default_apps_settings_uri() -> String {
    format!(
        "ms-settings:defaultapps?registeredAppUser={}",
        APP_NAME.replace(' ', "%20")
    )
}

pub fn file_extension_settings_uri(ext: &str) -> String {
    format!("ms-settings:defaultapps?fileExtension=.{ext}")
}

pub fn prog_id_for(ext: &str) -> String {
    format!("{BUNDLE_ID}.{ext}")
}

pub fn parse_extension(raw: &str) -> Result<String, AppError> {
    let ext = raw.trim().trim_start_matches('.').to_lowercase();
    if ext.is_empty() {
        return Err(AppError::invalid_input("Extension is empty"));
    }
    if !SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::unsupported(format!(
            "Unsupported extension: {raw}"
        )));
    }
    Ok(ext)
}

pub fn is_our_prog_id(prog_id: &str) -> bool {
    let bundle_prefix = format!("{BUNDLE_ID}.");
    if prog_id.eq_ignore_ascii_case(BUNDLE_ID)
        || starts_with_ignore_ascii_case(prog_id, &bundle_prefix)
    {
        return true;
    }
    let installer_prefix = format!("{APP_NAME}.");
    prog_id.eq_ignore_ascii_case(APP_NAME)
        || starts_with_ignore_ascii_case(prog_id, &installer_prefix)
}

pub fn extract_exe_from_command(command: &str) -> Option<String> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }
    if let Some(rest) = command.strip_prefix('"') {
        return rest.split('"').next().map(ToOwned::to_owned);
    }
    command.split_whitespace().next().map(ToOwned::to_owned)
}

fn current_exe() -> Result<PathBuf, AppError> {
    std::env::current_exe()
        .map_err(|e| AppError::unknown(format!("Failed to get executable path: {e}")))
}

fn status_for(ext: &str, exe: &Path) -> Result<FileAssociation, AppError> {
    let user_choice = read_user_choice_latest(ext).or_else(|| read_user_choice(ext));
    let classes_default = read_classes_default(HKEY_CURRENT_USER, ext)
        .or_else(|| read_classes_default(HKEY_LOCAL_MACHINE, ext));
    let current_prog_id = user_choice.clone().or(classes_default);
    let associated = current_prog_id
        .as_deref()
        .map(|prog_id| is_handled_by_us(prog_id, exe))
        .unwrap_or(false);
    let needs_os_confirmation = user_choice.is_some() && !associated;

    Ok(FileAssociation {
        extension: ext.to_string(),
        associated,
        current_prog_id,
        needs_os_confirmation,
    })
}

fn is_handled_by_us(prog_id: &str, exe: &Path) -> bool {
    if is_our_prog_id(prog_id) {
        return true;
    }
    read_prog_id_command(HKEY_CURRENT_USER, prog_id)
        .or_else(|| read_prog_id_command(HKEY_LOCAL_MACHINE, prog_id))
        .and_then(|command| extract_exe_from_command(&command))
        .is_some_and(|command_exe| paths_equal(&command_exe, exe))
}

fn ensure_application_registration(exe: &Path) -> Result<(), AppError> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (capabilities, _) = hkcu.create_subkey(CAPABILITIES_PATH).map_err(reg_err)?;
    capabilities
        .set_value("ApplicationName", &APP_NAME)
        .map_err(reg_err)?;
    capabilities
        .set_value("ApplicationDescription", &APP_DESCRIPTION)
        .map_err(reg_err)?;
    capabilities
        .set_value("ApplicationIcon", &default_icon(exe))
        .map_err(reg_err)?;

    let (file_associations, _) = capabilities
        .create_subkey("FileAssociations")
        .map_err(reg_err)?;
    for ext in SUPPORTED_EXTENSIONS {
        file_associations
            .set_value(format!(".{ext}"), &prog_id_for(ext))
            .map_err(reg_err)?;
        write_prog_id(&prog_id_for(ext), ext, exe)?;
        register_open_with(ext)?;
    }

    let (registered, _) = hkcu
        .create_subkey(r"Software\RegisteredApplications")
        .map_err(reg_err)?;
    registered
        .set_value(APP_NAME, &CAPABILITIES_PATH)
        .map_err(reg_err)?;
    Ok(())
}

fn register_open_with(ext: &str) -> Result<(), AppError> {
    let prog_id = prog_id_for(ext);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (ext_key, _) = hkcu.create_subkey(classes_ext_path(ext)).map_err(reg_err)?;
    let (open_with, _) = ext_key.create_subkey("OpenWithProgids").map_err(reg_err)?;
    open_with.set_value(&prog_id, &"").map_err(reg_err)?;

    let (file_exts, _) = hkcu
        .create_subkey(file_exts_open_with_path(ext))
        .map_err(reg_err)?;
    file_exts.set_value(&prog_id, &"").map_err(reg_err)?;
    Ok(())
}

fn write_prog_id(prog_id: &str, ext: &str, exe: &Path) -> Result<(), AppError> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (prog_key, _) = hkcu
        .create_subkey(format!(r"Software\Classes\{prog_id}"))
        .map_err(reg_err)?;
    prog_key
        .set_value("", &format!("{APP_NAME} {ext} file"))
        .map_err(reg_err)?;

    let (icon_key, _) = prog_key.create_subkey("DefaultIcon").map_err(reg_err)?;
    icon_key
        .set_value("", &default_icon(exe))
        .map_err(reg_err)?;

    let (shell_key, _) = prog_key.create_subkey("shell").map_err(reg_err)?;
    shell_key.set_value("", &"open").map_err(reg_err)?;

    let (open_key, _) = shell_key.create_subkey("open").map_err(reg_err)?;
    open_key
        .set_value("", &format!("Open with {APP_NAME}"))
        .map_err(reg_err)?;

    let (command_key, _) = open_key.create_subkey("command").map_err(reg_err)?;
    command_key
        .set_value("", &open_command(exe))
        .map_err(reg_err)?;
    Ok(())
}

fn read_user_choice(ext: &str) -> Option<String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey(format!(
            r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.{ext}\UserChoice"
        ))
        .ok()?;
    nonempty_reg_string(key.get_value("ProgId").ok())
}

fn read_user_choice_latest(ext: &str) -> Option<String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey(user_choice_latest_prog_id_path(ext))
        .ok()?;
    nonempty_reg_string(key.get_value("ProgId").ok())
        .or_else(|| nonempty_reg_string(key.get_value("").ok()))
}

fn user_choice_latest_prog_id_path(ext: &str) -> String {
    format!(
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.{ext}\UserChoiceLatest\ProgId"
    )
}

fn read_classes_default(hive: HKEY, ext: &str) -> Option<String> {
    let root = RegKey::predef(hive);
    let key = root.open_subkey(classes_ext_path(ext)).ok()?;
    nonempty_reg_string(key.get_value("").ok())
}

fn read_prog_id_command(hive: HKEY, prog_id: &str) -> Option<String> {
    let root = RegKey::predef(hive);
    let key = root
        .open_subkey(format!(r"Software\Classes\{prog_id}\shell\open\command"))
        .ok()?;
    nonempty_reg_string(key.get_value("").ok())
}

fn nonempty_reg_string(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}

fn classes_ext_path(ext: &str) -> String {
    format!(r"Software\Classes\.{ext}")
}

fn file_exts_open_with_path(ext: &str) -> String {
    format!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.{ext}\OpenWithProgids")
}

fn default_icon(exe: &Path) -> String {
    format!("\"{}\",0", exe.display())
}

fn open_command(exe: &Path) -> String {
    format!("\"{}\" \"%1\"", exe.display())
}

fn paths_equal(command_exe: &str, exe: &Path) -> bool {
    Path::new(command_exe)
        .to_string_lossy()
        .eq_ignore_ascii_case(&exe.to_string_lossy())
}

fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value.len() >= prefix.len()
        && value.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

fn reg_err(error: impl std::fmt::Display) -> AppError {
    AppError::unknown(format!("Registry error: {error}"))
}

fn open_settings_uri(uri: &str) -> Result<(), AppError> {
    let file: Vec<u16> = std::ffi::OsStr::new(uri)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    const SW_SHOWNORMAL: i32 = 1;
    // SAFETY: ShellExecuteW is called with a NUL-terminated UTF-16 URI and null
    // optional arguments, which matches the documented "open a URI" usage.
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            ptr::null(),
            file.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result <= 32 {
        return Err(AppError::unknown(format!(
            "Failed to open Windows default apps settings (code {result})"
        )));
    }
    Ok(())
}

fn status_after_picker(ext: &str, exe: &Path) -> Result<FileAssociation, AppError> {
    // 사용자가 피커에서 선택하면 레지스트리가 바뀌므로 변경 시 즉시 반환.
    // 피커를 그냥 닫으면 변경이 없어 최대 대기 후 현재 상태를 돌려준다.
    // FE도 창 포커스 복귀 시 목록을 새로고침하므로 이 폴링은 보조 수단이다.
    let first = status_for(ext, exe)?;
    for _ in 0..12 {
        std::thread::sleep(Duration::from_millis(250));
        let next = status_for(ext, exe)?;
        if next != first {
            return Ok(next);
        }
    }
    status_for(ext, exe)
}

fn open_extension_picker(ext: &str, parent_hwnd: *mut c_void) -> Result<(), AppError> {
    let dotted = format!(".{ext}");
    let wide: Vec<u16> = std::ffi::OsStr::new(&dotted)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let clsid = read_open_with_launcher_clsid()?;

    // SAFETY: COM is initialized on this thread, CLSID/IID are well-formed, and
    // the IOpenWithLauncher pointer is released before CoUninitialize.
    unsafe {
        let init_hr = CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED);
        if init_hr < 0 && init_hr != RPC_E_CHANGED_MODE {
            return Err(AppError::unknown(format!(
                "COM initialize failed ({init_hr:#x})"
            )));
        }
        let should_uninit = init_hr == 0;

        let mut punk: *mut c_void = ptr::null_mut();
        let create_hr = CoCreateInstance(
            &clsid,
            ptr::null_mut(),
            CLSCTX_LOCAL_SERVER,
            &IID_IOPEN_WITH_LAUNCHER,
            &mut punk,
        );
        if create_hr < 0 || punk.is_null() {
            if should_uninit {
                CoUninitialize();
            }
            return Err(AppError::unknown(format!(
                "OpenWithLauncher create failed ({create_hr:#x})"
            )));
        }

        CoAllowSetForegroundWindow(punk, ptr::null_mut());

        let launcher = punk as *mut IOpenWithLauncher;
        let launch_hr = ((*(*launcher).vtbl).launch)(launcher, parent_hwnd, wide.as_ptr(), 0x2004);
        ((*(*launcher).vtbl).release)(launcher);

        if should_uninit {
            CoUninitialize();
        }

        if launch_hr >= 0 || launch_hr == HRESULT_CANCELLED {
            Ok(())
        } else {
            Err(AppError::unknown(format!(
                "OpenWithLauncher launch failed ({launch_hr:#x})"
            )))
        }
    }
}

fn read_open_with_launcher_clsid() -> Result<Guid, AppError> {
    guid_from_string(&read_open_with_launcher_clsid_string()?)
}

fn read_open_with_launcher_clsid_string() -> Result<String, AppError> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm
        .open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\OpenWith")
        .map_err(reg_err)?;
    key.get_value("OpenWithLauncher").map_err(reg_err)
}

fn guid_from_string(value: &str) -> Result<Guid, AppError> {
    let wide: Vec<u16> = std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut guid = Guid {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    let hr = unsafe { CLSIDFromString(wide.as_ptr(), &mut guid) };
    if hr < 0 {
        return Err(AppError::unknown(format!(
            "Invalid OpenWithLauncher CLSID: {value}"
        )));
    }
    Ok(guid)
}

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[repr(C)]
struct IOpenWithLauncher {
    vtbl: *const IOpenWithLauncherVtbl,
}

#[repr(C)]
struct IOpenWithLauncherVtbl {
    query_interface:
        unsafe extern "system" fn(*mut IOpenWithLauncher, *const Guid, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut IOpenWithLauncher) -> u32,
    release: unsafe extern "system" fn(*mut IOpenWithLauncher) -> u32,
    launch: unsafe extern "system" fn(*mut IOpenWithLauncher, *mut c_void, *const u16, i32) -> i32,
}

const COINIT_APARTMENTTHREADED: u32 = 0x2;
const CLSCTX_LOCAL_SERVER: u32 = 0x4;
const RPC_E_CHANGED_MODE: i32 = 0x8001_0106u32 as i32;
const HRESULT_CANCELLED: i32 = 0x8007_04C7u32 as i32;
const IID_IOPEN_WITH_LAUNCHER: Guid = Guid {
    data1: 0x6A28_3FE2,
    data2: 0xECFA,
    data3: 0x4599,
    data4: [0x91, 0xC4, 0xE8, 0x09, 0x57, 0x13, 0x7B, 0x26],
};

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(pvreserved: *mut c_void, dwcoinit: u32) -> i32;
    fn CoUninitialize();
    fn CoCreateInstance(
        rclsid: *const Guid,
        punkouter: *mut c_void,
        dwclscontext: u32,
        riid: *const Guid,
        ppv: *mut *mut c_void,
    ) -> i32;
    fn CLSIDFromString(lpsz: *const u16, pclsid: *mut Guid) -> i32;
    fn CoAllowSetForegroundWindow(punk: *mut c_void, reserved: *mut c_void) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: *mut c_void,
        lp_operation: *const u16,
        lp_file: *const u16,
        lp_parameters: *const u16,
        lp_directory: *const u16,
        n_show_cmd: i32,
    ) -> isize;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prog_id_uses_bundle_id_and_extension() {
        assert_eq!(prog_id_for("png"), "com.tauri-image-viewer.app.png");
        assert_eq!(prog_id_for("cbz"), "com.tauri-image-viewer.app.cbz");
    }

    #[test]
    fn parse_extension_normalizes_and_rejects_unknown() {
        assert_eq!(parse_extension(".PNG").unwrap(), "png");
        assert_eq!(parse_extension("Jpeg").unwrap(), "jpeg");
        assert!(parse_extension("").is_err());
        assert!(parse_extension("pdf").is_err());
    }

    #[test]
    fn is_our_prog_id_matches_bundle_prefix() {
        assert!(is_our_prog_id("com.tauri-image-viewer.app.png"));
        assert!(is_our_prog_id("COM.TAURI-IMAGE-VIEWER.APP.JPG"));
        assert!(is_our_prog_id("Image Viewer.png"));
        assert!(is_our_prog_id("image viewer.jpg"));
        assert!(!is_our_prog_id("Image"));
        assert!(!is_our_prog_id("jpegfile"));
        assert!(!is_our_prog_id("AppX123"));
    }

    #[test]
    fn extract_exe_from_command_handles_quoted_and_plain_paths() {
        assert_eq!(
            extract_exe_from_command(r#""C:\Program Files\Image Viewer\app.exe" "%1""#).as_deref(),
            Some(r"C:\Program Files\Image Viewer\app.exe")
        );
        assert_eq!(
            extract_exe_from_command(r"C:\Apps\viewer.exe %1").as_deref(),
            Some(r"C:\Apps\viewer.exe")
        );
        assert_eq!(extract_exe_from_command("   "), None);
    }

    #[test]
    fn open_command_quotes_executable_path() {
        let exe = Path::new(r"C:\Program Files\Image Viewer\app.exe");
        assert_eq!(
            open_command(exe),
            r#""C:\Program Files\Image Viewer\app.exe" "%1""#
        );
        assert_eq!(
            default_icon(exe),
            r#""C:\Program Files\Image Viewer\app.exe",0"#
        );
    }

    #[test]
    fn default_apps_settings_uri_uses_registered_app_user() {
        assert_eq!(
            default_apps_settings_uri(),
            "ms-settings:defaultapps?registeredAppUser=Image%20Viewer"
        );
    }

    #[test]
    fn file_extension_settings_uri_targets_the_extension() {
        assert_eq!(
            file_extension_settings_uri("png"),
            "ms-settings:defaultapps?fileExtension=.png"
        );
        assert_eq!(
            file_extension_settings_uri("cbz"),
            "ms-settings:defaultapps?fileExtension=.cbz"
        );
    }

    #[test]
    fn guid_from_string_parses_braced_clsid() {
        let guid = guid_from_string("{6A283FE2-ECFA-4599-91C4-E80957137B26}").unwrap();
        assert_eq!(guid.data1, 0x6A28_3FE2);
        assert_eq!(guid.data2, 0xECFA);
        assert_eq!(guid.data3, 0x4599);
        assert_eq!(guid.data4, [0x91, 0xC4, 0xE8, 0x09, 0x57, 0x13, 0x7B, 0x26]);
    }

    #[test]
    fn user_choice_latest_path_targets_prog_id_subkey() {
        assert_eq!(
            user_choice_latest_prog_id_path("png"),
            r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.png\UserChoiceLatest\ProgId"
        );
    }

    #[test]
    fn open_with_launcher_clsid_is_registered() {
        let value = read_open_with_launcher_clsid_string().unwrap();
        assert!(value.starts_with('{'));
        assert!(guid_from_string(&value).is_ok());
    }
}
