//! 사용자에게 결과를 알리는 토스트.
//!
//! gpui-kit의 알림 레이어는 다이얼로그보다 아래에 그려져, 설정 같은 다이얼로그가 열려
//! 있으면 토스트가 가려진다. 그래서 다이얼로그가 열려 있을 때는 같은 내용을 그 위에
//! 쌓이는 경고창으로 보여주고, 그렇지 않을 때만 일반 토스트를 쓴다.

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::notification::Notification;

use crate::i18n::t;
use gpui_kit::{App, SharedString, Window};

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

pub trait WindowToast {
    fn toast(&mut self, toast: Toast, cx: &mut App);
}

impl WindowToast for Window {
    fn toast(&mut self, toast: Toast, cx: &mut App) {
        if self.has_active_dialog(cx) {
            // 제목이 있으면 제목과 설명으로, 없으면 메시지를 제목으로 보여준다.
            let Toast { title, message, .. } = toast;
            self.open_alert_dialog(cx, move |alert, _, _| {
                let alert = alert.ok_text(t("settings.confirm"));
                match title.clone() {
                    Some(title) => alert.title(title).description(message.clone()),
                    None => alert.title(message.clone()),
                }
            });
        } else {
            self.push_notification(toast.notification(), cx);
        }
    }
}
