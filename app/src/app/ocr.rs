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

impl AraView {
    /// 지금 보는 이미지에 글자 강조가 켜져 있는지.
    pub(super) fn ocr_visible(&self) -> bool {
        match (&self.ocr_shown, &self.info) {
            (Some(shown), Some(info)) => *shown == info.file_path,
            _ => false,
        }
    }

    /// 지금 보는 이미지의 인식 결과. 강조가 꺼져 있거나 다른 이미지면 없다.
    pub(super) fn ocr_current(&self) -> Option<&Highlight> {
        let info = self.info.as_ref()?;
        self.ocr_visible()
            .then_some(())
            .and(self.ocr_highlight.as_ref())
            .filter(|highlight| highlight.path() == info.file_path)
    }

    /// 선택한 글자. 선택이 없으면 `None`.
    pub(super) fn ocr_selected_text(&self) -> Option<String> {
        let (anchor, head) = self.ocr_select?;
        let text = self.ocr_current()?.text(anchor, head);
        (!text.is_empty()).then_some(text)
    }

    pub(super) fn clear_ocr_selection(&mut self, cx: &mut Context<Self>) {
        self.ocr_drag = None;
        if self.ocr_select.take().is_some() {
            cx.notify();
        }
    }

    /// 글자 강조를 켜고 끈다. 이 이미지를 처음 켤 때만 글자를 인식하고, 이후에는 결과를
    /// 다시 쓴다.
    pub(super) fn toggle_text_highlight(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocr_visible() {
            self.ocr_shown = None;
            self.clear_ocr_selection(cx);
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
        self.ocr_select = None;
        if self
            .ocr_highlight
            .as_ref()
            .is_some_and(|highlight| highlight.path() == path)
        {
            self.ocr_shown = Some(path);
            cx.notify();
            return;
        }
        let busy = self.ocr_job.is_some();
        self.ocr_shown = Some(path);
        self.recognize_text(OcrOutcome::Highlight, window, cx);
        // 시작하지 못했다(미지원, 다른 인식 진행 중)면 켜진 것으로 두지 않는다.
        if busy || self.ocr_job.is_none() {
            self.ocr_shown = None;
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
        match clicks {
            0 | 1 => {
                self.ocr_drag = Some((glyph, position));
                self.ocr_select = None;
            }
            2 => {
                self.ocr_drag = None;
                self.ocr_select = word;
            }
            _ => {
                self.ocr_drag = None;
                self.ocr_select = line;
            }
        }
        cx.notify();
    }

    /// 드래그 중인 선택의 끝을 글자 `glyph`로 옮긴다.
    pub(super) fn ocr_extend(
        &mut self,
        glyph: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some((anchor, from)) = self.ocr_drag else {
            return;
        };
        // 글자 하나 위에서 끌어도 그 글자가 골라지게 하되, 클릭 중의 손떨림은 거른다.
        let moved =
            f32::from(position.x - from.x).hypot(f32::from(position.y - from.y)) >= DRAG_SLOP;
        let next =
            (anchor != glyph || moved || self.ocr_select.is_some()).then_some((anchor, glyph));
        if next != self.ocr_select {
            self.ocr_select = next;
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
        if !platform::OCR_SUPPORTED {
            window.toast(Toast::warning(t("toast.ocr.unsupported")), cx);
            return;
        }
        if !self.current_page_is_full() {
            window.toast(Toast::warning(t("toast.ocr.empty")), cx);
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
                    this.finish_recognition(outcome, &render_path, result, window, cx);
                } else if this.ocr_shown.as_deref() == Some(render_path.as_str()) {
                    this.ocr_shown = None;
                }
            })
            .ok();
        })
        .detach();
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
            self.ocr_shown = None;
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
                        self.ocr_highlight = Some(Highlight::new(render_path.to_owned(), &result));
                        self.ocr_select = None;
                        self.ocr_drag = None;
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

    use super::{Highlight, OcrRect, Orientation, orient_rect};

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
}
