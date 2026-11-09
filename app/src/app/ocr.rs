//! 이미지 속 글자 인식(OCR)과 텍스트 복사, 강조·선택(SPEC §11.5, §15 `recognize_text`).

use std::path::PathBuf;

use araview_core::app_error::ErrorCode;
use araview_core::ocr::{OcrLanguage, OcrRect, OcrResult};
use araview_core::ops;
use gpui_kit::*;

use super::AraView;
use crate::i18n::{t, t_with};
use crate::picture::Orientation;
use crate::platform::{self, SystemOcr};
use crate::toast::{Toast, WindowToast};

/// 이 거리(논리 px) 이상 끌어야 한 단어 안의 드래그도 선택으로 본다.
const DRAG_SLOP: f32 = 4.0;

/// 인식이 끝난 뒤 결과로 할 일.
#[derive(Clone, Copy)]
pub(super) enum OcrOutcome {
    /// 글자를 클립보드에 복사한다.
    Copy,
    /// 이미지 위에 글자 자리를 강조하고 선택할 수 있게 한다. 복사하지 않는다.
    Highlight,
}

/// 인식한 글자 하나. OCR 엔진은 단어 자리까지만 주므로, 글자 자리는 단어 사각형을 글자
/// 폭 비율로 나눠 어림한 값이다. 좌표는 방향을 돌리기 전 원본 이미지를 1로 본 값이다.
struct Glyph {
    ch: char,
    rect: OcrRect,
    /// 이 글자가 속한 단어의 번호.
    word: usize,
}

/// 인식한 단어. `first`와 `last`는 `glyphs` 안의 번호(양끝 포함)다.
struct Word {
    first: usize,
    last: usize,
    /// 이 단어가 속한 줄의 번호.
    line: usize,
}

struct Line {
    text: String,
    rect: OcrRect,
    /// 이 줄의 첫/마지막 글자 번호(`glyphs` 안).
    first: usize,
    last: usize,
    /// 단어 사이에 공백이 없는 표기(중국어, 일본어)인지.
    spaceless: bool,
}

/// 글자 폭의 어림 비율. 가변 폭 글꼴의 좁은 글자와 넓은 글자, 전각 글자를 구분한다.
fn glyph_weight(ch: char) -> f32 {
    match ch {
        'i' | 'j' | 'l' | 'I' | 't' | 'f' | 'r' | '.' | ',' | ':' | ';' | '!' | '\'' | '|' => 0.55,
        'm' | 'w' => 1.5,
        'M' | 'W' => 1.8,
        c if c.is_ascii_uppercase() => 1.2,
        c if c.is_ascii() => 1.0,
        _ => 1.8,
    }
}

/// 인식 결과 한 벌. 좌표는 원본 기준이라, 회전과 반전은 그릴 때 현재 방향으로 옮긴다.
pub(super) struct Highlight {
    /// 인식한 이미지의 렌더 경로.
    path: String,
    lines: Vec<Line>,
    words: Vec<Word>,
    glyphs: Vec<Glyph>,
}

impl Highlight {
    fn new(path: String, result: &OcrResult) -> Self {
        let mut lines = Vec::new();
        let mut words = Vec::new();
        let mut glyphs: Vec<Glyph> = Vec::new();
        for line in &result.lines {
            let first = glyphs.len();
            let number = lines.len();
            // 엔진이 단어를 나누지 못했으면 줄 전체를 단어 하나로 둔다.
            let fallback = [(line.text.as_str(), line.rect)];
            let sources: Vec<(&str, OcrRect)> = if line.words.is_empty() {
                fallback.to_vec()
            } else {
                line.words
                    .iter()
                    .map(|word| (word.text.as_str(), word.rect))
                    .collect()
            };
            for (text, rect) in sources {
                let chars: Vec<char> = text.chars().filter(|ch| !ch.is_whitespace()).collect();
                if chars.is_empty() {
                    continue;
                }
                let total: f32 = chars.iter().map(|ch| glyph_weight(*ch)).sum();
                let word = words.len();
                let word_first = glyphs.len();
                let mut x = rect.x;
                for ch in chars {
                    let width = rect.width * glyph_weight(ch) / total;
                    glyphs.push(Glyph {
                        ch,
                        rect: OcrRect {
                            x,
                            y: rect.y,
                            width,
                            height: rect.height,
                        },
                        word,
                    });
                    x += width;
                }
                words.push(Word {
                    first: word_first,
                    last: glyphs.len() - 1,
                    line: number,
                });
            }
            if glyphs.len() == first {
                continue;
            }
            lines.push(Line {
                text: line.text.clone(),
                rect: line.rect,
                first,
                last: glyphs.len() - 1,
                spaceless: line.language.starts_with("ja") || line.language.starts_with("zh"),
            });
        }
        Self {
            path,
            lines,
            words,
            glyphs,
        }
    }

    pub(super) fn path(&self) -> &str {
        &self.path
    }

    /// 줄마다 `orientation`을 적용한 사각형(이미지 전체를 1로 본 값).
    pub(super) fn line_rects(&self, orientation: Orientation) -> Vec<OcrRect> {
        self.lines
            .iter()
            .map(|line| orient_rect(line.rect, orientation))
            .collect()
    }

    /// 글자마다 `orientation`을 적용한 사각형.
    pub(super) fn glyph_rects(&self, orientation: Orientation) -> Vec<OcrRect> {
        self.glyphs
            .iter()
            .map(|glyph| orient_rect(glyph.rect, orientation))
            .collect()
    }

    /// 글자 `glyph`가 속한 줄의 번호.
    pub(super) fn glyph_line(&self, glyph: usize) -> Option<usize> {
        Some(self.words.get(self.glyphs.get(glyph)?.word)?.line)
    }

    /// 글자 `glyph`가 속한 단어의 첫/마지막 글자 번호.
    pub(super) fn word_span(&self, glyph: usize) -> Option<(usize, usize)> {
        let word = self.words.get(self.glyphs.get(glyph)?.word)?;
        Some((word.first, word.last))
    }

    /// 글자 `glyph`가 속한 줄의 첫/마지막 글자 번호.
    pub(super) fn line_span(&self, glyph: usize) -> Option<(usize, usize)> {
        let line = self.lines.get(self.glyph_line(glyph)?)?;
        Some((line.first, line.last))
    }

    /// 글자 번호 `a`와 `b` 사이(양끝 포함)의 글자. 한 줄 전체를 고르면 엔진이 준 줄 글자를
    /// 그대로 쓰고, 일부면 단어 사이에 공백을 넣으며, 줄이 바뀌면 줄바꿈을 넣는다.
    pub(super) fn text(&self, a: usize, b: usize) -> String {
        let (low, high) = (a.min(b), a.max(b).min(self.glyphs.len().saturating_sub(1)));
        let mut parts = Vec::new();
        for line in &self.lines {
            if line.last < low || line.first > high {
                continue;
            }
            let (from, to) = (line.first.max(low), line.last.min(high));
            if from == line.first && to == line.last {
                parts.push(line.text.clone());
                continue;
            }
            let mut out = String::new();
            let mut previous = None;
            for glyph in &self.glyphs[from..=to] {
                if previous.is_some_and(|word| word != glyph.word) && !line.spaceless {
                    out.push(' ');
                }
                out.push(glyph.ch);
                previous = Some(glyph.word);
            }
            parts.push(out);
        }
        parts.join("\n")
    }
}

/// 원본 기준 사각형을 `Orientation`(회전 뒤 반전, `picture::orient`와 같은 순서)이 적용된
/// 이미지 기준으로 옮긴다.
fn orient_rect(rect: OcrRect, orientation: Orientation) -> OcrRect {
    let map = |u: f32, v: f32| {
        let (mut u, mut v) = match orientation.rotation {
            90 => (1.0 - v, u),
            180 => (1.0 - u, 1.0 - v),
            270 => (v, 1.0 - u),
            _ => (u, v),
        };
        if orientation.flip_h {
            u = 1.0 - u;
        }
        if orientation.flip_v {
            v = 1.0 - v;
        }
        (u, v)
    };
    let (u0, v0) = map(rect.x, rect.y);
    let (u1, v1) = map(rect.x + rect.width, rect.y + rect.height);
    OcrRect {
        x: u0.min(u1),
        y: v0.min(v1),
        width: (u1 - u0).abs(),
        height: (v1 - v0).abs(),
    }
}

/// 고른 글자와 진행 중인 드래그. 강조가 보이는 동안만 있다.
#[derive(Default)]
struct Selection {
    /// 고른 글자 범위(시작, 끝). 둘 다 인식 결과의 글자 번호다.
    range: Option<(usize, usize)>,
    /// 드래그 선택을 시작한 글자 번호와 누른 위치.
    drag: Option<(usize, Point<Pixels>)>,
}

/// 글자 강조의 표시 단계. 경로는 강조를 켠 이미지의 렌더 경로다.
#[derive(Default)]
enum OcrDisplay {
    /// 꺼져 있다.
    #[default]
    Off,
    /// 이 이미지의 강조를 켜려고 인식을 요청했다. 결과가 오기 전이라 그릴 것은 없지만 헤더
    /// 버튼은 켜진 모양이고, 다시 누르면 끈다. 인식에 실패하면 꺼진다.
    Requested(String),
    /// 이 이미지 위에 인식 결과를 그린다. 지금 보는 이미지가 다르면 그리지 않는다.
    Shown { path: String, selection: Selection },
}

impl OcrDisplay {
    fn path(&self) -> Option<&str> {
        match self {
            Self::Off => None,
            Self::Requested(path) | Self::Shown { path, .. } => Some(path),
        }
    }
}

/// 글자 인식(복사, 강조)과 강조 위 글자 선택의 상태(SPEC §11.5). `AraView`가 하나를 들고,
/// 전이는 이 타입의 메서드로만 한다.
#[derive(Default)]
pub(super) struct OcrState {
    /// 인식 중인 이미지의 렌더 경로. 복사와 강조가 함께 쓰며 인식은 한 번에 하나만 돈다.
    job: Option<String>,
    /// 가장 최근에 강조용으로 인식한 결과. 같은 이미지를 다시 켤 때 재사용한다.
    result: Option<Highlight>,
    display: OcrDisplay,
}

impl OcrState {
    /// `path`의 강조가 켜져 있는지(결과를 기다리는 중도 포함). 헤더 버튼의 눌린 모양이다.
    fn is_on(&self, path: &str) -> bool {
        self.display.path() == Some(path)
    }

    /// `path` 위에 그릴 인식 결과. 결과를 기다리는 중이거나 다른 이미지면 없다.
    fn shown(&self, path: &str) -> Option<&Highlight> {
        match &self.display {
            OcrDisplay::Shown { path: shown, .. } if shown == path => self
                .result
                .as_ref()
                .filter(|highlight| highlight.path() == path),
            _ => None,
        }
    }

    fn selection_state(&self) -> Option<&Selection> {
        match &self.display {
            OcrDisplay::Shown { selection, .. } => Some(selection),
            _ => None,
        }
    }

    fn selection_state_mut(&mut self) -> Option<&mut Selection> {
        match &mut self.display {
            OcrDisplay::Shown { selection, .. } => Some(selection),
            _ => None,
        }
    }

    /// 고른 글자 범위(시작, 끝).
    pub(super) fn selection(&self) -> Option<(usize, usize)> {
        self.selection_state()?.range
    }

    pub(super) fn has_selection(&self) -> bool {
        self.selection().is_some()
    }

    /// 글자를 누른 채 끌고 있는지.
    pub(super) fn dragging(&self) -> bool {
        self.selection_state()
            .is_some_and(|selection| selection.drag.is_some())
    }

    /// 드래그를 끝낸다. 고른 글자는 남는다.
    pub(super) fn end_drag(&mut self) {
        if let Some(selection) = self.selection_state_mut() {
            selection.drag = None;
        }
    }

    /// 선택과 드래그를 푼다. 고른 글자가 있었으면 true(다시 그려야 한다).
    fn clear_selection(&mut self) -> bool {
        let Some(selection) = self.selection_state_mut() else {
            return false;
        };
        selection.drag = None;
        selection.range.take().is_some()
    }

    /// 강조를 끈다. 선택도 함께 사라진다. 진행 중인 인식과 마지막 결과는 그대로 둔다.
    pub(super) fn hide(&mut self) {
        self.display = OcrDisplay::Off;
    }

    /// `path`의 결과가 이미 있으면 다시 인식하지 않고 바로 보인다.
    fn show_cached(&mut self, path: &str) -> bool {
        if self
            .result
            .as_ref()
            .is_none_or(|highlight| highlight.path() != path)
        {
            return false;
        }
        self.display = OcrDisplay::Shown {
            path: path.to_owned(),
            selection: Selection::default(),
        };
        true
    }

    /// `path`의 강조를 켜려고 인식을 시작했다.
    fn request(&mut self, path: String) {
        self.display = OcrDisplay::Requested(path);
    }

    fn job_running(&self) -> bool {
        self.job.is_some()
    }

    /// 인식을 시작한다. 이미 하나가 돌고 있으면 false다.
    fn begin_job(&mut self, path: String) -> bool {
        if self.job.is_some() {
            return false;
        }
        self.job = Some(path);
        true
    }

    fn finish_job(&mut self) {
        self.job = None;
    }

    /// 강조용 인식이 글자를 찾았다. 결과를 기억하고, 그 이미지의 강조를 요청해 둔 상태면 보인다.
    fn recognized(&mut self, highlight: Highlight) {
        let path = highlight.path().to_owned();
        self.result = Some(highlight);
        if matches!(&self.display, OcrDisplay::Requested(requested) if *requested == path) {
            self.display = OcrDisplay::Shown {
                path,
                selection: Selection::default(),
            };
        } else if let Some(selection) = self.selection_state_mut() {
            *selection = Selection::default();
        }
    }

    /// 강조용 인식이 실패했거나 글자가 없다. 켜려던 강조를 되돌린다.
    fn request_failed(&mut self, path: &str) {
        if matches!(&self.display, OcrDisplay::Requested(requested) if requested == path) {
            self.display = OcrDisplay::Off;
        }
    }

    /// 인식하는 사이 다른 이미지로 넘어가 결과를 버렸다. 그 이미지의 강조는 끈다.
    fn abandon(&mut self, path: &str) {
        if self.is_on(path) {
            self.display = OcrDisplay::Off;
        }
    }

    /// 글자를 눌렀다. 한 번은 드래그의 시작, 두 번은 `word`, 세 번은 `line`을 고른다.
    fn press(
        &mut self,
        glyph: usize,
        clicks: usize,
        position: Point<Pixels>,
        word: Option<(usize, usize)>,
        line: Option<(usize, usize)>,
    ) {
        let Some(selection) = self.selection_state_mut() else {
            return;
        };
        *selection = match clicks {
            0 | 1 => Selection {
                range: None,
                drag: Some((glyph, position)),
            },
            2 => Selection {
                range: word,
                drag: None,
            },
            _ => Selection {
                range: line,
                drag: None,
            },
        };
    }

    /// 드래그 중인 선택의 끝을 글자 `glyph`로 옮긴다. 선택이 바뀌었으면 true다.
    fn extend(&mut self, glyph: usize, position: Point<Pixels>) -> bool {
        let Some(selection) = self.selection_state_mut() else {
            return false;
        };
        let Some((anchor, from)) = selection.drag else {
            return false;
        };
        // 글자 하나 위에서 끌어도 그 글자가 골라지게 하되, 클릭 중의 손떨림은 거른다.
        let moved =
            f32::from(position.x - from.x).hypot(f32::from(position.y - from.y)) >= DRAG_SLOP;
        let next =
            (anchor != glyph || moved || selection.range.is_some()).then_some((anchor, glyph));
        if next == selection.range {
            return false;
        }
        selection.range = next;
        true
    }
}

impl AraView {
    /// 지금 보는 이미지에 글자 강조가 켜져 있는지(결과를 기다리는 중도 포함).
    pub(super) fn ocr_visible(&self) -> bool {
        self.info
            .as_ref()
            .is_some_and(|info| self.ocr.is_on(&info.file_path))
    }

    /// 지금 보는 이미지의 인식 결과. 강조가 꺼져 있거나, 결과를 기다리는 중이거나, 다른
    /// 이미지면 없다.
    pub(super) fn ocr_current(&self) -> Option<&Highlight> {
        self.ocr.shown(&self.info.as_ref()?.file_path)
    }

    /// 선택한 글자. 선택이 없으면 `None`.
    pub(super) fn ocr_selected_text(&self) -> Option<String> {
        let (anchor, head) = self.ocr.selection()?;
        let text = self.ocr_current()?.text(anchor, head);
        (!text.is_empty()).then_some(text)
    }

    pub(super) fn clear_ocr_selection(&mut self, cx: &mut Context<Self>) {
        if self.ocr.clear_selection() {
            cx.notify();
        }
    }

    /// 글자 강조를 켜고 끈다. 이 이미지를 처음 켤 때만 글자를 인식하고, 이후에는 결과를
    /// 다시 쓴다.
    pub(super) fn toggle_text_highlight(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocr_visible() {
            self.ocr.hide();
            cx.notify();
            return;
        }
        if self.view_mode(cx) != crate::settings::ViewMode::Single {
            window.toast(Toast::warning(t("toast.ocr.singleOnly")), cx);
            return;
        }
        let Some(path) = self.info.as_ref().map(|info| info.file_path.clone()) else {
            window.toast(Toast::warning(t("toast.ocr.empty")), cx);
            return;
        };
        if self.ocr.show_cached(&path) {
            cx.notify();
            return;
        }
        if self.start_recognition(OcrOutcome::Highlight, window, cx) {
            self.ocr.request(path);
        } else {
            // 시작하지 못했다(미지원, 다른 인식 진행 중)면 켜진 것으로 두지 않는다.
            self.ocr.hide();
        }
        cx.notify();
    }

    /// 글자 위에서 마우스를 눌렀을 때: 한 번은 드래그 선택의 시작, 두 번은 단어, 세 번은 줄.
    pub(super) fn ocr_press(
        &mut self,
        glyph: usize,
        clicks: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus.focus(window, cx);
        let Some((word, line)) = self
            .ocr_current()
            .map(|highlight| (highlight.word_span(glyph), highlight.line_span(glyph)))
        else {
            return;
        };
        self.ocr.press(glyph, clicks, position, word, line);
        cx.notify();
    }

    /// 드래그 중인 선택의 끝을 글자 `glyph`로 옮긴다.
    pub(super) fn ocr_extend(
        &mut self,
        glyph: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.ocr.extend(glyph, position) {
            cx.notify();
        }
    }

    /// 지금 보는 이미지의 글자를 인식해 `outcome`대로 처리한다.
    pub(super) fn recognize_text(
        &mut self,
        outcome: OcrOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.start_recognition(outcome, window, cx);
    }

    /// 인식을 시작한다. 시작하지 못하면(미지원, 그릴 이미지 없음, 다른 인식 진행 중) 이유를
    /// 토스트로 알리고 false다.
    fn start_recognition(
        &mut self,
        outcome: OcrOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !platform::OCR_SUPPORTED {
            window.toast(Toast::warning(t("toast.ocr.unsupported")), cx);
            return false;
        }
        if !self.current_page_is_full() {
            window.toast(Toast::warning(t("toast.ocr.empty")), cx);
            return false;
        }
        let Some(info) = &self.info else {
            window.toast(Toast::warning(t("toast.ocr.empty")), cx);
            return false;
        };
        if self.ocr.job_running() {
            window.toast(Toast::info(t("toast.ocr.busy")), cx);
            return false;
        }
        // 화면이 디코드하는 경로를 쓴다. HEIC, PSD 같은 포맷도 sidecar로 읽을 수 있다.
        let render_path = info.file_path.clone();
        self.ocr.begin_job(render_path.clone());
        let path = PathBuf::from(&render_path);
        let task = cx.background_spawn(async move {
            ops::recognize_text_blocking(&SystemOcr, &path, &OcrLanguage::Auto)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.ocr.finish_job();
                // 인식하는 사이 다른 이미지로 넘어갔으면 결과를 버린다.
                let still_current = this
                    .info
                    .as_ref()
                    .is_some_and(|info| info.file_path == render_path);
                if still_current {
                    this.finish_recognition(outcome, &render_path, result, window, cx);
                } else {
                    this.ocr.abandon(&render_path);
                }
            })
            .ok();
        })
        .detach();
        true
    }

    fn finish_recognition(
        &mut self,
        outcome: OcrOutcome,
        render_path: &str,
        result: Result<OcrResult, araview_core::app_error::AppError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 강조를 켜려던 인식이 성공하지 못하면 켜진 상태를 되돌린다.
        if matches!(outcome, OcrOutcome::Highlight)
            && !result.as_ref().is_ok_and(|result| !result.is_empty())
        {
            self.ocr.request_failed(render_path);
            cx.notify();
        }
        match result {
            Ok(result) if result.is_empty() => {
                window.toast(Toast::warning(t("toast.ocr.none")), cx);
            }
            Ok(result) => {
                let count = result.lines.len();
                match outcome {
                    OcrOutcome::Copy => {
                        cx.write_to_clipboard(ClipboardItem::new_string(result.text()));
                        let message = t_with("toast.ocr.done", &[("count", &count)]);
                        window.toast(Toast::success(message), cx);
                    }
                    OcrOutcome::Highlight => {
                        self.ocr
                            .recognized(Highlight::new(render_path.to_owned(), &result));
                        let message = t_with("toast.ocr.highlighted", &[("count", &count)]);
                        window.toast(Toast::success(message), cx);
                        cx.notify();
                    }
                }
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

#[cfg(test)]
mod tests {
    // `super::*`는 gpui의 `test` 매크로까지 끌어와 `#[test]`를 가린다.
    use araview_core::ocr::{OcrLine, OcrResult, OcrWord};

    use gpui_kit::{point, px};

    use super::{Highlight, OcrRect, OcrState, Orientation, orient_rect};

    const RECT: OcrRect = OcrRect {
        x: 0.1,
        y: 0.2,
        width: 0.3,
        height: 0.1,
    };

    fn close(a: OcrRect, b: OcrRect) -> bool {
        [a.x - b.x, a.y - b.y, a.width - b.width, a.height - b.height]
            .iter()
            .all(|d| d.abs() < 1e-6)
    }

    fn orientation(rotation: u16, flip_h: bool, flip_v: bool) -> Orientation {
        Orientation {
            rotation,
            flip_h,
            flip_v,
        }
    }

    #[test]
    fn identity_keeps_rect() {
        assert!(close(orient_rect(RECT, Orientation::default()), RECT));
    }

    #[test]
    fn clockwise_quarter_turn_moves_top_left_to_top_right() {
        let turned = orient_rect(RECT, orientation(90, false, false));
        let expected = OcrRect {
            x: 0.7,
            y: 0.1,
            width: 0.1,
            height: 0.3,
        };
        assert!(close(turned, expected), "{turned:?}");
    }

    #[test]
    fn half_turn_equals_both_flips() {
        let turned = orient_rect(RECT, orientation(180, false, false));
        let flipped = orient_rect(RECT, orientation(0, true, true));
        assert!(close(turned, flipped));
    }

    #[test]
    fn three_quarter_turn_moves_top_left_to_bottom_left() {
        assert!(close(
            orient_rect(RECT, orientation(270, false, false)),
            OcrRect {
                x: 0.2,
                y: 0.6,
                width: 0.1,
                height: 0.3,
            }
        ));
    }

    #[test]
    fn flips_mirror_the_axis() {
        let horizontal = orient_rect(RECT, orientation(0, true, false));
        assert!(close(horizontal, OcrRect { x: 0.6, ..RECT }));
        let vertical = orient_rect(RECT, orientation(0, false, true));
        assert!(close(vertical, OcrRect { y: 0.7, ..RECT }));
    }

    fn word(text: &str, x: f32) -> OcrWord {
        OcrWord {
            text: text.to_owned(),
            rect: OcrRect {
                x,
                y: 0.1,
                width: 0.1,
                height: 0.05,
            },
        }
    }

    fn highlight() -> Highlight {
        let line = |text: &str, words: Vec<OcrWord>, language: &str| OcrLine {
            text: text.to_owned(),
            rect: RECT,
            words,
            language: language.to_owned(),
        };
        let result = OcrResult {
            lines: vec![
                line(
                    "Hello big world",
                    vec![word("Hello", 0.1), word("big", 0.3), word("world", 0.5)],
                    "en-US",
                ),
                line(
                    "second line",
                    vec![word("second", 0.1), word("line", 0.3)],
                    "en-US",
                ),
                line("日本語", vec![word("日本", 0.1), word("語", 0.3)], "ja"),
            ],
            width: 100,
            height: 100,
        };
        Highlight::new("a.png".to_owned(), &result)
    }

    #[test]
    fn selecting_part_of_a_word_picks_single_characters() {
        // 줄 0: Hello(0-4) big(5-7) world(8-12)
        assert_eq!(highlight().text(1, 3), "ell");
        assert_eq!(highlight().text(3, 1), "ell");
        assert_eq!(highlight().text(5, 7), "big");
    }

    #[test]
    fn selection_across_words_joins_them_with_spaces() {
        assert_eq!(highlight().text(3, 9), "lo big wo");
    }

    #[test]
    fn selecting_a_whole_line_keeps_the_engine_text() {
        assert_eq!(highlight().text(0, 12), "Hello big world");
    }

    #[test]
    fn selecting_across_lines_inserts_a_newline() {
        // 줄 1: second(13-18) line(19-22)
        assert_eq!(highlight().text(10, 17), "rld\nsecon");
    }

    #[test]
    fn spaceless_languages_join_without_spaces() {
        // 줄 2: 日本(23-24) 語(25)
        assert_eq!(highlight().text(24, 25), "本語");
        assert_eq!(highlight().text(23, 25), "日本語");
    }

    #[test]
    fn word_and_line_spans_cover_their_characters() {
        let highlight = highlight();
        assert_eq!(highlight.word_span(6), Some((5, 7)));
        assert_eq!(highlight.line_span(6), Some((0, 12)));
        assert_eq!(highlight.line_span(99), None);
    }

    #[test]
    fn glyph_rects_tile_the_word_rect() {
        let rects = highlight().glyph_rects(Orientation::default());
        assert_eq!(rects.len(), 26);
        assert!((rects[0].x - 0.1).abs() < 1e-6);
        let last = rects[4];
        assert!((last.x + last.width - 0.2).abs() < 1e-6, "{last:?}");
        // 좁은 글자(l)가 넓은 글자(H)보다 좁다.
        assert!(rects[2].width < rects[0].width);
    }

    fn highlight_for(path: &str) -> Highlight {
        let result = OcrResult {
            lines: vec![OcrLine {
                text: "Hello big world".to_owned(),
                rect: RECT,
                words: vec![word("Hello", 0.1), word("big", 0.3), word("world", 0.5)],
                language: "en-US".to_owned(),
            }],
            width: 100,
            height: 100,
        };
        Highlight::new(path.to_owned(), &result)
    }

    /// 강조를 요청한 동안은 켜진 것으로 보이지만 그릴 결과가 없고, 결과가 오면 그린다.
    #[test]
    fn a_requested_highlight_is_on_but_draws_nothing_until_recognized() {
        let mut state = OcrState::default();
        assert!(!state.is_on("a.png"));
        assert!(state.begin_job("a.png".to_owned()));
        state.request("a.png".to_owned());
        assert!(state.is_on("a.png"));
        assert!(state.shown("a.png").is_none());
        assert!(!state.is_on("b.png"));

        state.finish_job();
        state.recognized(highlight_for("a.png"));
        assert!(state.is_on("a.png"));
        assert!(state.shown("a.png").is_some());
        assert!(state.shown("b.png").is_none());
    }

    /// 인식이 실패하거나 글자가 없으면 요청한 강조는 꺼진다.
    #[test]
    fn a_failed_request_turns_the_highlight_off() {
        let mut state = OcrState::default();
        state.request("a.png".to_owned());
        state.request_failed("a.png");
        assert!(!state.is_on("a.png"));
    }

    /// 결과를 기다리는 중에 끄면, 늦게 온 결과는 기억만 하고 보이지 않는다.
    #[test]
    fn a_result_after_turning_off_is_kept_but_not_shown() {
        let mut state = OcrState::default();
        state.request("a.png".to_owned());
        state.hide();
        state.recognized(highlight_for("a.png"));
        assert!(!state.is_on("a.png"));
        // 다시 켜면 인식 없이 결과를 다시 쓴다.
        assert!(state.show_cached("a.png"));
        assert!(state.shown("a.png").is_some());
        assert!(!state.show_cached("b.png"));
    }

    /// 다른 이미지로 넘어가 결과를 버리면 그 이미지의 강조만 꺼진다.
    #[test]
    fn abandoning_a_job_only_turns_off_its_own_image() {
        let mut state = OcrState::default();
        state.request("a.png".to_owned());
        state.abandon("b.png");
        assert!(state.is_on("a.png"));
        state.abandon("a.png");
        assert!(!state.is_on("a.png"));
    }

    /// 인식은 한 번에 하나만 돈다.
    #[test]
    fn only_one_job_runs_at_a_time() {
        let mut state = OcrState::default();
        assert!(state.begin_job("a.png".to_owned()));
        assert!(state.job_running());
        assert!(!state.begin_job("b.png".to_owned()));
        state.finish_job();
        assert!(!state.job_running());
        assert!(state.begin_job("b.png".to_owned()));
    }

    /// 선택은 강조가 보일 때만 생기고, 강조를 끄면 함께 사라진다.
    #[test]
    fn selection_lives_only_while_shown() {
        let mut state = OcrState::default();
        state.request("a.png".to_owned());
        state.press(0, 2, point(px(0.), px(0.)), Some((0, 4)), Some((0, 12)));
        assert_eq!(state.selection(), None);

        state.recognized(highlight_for("a.png"));
        state.press(6, 2, point(px(0.), px(0.)), Some((5, 7)), Some((0, 12)));
        assert_eq!(state.selection(), Some((5, 7)));
        state.press(6, 3, point(px(0.), px(0.)), Some((5, 7)), Some((0, 12)));
        assert_eq!(state.selection(), Some((0, 12)));
        assert!(state.has_selection());

        state.hide();
        assert!(!state.has_selection());
        assert!(state.show_cached("a.png"));
        assert!(!state.has_selection(), "다시 켜면 선택 없이 시작한다");
    }

    /// 한 번 누르면 드래그가 시작되고, 손떨림은 거르며, 끌면 범위가 생긴다.
    #[test]
    fn dragging_selects_from_the_pressed_glyph() {
        let mut state = OcrState::default();
        state.request("a.png".to_owned());
        state.recognized(highlight_for("a.png"));
        state.press(1, 1, point(px(10.), px(10.)), Some((0, 4)), Some((0, 12)));
        assert!(state.dragging());
        assert_eq!(state.selection(), None);
        // 같은 글자 위에서 조금 움직이면 아직 선택이 아니다.
        assert!(!state.extend(1, point(px(11.), px(10.))));
        assert_eq!(state.selection(), None);
        // 같은 글자라도 충분히 끌면 그 글자를 고른다.
        assert!(state.extend(1, point(px(20.), px(10.))));
        assert_eq!(state.selection(), Some((1, 1)));
        assert!(state.extend(3, point(px(30.), px(10.))));
        assert_eq!(state.selection(), Some((1, 3)));
        assert!(!state.extend(3, point(px(31.), px(10.))));

        state.end_drag();
        assert!(!state.dragging());
        assert_eq!(state.selection(), Some((1, 3)));
        assert!(!state.extend(5, point(px(40.), px(10.))));
        assert!(state.clear_selection());
        assert!(!state.clear_selection());
    }

    /// 새 결과가 오면 남아 있던 선택은 풀린다.
    #[test]
    fn a_new_result_clears_the_selection() {
        let mut state = OcrState::default();
        state.request("a.png".to_owned());
        state.recognized(highlight_for("a.png"));
        state.press(6, 2, point(px(0.), px(0.)), Some((5, 7)), Some((0, 12)));
        state.recognized(highlight_for("a.png"));
        assert_eq!(state.selection(), None);
        assert!(state.shown("a.png").is_some());
    }
}
