//! 정보 패널: 만화 정보, 파일 상세, 히스토그램, EXIF(SPEC §10).

use std::collections::HashMap;
use std::path::Path;

use araview_core::comic_info::ComicInfo;
use araview_core::image_info::{Histogram, IccStatus, ImageDetails};
use araview_core::ops;
use gpui_kit::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{AraView, format_bytes};
use crate::i18n::{t, t_with};
use crate::settings::SettingsStore;

pub(super) const PANEL_WIDTH: f32 = 320.0;
const HISTOGRAM_HEIGHT: f32 = 96.0;

const EXIF_CATEGORIES: &[(&str, &[&str])] = &[
    (
        "Camera",
        &["Make", "Model", "LensModel", "LensMake", "BodySerialNumber"],
    ),
    (
        "Exposure",
        &[
            "ExposureTime",
            "FNumber",
            "PhotographicSensitivity",
            "ISOSpeedRatings",
            "ExposureBiasValue",
            "ExposureProgram",
            "ExposureMode",
            "MeteringMode",
            "Flash",
            "WhiteBalance",
        ],
    ),
    (
        "Image",
        &[
            "ImageWidth",
            "ImageLength",
            "PixelXDimension",
            "PixelYDimension",
            "Orientation",
            "ColorSpace",
            "BitsPerSample",
            "Compression",
        ],
    ),
    (
        "Lens",
        &["FocalLength", "FocalLengthIn35mmFilm", "MaxApertureValue"],
    ),
    (
        "DateTime",
        &[
            "DateTime",
            "DateTimeOriginal",
            "DateTimeDigitized",
            "OffsetTime",
            "OffsetTimeOriginal",
        ],
    ),
    (
        "GPS",
        &[
            "GPSLatitude",
            "GPSLatitudeRef",
            "GPSLongitude",
            "GPSLongitudeRef",
            "GPSAltitude",
            "GPSAltitudeRef",
        ],
    ),
    (
        "Software",
        &[
            "Software",
            "ProcessingSoftware",
            "ImageDescription",
            "Copyright",
        ],
    ),
];

/// 한 섹션의 조회 상태. 읽는 중에는 "없음"으로 표시하지 않는다.
#[derive(Default)]
pub(super) enum Section<T> {
    #[default]
    Loading,
    Ready(T),
    Failed,
}

#[derive(Default)]
pub(super) struct InfoData {
    exif: Section<HashMap<String, String>>,
    details: Section<ImageDetails>,
    histogram: Section<Histogram>,
}

/// `PixelXDimension` → `Pixel X Dimension`.
fn format_tag_name(tag: &str) -> String {
    let mut out = String::with_capacity(tag.len() + 4);
    for ch in tag.chars() {
        if ch.is_ascii_uppercase() && !out.is_empty() {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

/// 유닉스 초를 로컬 시각 문자열로 바꾼다.
fn format_unix(seconds: i64) -> String {
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn FileTimeToSystemTime(file_time: *const u64, system_time: *mut SystemTime) -> i32;
        fn SystemTimeToTzSpecificLocalTime(
            time_zone: *const core::ffi::c_void,
            universal: *const SystemTime,
            local: *mut SystemTime,
        ) -> i32;
    }
    // FILETIME은 1601-01-01부터의 100ns 단위다.
    const UNIX_EPOCH_AS_FILETIME: i64 = 116_444_736_000_000_000;
    let Some(ticks) = seconds
        .checked_mul(10_000_000)
        .and_then(|value| value.checked_add(UNIX_EPOCH_AS_FILETIME))
        .and_then(|value| u64::try_from(value).ok())
    else {
        return String::new();
    };
    let mut utc = SystemTime::default();
    let mut local = SystemTime::default();
    // SAFETY: 두 호출 모두 유효한 스택 값의 포인터만 받으며, 실패하면 0을 돌려준다.
    // FILETIME은 u64와 크기·정렬이 같은 두 DWORD 구조체다.
    let ok = unsafe {
        FileTimeToSystemTime(&ticks, &mut utc) != 0
            && SystemTimeToTzSpecificLocalTime(core::ptr::null(), &utc, &mut local) != 0
    };
    if !ok {
        return String::new();
    }
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        local.year, local.month, local.day, local.hour, local.minute, local.second
    )
}

impl AraView {
    pub(super) fn toggle_info(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.info_open = !self.info_open;
        if self.info_open {
            self.load_info(window, cx);
        }
        cx.notify();
    }

    /// 패널이 열려 있으면 현재 이미지의 정보를 다시 읽는다. 패널은 데이터를
    /// 기다리지 않고 바로 열리며 섹션은 도착하는 대로 채운다.
    pub(super) fn load_info(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.info_open {
            return;
        }
        let Some(info) = &self.info else {
            return;
        };
        self.info_data = InfoData::default();
        self.info_seq += 1;
        let seq = self.info_seq;
        // 히스토그램은 렌더 바이트, EXIF와 파일 상세는 원본 기준이다(SPEC §9.4).
        let render_path = info.file_path.clone();
        let source_path = info.source_path.clone();

        let exif_source = source_path.clone();
        let exif = cx.background_spawn(async move { ops::get_exif_data_impl(&exif_source) });
        cx.spawn_in(window, async move |this, cx| {
            let result = exif.await;
            this.update(cx, |this, cx| {
                if this.info_seq == seq {
                    this.info_data.exif = match result {
                        Ok(map) => Section::Ready(map),
                        Err(_) => Section::Failed,
                    };
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();

        let details = cx.background_spawn(async move {
            araview_core::image_info::details_for_path(Path::new(&source_path))
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = details.await;
            this.update(cx, |this, cx| {
                if this.info_seq == seq {
                    this.info_data.details = match result {
                        Ok(details) => Section::Ready(details),
                        Err(_) => Section::Failed,
                    };
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();

        let histogram = cx.background_spawn(async move {
            araview_core::image_info::histogram_for_path(Path::new(&render_path))
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = histogram.await;
            this.update(cx, |this, cx| {
                if this.info_seq == seq {
                    this.info_data.histogram = match result {
                        Ok(histogram) => Section::Ready(histogram),
                        Err(_) => Section::Failed,
                    };
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn render_info_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let show_comic = SettingsStore::global(cx).settings.show_comic_info;
        let loading = matches!(self.info_data.exif, Section::Loading)
            || matches!(self.info_data.details, Section::Loading);

        v_flex()
            .id("info-panel")
            .w(px(PANEL_WIDTH))
            .h_full()
            .flex_none()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .p_3()
                    .gap_1()
                    .child(div().text_sm().font_semibold().child(t("exif.title")))
                    .child(div().text_xs().text_color(muted).child(t("exif.desc"))),
            )
            .child(
                self.info_scroll
                    .area(
                        "info-scroll",
                        v_flex()
                            .px_3()
                            .pb_3()
                            .gap_4()
                            .when(loading, |panel| {
                                panel.child(
                                    div().text_xs().text_color(muted).child(t("exif.loading")),
                                )
                            })
                            .when_some(
                                self.archive
                                    .as_ref()
                                    .filter(|_| show_comic)
                                    .and(self.comic.as_ref()),
                                |panel, comic| panel.child(self.render_comic(comic, cx)),
                            )
                            .when(
                                self.archive.is_some() && show_comic && self.comic_failed,
                                |panel| {
                                    panel.child(section(
                                        t("comic.section"),
                                        div()
                                            .text_xs()
                                            .text_color(muted)
                                            .child(t("comic.loadFail")),
                                        cx,
                                    ))
                                },
                            )
                            .child(self.render_details(cx))
                            .child(self.render_histogram(cx))
                            .children(self.render_exif(cx)),
                    )
                    .flex_1()
                    .min_h_0(),
            )
    }

    fn render_comic(&self, comic: &ComicInfo, cx: &mut Context<Self>) -> AnyElement {
        let heading = match (&comic.series, &comic.number) {
            (Some(series), Some(number)) => Some(format!("{series} #{number}")),
            (Some(series), None) => Some(series.clone()),
            (None, Some(number)) => Some(format!("#{number}")),
            (None, None) => None,
        };
        let published = comic.year.map(|year| match (comic.month, comic.day) {
            (Some(month), Some(day)) => format!("{year}-{month:02}-{day:02}"),
            (Some(month), None) => format!("{year}-{month:02}"),
            _ => year.to_string(),
        });
        let direction = match comic.manga.as_deref() {
            Some("YesAndRightToLeft") => Some(t("comic.directionRtl")),
            Some("No") => Some(t("comic.directionLtr")),
            _ => None,
        };
        let number = |value: Option<i32>| value.map(|value| value.to_string());
        let fields: Vec<(&str, Option<String>)> = vec![
            ("comic.writer", comic.writer.clone()),
            ("comic.penciller", comic.penciller.clone()),
            ("comic.inker", comic.inker.clone()),
            ("comic.colorist", comic.colorist.clone()),
            ("comic.letterer", comic.letterer.clone()),
            ("comic.coverArtist", comic.cover_artist.clone()),
            ("comic.editor", comic.editor.clone()),
            ("comic.publisher", comic.publisher.clone()),
            ("comic.published", published),
            ("comic.direction", direction.map(|text| text.to_string())),
            ("comic.genre", comic.genre.clone()),
            ("comic.tags", comic.tags.clone()),
            ("comic.volume", number(comic.volume)),
            ("comic.count", number(comic.count)),
            ("comic.pageCount", number(comic.page_count)),
            ("comic.language", comic.language_iso.clone()),
            ("comic.ageRating", comic.age_rating.clone()),
            ("comic.rating", comic.community_rating.clone()),
        ];
        let body = v_flex()
            .gap_1()
            .when_some(heading, |body, text| {
                body.child(div().text_sm().font_semibold().child(text))
            })
            .when_some(comic.title.clone(), |body, title| {
                body.child(div().text_sm().child(title))
            })
            .children(
                fields
                    .into_iter()
                    .filter_map(|(key, value)| value.map(|value| row(t(key), value, cx))),
            )
            .when_some(comic.summary.clone(), |body, summary| {
                body.child(div().pt_1().text_xs().whitespace_normal().child(summary))
            });
        section(t("comic.section"), body, cx)
    }

    fn render_details(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let details = match &self.info_data.details {
            Section::Loading => return div().into_any_element(),
            Section::Failed => {
                return section(
                    t("details.title"),
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(t("details.unavailable")),
                    cx,
                );
            }
            Section::Ready(details) => details,
        };
        let path = match (&self.archive, self.list.get(self.index)) {
            (Some(archive), Some(entry)) => format!("{archive}\n{entry}"),
            _ => details.file_path.clone(),
        };
        let dimensions = details.width.zip(details.height).map(|(w, h)| {
            let pixels = u64::from(w) * u64::from(h);
            format!(
                "{w} × {h} ({})",
                t_with("details.pixels", &[("count", &group_digits(pixels))])
            )
        });
        let color_key = match details.color_mode.as_str() {
            "rgb" => "details.colorRgb",
            "rgba" => "details.colorRgba",
            "grayscale" => "details.colorGrayscale",
            "grayscale-alpha" => "details.colorGrayscaleAlpha",
            "vector" => "details.colorVector",
            _ => "details.colorUnknown",
        };
        let color = match details.bits_per_channel {
            Some(bits) => format!(
                "{}, {}",
                t(color_key),
                t_with("details.bitsPerChannel", &[("bits", &bits)])
            ),
            None => t(color_key).to_string(),
        };
        let dpi = details.dpi_x.zip(details.dpi_y).map(|(x, y)| {
            if (x - y).abs() < 0.5 {
                format!("{} DPI", x.round() as i64)
            } else {
                format!("{} × {} DPI", x.round() as i64, y.round() as i64)
            }
        });
        let icc = match details.icc_status {
            IccStatus::Present => {
                let size = format_bytes(details.icc_bytes.unwrap_or(0));
                match &details.icc_name {
                    Some(name) => t_with("details.iccPresent", &[("name", name), ("size", &size)]),
                    None => t_with("details.iccPresentNoName", &[("size", &size)]),
                }
            }
            IccStatus::Absent => t("details.iccAbsent"),
            IccStatus::Unchecked => t("details.iccUnchecked"),
        };
        // 아카이브 엔트리의 시각은 추출 시각이라 의미가 없어 숨긴다.
        let show_times = self.archive.is_none();
        let body = v_flex()
            .gap_1()
            .child(row(t("details.path"), path, cx))
            .child(row(t("details.size"), format_bytes(details.file_size), cx))
            .when_some(dimensions, |body, text| {
                body.child(row(t("details.dimensions"), text, cx))
            })
            .when_some(details.created_unix.filter(|_| show_times), |body, at| {
                body.child(row(t("details.created"), format_unix(at), cx))
            })
            .when_some(details.modified_unix.filter(|_| show_times), |body, at| {
                body.child(row(t("details.modified"), format_unix(at), cx))
            })
            .child(row(t("details.color"), color, cx))
            .when_some(dpi, |body, text| {
                body.child(row(t("details.dpi"), text, cx))
            })
            .child(row(t("details.icc"), icc.to_string(), cx));
        section(t("details.title"), body, cx)
    }

    fn render_histogram(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let histogram = match &self.info_data.histogram {
            Section::Loading => return div().into_any_element(),
            Section::Failed => {
                let key = if self
                    .picture
                    .as_ref()
                    .is_some_and(|picture| picture.is_vector)
                {
                    "histogram.vectorUnavailable"
                } else {
                    "histogram.unavailable"
                };
                return section(
                    t("histogram.title"),
                    div().text_xs().text_color(muted).child(t(key)),
                    cx,
                );
            }
            Section::Ready(histogram) => histogram,
        };
        let channels = [
            (histogram.r.clone(), rgb(0xef4444)),
            (histogram.g.clone(), rgb(0x22c55e)),
            (histogram.b.clone(), rgb(0x3b82f6)),
        ];
        let peak = channels
            .iter()
            .flat_map(|(bins, _)| bins.iter().copied())
            .max()
            .unwrap_or(0)
            .max(1) as f32;
        let chart = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let step = bounds.size.width / 256.0;
                for (bins, color) in &channels {
                    let mut builder = PathBuilder::fill();
                    builder.move_to(point(bounds.left(), bounds.bottom()));
                    for (ix, count) in bins.iter().enumerate() {
                        let height = bounds.size.height * (*count as f32 / peak);
                        builder.line_to(point(
                            bounds.left() + step * (ix as f32 + 0.5),
                            bounds.bottom() - height,
                        ));
                    }
                    builder.line_to(point(bounds.right(), bounds.bottom()));
                    builder.close();
                    if let Ok(path) = builder.build() {
                        let mut fill: Hsla = (*color).into();
                        fill.a = 0.45;
                        window.paint_path(path, fill);
                    }
                }
            },
        )
        .w_full()
        .h(px(HISTOGRAM_HEIGHT));
        section(
            t("histogram.title"),
            div()
                .rounded_sm()
                .overflow_hidden()
                .bg(cx.theme().muted)
                .child(chart),
            cx,
        )
    }

    fn render_exif(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let muted = cx.theme().muted_foreground;
        let exif = match &self.info_data.exif {
            Section::Loading => return Vec::new(),
            Section::Failed => {
                return vec![
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(t("exif.loadFail"))
                        .into_any_element(),
                ];
            }
            Section::Ready(exif) => exif,
        };
        if exif.is_empty() {
            return vec![
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(t("exif.empty"))
                    .into_any_element(),
            ];
        }
        let mut sections = Vec::new();
        let mut categorized: Vec<&str> = Vec::new();
        for (category, tags) in EXIF_CATEGORIES {
            let entries: Vec<(&str, &String)> = tags
                .iter()
                .filter_map(|tag| exif.get(*tag).map(|value| (*tag, value)))
                .collect();
            if entries.is_empty() {
                continue;
            }
            categorized.extend(entries.iter().map(|(tag, _)| *tag));
            sections.push(section(
                SharedString::new_static(category),
                v_flex().gap_1().children(
                    entries
                        .into_iter()
                        .map(|(tag, value)| row(format_tag_name(tag).into(), value.clone(), cx)),
                ),
                cx,
            ));
        }
        let mut others: Vec<(&String, &String)> = exif
            .iter()
            .filter(|(tag, _)| !categorized.contains(&tag.as_str()))
            .collect();
        others.sort();
        if !others.is_empty() {
            sections.push(section(
                t("exif.other"),
                v_flex().gap_1().children(
                    others
                        .into_iter()
                        .map(|(tag, value)| row(format_tag_name(tag).into(), value.clone(), cx)),
                ),
                cx,
            ));
        }
        sections
    }
}

fn section(title: SharedString, body: impl IntoElement, cx: &App) -> AnyElement {
    v_flex()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(cx.theme().muted_foreground)
                .child(title),
        )
        .child(body)
        .into_any_element()
}

fn row(label: SharedString, value: String, cx: &App) -> AnyElement {
    h_flex()
        .items_start()
        .gap_2()
        .text_xs()
        .child(
            div()
                .w(px(96.))
                .flex_none()
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(div().flex_1().min_w_0().whitespace_normal().child(value))
        .into_any_element()
}

/// `1234567` → `1,234,567`.
fn group_digits(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (ix, ch) in digits.chars().enumerate() {
        if ix > 0 && (digits.len() - ix).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
