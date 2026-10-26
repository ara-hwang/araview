//! 다이얼로그 수직 배치: gpui-kit `Dialog`는 기본으로 뷰포트 상단 1/10에 뜨므로,
//! 화면 가운데에 오게 하는 상단 오프셋을 계산한다. 실제 높이는 콘텐츠·언어에
//! 따라 달라 대략적인 가운데 정렬이다. 배경 누름이 클릭인지 창 이동 드래그인지
//! 가르는 판정도 여기 둔다(다이얼로그 배경이 캡션 판정이라 누름이 곧 드래그일 수 있다).

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::component::dialog::Dialog;
use gpui_kit::{App, Pixels, Point, Window, px};

use crate::platform;

/// 짧은 확인 경고창(제목+설명+버튼) 추정 높이.
pub const ALERT_HEIGHT: f32 = 240.0;
/// 이름 변경(제목+설명+입력+버튼) 추정 높이.
pub const RENAME_HEIGHT: f32 = 300.0;
/// 업데이트(제목+설명+릴리스 노트+버튼) 추정 높이.
pub const UPDATE_HEIGHT: f32 = 430.0;
/// 업데이트 다운로드(제목+설명+진행 막대+버튼) 추정 높이.
pub const DOWNLOAD_HEIGHT: f32 = 260.0;
/// 명령 팔레트(검색+목록) 추정 높이.
pub const PALETTE_HEIGHT: f32 = 480.0;
/// 라이선스(제목+520 목록) 추정 높이.
pub const LICENSES_HEIGHT: f32 = 600.0;
/// 설정(제목+520 본문) 추정 높이.
pub const SETTINGS_HEIGHT: f32 = 600.0;
/// 설정 다이얼로그 폭과 제목 줄 추정 높이(제목 줄 클릭은 바깥 클릭이 아니다).
pub const SETTINGS_WIDTH: f32 = 760.0;
pub const SETTINGS_TITLE_HEIGHT: f32 = 64.0;

/// 추정 높이를 받아 다이얼로그가 화면 가운데에 오게 하는 상단 오프셋을 구한다.
/// 작은 창에서는 잘리지 않게 최소 여백만 둔다.
pub fn centered_margin_top(window: &mut Window, estimated_height: f32) -> Pixels {
    let view_h = f32::from(window.viewport_size().height);
    px((view_h - estimated_height).max(32.0) / 2.0)
}

/// 가운데 배치. 열린 다이얼로그는 gpui-kit 기본 배경으로 창 전체를 덮어 뒤 화면을
/// 어둡게 한다(색은 테마 `overlay` 토큰, `system::apply_theme`가 라이트·다크 모두 20%
/// 검정으로 맞춘다). 배경 클릭으로 닫지는 않는다: 이 앱의 바깥 클릭 닫기는 명령
/// 팔레트와 설정이 다이얼로그 안에서 직접 처리하며(`BackdropClick`), 나머지는
/// `Esc`·버튼으로만 닫는다(SPEC §17). 배경이 창 전체에 `WindowControlArea::Drag` 히트 영역을 깔기 때문에
/// 다이얼로그가 열려 있는 동안 Windows는 창 전체를 캡션으로 판정한다: 최소화/최대화/
/// 닫기 버튼은 동작하지 않고(다이얼로그 호스트의 `occlude()`가 OS 히트테스트용 마우스
/// 좌표 수집을 막는다), 배경을 잡아 끌면 창이 움직인다.
pub fn centered(dialog: Dialog, window: &mut Window, estimated_height: f32) -> Dialog {
    dialog
        .margin_top(centered_margin_top(window, estimated_height))
        .overlay_closable(false)
}

/// 누른 자리에서 이만큼(창 논리 px) 넘게 떨어져 떼면 클릭이 아니라 드래그로 본다.
const DRAG_SLOP: f32 = 6.0;
/// 배경 누름 뒤 물리 왼쪽 버튼 상태를 확인하는 간격.
const RELEASE_POLL: Duration = Duration::from_millis(25);

/// 다이얼로그 배경 누름이 "클릭"인지 "창 이동 드래그"인지 가른다.
///
/// 다이얼로그가 열려 있으면 배경은 Windows에게 캡션(`HTCAPTION`)이라, 배경을 누르면
/// OS가 창 이동 루프를 돌린다. 그래서 누름 하나만으로 닫으면 배경을 잡아 끄는 순간에도
/// 닫혀 버린다. 누름에서는 닫지 않고, 뗄 때 그 사이에 창이 움직이지 않았을 때만
/// 클릭으로 본다.
#[derive(Clone, Default)]
pub struct BackdropClick {
    press: Rc<RefCell<Option<Press>>>,
}

#[derive(Clone, Copy)]
struct Press {
    /// 누른 자리(창 논리 px).
    position: Point<Pixels>,
    /// 누를 때의 창 원점(화면 좌표). 얻지 못했으면 None이다.
    origin: Option<(i32, i32)>,
}

impl BackdropClick {
    /// 배경을 눌렀다. 닫지 않고 기록만 한다. 이미 지켜보는 누름이 있으면 false다.
    pub fn press(&self, position: Point<Pixels>, origin: Option<(i32, i32)>) -> bool {
        let mut slot = self.press.borrow_mut();
        if slot.is_some() {
            return false;
        }
        *slot = Some(Press { position, origin });
        true
    }

    /// 지켜보는 누름이 남아 있는가.
    pub fn pending(&self) -> bool {
        self.press.borrow().is_some()
    }

    /// 뗌을 처리한다. `position`은 뗀 자리(모르면 None), `origin`은 지금 창 원점(모르면
    /// None)이다. 배경 클릭으로 끝났으면 true를 돌려준다.
    pub fn release(&self, position: Option<Point<Pixels>>, origin: Option<(i32, i32)>) -> bool {
        let Some(press) = self.press.borrow_mut().take() else {
            return false;
        };
        // 창이 움직였으면 배경을 잡아 끌어 창을 옮긴 것이다.
        if let (Some(before), Some(now)) = (press.origin, origin)
            && before != now
        {
            return false;
        }
        // 뗀 자리를 알면 누른 자리와 견준다(뗌이 오는 합성 입력 경로).
        !matches!(position, Some(now) if distance(press.position, now) > DRAG_SLOP)
    }
}

/// 두 지점 사이의 거리(창 논리 px).
fn distance(a: Point<Pixels>, b: Point<Pixels>) -> f32 {
    let dx = f32::from(b.x) - f32::from(a.x);
    let dy = f32::from(b.y) - f32::from(a.y);
    (dx * dx + dy * dy).sqrt()
}

/// 배경 누름의 뗌을 지켜보다가, 배경 클릭으로 끝났으면 `close`를 부른다.
///
/// 배경이 캡션 판정이라 OS 이동 루프가 뗌을 가져가는 경로에서는 gpui가 뗌을 주지
/// 않는다. 그 경로의 뗌은 물리 왼쪽 버튼 상태로 알아낸다. 뗌이 gpui로 오는 경로는
/// `BackdropClick::release`가 먼저 처리하고 이 감시는 곧바로 끝난다.
pub fn watch_backdrop_click(
    press: BackdropClick,
    window: &mut Window,
    cx: &App,
    close: impl Fn(&mut Window, &mut App) + Clone + 'static,
) {
    // 물리 버튼을 보는 감시는 실제 창(HWND)이 있을 때만 의미가 있다. 창이 없으면
    // 뗌이 gpui로 그대로 오므로 감시할 것이 없다(헤드리스 테스트가 이 경우다).
    if platform::window_origin(window).is_none() {
        return;
    }
    window
        .spawn(cx, async move |cx| {
            loop {
                cx.background_executor().timer(RELEASE_POLL).await;
                if !press.pending() {
                    // gpui가 뗌을 먼저 처리했다.
                    return;
                }
                if platform::left_button_down() {
                    continue;
                }
                // 이 경로에서는 뗀 자리를 모른다. 창 원점만으로 가른다.
                let clicked = cx
                    .update(|window, _| press.release(None, platform::window_origin(window)))
                    .unwrap_or(false);
                if clicked {
                    cx.update(move |window, cx| close(window, cx)).ok();
                }
                return;
            }
        })
        .detach();
}
