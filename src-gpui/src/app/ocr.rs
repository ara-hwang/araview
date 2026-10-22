//! 이미지 속 글자 인식(OCR)과 텍스트 복사(SPEC §15 `recognize_text`).

use std::path::PathBuf;

use araview_core::app_error::ErrorCode;
use araview_core::ocr::{OcrLanguage, OcrResult};
use araview_core::ops;
use gpui_kit::*;

use super::AraView;
use crate::i18n::{t, t_with};
use crate::platform::{self, SystemOcr};
use crate::toast::{Toast, WindowToast};

impl AraView {
    /// 지금 보는 이미지의 글자를 인식해 클립보드에 텍스트로 복사한다.
    pub(super) fn recognize_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !platform::OCR_SUPPORTED {
            window.toast(Toast::warning(t("toast.ocr.unsupported")), cx);
            return;
        }
        let Some(info) = &self.info else {
            window.toast(Toast::warning(t("toast.ocr.empty")), cx);
            return;
        };
        if self.ocr_job.is_some() {
            window.toast(Toast::info(t("toast.ocr.busy")), cx);
            return;
        }
        // 화면이 디코드하는 경로를 쓴다. HEIC, PSD 같은 포맷도 sidecar로 읽을 수 있다.
        let render_path = info.file_path.clone();
        self.ocr_job = Some(render_path.clone());
        let path = PathBuf::from(&render_path);
        let task = cx.background_spawn(async move {
            ops::recognize_text_blocking(&SystemOcr, &path, &OcrLanguage::Auto)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.ocr_job = None;
                // 인식하는 사이 다른 이미지로 넘어갔으면 결과를 버린다.
                let still_current = this
                    .info
                    .as_ref()
                    .is_some_and(|info| info.file_path == render_path);
                if still_current {
                    this.finish_recognition(result, window, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn finish_recognition(
        &mut self,
        result: Result<OcrResult, araview_core::app_error::AppError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(result) if result.is_empty() => {
                window.toast(Toast::warning(t("toast.ocr.none")), cx);
            }
            Ok(result) => {
                cx.write_to_clipboard(ClipboardItem::new_string(result.text()));
                let message = t_with("toast.ocr.done", &[("count", &result.lines.len())]);
                window.toast(Toast::success(message), cx);
            }
            // 언어 팩이 없을 때만 지원하지 않음으로 돌아온다. 설치 방법을 안내한다.
            Err(error) if error.code == ErrorCode::Unsupported => {
                window.toast(Toast::warning(t("toast.ocr.noLanguage")), cx);
            }
            Err(error) => {
                window.toast(Toast::error(error.message).title(t("toast.ocr.fail")), cx);
            }
        }
    }
}
