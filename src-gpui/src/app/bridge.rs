//! 개발 빌드 전용 제어 브리지: 실행 중인 창에 입력을 넣고 상태를 돌려준다.
//!
//! 에이전트가 실행 중인 창을 직접 확인할 수 있게, 입력을 창 메시지가 아니라 GPUI의
//! 입력 경로(`Window::dispatch_event`, `Window::dispatch_keystroke`)에 넣는다. 밖에서
//! 창 메시지(PostMessage)를 보내는 방식과 달리 수식키가 함께 실리고(수식키는
//! `GetKeyState`에서 읽히므로 밖에서는 만들 수 없다), 사용자의 마우스와 키보드를
//! 건드리지 않으며, 창이 활성 상태가 아니어도 같은 경로를 지난다.
//!
//! 통로는 `\\.\pipe\<식별자>-control` 위의 한 줄 JSON 요청과 한 줄 JSON 응답이다.
//! 명령 목록과 사용 절차는 `docs/playbooks.md`의 "Runtime check"를 본다.

use std::path::PathBuf;

use gpui_kit::component::WindowExt as _;
use gpui_kit::*;
use serde_json::{Value, json};
use smallvec::SmallVec;

use super::AraView;
use crate::settings::ViewMode;

/// 제어 파이프를 열고 요청을 처리하는 루프를 띄운다(개발 빌드의 실행 파일 전용).
#[cfg(not(test))]
pub(super) fn listen(
    identifier: &str,
    view: Entity<AraView>,
    window: &mut Window,
    cx: &mut Context<AraView>,
) {
    use std::sync::mpsc::TryRecvError;
    use std::time::Duration;

    /// 새 요청을 확인하는 주기.
    const POLL: Duration = Duration::from_millis(15);
    /// 한 번에 기다릴 수 있는 최대 시간(ms).
    const MAX_WAIT: u64 = 10_000;

    /// 명령 뒤에 붙는 대기 시간(ms).
    fn wait_field(request: &Value) -> u64 {
        request
            .get("waitMs")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(MAX_WAIT)
    }

    // 루프가 앱을 붙잡고 있지 않게 약한 참조만 든다.
    let weak = view.downgrade();
    drop(view);
    let crate::platform::Control { requests, replies } =
        crate::platform::control_channel(identifier);
    cx.spawn_in(window, async move |_, cx| {
        loop {
            cx.background_executor().timer(POLL).await;
            let line = match requests.try_recv() {
                Ok(line) => line,
                Err(TryRecvError::Empty) => continue,
                Err(TryRecvError::Disconnected) => break,
            };
            let request: Value = match serde_json::from_str(&line) {
                Ok(request) => request,
                Err(error) => {
                    let reply = error_reply(&format!("invalid request: {error}"));
                    if replies.send(reply).is_err() {
                        break;
                    }
                    continue;
                }
            };
            let Some(view) = weak.upgrade() else {
                break;
            };
            let applied = cx.update(|window, cx| apply_control(&request, window, cx, &view));
            match applied {
                // 명령이 실패해도 창은 살아 있으니 이유만 돌려준다.
                Ok(Err(error)) => {
                    if replies.send(error_reply(&error)).is_err() {
                        break;
                    }
                }
                Ok(Ok(())) => {
                    // 상태를 읽기 전에 배경 작업과 다음 프레임을 기다린다.
                    let wait = wait_field(&request);
                    if wait > 0 {
                        cx.background_executor()
                            .timer(Duration::from_millis(wait))
                            .await;
                    }
                    let reply = cx.update(|window, cx| reply(&request, window, cx, &view));
                    match reply {
                        Ok(reply) => {
                            if replies.send(reply).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                Err(_) => break,
            }
        }
    })
    .detach();
}

/// 제어 요청 하나를 실행하고 응답 한 줄(JSON)을 만든다. 헤드리스 테스트가 파이프 없이 쓴다.
/// 입력 주입이 곧바로 뷰를 업데이트하므로 뷰 업데이트 안이 아니라 앱 수준에서 부른다.
#[cfg(test)]
pub(super) fn handle_control(
    request: &Value,
    window: &mut Window,
    cx: &mut App,
    view: &Entity<AraView>,
) -> String {
    match apply_control(request, window, cx, view) {
        Ok(()) => reply(request, window, cx, view),
        Err(error) => error_reply(&error),
    }
}

/// 요청을 실행한다. 읽기만 하는 명령은 상태를 바꾸지 않는다.
fn apply_control(
    request: &Value,
    window: &mut Window,
    cx: &mut App,
    view: &Entity<AraView>,
) -> Result<(), String> {
    let command = text_field(request, "cmd")?;
    match command {
        "state" => {}
        "activate" => window.activate_window(),
        "resize" => apply_resize(request, window)?,
        "key" | "text" | "mouse" => apply_input(request, window, cx)?,
        "drop" => apply_drop(request, window, cx)?,
        "action" => {
            let id = text_field(request, "id")?.to_owned();
            view.update(cx, |this, cx| this.run_action(&id, window, cx));
        }
        "open" => {
            let path = text_field(request, "path")?.to_owned();
            view.update(cx, |this, cx| this.open_path(path, window, cx));
        }
        "wait" => {}
        other => return Err(format!("unknown cmd {other:?}")),
    }
    Ok(())
}

/// 창의 내용 크기를 논리 px로 바꾼다. 한쪽만 주면 다른 쪽은 지금 값을 유지한다.
/// 플랫폼이 창 크기를 비동기로 바꾸므로 결과는 `waitMs` 뒤 상태의 `window`로 확인한다.
/// 모니터보다 큰 값은 OS가 줄이고, 배율이 125%처럼 정수가 아니면 물리 px 반올림 오차가 난다.
/// 최대화나 전체화면에서는 창이 그 상태를 유지한 채 크기만 줄어 상태가 어긋나므로 거절한다.
fn apply_resize(request: &Value, window: &mut Window) -> Result<(), String> {
    /// 창을 이 범위 밖으로 만들지 않는다(논리 px). 아래쪽은 앱의 최소 창 크기다.
    const MAX_SIDE: f32 = 8000.0;

    if window.is_fullscreen() || window.is_maximized() {
        return Err("resize is unavailable while the window is maximized or fullscreen".to_owned());
    }
    let current = window.viewport_size();
    let dimension = |name: &str, current: Pixels, min: f32| -> Result<f32, String> {
        let value = match number_field(request, name) {
            Ok(value) => value,
            Err(_) if request.get(name).is_none() => return Ok(f32::from(current)),
            Err(_) => return Err(format!("{name} must be a number")),
        };
        if (min..=MAX_SIDE).contains(&value) {
            Ok(value)
        } else {
            Err(format!(
                "{name} must be between {min} and {MAX_SIDE} logical px"
            ))
        }
    };
    let (min_width, min_height) = crate::MIN_SIZE;
    let width = dimension("width", current.width, min_width)?;
    let height = dimension("height", current.height, min_height)?;
    if request.get("width").is_none() && request.get("height").is_none() {
        return Err("resize needs width or height".to_owned());
    }
    window.resize(size(px(width), px(height)));
    Ok(())
}

/// 키, 글자, 마우스 입력을 창의 입력 경로에 넣는다.
fn apply_input(request: &Value, window: &mut Window, cx: &mut App) -> Result<(), String> {
    match text_field(request, "cmd")? {
        "key" => {
            let spec = text_field(request, "spec")?;
            let keystroke = Keystroke::parse(spec)
                .map_err(|error| format!("invalid keystroke {spec:?}: {error}"))?;
            window.dispatch_keystroke(keystroke, cx);
        }
        "text" => {
            // `VisualTestContext::simulate_input`과 같은 경로로 글자를 넣는다.
            for character in text_field(request, "value")?.chars() {
                let key = character.to_string();
                window.dispatch_keystroke(
                    Keystroke {
                        modifiers: Modifiers::default(),
                        key: key.clone(),
                        key_char: Some(key),
                    },
                    cx,
                );
            }
        }
        _ => apply_mouse(request, window, cx)?,
    }
    Ok(())
}

/// 파일을 창에 끌어다 놓은 것처럼 만든다(드롭으로 열기).
fn apply_drop(request: &Value, window: &mut Window, cx: &mut App) -> Result<(), String> {
    let paths: Vec<PathBuf> = request
        .get("paths")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default();
    if paths.is_empty() {
        return Err("drop needs paths".to_owned());
    }
    let (x, y) = logical_coords(request, window.scale_factor())?;
    let position = point(px(x), px(y));
    window.dispatch_event(
        FileDropEvent::Entered {
            position,
            paths: ExternalPaths(SmallVec::from_vec(paths)),
        }
        .to_platform_input(),
        cx,
    );
    // 플랫폼은 끌어오는 동안 마우스 이동 이벤트를 계속 보낸다. 그 이벤트가 입력
    // 종류를 마우스로 바꾸고(호버 판정이 이 값을 본다), 드롭 대상이 그려진다.
    window.dispatch_event(
        MouseMoveEvent {
            position,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::default(),
        }
        .to_platform_input(),
        cx,
    );
    // 놓기는 한 프레임 뒤에 보낸다. 실제 플랫폼도 끌어온 뒤 프레임을 한 번 그린 다음
    // 놓기를 받으므로, 같은 순서로 맞춘다.
    window.on_next_frame(move |window, cx| {
        window.dispatch_event(FileDropEvent::Submit { position }.to_platform_input(), cx);
    });
    window.refresh();
    Ok(())
}

/// 요청의 한 쌍 좌표를 창 논리 px로 옮긴다. 기본 좌표계는 캡처 이미지의 픽셀(물리 px)이다.
fn convert_coords(request: &Value, scale: f32, x: f32, y: f32) -> (f32, f32) {
    if request.get("space").and_then(Value::as_str) == Some("logical") {
        (x, y)
    } else {
        (x / scale, y / scale)
    }
}

/// 요청의 `x`/`y`를 창 논리 px로 옮긴다.
fn logical_coords(request: &Value, scale: f32) -> Result<(f32, f32), String> {
    let x = number_field(request, "x")?;
    let y = number_field(request, "y")?;
    Ok(convert_coords(request, scale, x, y))
}

/// 마우스 입력을 만든다.
fn apply_mouse(request: &Value, window: &mut Window, cx: &mut App) -> Result<(), String> {
    let kind = text_field(request, "kind")?;
    let scale = window.scale_factor();
    let at = |x: f32, y: f32| point(px(x), px(y));
    let modifiers = modifiers_field(request)?;
    let button = match request.get("button").and_then(Value::as_str) {
        None | Some("left") => MouseButton::Left,
        Some("right") => MouseButton::Right,
        Some("middle") => MouseButton::Middle,
        Some(other) => return Err(format!("unknown button {other:?}")),
    };
    let (x, y) = logical_coords(request, scale)?;
    match kind {
        "move" => {
            window.dispatch_event(
                MouseMoveEvent {
                    position: at(x, y),
                    pressed_button: None,
                    modifiers,
                }
                .to_platform_input(),
                cx,
            );
        }
        "down" | "up" => {
            let click_count = number_field(request, "clickCount").unwrap_or(1.0).max(1.0) as usize;
            dispatch_click(
                at(x, y),
                button,
                click_count,
                modifiers,
                kind == "down",
                window,
                cx,
            );
        }
        "click" => {
            // 두 번 누르면 더블클릭으로 본다(캡션 더블클릭은 전체화면 전환).
            let count = number_field(request, "count")
                .unwrap_or(1.0)
                .clamp(1.0, 2.0) as usize;
            let position = at(x, y);
            for click_count in 1..=count {
                dispatch_click(position, button, click_count, modifiers, true, window, cx);
                dispatch_click(position, button, click_count, modifiers, false, window, cx);
            }
        }
        "drag" => {
            let (to_x, to_y) = convert_coords(
                request,
                scale,
                number_field(request, "toX")?,
                number_field(request, "toY")?,
            );
            let steps = number_field(request, "steps")
                .unwrap_or(8.0)
                .clamp(1.0, 120.0) as usize;
            dispatch_click(at(x, y), button, 1, modifiers, true, window, cx);
            for step in 1..=steps {
                // 마지막 이동이 목표 지점에 정확히 닿게 보간한다.
                let progress = step as f32 / steps as f32;
                window.dispatch_event(
                    MouseMoveEvent {
                        position: at(x + (to_x - x) * progress, y + (to_y - y) * progress),
                        pressed_button: Some(button),
                        modifiers,
                    }
                    .to_platform_input(),
                    cx,
                );
            }
            dispatch_click(at(to_x, to_y), button, 1, modifiers, false, window, cx);
        }
        "wheel" => {
            let lines = number_field(request, "lines").ok();
            let pixels = number_field(request, "delta").ok();
            let dx = number_field(request, "dx").unwrap_or(0.0);
            let delta = match (lines, pixels) {
                (Some(lines), _) => ScrollDelta::Lines(point(dx, lines)),
                (None, Some(pixels)) => ScrollDelta::Pixels(point(px(dx), px(pixels))),
                (None, None) => return Err("wheel needs lines or delta".to_owned()),
            };
            window.dispatch_event(
                ScrollWheelEvent {
                    position: at(x, y),
                    delta,
                    modifiers,
                    touch_phase: TouchPhase::Moved,
                }
                .to_platform_input(),
                cx,
            );
        }
        other => return Err(format!("unknown mouse kind {other:?}")),
    }
    Ok(())
}

/// 버튼 하나를 누르거나 뗀다.
#[allow(clippy::too_many_arguments)]
fn dispatch_click(
    position: Point<Pixels>,
    button: MouseButton,
    click_count: usize,
    modifiers: Modifiers,
    down: bool,
    window: &mut Window,
    cx: &mut App,
) {
    if down {
        window.dispatch_event(
            MouseDownEvent {
                position,
                modifiers,
                button,
                click_count,
                first_mouse: false,
            }
            .to_platform_input(),
            cx,
        );
    } else {
        window.dispatch_event(
            MouseUpEvent {
                position,
                modifiers,
                button,
                click_count,
            }
            .to_platform_input(),
            cx,
        );
    }
}

/// 요청 하나의 응답. 확인할 수 있게 항상 상태를 함께 싣는다.
fn reply(request: &Value, window: &mut Window, cx: &mut App, view: &Entity<AraView>) -> String {
    let state = view.update(cx, |this, cx| this.state_snapshot(window, cx));
    let mut reply = json!({ "ok": true, "state": state });
    if let Some(command) = request.get("cmd").and_then(Value::as_str) {
        reply["cmd"] = json!(command);
    }
    reply.to_string()
}

impl AraView {
    /// 뷰어 상태. 에이전트가 화면을 읽지 않고도 확인할 수 있는 값만 담는다.
    pub(super) fn state_snapshot(&self, window: &mut Window, cx: &mut Context<Self>) -> Value {
        let info = self.info.as_ref();
        let bounds = self.viewport.get();
        let view_mode = match self.view_mode(cx) {
            ViewMode::Single => "single",
            ViewMode::LeftToRight => "ltr",
            ViewMode::RightToLeft => "rtl",
            ViewMode::Webtoon => "webtoon",
        };
        json!({
            "kind": if self.archive.is_some() {
                "archive"
            } else if info.is_some() {
                "image"
            } else {
                "home"
            },
            "name": info.map(|info| info.file_name.clone()),
            "path": info.map(|info| info.source_path.clone()),
            "mimeType": info.map(|info| info.mime_type.clone()),
            "fileSize": info.map(|info| info.file_size),
            "imageSize": info.map(|info| json!({ "width": info.width, "height": info.height })),
            "index": self.index,
            "count": self.list.len(),
            "archive": self.archive.as_ref().map(|archive| archive.to_string()),
            "comic": self.comic.is_some(),
            "comicFailed": self.comic_failed,
            "viewMode": view_mode,
            "zoom": self.zoom,
            "fitLocked": self.fit_locked,
            "position": { "x": self.position.x, "y": self.position.y },
            "scaleFactor": window.scale_factor(),
            "viewport": {
                "x": f32::from(bounds.origin.x),
                "y": f32::from(bounds.origin.y),
                "width": f32::from(bounds.size.width),
                "height": f32::from(bounds.size.height),
            },
            "window": {
                "width": f32::from(window.viewport_size().width),
                "height": f32::from(window.viewport_size().height),
            },
            "infoOpen": self.info_open,
            "gridOpen": self.grid_open,
            "dialogOpen": window.has_active_dialog(cx),
            "fullscreen": window.is_fullscreen(),
            "playing": self.playing,
            "frame": self.frame,
            "loopsDone": self.loops_done,
            "opening": self.opening,
            "pixelHint": self.pixel_hint,
            "pagesLoaded": self.pages.len(),
            "webtoon": { "anchor": self.wt_anchor, "offset": self.wt_offset },
            "error": self.error.clone(),
        })
    }
}

fn error_reply(message: &str) -> String {
    json!({ "ok": false, "error": message }).to_string()
}

fn text_field<'a>(request: &'a Value, name: &str) -> Result<&'a str, String> {
    request
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing {name}"))
}

fn number_field(request: &Value, name: &str) -> Result<f32, String> {
    request
        .get(name)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .ok_or_else(|| format!("missing {name}"))
}

fn modifiers_field(request: &Value) -> Result<Modifiers, String> {
    let mut modifiers = Modifiers::default();
    let items = request.get("modifiers").and_then(Value::as_array);
    for item in items.into_iter().flatten() {
        match item.as_str() {
            Some("ctrl" | "control") => modifiers.control = true,
            Some("shift") => modifiers.shift = true,
            Some("alt") => modifiers.alt = true,
            Some("platform" | "win") => modifiers.platform = true,
            other => return Err(format!("unknown modifier {other:?}")),
        }
    }
    Ok(modifiers)
}
