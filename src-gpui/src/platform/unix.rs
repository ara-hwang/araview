//! Linux와 macOS 연동: 유닉스 도메인 소켓 단일 인스턴스와 개발용 제어 소켓.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};

use gpui_kit::Window;

use super::{Instance, MAX_MESSAGE};

/// Windows 전용 개념이라 창 핸들은 없다.
/// HDR 출력은 Windows에서만 지원한다.
pub fn prepare_hdr_output(_enabled: bool) {}

pub fn hdr_output_enabled() -> bool {
    false
}

pub fn register_hdr_image(_image_id: usize, _width: u32, _height: u32, _rgba_f16: &[u16]) {}

pub fn unregister_hdr_image(_image_id: usize) {}

pub fn hwnd(_window: &Window) -> isize {
    0
}

/// 이미지를 움직일 여유가 없을 때 왼쪽 드래그로 창을 옮긴다.
/// 더블클릭은 호출 전에 걸러야 한다.
pub fn start_window_drag(window: &Window) {
    window.start_window_move();
}

/// 물리 버튼 상태는 Windows 캡션 이동 루프를 우회하는 용도라 다른 OS에서는 필요 없다.
pub fn left_button_down() -> bool {
    false
}

/// 창 좌표 조회는 Windows 캡션 드래그 판정 전용이다. 다른 OS에서는 항상 None이다.
pub fn window_origin(_window: &Window) -> Option<(i32, i32)> {
    None
}

/// 창을 항상 위에 두는 API가 GPUI에 없어 지원하지 않는다.
pub fn set_always_on_top(_window: &Window, _on_top: bool) -> bool {
    false
}

/// 항상 위 기능을 이 OS에서 쓸 수 있는지.
pub const ALWAYS_ON_TOP_SUPPORTED: bool = false;

/// 고대비 테마 조회는 Windows 전용이다.
pub fn high_contrast() -> bool {
    false
}

/// 시스템의 "동작 줄이기" 설정이 켜져 있는지(모션 최소화).
///
/// 조회에 외부 명령을 쓰므로 프로세스당 한 번만 읽는다. 읽지 못하면 꺼진 것으로 본다.
pub fn reduced_motion() -> bool {
    // 테스트는 실행하는 PC의 설정과 무관하게 재생 경로를 확인한다.
    if cfg!(test) {
        return false;
    }
    static REDUCED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *REDUCED.get_or_init(query_reduced_motion)
}

#[cfg(target_os = "macos")]
fn query_reduced_motion() -> bool {
    std::process::Command::new("defaults")
        .args(["read", "com.apple.universalaccess", "reduceMotion"])
        .output()
        .ok()
        .is_some_and(|out| String::from_utf8_lossy(&out.stdout).trim() == "1")
}

#[cfg(not(target_os = "macos"))]
fn query_reduced_motion() -> bool {
    std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "enable-animations"])
        .output()
        .ok()
        .is_some_and(|out| String::from_utf8_lossy(&out.stdout).trim() == "false")
}

/// 소켓 파일을 둘 폴더. Linux는 사용자 전용 런타임 폴더, 없으면 임시 폴더다.
fn socket_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|value| !value.is_empty())
        .map_or_else(std::env::temp_dir, PathBuf::from)
}

/// 소켓 경로 길이 상한(바이트). `sockaddr_un`의 경로 칸은 macOS 104, Linux 108바이트다.
const MAX_SOCKET_PATH: usize = 100;

fn socket_path(name: &str) -> PathBuf {
    let path = socket_dir().join(format!("{name}.sock"));
    if path.as_os_str().len() <= MAX_SOCKET_PATH {
        return path;
    }
    // 런타임 폴더가 길면 짧은 고정 위치로 옮긴다. 사용자별로 나누고, 원래 경로를 해시해
    // 이름이 겹치지 않게 한다(FNV-1a. 실행마다 같은 값이어야 두 번째 실행이 찾는다).
    let hash = path
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    // SAFETY: 인자가 없고 항상 성공하는 조회다.
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/araview-{uid}-{hash:016x}.sock"))
}

/// 남아 있는 소켓 파일을 지우고 새로 연다. 소유자만 접근하도록 권한을 좁힌다.
fn bind_socket(path: &PathBuf) -> std::io::Result<UnixListener> {
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

/// 상한까지 읽는다. 상한을 넘는 입력은 잘라낸다.
fn read_limited(stream: &mut UnixStream) -> Vec<u8> {
    let mut message = Vec::new();
    stream
        .take(MAX_MESSAGE as u64)
        .read_to_end(&mut message)
        .ok();
    message
}

/// 첫 인스턴스의 수신 채널로 가는 송신단. 운영체제가 실행 중인 앱에 파일 열기를 요청할 때
/// (macOS의 Finder 열기 이벤트) 두 번째 실행이 보내는 경로와 같은 길로 앱에 전한다.
static OPEN_REQUESTS: OnceLock<Sender<String>> = OnceLock::new();

/// 운영체제가 넘긴 URL 중 로컬 파일 경로를 기존 창으로 열게 한다.
pub fn open_urls(urls: &[String]) {
    let Some(sender) = OPEN_REQUESTS.get() else {
        return;
    };
    // 한 번에 여러 파일이 와도 앱은 첫 항목만 열고 그 폴더의 나머지를 목록으로 보여준다.
    if let Some(path) = urls.iter().find_map(|url| file_url_to_path(url)) {
        let _ = sender.send(path);
    }
}

/// `file:///Users/a/b%20c.png` 같은 URL을 경로로 바꾼다. 파일 URL이 아니면 None이다.
fn file_url_to_path(url: &str) -> Option<String> {
    let rest = url.strip_prefix("file://")?;
    // 호스트 부분은 비어 있거나 `localhost`다. 경로는 첫 `/`부터다.
    let path = &rest[rest.find('/')?..];
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = (bytes[index] == b'%')
            .then(|| bytes.get(index + 1..index + 3))
            .flatten()
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match hex {
            Some(byte) => {
                decoded.push(byte);
                index += 3;
            }
            None => {
                decoded.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).ok()
}

/// 두 번째 실행이면 경로를 기존 인스턴스에 넘기고, 첫 실행이면 수신 스레드를 띄운다.
pub fn single_instance(identifier: &str, path: Option<&str>) -> Instance {
    let socket = socket_path(identifier);
    if let Ok(mut stream) = UnixStream::connect(&socket) {
        if stream
            .write_all(path.unwrap_or_default().as_bytes())
            .is_ok()
        {
            return Instance::Forwarded;
        }
        // 기존 인스턴스가 받지 못하면(시작 중이거나 종료 중) 독립 창으로 연다.
        log::warn!("[instance] could not reach the running instance; opening a new window");
    }
    let (sender, receiver) = channel();
    let _ = OPEN_REQUESTS.set(sender.clone());
    match bind_socket(&socket) {
        Ok(listener) => {
            std::thread::Builder::new()
                .name("araview-instance-socket".into())
                .spawn(move || {
                    for mut stream in listener.incoming().flatten() {
                        let message = read_limited(&mut stream);
                        if sender
                            .send(String::from_utf8_lossy(&message).into_owned())
                            .is_err()
                        {
                            return;
                        }
                    }
                })
                .ok();
        }
        Err(error) => log::warn!("[instance] failed to create the instance socket: {error}"),
    }
    Instance::Primary(receiver)
}

/// 개발 빌드의 제어 통로를 열고 앱과 주고받을 채널을 돌려준다.
#[cfg(debug_assertions)]
pub fn control_channel(identifier: &str) -> super::Control {
    use std::io::{BufRead, BufReader};
    use std::time::Duration;

    const REPLY_TIMEOUT: Duration = Duration::from_secs(10);
    let (requests_tx, requests) = channel();
    let (replies, replies_rx) = channel::<String>();
    let socket = socket_path(&format!("{identifier}-control"));
    std::thread::Builder::new()
        .name("araview-control-socket".into())
        .spawn(move || {
            let listener = match bind_socket(&socket) {
                Ok(listener) => listener,
                Err(error) => {
                    log::warn!("[control] failed to create the control socket: {error}");
                    return;
                }
            };
            // 한 번에 한 클라이언트만 다루며, 응답이 오지 않으면 시간 안에 포기한다.
            for mut stream in listener.incoming().flatten() {
                let mut line = String::new();
                let read = BufReader::new((&stream).take(MAX_MESSAGE as u64)).read_line(&mut line);
                let line = line.trim_end().to_owned();
                if read.is_err() || line.is_empty() {
                    continue;
                }
                if requests_tx.send(line).is_err() {
                    return;
                }
                let reply = replies_rx.recv_timeout(REPLY_TIMEOUT).unwrap_or_else(|_| {
                    r#"{"ok":false,"error":"the app did not reply"}"#.to_owned()
                });
                let _ = stream.write_all(format!("{reply}\n").as_bytes());
            }
        })
        .ok();
    super::Control { requests, replies }
}

#[cfg(test)]
mod tests {
    use super::file_url_to_path;

    #[test]
    fn long_runtime_dirs_fall_back_to_a_short_socket_path() {
        let name = "com.araview.viewer.dev";
        let short = super::socket_path(name);
        assert!(short.as_os_str().len() <= super::MAX_SOCKET_PATH);
        assert_eq!(short, super::socket_path(name));
    }

    #[test]
    fn file_urls_become_paths() {
        assert_eq!(
            file_url_to_path("file:///Users/a/b%20c.png").as_deref(),
            Some("/Users/a/b c.png")
        );
        assert_eq!(
            file_url_to_path("file://localhost/tmp/%ED%95%9C.png").as_deref(),
            Some("/tmp/한.png")
        );
        assert_eq!(file_url_to_path("https://example.com/a.png"), None);
        assert_eq!(
            file_url_to_path("file:///100%.png").as_deref(),
            Some("/100%.png")
        );
    }
}
