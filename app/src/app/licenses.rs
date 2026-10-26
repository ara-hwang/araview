//! 오픈소스 라이선스 창: 왼쪽 목록에서 고르고 오른쪽에서 원문을 본다(Tauri 앱과 같은 구성).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::component::alert::Alert;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::AraView;
use crate::dialog::centered;
use crate::i18n::t;
use crate::smooth::SmoothScroll;
use crate::toast::{Toast, WindowToast};

/// `node scripts/generate-license-data.mjs --gpui`가 만든 GPUI 앱의 패키지별 라이선스 데이터.
const LICENSE_DATA: &str = include_str!("../../THIRD_PARTY_LICENSES.json");

const ROW_HEIGHT: f32 = 52.;
const LIST_WIDTH: f32 = 288.;
const BODY_HEIGHT: f32 = 520.;

/// 동봉된 라이선스 문서 폴더. 실행 파일 옆 `licenses/`다(`build.rs`가 빌드 폴더에 모아 둔다).
fn licenses_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("licenses")))
        .unwrap_or_default()
}

/// libheif 등 네이티브 라이브러리의 라이선스 원문(패키지 데이터 JSON은 제외).
fn bundled_documents() -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(licenses_dir()) else {
        return Vec::new();
    };
    let mut documents: Vec<(String, String)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_document = name.ends_with("-copyright.txt");
            let content = is_document
                .then(|| std::fs::read(entry.path()).ok())
                .flatten()?;
            Some((name, String::from_utf8_lossy(&content).into_owned()))
        })
        .collect();
    documents.sort();
    documents
}

#[derive(Clone)]
struct LicenseFile {
    name: String,
    id: String,
}

#[derive(Clone)]
struct Package {
    name: String,
    version: String,
    license: String,
    ecosystem: String,
    /// 라이선스 파일이 없어 표준 문구로 대체한 패키지.
    templated: bool,
    files: Vec<LicenseFile>,
}

struct Bundle {
    packages: Vec<Package>,
    /// 라이선스 파일 id별 원문.
    texts: HashMap<String, String>,
    documents: Vec<(String, String)>,
}

impl Bundle {
    /// 목록 행에 보이는 제목과 부제.
    fn entry_title_subtitle(&self, entry: Entry) -> (String, String) {
        match entry {
            Entry::Document(index) => (
                self.documents[index]
                    .0
                    .trim_end_matches("-copyright.txt")
                    .to_owned(),
                t("settings.licenses.documentsGroup").to_string(),
            ),
            Entry::Package(index) => {
                let package = &self.packages[index];
                let ecosystem = if package.ecosystem == "npm" {
                    "npm"
                } else {
                    "Rust"
                };
                (
                    format!("{} {}", package.name, package.version),
                    format!("{ecosystem} · {}", package.license),
                )
            }
        }
    }
}

fn load_bundle() -> Bundle {
    let data = serde_json::from_str::<serde_json::Value>(LICENSE_DATA).unwrap_or_default();
    let packages = data["packages"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|item| {
                    Some(Package {
                        name: item["name"].as_str()?.to_owned(),
                        version: item["version"].as_str()?.to_owned(),
                        license: item["license"].as_str().unwrap_or("unknown").to_owned(),
                        ecosystem: item["ecosystem"].as_str().unwrap_or("rust").to_owned(),
                        templated: item["templated"].as_bool().unwrap_or(false),
                        files: item["files"]
                            .as_array()
                            .map(|files| {
                                files
                                    .iter()
                                    .filter_map(|file| {
                                        Some(LicenseFile {
                                            name: file["name"].as_str()?.to_owned(),
                                            id: file["id"].as_str()?.to_owned(),
                                        })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let texts = data["texts"]
        .as_object()
        .map(|map| {
            map.iter()
                .filter_map(|(id, text)| Some((id.clone(), text.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    Bundle {
        packages,
        texts,
        documents: bundled_documents(),
    }
}

#[derive(Clone, Copy)]
enum Entry {
    Document(usize),
    Package(usize),
}

struct LicensesView {
    bundle: Arc<Bundle>,
    entries: Vec<Entry>,
    selected: usize,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    list_smooth: SmoothScroll,
    detail_scroll: SmoothScroll,
}

impl LicensesView {
    fn new(bundle: Bundle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let bundle = Arc::new(bundle);
        let entries: Vec<Entry> = (0..bundle.documents.len())
            .map(Entry::Document)
            .chain((0..bundle.packages.len()).map(Entry::Package))
            .collect();
        let scroll = UniformListScrollHandle::new();
        let list_smooth = SmoothScroll::from_handle(scroll.0.borrow().base_handle.clone());
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            bundle,
            entries,
            selected: 0,
            focus,
            scroll,
            list_smooth,
            detail_scroll: SmoothScroll::default(),
        }
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.entries.len() || index == self.selected {
            return;
        }
        self.selected = index;
        // 항목을 바꾸면 원문을 맨 위에서 보여준다.
        self.detail_scroll.reset();
        // 즉시 이동과 휠 감속이 겹치지 않게 감속을 먼저 멈춘다.
        self.list_smooth.cancel();
        self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let count = self.entries.len() as isize;
        if count == 0 {
            return;
        }
        let current = self.selected as isize;
        let next = match key {
            "up" => current - 1,
            "down" => current + 1,
            "home" => 0,
            "end" => count - 1,
            "pageup" => current - 10,
            "pagedown" => current + 10,
            _ => return,
        };
        cx.stop_propagation();
        self.select(next.clamp(0, count - 1) as usize, cx);
    }

    fn row(&self, position: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(entry) = self.entries.get(position).copied() else {
            return div().into_any_element();
        };
        let (title, subtitle) = self.bundle.entry_title_subtitle(entry);
        let active = self.selected == position;
        let theme = cx.theme();
        let (primary, muted) = (theme.primary, theme.muted_foreground);
        div()
            .id(("license-row", position))
            .h(px(ROW_HEIGHT))
            .w_full()
            .py_0()
            .px_3()
            .flex()
            .flex_col()
            .justify_center()
            .cursor_pointer()
            .rounded_md()
            .when(active, |row| row.bg(theme.accent.opacity(0.12)))
            .hover(|style| style.bg(theme.muted.opacity(0.5)))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select(position, cx);
                // 행을 눌러도 목록 포커스가 남아 화살표 키를 계속 쓸 수 있게 한다.
                this.focus.focus(window, cx);
            }))
            .child(
                v_flex()
                    .w_full()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(if active { primary } else { theme.foreground })
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(title),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(subtitle),
                    ),
            )
            .into_any_element()
    }

    /// 고정폭 글꼴로 원문을 보여주는 상자.
    fn text_box(text: String, cx: &App) -> Div {
        div()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted.opacity(0.3))
            .font_family(cx.theme().mono_font_family.clone())
            .text_xs()
            .whitespace_normal()
            .child(text)
    }

    fn detail(&self, cx: &App) -> Div {
        let muted = cx.theme().muted_foreground;
        let note = |id: &'static str, text: SharedString, warning: bool| {
            if warning {
                Alert::warning(id, text).small()
            } else {
                Alert::info(id, text).small()
            }
        };
        match self.entries.get(self.selected).copied() {
            Some(Entry::Document(index)) => {
                Self::text_box(self.bundle.documents[index].1.clone(), cx)
            }
            Some(Entry::Package(index)) => {
                let package = &self.bundle.packages[index];
                v_flex()
                    .gap_3()
                    .child(
                        v_flex()
                            .child(
                                div()
                                    .text_base()
                                    .font_semibold()
                                    .child(format!("{} {}", package.name, package.version)),
                            )
                            .child(div().text_sm().text_color(muted).child(format!(
                                "{}: {}",
                                t("settings.licenses.declared"),
                                package.license
                            ))),
                    )
                    .when(package.templated, |column| {
                        column.child(note(
                            "license-templated-note",
                            t("settings.licenses.templatedNote"),
                            false,
                        ))
                    })
                    .when(package.files.is_empty(), |column| {
                        column.child(note(
                            "license-missing-file",
                            t("settings.licenses.noLicenseFile"),
                            true,
                        ))
                    })
                    .children(package.files.iter().map(|file| {
                        v_flex()
                            .gap_1()
                            .when(package.files.len() > 1, |section| {
                                section.child(
                                    div()
                                        .text_xs()
                                        .font_medium()
                                        .text_color(muted)
                                        .child(file.name.clone()),
                                )
                            })
                            .child(Self::text_box(
                                self.bundle.texts.get(&file.id).cloned().unwrap_or_default(),
                                cx,
                            ))
                    }))
            }
            None => div(),
        }
    }
}

impl Render for LicensesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let list = uniform_list(
            "license-rows",
            self.entries.len(),
            cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                range.map(|row| this.row(row, cx)).collect::<Vec<_>>()
            }),
        )
        .track_scroll(&self.scroll)
        .size_full()
        // 휠은 바깥 SmoothScroll이 처리하고, 목록 자체의 기본 스크롤은 끈다.
        .overflow_hidden();
        let list_scroller = div()
            .id("license-list-scroller")
            .relative()
            .w(px(LIST_WIDTH))
            .h_full()
            .flex_none()
            .border_r_1()
            .border_color(border)
            .on_scroll(self.list_smooth.wheel_handler(false))
            .vertical_scrollbar(&self.scroll)
            .child(list);
        h_flex()
            .h(px(BODY_HEIGHT))
            .border_t_1()
            .border_color(border)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .child(list_scroller)
            .child(
                self.detail_scroll
                    .area("license-detail", div().p_4().child(self.detail(cx)))
                    .min_w_0()
                    .flex_1(),
            )
    }
}

impl AraView {
    pub(super) fn open_licenses(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let task = cx.background_spawn(async { load_bundle() });
        cx.spawn_in(window, async move |_, cx| {
            let bundle = task.await;
            cx.update(|window, cx| {
                if bundle.packages.is_empty() && bundle.documents.is_empty() {
                    window.toast(Toast::error(t("settings.licenses.loadFail")), cx);
                    return;
                }
                let view = cx.new(|cx| LicensesView::new(bundle, window, cx));
                let focus = view.read(cx).focus.clone();
                window.open_dialog(cx, move |dialog, _window, _| {
                    centered(dialog)
                        .title(t("settings.licenses.title"))
                        .width(px(920.))
                        .child(view.clone())
                });
                // 다이얼로그를 연 직후 목록에 포커스를 준다. 화살표 키로 바로 고를 수 있다.
                focus.focus(window, cx);
            })
            .ok();
        })
        .detach();
    }
}
