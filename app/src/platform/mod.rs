//! GPUI가 제공하지 않는 OS 연동: 창 핸들, 항상 위, 단일 인스턴스, 제어 채널.
//!
//! Win32 직접 호출(`windows.rs`)로 구현한다.

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

mod windows;
pub use windows::*;
