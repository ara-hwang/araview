//! GPUI가 제공하지 않는 Windows 연동: 창 핸들, 항상 위, 단일 인스턴스, 제어 파이프.

#[cfg(debug_assertions)]
use std::sync::mpsc::Receiver;
use std::sync::mpsc::{Sender, channel};

#[cfg(debug_assertions)]
use super::Control;
use super::{Instance, MAX_MESSAGE};

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

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 창의 HWND 값. 얻지 못하면 0이다.
/// HDR 출력을 켜면 GPUI가 DirectComposition 대신 hwnd 스왑체인을 쓰게 한다.
/// DWM이 composition 스왑체인의 FP16 scRGB 버퍼를 표시하지 못해서다(`docs/hdr-display.md` M2).
/// GPUI 플랫폼을 만들기 전에 불러야 한다.
pub fn prepare_hdr_output(enabled: bool) {
    gpui_windows::set_hdr_output(enabled);
}

/// HDR 출력 모드인지. 켜져 있을 때만 확장 범위 픽셀을 만든다.
pub fn hdr_output_enabled() -> bool {
    gpui_windows::hdr_output()
}

/// `RenderImage`(`image_id`)를 HDR 출력에서 RGBA f16 픽셀(sRGB 인코딩, 1.0 = SDR 흰색)로 그린다.
pub fn register_hdr_image(image_id: usize, width: u32, height: u32, rgba_f16: &[u16]) {
    gpui_windows::register_hdr_image(image_id, width, height, rgba_f16);
}

pub fn unregister_hdr_image(image_id: usize) {
    gpui_windows::unregister_hdr_image(image_id);
}

pub fn hwnd(window: &Window) -> isize {
    match HasWindowHandle::window_handle(window).map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Win32(handle)) => handle.hwnd.get(),
        _ => 0,
    }
}

/// 이미지를 움직일 여유가 없을 때 왼쪽 드래그로 창을 옮긴다.
/// `WM_NCLBUTTONDOWN` + `HTCAPTION`으로 OS 이동 루프에 진입하므로
/// 더블클릭은 호출 전에 걸러야 한다.
pub fn start_window_drag(window: &Window) {
    const WM_NCLBUTTONDOWN: u32 = 0x00A1;
    const HTCAPTION: usize = 2;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn ReleaseCapture() -> i32;
        fn SendMessageW(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize;
    }
    let hwnd = hwnd(window);
    if hwnd == 0 {
        return;
    }
    // SAFETY: 살아 있는 창의 HWND에 캡션 드래그 메시지만 보낸다.
    unsafe {
        ReleaseCapture();
        SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION, 0);
    }
}

/// 마우스 왼쪽 버튼이 실제로 눌려 있는지 물리 상태로 본다.
///
/// 다이얼로그가 열려 있으면 배경이 Windows에게 캡션(`HTCAPTION`)이라, 배경을 누르면
/// OS가 창 이동 루프를 돌리고 그 뗌(`WM_LBUTTONUP`)을 가져간다. 그래서 gpui는 그 뗌을
/// 보지 못한다. 배경 누름이 언제 끝났는지 확인할 때 이 물리 상태를 쓴다.
pub fn left_button_down() -> bool {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetAsyncKeyState(key: i32) -> i16;
    }
    const VK_LBUTTON: i32 = 0x01;
    // SAFETY: 키보드·마우스의 물리 상태만 읽는다.
    unsafe { GetAsyncKeyState(VK_LBUTTON) as u16 & 0x8000 != 0 }
}

/// 창 테두리의 화면 좌표. 얻지 못하면 None이다.
///
/// 다이얼로그 배경은 캡션 판정이라 배경을 잡아 끌면 OS 이동 루프가 창을 옮긴다.
/// 누를 때와 견주면 그 누름이 창 이동 드래그였는지 알 수 있다.
pub fn window_origin(window: &Window) -> Option<(i32, i32)> {
    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetWindowRect(hwnd: Handle, rect: *mut Rect) -> i32;
    }
    let hwnd = hwnd(window);
    if hwnd == 0 {
        return None;
    }
    let mut rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: 살아 있는 창의 HWND와 그 크기만큼의 버퍼를 넘긴다.
    let ok = unsafe { GetWindowRect(hwnd, &raw mut rect) } != 0;
    ok.then_some((rect.left, rect.top))
}

/// 항상 위 기능을 이 OS에서 쓸 수 있는지.
pub const ALWAYS_ON_TOP_SUPPORTED: bool = true;

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

/// Windows "애니메이션 효과"가 꺼져 있는지(모션 최소화).
pub fn reduced_motion() -> bool {
    // 테스트는 실행하는 PC의 설정과 무관하게 재생 경로를 확인한다.
    if cfg!(test) {
        return false;
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
    const SPI_GETCLIENTAREAANIMATION: u32 = 0x1042;
    let mut enabled: i32 = 1;
    // SAFETY: 이 조회는 BOOL 하나만 쓰며, 그 크기의 유효한 버퍼를 넘긴다.
    let ok = unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut enabled).cast(), 0) != 0
    };
    ok && enabled == 0
}

/// 제어 파이프를 열고 앱과 주고받을 채널을 돌려준다.
#[cfg(debug_assertions)]
pub fn control_channel(identifier: &str) -> Control {
    let (requests_tx, requests) = channel();
    let (replies, replies_rx) = channel();
    let name = format!(r"\\.\pipe\{identifier}-control");
    std::thread::Builder::new()
        .name("araview-control-pipe".into())
        .spawn(move || serve_control(&name, requests_tx, replies_rx))
        .ok();
    Control { requests, replies }
}

/// 한 연결에서 요청 한 줄을 받아 앱에 넘기고, 응답 한 줄을 돌려준다.
/// 한 번에 한 클라이언트만 다루며, 응답이 오지 않으면 시간 안에 포기한다.
#[cfg(debug_assertions)]
fn serve_control(name: &str, requests: Sender<String>, replies: Receiver<String>) {
    /// 파이프를 양방향으로 연다(응답을 돌려주기 위함).
    const PIPE_ACCESS_DUPLEX: u32 = 0x0000_0003;
    const REPLY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
    let name = wide(name);
    // SAFETY: NUL로 끝나는 이름과 상수 인자만 넘긴다.
    let pipe = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_DUPLEX,
            0,
            PIPE_UNLIMITED_INSTANCES,
            4096,
            4096,
            0,
            core::ptr::null(),
        )
    };
    if pipe == INVALID_HANDLE {
        log::warn!("[control] failed to create the control pipe");
        return;
    }
    loop {
        // SAFETY: 위에서 만든 유효한 파이프 핸들이며 동기 호출이라 overlapped는 null이다.
        let connected = unsafe {
            ConnectNamedPipe(pipe, core::ptr::null_mut()) != 0
                || GetLastError() == ERROR_PIPE_CONNECTED
        };
        if !connected {
            return;
        }
        let line = read_control_line(pipe);
        let reply = if line.is_empty() {
            String::new()
        } else if requests.send(line).is_err() {
            return;
        } else {
            match replies.recv_timeout(REPLY_TIMEOUT) {
                Ok(reply) => reply,
                Err(_) => r#"{"ok":false,"error":"the app did not reply"}"#.to_owned(),
            }
        };
        if !reply.is_empty() {
            write_control_line(pipe, &reply);
        }
        // SAFETY: 연결을 끊어 다음 클라이언트를 받을 수 있게 한다.
        unsafe { DisconnectNamedPipe(pipe) };
    }
}

/// 요청 한 줄을 읽는다. 개행까지 읽거나 상한에 닿으면 멈춘다.
#[cfg(debug_assertions)]
fn read_control_line(pipe: Handle) -> String {
    let mut line = Vec::new();
    let mut chunk = [0u8; 1024];
    while line.len() < MAX_MESSAGE {
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
        if !ok || read == 0 {
            break;
        }
        let bytes = &chunk[..read as usize];
        match bytes.iter().position(|byte| *byte == b'\n') {
            Some(end) => {
                line.extend_from_slice(&bytes[..end]);
                break;
            }
            None => line.extend_from_slice(bytes),
        }
    }
    String::from_utf8_lossy(&line).into_owned()
}

/// 응답 한 줄을 돌려준다. 클라이언트가 개행까지 받도록 밀어낸다.
#[cfg(debug_assertions)]
fn write_control_line(pipe: Handle, line: &str) {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn WriteFile(
            file: Handle,
            buffer: *const u8,
            length: u32,
            written: *mut u32,
            overlapped: *mut core::ffi::c_void,
        ) -> i32;
        fn FlushFileBuffers(file: Handle) -> i32;
    }
    let mut payload = line.as_bytes().to_vec();
    payload.push(b'\n');
    let mut written = 0u32;
    // SAFETY: 유효한 파이프 핸들과 버퍼의 실제 길이를 넘긴다.
    unsafe {
        WriteFile(
            pipe,
            payload.as_ptr(),
            payload.len() as u32,
            &mut written,
            core::ptr::null_mut(),
        );
        FlushFileBuffers(pipe);
    }
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
