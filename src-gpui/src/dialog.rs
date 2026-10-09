//! 다이얼로그 수직 배치: gpui-kit `Dialog`는 기본으로 뷰포트 상단 1/10에 뜨므로,
//! 화면 가운데에 오게 하는 상단 오프셋을 계산한다. 실제 높이는 콘텐츠·언어에
//! 따라 달라 대략적인 가운데 정렬이다.

use gpui_kit::{Pixels, Window, px};

/// 짧은 확인 경고창(제목+설명+버튼) 추정 높이.
pub const ALERT_HEIGHT: f32 = 240.0;
/// 이름 변경(제목+설명+입력+버튼) 추정 높이.
pub const RENAME_HEIGHT: f32 = 300.0;
/// 업데이트(제목+설명+릴리스 노트+버튼) 추정 높이.
pub const UPDATE_HEIGHT: f32 = 430.0;
/// 명령 팔레트(검색+목록) 추정 높이.
pub const PALETTE_HEIGHT: f32 = 480.0;
/// 라이선스(제목+520 목록) 추정 높이.
pub const LICENSES_HEIGHT: f32 = 600.0;
/// 설정(제목+520 본문) 추정 높이.
pub const SETTINGS_HEIGHT: f32 = 600.0;

/// 추정 높이를 받아 다이얼로그가 화면 가운데에 오게 하는 상단 오프셋을 구한다.
/// 작은 창에서는 잘리지 않게 최소 여백만 둔다.
pub fn centered_margin_top(window: &mut Window, estimated_height: f32) -> Pixels {
    let view_h = f32::from(window.viewport_size().height);
    px((view_h - estimated_height).max(32.0) / 2.0)
}
