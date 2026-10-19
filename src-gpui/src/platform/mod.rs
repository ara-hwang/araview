//! GPUI가 제공하지 않는 OS 연동: 창 핸들, 항상 위, 단일 인스턴스, 제어 채널.
//!
//! Windows는 Win32 직접 호출(`windows.rs`), Linux와 macOS는 유닉스 도메인 소켓 기반
//! (`unix.rs`)으로 구현한다. 두 구현은 같은 공개 함수 집합을 가진다.

use std::sync::mpsc::Receiver;
#[cfg(debug_assertions)]
use std::sync::mpsc::Sender;

/// 단일 인스턴스 판정 결과.
pub enum Instance {
    /// 첫 실행. 이후 실행이 넘기는 파일 경로(없으면 빈 문자열)를 받는다.
    Primary(Receiver<String>),
    /// 이미 실행 중인 창에 경로를 넘겼다. 이 프로세스는 종료한다.
    Forwarded,
}

/// 개발 빌드의 제어 통로. 에이전트가 실행 중인 창의 상태를 읽고 입력을 넣는다.
#[cfg(debug_assertions)]
pub struct Control {
    /// 요청 한 줄(JSON)을 앱으로 넘긴다.
    pub requests: Receiver<String>,
    /// 앱이 만든 응답 한 줄(JSON)을 제어 스레드로 넘긴다.
    pub replies: Sender<String>,
}

/// 전달받는 메시지 길이 상한(바이트).
const MAX_MESSAGE: usize = 64 * 1024;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::*;

#[cfg(test)]
mod tests {
    /// 배포 패키지의 파일 형식 선언이 지원 확장자와 어긋나지 않는지 확인한다.
    #[test]
    fn macos_bundle_declares_every_supported_extension() {
        let plist = include_str!("../../packaging/macos/Info.plist");
        for extension in araview_core::image::SUPPORTED_EXTENSIONS {
            assert!(
                plist.contains(&format!("<string>{extension}</string>")),
                "Info.plist에 `{extension}`이 없다"
            );
        }
    }

    #[test]
    fn linux_desktop_file_matches_the_app_identity() {
        let desktop = include_str!("../../packaging/linux/com.araview.viewer.desktop");
        assert!(desktop.contains("StartupWMClass=com.araview.viewer\n"));
        assert!(desktop.contains("Exec=araview %F\n"));
        for mime in ["image/png", "image/jpeg", "application/vnd.comicbook+zip"] {
            assert!(desktop.contains(mime), "{mime}");
        }
    }
}
