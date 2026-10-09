//! 사용자에게 결과를 알리는 토스트.
//!
//! gpui-kit의 알림 레이어는 다이얼로그보다 아래에 그려져, 설정 같은 다이얼로그가 열려
//! 있으면 토스트가 가려진다. 그래서 다이얼로그가 열려 있을 때는 같은 내용을 그 위에
//! 쌓이는 경고창으로 보여주고, 그렇지 않을 때만 일반 토스트를 쓴다.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{DialogDescription, DialogFooter};
use gpui_kit::component::notification::Notification;

use crate::dialog::{ALERT_HEIGHT, centered_margin_top};
use crate::i18n::t;
use gpui_kit::{App, ParentElement as _, SharedString, Window};

pub(crate) const TOAST_DURATION: Duration = Duration::from_secs(5);
static NEXT_TOAST_ID: AtomicUsize = AtomicUsize::new(0);

struct TimedToast;

#[derive(Clone, Copy)]
enum Tone {
    Info,
    Success,
    Warning,
    Error,
}

pub struct Toast {
    tone: Tone,
    title: Option<SharedString>,
    message: SharedString,
}

impl Toast {
    fn new(tone: Tone, message: impl Into<SharedString>) -> Self {
        Self {
            tone,
            title: None,
            message: message.into(),
        }
    }

    pub fn info(message: impl Into<SharedString>) -> Self {
        Self::new(Tone::Info, message)
    }

    pub fn success(message: impl Into<SharedString>) -> Self {
        Self::new(Tone::Success, message)
    }

    pub fn warning(message: impl Into<SharedString>) -> Self {
        Self::new(Tone::Warning, message)
    }

    pub fn error(message: impl Into<SharedString>) -> Self {
        Self::new(Tone::Error, message)
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    fn notification(self) -> Notification {
        let note = match self.tone {
            Tone::Info => Notification::info(self.message),
            Tone::Success => Notification::success(self.message),
            Tone::Warning => Notification::warning(self.message),
            Tone::Error => Notification::error(self.message),
        };
        match self.title {
            Some(title) => note.title(title),
            None => note,
        }
    }
}

/// 알림 스택의 hover/focus에 영향받지 않고 지정 시간 뒤 토스트를 닫는다.
pub(crate) fn push_timed_notification(
    window: &mut Window,
    notification: Notification,
    cx: &mut App,
) {
    let id = NEXT_TOAST_ID.fetch_add(1, Ordering::Relaxed);
    let window_handle = window.window_handle();
    window.push_notification(notification.id1::<TimedToast>(id).autohide(false), cx);

    cx.spawn(async move |cx| {
        cx.background_executor().timer(TOAST_DURATION).await;
        // 타이머가 끝나기 전에 창을 닫았다면 남은 알림도 없다.
        let result = window_handle.update(cx, |_, window, cx| {
            window.remove_notification1::<TimedToast>(id, cx);
        });
        if let Err(error) = result {
            log::debug!("[toast] window closed before timeout: {error}");
        }
    })
    .detach();
}

pub trait WindowToast {
    fn toast(&mut self, toast: Toast, cx: &mut App);
}

impl WindowToast for Window {
    fn toast(&mut self, toast: Toast, cx: &mut App) {
        if self.has_active_dialog(cx) {
            // 제목이 있으면 제목과 설명으로, 없으면 메시지를 제목으로 보여준다.
            // `AlertDialog`에는 상단 오프셋이 없어 가운데 배치가 안 되니,
            // 같은 모양의 `Dialog`로 쌓는다.
            let Toast { title, message, .. } = toast;
            self.open_dialog(cx, move |dialog, window, _| {
                let (title_text, desc): (SharedString, Option<SharedString>) = match title.clone() {
                    Some(title) => (title, Some(message.clone())),
                    None => (message.clone(), None),
                };
                let mut out = dialog
                    .margin_top(centered_margin_top(window, ALERT_HEIGHT))
                    .close_button(false)
                    .overlay_closable(false)
                    .title(title_text);
                if let Some(desc) = desc {
                    out = out.child(DialogDescription::new().child(desc));
                }
                out.footer(
                    DialogFooter::new().child(
                        Button::new("toast-ok")
                            .primary()
                            .label(t("settings.confirm"))
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
                )
            });
        } else {
            push_timed_notification(self, toast.notification(), cx);
        }
    }
}
