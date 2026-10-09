//! GPUI가 제공하지 않는 Windows 연동: 창 핸들, 항상 위, 단일 인스턴스.

use std::sync::mpsc::{Receiver, Sender, channel};

use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

type Handle = isize;

#[link(name = "user32")]
unsafe extern "system" {
    fn SetWindowPos(
        hwnd: Handle,
        insert_after: Handle,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateMutexW(attributes: *const core::ffi::c_void, owner: i32, name: *const u16) -> Handle;
    fn GetLastError() -> u32;
    fn CreateNamedPipeW(
        name: *const u16,
        open_mode: u32,
        pipe_mode: u32,
        max_instances: u32,
        out_buffer: u32,
        in_buffer: u32,
        timeout: u32,
        attributes: *const core::ffi::c_void,
    ) -> Handle;
    fn ConnectNamedPipe(pipe: Handle, overlapped: *mut core::ffi::c_void) -> i32;
    fn DisconnectNamedPipe(pipe: Handle) -> i32;
    fn ReadFile(
        file: Handle,
        buffer: *mut u8,
        length: u32,
        read: *mut u32,
        overlapped: *mut core::ffi::c_void,
    ) -> i32;
}

const HWND_TOPMOST: Handle = -1;
const HWND_NOTOPMOST: Handle = -2;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOACTIVATE: u32 = 0x0010;
const ERROR_ALREADY_EXISTS: u32 = 183;
const ERROR_PIPE_CONNECTED: u32 = 535;
const PIPE_ACCESS_INBOUND: u32 = 0x0000_0001;
const PIPE_UNLIMITED_INSTANCES: u32 = 255;
const INVALID_HANDLE: Handle = -1;
/// 전달받는 경로 길이 상한(바이트).
const MAX_MESSAGE: usize = 64 * 1024;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 창의 HWND 값. 얻지 못하면 0이다.
pub fn hwnd(window: &Window) -> isize {
    match HasWindowHandle::window_handle(window).map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Win32(handle)) => handle.hwnd.get(),
        _ => 0,
    }
}

/// 창을 항상 위에 두거나 해제한다. 성공 여부를 돌려준다.
pub fn set_always_on_top(window: &Window, on_top: bool) -> bool {
    let hwnd = hwnd(window);
    if hwnd == 0 {
        return false;
    }
    let after = if on_top { HWND_TOPMOST } else { HWND_NOTOPMOST };
    // SAFETY: 살아 있는 창의 HWND와 상수 플래그만 넘긴다. 위치와 크기는 플래그로 무시된다.
    unsafe {
        SetWindowPos(
            hwnd,
            after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        ) != 0
    }
}

/// Windows 대비 테마(고대비)가 켜져 있는지.
pub fn high_contrast() -> bool {
    #[repr(C)]
    struct HighContrast {
        size: u32,
        flags: u32,
        default_scheme: *mut u16,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn SystemParametersInfoW(
            action: u32,
            param: u32,
            data: *mut core::ffi::c_void,
            update: u32,
        ) -> i32;
    }
    const SPI_GETHIGHCONTRAST: u32 = 0x0042;
    const HCF_HIGHCONTRASTON: u32 = 0x0001;
    let mut info = HighContrast {
        size: size_of::<HighContrast>() as u32,
        flags: 0,
        default_scheme: core::ptr::null_mut(),
    };
    // SAFETY: 구조체 크기를 채운 유효한 버퍼를 넘기며, 함수는 그 크기 안에서만 쓴다.
    let ok = unsafe {
        SystemParametersInfoW(SPI_GETHIGHCONTRAST, info.size, (&raw mut info).cast(), 0) != 0
    };
    ok && info.flags & HCF_HIGHCONTRASTON != 0
}

/// 단일 인스턴스 판정 결과.
pub enum Instance {
    /// 첫 실행. 이후 실행이 넘기는 파일 경로(없으면 빈 문자열)를 받는다.
    Primary(Receiver<String>),
    /// 이미 실행 중인 창에 경로를 넘겼다. 이 프로세스는 종료한다.
    Forwarded,
}

/// 두 번째 실행이면 경로를 기존 인스턴스에 넘기고, 첫 실행이면 수신 스레드를 띄운다.
pub fn single_instance(identifier: &str, path: Option<&str>) -> Instance {
    let pipe_name = format!(r"\\.\pipe\{identifier}");
    let mutex_name = wide(&format!("Local\\{identifier}-instance"));
    // SAFETY: NUL로 끝나는 유효한 이름 버퍼를 넘긴다. 뮤텍스 핸들은 프로세스가
    // 끝날 때까지 일부러 닫지 않아 단일 인스턴스 표식으로 남긴다.
    let already_running = unsafe {
        CreateMutexW(core::ptr::null(), 0, mutex_name.as_ptr());
        GetLastError() == ERROR_ALREADY_EXISTS
    };
    if already_running {
        let forwarded = std::fs::OpenOptions::new()
            .write(true)
            .open(&pipe_name)
            .and_then(|mut pipe| {
                std::io::Write::write_all(&mut pipe, path.unwrap_or_default().as_bytes())
            });
        if forwarded.is_ok() {
            return Instance::Forwarded;
        }
        // 기존 인스턴스가 받지 못하면(시작 중이거나 종료 중) 독립 창으로 연다.
        log::warn!("[instance] could not reach the running instance; opening a new window");
    }
    let (sender, receiver) = channel();
    std::thread::Builder::new()
        .name("araview-instance-pipe".into())
        .spawn(move || serve_pipe(&pipe_name, sender))
        .ok();
    Instance::Primary(receiver)
}

fn serve_pipe(name: &str, sender: Sender<String>) {
    let name = wide(name);
    // SAFETY: NUL로 끝나는 이름과 상수 인자만 넘긴다.
    let pipe = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_INBOUND,
            0,
            PIPE_UNLIMITED_INSTANCES,
            0,
            4096,
            0,
            core::ptr::null(),
        )
    };
    if pipe == INVALID_HANDLE {
        log::warn!("[instance] failed to create the instance pipe");
        return;
    }
    loop {
        // SAFETY: 위에서 만든 유효한 파이프 핸들이다. 동기 호출이라 overlapped는 null이다.
        let connected = unsafe {
            ConnectNamedPipe(pipe, core::ptr::null_mut()) != 0
                || GetLastError() == ERROR_PIPE_CONNECTED
        };
        if !connected {
            return;
        }
        let mut message = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let mut read = 0u32;
            // SAFETY: 버퍼 포인터와 길이가 일치하고 `read`는 유효한 출력 위치다.
            let ok = unsafe {
                ReadFile(
                    pipe,
                    chunk.as_mut_ptr(),
                    chunk.len() as u32,
                    &mut read,
                    core::ptr::null_mut(),
                ) != 0
            };
            if !ok || read == 0 || message.len() > MAX_MESSAGE {
                break;
            }
            message.extend_from_slice(&chunk[..read as usize]);
        }
        // SAFETY: 연결을 끊어 다음 클라이언트를 받을 수 있게 한다.
        unsafe { DisconnectNamedPipe(pipe) };
        if sender
            .send(String::from_utf8_lossy(&message).into_owned())
            .is_err()
        {
            return;
        }
    }
}
