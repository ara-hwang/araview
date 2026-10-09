use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{LazyLock, Mutex};

use serde::Serialize;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::{RegKey, HKEY};

use crate::app_error::AppError;
use crate::image::SUPPORTED_EXTENSIONS;

const BUNDLE_ID: &str = "com.araview.viewer";
/// 개발(디버그) 빌드는 설치 버전과 확장자 연결이 서로 덮어쓰지 않도록
/// 별도 이름/레지스트리 키/ProgID를 사용한다.
const IS_DEV_BUILD: bool = cfg!(debug_assertions);
const APP_NAME: &str = if IS_DEV_BUILD {
    "AraView (Dev)"
} else {
    "AraView"
};
const APP_DESCRIPTION: &str = APP_NAME;
const CAPABILITIES_PATH: &str = if IS_DEV_BUILD {
    r"Software\AraView (Dev)\Capabilities"
} else {
    r"Software\AraView\Capabilities"
};
const PROG_ID_SUFFIX: &str = if IS_DEV_BUILD { ".dev" } else { "" };

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

/// OS picker를 열어 사용자가 직접 기본 앱을 고르게 한다.
///
/// `associate`는 FE 호환용으로 유지되지만 무시된다. Windows UserChoice는
/// 앱이 직접 쓸 수 없어 연결/해제 모두 OS 확인이 필요하기 때문이다.
/// `parent_hwnd`는 소유자 창의 HWND 값(없으면 0)이다. raw 포인터를 safe
/// 시그니처로 노출하지 않기 위해 정수로 받고 FFI 직전에만 변환한다.
pub fn set_association(
    extension: &str,
    _associate: bool,
    parent_hwnd: isize,
) -> Result<FileAssociation, AppError> {
    let ext = parse_extension(extension)?;
    let exe = current_exe()?;
    ensure_application_registration(&exe)?;
    if open_extension_picker(&ext, parent_hwnd).is_err() {
        open_settings_uri(&file_extension_settings_uri(&ext))?;
    }
    status_after_picker(&ext, &exe)
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
    format!("{BUNDLE_ID}{PROG_ID_SUFFIX}.{ext}")
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
    // 현재 빌드 채널(개발/릴리스)의 ProgID만 우리 것으로 본다.
    // 다른 채널의 ProgID는 연결 명령의 실행 파일 경로 비교로만 판정한다.
    if SUPPORTED_EXTENSIONS
        .iter()
        .any(|ext| prog_id.eq_ignore_ascii_case(&prog_id_for(ext)))
    {
        return true;
    }
    // 설치 관리자가 등록한 "AraView.png" 형태의 ProgID 호환(릴리스 빌드 한정).
    if IS_DEV_BUILD {
        return false;
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

/// 프로세스당 1회만 등록한다. 읽기 경로(`list_associations`)에서 매번
/// 21개 ProgID를 다시 쓰는 churn을 없앤다. 레지스트리가 실행 중 외부에서
/// 지워지면 다음 프로세스 시작 시 다시 등록된다.
static REGISTERED_EXE: LazyLock<Mutex<Option<PathBuf>>> = LazyLock::new(|| Mutex::new(None));

fn ensure_application_registration(exe: &Path) -> Result<(), AppError> {
    if REGISTERED_EXE
        .lock()
        .map(|guard| guard.as_deref() == Some(exe))
        .unwrap_or(false)
    {
        return Ok(());
    }
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
    // ProgID/OpenWith/RegisteredApplications를 새로 썼으니 Explorer가 아이콘과
    // 연결 프로그램 목록을 즉시 다시 읽게 한다. 없으면 셸 재시작 전까지
    // 이전 상태가 보일 수 있다 (thumb_shell과 같은 알림).
    crate::thumb_shell::notify_shell();
    if let Ok(mut guard) = REGISTERED_EXE.lock() {
        *guard = Some(exe.to_path_buf());
    }
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
    // short(8.3)/long 경로, 슬래시 방향, 대소문자 차이를 흡수한다.
    let normalized_command = normalize_exe_for_compare(command_exe);
    let normalized_exe = normalize_exe_for_compare(&exe.to_string_lossy());
    if normalized_command == normalized_exe {
        return true;
    }
    // 둘 다 존재하면 canonicalize로 최종 판정한다.
    let command_path = Path::new(command_exe.trim().trim_matches('"'));
    match (
        std::fs::canonicalize(command_path),
        std::fs::canonicalize(exe),
    ) {
        (Ok(a), Ok(b)) => a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase(),
        _ => false,
    }
}

fn normalize_exe_for_compare(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace('/', "\\")
        .to_lowercase()
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
    // 피커는 모달이 아니라 사용자가 몇 초~몇 분 뒤에 고를 수 있어, command
    // 스레드를 sleep 폴링으로 묶지 않고 현재 상태를 즉시 반환한다.
    // FE가 창 포커스 복귀 시 목록을 새로고침하므로 선택 결과는 거기서 반영된다.
    status_for(ext, exe)
}

fn open_extension_picker(ext: &str, parent_hwnd: isize) -> Result<(), AppError> {
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
        // 성공한 CoInitializeEx는 S_OK/S_FALSE 모두 CoUninitialize로 짝을
        // 맞춰야 한다. RPC_E_CHANGED_MODE만 초기화가 일어나지 않은 경우다.
        let should_uninit = init_hr >= 0 && init_hr != RPC_E_CHANGED_MODE;

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

        // vtbl 레이아웃 가정: IUnknown 3개(QueryInterface/AddRef/Release) 뒤에
        // `Launch(HWND hwndParent, LPCWSTR pszPath, OPENASINFO_FLAGS)` 하나뿐인
        // 인터페이스다. CoCreateInstance가 IID_IOpenWithLauncher로 성공했으므로
        // 이 레이아웃이 성립한다. 플래그 0x2004는 OAIF_EXEC(0x4)와
        // 문서화되지 않은 0x2000의 조합으로, 셸이 "연결 프로그램" 대화상자를
        // 띄울 때 쓰는 값이다. 실패하면 호출자가 ms-settings URI로 폴백한다.
        let launcher = punk as *mut IOpenWithLauncher;
        let launch_hr = ((*(*launcher).vtbl).launch)(
            launcher,
            parent_hwnd as *mut c_void,
            wide.as_ptr(),
            0x2004,
        );
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
    // SAFETY: `wide`는 NUL로 끝나는 UTF-16 버퍼이고 `guid`는 초기화된 로컬
    // out-param이다. 둘 다 호출 동안 살아 있다.
    let hr = unsafe { CLSIDFromString(wide.as_ptr(), &mut guid) };
    if hr < 0 {
        return Err(AppError::unknown(format!(
            "Invalid OpenWithLauncher CLSID: {value}"
        )));
    }
    Ok(guid)
}

/// `windows` 크레이트를 앱 쪽에 끌어들이지 않기 위한 최소 선언
/// (`thumb_shell.rs` 상단 주석 참조). 레이아웃은 Win32 `GUID`와 동일하다.
#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

/// 문서화되지 않은 셸 내부 인터페이스. `windows` 크레이트에도 바인딩이 없어
/// 손으로 선언한다. 메서드는 IUnknown 3개 뒤에 `Launch` 하나뿐이다.
/// <https://learn.microsoft.com/windows/win32/api/shobjidl_core/> 에는 없고
/// CLSID는 `HKLM\...\CurrentVersion\OpenWith` 의 `OpenWithLauncher` 값에서 읽는다.
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

// ole32/shell32 바인딩을 직접 선언한다. 앱은 의도적으로 `windows` 크레이트를
// 쓰지 않으므로(`thumb_shell.rs` 상단 주석), 시그니처를 바꿀 때는 MSDN의
// 원본 선언과 대조할 것. 인자 하나라도 어긋나면 호출 규약이 깨진다.
//   CoInitializeEx(LPVOID, DWORD) -> HRESULT
//   CoUninitialize(void)
//   CoCreateInstance(REFCLSID, LPUNKNOWN, DWORD, REFIID, LPVOID*) -> HRESULT
//   CLSIDFromString(LPCOLESTR, LPCLSID) -> HRESULT
//   CoAllowSetForegroundWindow(IUnknown*, LPVOID) -> HRESULT
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

//   ShellExecuteW(HWND, LPCWSTR, LPCWSTR, LPCWSTR, LPCWSTR, INT) -> HINSTANCE
// 반환값은 실제로는 정수 코드이며 32 이하가 실패다(MSDN).
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
