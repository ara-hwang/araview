//! 헤드리스 창에서 실제 입력 경로(키, 휠, 드래그, 드롭, 다이얼로그)를 거치는 UI 통합 테스트.

use std::path::{Path, PathBuf};
use std::sync::Once;

use gpui_kit::component::{Root, WindowExt as _};
use gpui_kit::{
    AppContext as _, ClipboardEntry, Entity, ExternalPaths, FileDropEvent, KeyDownEvent, Keystroke,
    Modifiers, MouseButton, ParentElement as _, Pixels, Point, ScrollDelta, ScrollWheelEvent,
    TestAppContext, TouchPhase, VisualTestContext, point, px,
};

use super::AraView;
use super::settings_panel::SettingsPanel;
use crate::settings::{ArchiveProgress, SettingsStore, ViewMode};
use crate::toast::{TOAST_DURATION, Toast, WindowToast};

fn sample(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../samples")
        .join(name)
        .canonicalize()
        .expect(name)
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_owned()
}

fn open_app(cx: &mut TestAppContext) -> (Entity<AraView>, &mut VisualTestContext) {
    static CACHE: Once = Once::new();
    CACHE.call_once(|| {
        araview_core::process_temp::initialize_temporary_fallback().expect("cache root");
    });
    cx.update(|cx| {
        gpui_kit::init(cx);
        // 경로 없이 만들어 디스크에 쓰지 않는다.
        cx.set_global(SettingsStore::load(None));
    });
    let mut view = None;
    let (_root, cx) = cx.add_window_view(|window, cx| {
        let app = cx.new(|cx| AraView::new(None, None, window, cx));
        view = Some(app.clone());
        Root::new(app, window, cx)
    });
    (view.expect("view"), cx)
}

/// Windows 캡션 버튼(최소화/최대화/닫기) 3개가 차지하는 폭.
const CAPTION_CONTROLS_WIDTH: f32 = 3.0 * 34.0;
const HEADER_BUTTON_MIN_WIDTH: f32 = 28.0;

fn header_viewport_width(cx: &mut VisualTestContext) -> f32 {
    f32::from(cx.update(|window, _| window.viewport_size().width))
}

#[gpui_kit::test]
fn compact_header_keeps_menus_within_caption_area(cx: &mut TestAppContext) {
    let (view, cx) = open_app(cx);
    cx.simulate_resize(gpui_kit::size(px(500.), px(400.)));
    open(&view, sample("sample.jpg"), cx);

    let left = cx.debug_bounds("header-left-menu").expect("left menu");
    let right = cx.debug_bounds("header-right-menu").expect("right menu");
    let right_edge = f32::from(right.right());

    assert!(f32::from(left.left()) >= 0.0);
    assert!(
        right_edge <= header_viewport_width(cx) - CAPTION_CONTROLS_WIDTH,
        "right menu bounds: {right:?}"
    );
    assert!(right_edge > f32::from(left.right()));
    // 보이는 그룹은 버튼 폭만큼 자리를 유지해야 한다(압축돼 사라지지 않음).
    assert!(f32::from(left.size.width) >= 2.0 * HEADER_BUTTON_MIN_WIDTH);
    assert!(
        f32::from(right.size.width) >= 4.0 * HEADER_BUTTON_MIN_WIDTH,
        "right menu bounds: {right:?}"
    );
}

#[gpui_kit::test]
fn wide_header_pushes_right_menu_to_the_edge(cx: &mut TestAppContext) {
    let (view, cx) = open_app(cx);
    cx.simulate_resize(gpui_kit::size(px(1400.), px(600.)));
    open(&view, sample("sample.jpg"), cx);

    let info = cx.debug_bounds("header-center-info").expect("info");
    let right = cx.debug_bounds("header-right-menu").expect("right menu");
    let viewport = header_viewport_width(cx);

    // 우측 메뉴는 캡션 버튼 바로 앞(그립 포함 여유 안)에 붙고, 정보 버튼과는 떨어져 있다.
    assert!(
        f32::from(right.right()) >= viewport - CAPTION_CONTROLS_WIDTH - 80.0,
        "right menu bounds: {right:?}"
    );
    assert!(f32::from(right.left()) - f32::from(info.right()) > 100.0);
}

#[gpui_kit::test]
fn header_groups_hide_in_order_as_window_narrows(cx: &mut TestAppContext) {
    let (view, cx) = open_app(cx);
    open(&view, sample("sample.jpg"), cx);

    let has = |cx: &mut VisualTestContext, id: &'static str| cx.debug_bounds(id).is_some();
    for (width, zoom, modes, transform) in [
        (1100.0, true, true, true),
        (900.0, true, true, false),
        (700.0, true, false, false),
        (600.0, false, false, false),
    ] {
        cx.simulate_resize(gpui_kit::size(px(width), px(500.)));
        cx.run_until_parked();
        assert_eq!(has(cx, "header-zoom"), zoom, "zoom at {width}");
        assert_eq!(has(cx, "header-view-modes"), modes, "modes at {width}");
        assert_eq!(
            has(cx, "header-transform"),
            transform,
            "transform at {width}"
        );
        assert!(has(cx, "header-left-menu") && has(cx, "header-right-menu"));
    }
}

/// 백그라운드 작업과 그 결과로 생긴 재렌더를 끝까지 돌린다.
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..4 {
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    cx.run_until_parked();
}

/// 웹툰의 부드러운 스크롤이 목표 위치에 닿을 때까지 프레임을 진행한다.
fn finish_webtoon_scroll(view: &Entity<AraView>, cx: &mut VisualTestContext) {
    for _ in 0..600 {
        cx.update(|window, cx| window.simulate_next_frame(cx));
        settle(cx);
        if !view.read_with(cx, |this, _| this.wt_animating) {
            return;
        }
        // 감속은 실제 경과 시간을 쓰므로 프레임 사이에 시간을 흘려보낸다.
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    panic!("webtoon scroll did not settle");
}

fn open(view: &Entity<AraView>, path: String, cx: &mut VisualTestContext) {
    view.update_in(cx, |this, window, cx| this.open_path(path, window, cx));
    settle(cx);
}

fn run(view: &Entity<AraView>, action: &'static str, cx: &mut VisualTestContext) {
    view.update_in(cx, |this, window, cx| this.run_action(action, window, cx));
    settle(cx);
}

fn viewport_center(view: &Entity<AraView>, cx: &mut VisualTestContext) -> Point<Pixels> {
    view.read_with(cx, |this, _| this.viewport.get().center())
}

fn wheel(position: Point<Pixels>, lines: f32, control: bool, cx: &mut VisualTestContext) {
    cx.simulate_event(ScrollWheelEvent {
        position,
        delta: ScrollDelta::Lines(point(0., lines)),
        modifiers: Modifiers {
            control,
            ..Modifiers::default()
        },
        touch_phase: TouchPhase::Moved,
    });
    settle(cx);
}

/// 제어 브리지에 요청 하나를 보내고 응답(JSON)을 돌려준다.
fn send_control(
    view: &Entity<AraView>,
    request: serde_json::Value,
    cx: &mut VisualTestContext,
) -> serde_json::Value {
    let reply = cx.update(|window, cx| super::bridge::handle_control(&request, window, cx, view));
    serde_json::from_str(&reply).unwrap_or_else(|error| panic!("control reply {reply}: {error}"))
}

/// 저장소 샘플을 건드리지 않게 임시 폴더에 복사해 쓴다.
fn temp_copy(names: &[&str]) -> (tempfile::TempDir, Vec<String>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = names
        .iter()
        .map(|name| {
            let target = dir.path().join(name);
            std::fs::copy(sample(name), &target).expect("copy sample");
            target.to_string_lossy().into_owned()
        })
        .collect();
    (dir, paths)
}

/// 세로 5장과 가로 1장(4번째)으로 된 폴더. 세로 장의 높이는 `tall`이다.
fn page_folder(tall: u32) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("temp dir");
    for index in 0..6u8 {
        let (width, height) = if index == 3 { (240, 160) } else { (120, tall) };
        image::RgbImage::from_pixel(width, height, image::Rgb([40 * index, 90, 200]))
            .save(dir.path().join(format!("p{}.png", index + 1)))
            .expect("write page");
    }
    let first = dir.path().join("p1.png").to_string_lossy().into_owned();
    (dir, first)
}

#[gpui_kit::test]
fn opening_a_file_lists_its_folder_and_keys_navigate(cx: &mut TestAppContext) {
    let (view, cx) = open_app(cx);
    open(&view, sample("sample.jpg"), cx);
    let start = view.read_with(cx, |this, _| {
        assert!(this.picture.is_some());
        assert!(this.list.len() > 10);
        assert!(this.list[this.index].ends_with("sample.jpg"));
        this.index
    });

    cx.simulate_keystrokes("right");
    settle(cx);
    assert_eq!(view.read_with(cx, |this, _| this.index), start + 1);
    cx.simulate_keystrokes("left");
    settle(cx);
    assert_eq!(view.read_with(cx, |this, _| this.index), start);
    cx.simulate_keystrokes("end");
    settle(cx);
    view.read_with(cx, |this, _| assert_eq!(this.index, this.list.len() - 1));
    cx.simulate_keystrokes("home");
    settle(cx);
    assert_eq!(view.read_with(cx, |this, _| this.index), 0);
    cx.simulate_keystrokes("pagedown");
    settle(cx);
    assert_eq!(view.read_with(cx, |this, _| this.index), 10);
}

#[gpui_kit::test]
fn ctrl_shortcuts_copy_the_path_and_the_image(cx: &mut TestAppContext) {
    let (view, cx) = open_app(cx);
    let path = sample("sample.png");
    open(&view, path.clone(), cx);

    cx.simulate_keystrokes("ctrl-shift-c");
    settle(cx);
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some(path.as_str()));

    cx.simulate_keystrokes("ctrl-c");
    settle(cx);
    let item = cx.read_from_clipboard().expect("clipboard item");
    assert!(
        item.entries()
            .iter()
            .any(|entry| matches!(entry, ClipboardEntry::Image(_)))
    );
}

#[gpui_kit::test]
fn wheel_navigates_and_ctrl_wheel_zooms_about_the_cursor(cx: &mut TestAppContext) {
    let (view, cx) = open_app(cx);
    open(&view, sample("sample.jpg"), cx);
    let center = viewport_center(&view, cx);
    let (index, zoom) = view.read_with(cx, |this, _| (this.index, this.zoom));

    wheel(center, -1.0, false, cx);
    assert_eq!(view.read_with(cx, |this, _| this.index), index + 1);
    wheel(center, 1.0, false, cx);
    assert_eq!(view.read_with(cx, |this, _| this.index), index);

    // 커서가 가운데면 위치는 그대로고 배율만 바뀐다.
    wheel(center, 1.0, true, cx);
    view.read_with(cx, |this, _| {
        assert!((this.zoom - zoom * 1.25).abs() < 1e-4);
        assert!(!this.fit_locked);
    });
    wheel(center, -1.0, true, cx);
    view.read_with(cx, |this, _| assert!((this.zoom - zoom).abs() < 1e-4));
}

#[gpui_kit::test]
fn dragging_pans_a_zoomed_image_within_its_bounds(cx: &mut TestAppContext) {
    let (view, cx) = open_app(cx);
    open(&view, sample("sample.jpg"), cx);
    for _ in 0..9 {
        cx.simulate_keystrokes("=");
    }
    settle(cx);
    let center = viewport_center(&view, cx);
    let moved = center + point(px(40.), px(-25.));
    cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(moved, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(moved, MouseButton::Left, Modifiers::default());
    settle(cx);
    view.read_with(cx, |this, _| {
        assert_eq!((this.position.x, this.position.y), (40.0, -25.0));
        assert!(this.drag.is_none());
    });

    // 키보드 팬은 48px씩 움직인다.
    cx.simulate_keystrokes("ctrl-left");
    settle(cx);
    view.read_with(cx, |this, _| assert_eq!(this.position.x, 88.0));
}

#[gpui_kit::test]
fn double_click_toggles_fullscreen(cx: &mut TestAppContext) {
    let (view, cx) = open_app(cx);
    open(&view, sample("sample.jpg"), cx);
    let center = viewport_center(&view, cx);
    cx.simulate_event(gpui_kit::MouseDownEvent {
        button: MouseButton::Left,
        position: center,
        modifiers: Modifiers::default(),
        click_count: 2,
        first_mouse: false,
    });
    settle(cx);
    assert!(cx.update(|window, _| window.is_fullscreen()));
}

#[gpui_kit::test]
fn dropping_a_file_opens_it(cx: &mut TestAppContext) {
    let (view, cx) = open_app(cx);
    settle(cx);
    let position = point(px(200.), px(200.));
    cx.simulate_event(FileDropEvent::Entered {
        position,
        paths: ExternalPaths(smallvec::smallvec![PathBuf::from(sample("sample.bmp"))]),
    });
    cx.simulate_event(FileDropEvent::Submit { position });
    settle(cx);
    view.read_with(cx, |this, _| {
        assert!(this.picture.is_some());
        assert!(this.list[this.index].ends_with("sample.bmp"));
    });
}

#[gpui_kit::test]
fn right_click_targets_a_page_and_cover_toggle_rewrites_comic_info(cx: &mut TestAppContext) {
    let (_dir, paths) = temp_copy(&["sample-comicinfo.cbz"]);
    let (view, cx) = open_app(cx);
    cx.update(|_, cx| {
        SettingsStore::update(cx, |settings| settings.view_mode = ViewMode::Single);
    });
    open(&view, paths[0].clone(), cx);
    run(&view, "viewSingle", cx);

    let center = viewport_center(&view, cx);
    cx.simulate_mouse_down(center, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(center, MouseButton::Right, Modifiers::default());
    settle(cx);
    let (page, was_cover) = view.read_with(cx, |this, cx| {
        let page = this.context_page.expect("context page");
        assert_eq!(page, this.index);
        (page, this.cover(cx).pages().contains(&page))
    });

    view.update_in(cx, |this, window, cx| this.toggle_cover(page, window, cx));
    settle(cx);
    view.read_with(cx, |this, cx| {
        assert_eq!(this.cover(cx).pages().contains(&page), !was_cover);
    });
    // 파일에 실제로 쓰였는지 다시 읽어 확인한다.
    let reread = araview_core::ops::get_comic_info_blocking(&paths[0])
        .expect("comic info")
        .expect("comic info present");
    let is_cover = crate::layout::explicit_cover_pages(Some(&reread), 5).contains(&page);
    assert_eq!(is_cover, !was_cover);
}

#[gpui_kit::test]
fn palette_runs_the_typed_command(cx: &mut TestAppContext) {
    let (view, cx) = open_app(cx);
    open(&view, sample("sample.jpg"), cx);
    let zoom = view.read_with(cx, |this, _| this.zoom);

    run(&view, "togglePalette", cx);
    assert!(cx.update(|window, cx| window.has_active_dialog(cx)));
    // 한국어 UI에서도 영문 별칭으로 찾는다.
    cx.simulate_input("zoom in");
    settle(cx);
    cx.simulate_keystrokes("enter");
    settle(cx);

    assert!(!cx.update(|window, cx| window.has_active_dialog(cx)));
    view.read_with(cx, |this, cx| {
        assert!((this.zoom - zoom * 1.25).abs() < 1e-4);
        assert_eq!(
            SettingsStore::global(cx)
                .palette_mru
                .first()
                .map(String::as_str),
            Some("zoomIn")
        );
    });
}

#[gpui_kit::test]
fn grid_moves_with_arrows_and_enter_jumps(cx: &mut TestAppContext) {
    let (view, cx) = open_app(cx);
    open(&view, sample("sample.jpg"), cx);
    let start = view.read_with(cx, |this, _| this.index);

    cx.simulate_keystrokes("g");
    settle(cx);
    assert!(view.read_with(cx, |this, _| this.grid_open));
    // 그리드가 열려 있는 동안 뷰어 단축키는 동작하지 않는다.
    cx.simulate_keystrokes("r");
    settle(cx);
    assert_eq!(view.read_with(cx, |this, _| this.orientation.rotation), 0);

    cx.simulate_keystrokes("right");
    cx.simulate_keystrokes("enter");
    settle(cx);
    view.read_with(cx, |this, _| {
        assert!(!this.grid_open);
        assert_eq!(this.index, start + 1);
    });

    cx.simulate_keystrokes("g");
    settle(cx);
    cx.simulate_keystrokes("escape");
    settle(cx);
    view.read_with(cx, |this, _| {
        assert!(!this.grid_open);
        // Esc는 그리드만 닫고 이미지는 닫지 않는다.
        assert!(this.picture.is_some());
    });
}

#[gpui_kit::test]
fn rename_dialog_submits_with_enter(cx: &mut TestAppContext) {
    let (dir, paths) = temp_copy(&["sample.png", "sample.jpg"]);
    let (view, cx) = open_app(cx);
    open(&view, paths[0].clone(), cx);

    cx.simulate_keystrokes("f2");
    settle(cx);
    assert!(cx.update(|window, cx| window.has_active_dialog(cx)));
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input("renamed.png");
    settle(cx);
    cx.simulate_keystrokes("enter");
    settle(cx);

    assert!(dir.path().join("renamed.png").is_file());
    assert!(!dir.path().join("sample.png").exists());
    view.read_with(cx, |this, _| {
        assert!(this.list.iter().any(|item| item.ends_with("renamed.png")));
        assert_eq!(
            this.info.as_ref().map(|info| info.file_name.as_str()),
            Some("renamed.png")
        );
    });
    assert!(!cx.update(|window, cx| window.has_active_dialog(cx)));
}

#[gpui_kit::test]
fn dual_view_pairs_pages_and_keeps_cover_and_wide_pages_alone(cx: &mut TestAppContext) {
    let (_dir, first) = page_folder(160);
    let (view, cx) = open_app(cx);
    cx.update(|_, cx| {
        SettingsStore::update(cx, |settings| settings.view_mode = ViewMode::LeftToRight);
    });
    open(&view, first, cx);
    assert_eq!(view.read_with(cx, |this, _| this.shown.clone()), [0]);

    cx.simulate_keystrokes("right");
    settle(cx);
    view.read_with(cx, |this, cx| {
        assert_eq!(this.shown, [1, 2]);
        let rects = this.dual_rects(cx);
        assert_eq!(rects.len(), 2);
        // 두 장은 가운데 이음매에서 맞닿는다.
        assert_eq!(rects[0].2.right(), rects[1].2.left());
        assert_eq!(rects[0].0, 1);
    });

    cx.simulate_keystrokes("right");
    settle(cx);
    assert_eq!(view.read_with(cx, |this, _| this.shown.clone()), [3]);
    cx.simulate_keystrokes("right");
    settle(cx);
    assert_eq!(view.read_with(cx, |this, _| this.shown.clone()), [4, 5]);

    // 뒤로 넘길 때도 넓은 페이지를 건너뛰지 않는다.
    cx.simulate_keystrokes("left");
    settle(cx);
    assert_eq!(view.read_with(cx, |this, _| this.shown.clone()), [3]);

    // 우→좌에서는 첫 장이 오른쪽에 온다.
    run(&view, "viewRtl", cx);
    cx.simulate_keystrokes("right");
    settle(cx);
    view.read_with(cx, |this, cx| {
        assert_eq!(this.shown, [4, 5]);
        assert_eq!(this.dual_rects(cx)[0].0, 5);
    });
}

#[gpui_kit::test]
fn resumed_archive_notification_autohides_after_five_seconds(cx: &mut TestAppContext) {
    use std::time::Duration;

    let (_dir, paths) = temp_copy(&["sample-comicinfo.cbz"]);
    let archive = paths[0].clone();
    let (view, cx) = open_app(cx);
    cx.update(|_, cx| {
        SettingsStore::set_archive_progress(
            cx,
            &archive,
            ArchiveProgress {
                entry: "002.jpg".to_owned(),
                index: 1,
                total: 5,
            },
        );
    });

    open(&view, archive, cx);
    assert_eq!(view.read_with(cx, |this, _| this.index), 1);
    assert_eq!(cx.update(|window, cx| window.notifications(cx).len()), 1);

    cx.executor().advance_clock(TOAST_DURATION);
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(300));
    settle(cx);
    assert_eq!(cx.update(|window, cx| window.notifications(cx).len()), 0);
}

#[gpui_kit::test]
fn regular_toasts_share_the_timed_autohide(cx: &mut TestAppContext) {
    use std::time::Duration;

    let (_view, cx) = open_app(cx);
    cx.update(|window, cx| window.toast(Toast::info("temporary toast"), cx));
    assert_eq!(cx.update(|window, cx| window.notifications(cx).len()), 1);

    cx.executor()
        .advance_clock(TOAST_DURATION - Duration::from_secs(1));
    cx.run_until_parked();
    settle(cx);
    assert_eq!(cx.update(|window, cx| window.notifications(cx).len()), 1);

    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(300));
    settle(cx);
    assert_eq!(cx.update(|window, cx| window.notifications(cx).len()), 0);
}

#[gpui_kit::test]
fn webtoon_scrolls_continuously_and_tracks_the_center_page(cx: &mut TestAppContext) {
    let (_dir, first) = page_folder(2400);
    let (view, cx) = open_app(cx);
    cx.update(|_, cx| {
        SettingsStore::update(cx, |settings| settings.view_mode = ViewMode::Webtoon);
    });
    open(&view, first, cx);
    view.read_with(cx, |this, _| {
        assert_eq!((this.wt_anchor, this.wt_offset), (0, 0.0));
        assert_eq!(this.index, 0);
    });

    let center = viewport_center(&view, cx);
    for _ in 0..40 {
        wheel(center, -60.0, false, cx);
    }
    finish_webtoon_scroll(&view, cx);
    view.read_with(cx, |this, _| {
        // 끝까지 내리면 마지막 장에서 멈춘다.
        assert_eq!(this.index, this.list.len() - 1);
        assert!(this.wt_anchor > 0);
    });

    // 위로는 첫 장 위로 넘어가지 않는다.
    for _ in 0..80 {
        wheel(center, 60.0, false, cx);
    }
    finish_webtoon_scroll(&view, cx);
    view.read_with(cx, |this, _| {
        assert_eq!((this.wt_anchor, this.wt_offset), (0, 0.0));
    });
}

#[gpui_kit::test]
fn shortcut_capture_rebinds_and_asks_before_replacing(cx: &mut TestAppContext) {
    let (view, cx) = open_app(cx);
    open(&view, sample("sample.jpg"), cx);
    let weak = view.downgrade();
    let panel = cx.update(|_, cx| cx.new(|cx| SettingsPanel::new(weak, cx)));
    let press = |key: &str| KeyDownEvent {
        keystroke: Keystroke::parse(key).expect("keystroke"),
        is_held: false,
        prefer_character_input: false,
    };

    panel.update_in(cx, |panel, window, cx| {
        panel.capturing = Some("zoomIn".to_owned());
        panel.on_key_down(&press("q"), window, cx);
    });
    settle(cx);
    cx.update(|_, cx| {
        assert_eq!(SettingsStore::global(cx).settings.shortcuts["zoomIn"], "Q");
    });
    // 새 키가 뷰어에 바로 적용된다.
    let zoom = view.read_with(cx, |this, _| this.zoom);
    cx.simulate_keystrokes("q");
    settle(cx);
    view.read_with(cx, |this, _| assert!(this.zoom > zoom));

    // 다른 동작이 쓰는 키는 묻기 전에는 바꾸지 않는다.
    panel.update_in(cx, |panel, window, cx| {
        panel.capturing = Some("zoomOut".to_owned());
        panel.on_key_down(&press("q"), window, cx);
    });
    settle(cx);
    assert!(cx.update(|window, cx| window.has_active_dialog(cx)));
    cx.update(|_, cx| {
        let shortcuts = &SettingsStore::global(cx).settings.shortcuts;
        assert_eq!(shortcuts["zoomOut"], "-");
        assert_eq!(shortcuts["zoomIn"], "Q");
    });
}

#[gpui_kit::test]
fn every_settings_tab_renders_inside_the_dialog(cx: &mut TestAppContext) {
    let (view, cx) = open_app(cx);
    open(&view, sample("sample.jpg"), cx);
    let weak = view.downgrade();
    let panel = cx.update(|_, cx| cx.new(|cx| SettingsPanel::new(weak, cx)));
    let shown = panel.clone();
    cx.update(|window, cx| {
        window.open_dialog(cx, move |dialog, _, _| dialog.child(shown.clone()));
    });
    for tab in 0..6 {
        panel.update_in(cx, |panel, window, cx| panel.select_tab(tab, window, cx));
        settle(cx);
        assert!(cx.update(|window, cx| window.has_active_dialog(cx)));
    }
    // 설정 변경은 저장소와 뷰어에 바로 반영된다.
    cx.update(|_, cx| {
        SettingsStore::update(cx, |settings| settings.loop_navigation = true);
    });
    cx.simulate_keystrokes("escape");
    settle(cx);
    cx.simulate_keystrokes("home");
    settle(cx);
    cx.simulate_keystrokes("left");
    settle(cx);
    view.read_with(cx, |this, _| assert_eq!(this.index, this.list.len() - 1));
}

#[gpui_kit::test]
fn clicking_a_grid_cell_jumps_to_it(cx: &mut TestAppContext) {
    use gpui_kit::test::TestWindowExt as _;

    let (view, cx) = open_app(cx);
    open(&view, sample("sample.jpg"), cx);
    cx.simulate_keystrokes("g");
    settle(cx);
    cx.update(|window, cx| window.click(("grid-cell", 3usize), cx));
    settle(cx);
    view.read_with(cx, |this, _| {
        assert!(!this.grid_open);
        assert_eq!(this.index, 3);
    });
}

#[gpui_kit::test]
fn context_menu_item_runs_its_action(cx: &mut TestAppContext) {
    let (view, cx) = open_app(cx);
    open(&view, sample("sample.jpg"), cx);
    let zoom = view.read_with(cx, |this, _| this.zoom);
    let center = viewport_center(&view, cx);
    cx.simulate_mouse_down(center, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(center, MouseButton::Right, Modifiers::default());
    settle(cx);
    // 메뉴가 열리면 포커스가 뷰어에서 메뉴로 넘어간다.
    view.update_in(cx, |this, window, _| {
        assert!(!this.focus.is_focused(window))
    });
    // 메뉴는 커서 위치에 열려 커서 아래 항목(이미지 닫기)부터 선택이 이어진다.
    // 한 칸 내리면 구분선 다음의 "확대"다.
    for key in ["down", "enter"] {
        cx.simulate_keystrokes(key);
        settle(cx);
    }
    view.read_with(cx, |this, _| {
        assert!(this.picture.is_some());
        assert!((this.zoom - zoom * 1.25).abs() < 1e-4);
    });
}

#[gpui_kit::test]
fn high_contrast_raises_text_and_border_contrast(cx: &mut TestAppContext) {
    use gpui_kit::component::theme::Theme;

    let (_view, cx) = open_app(cx);
    cx.update(|_, cx| {
        super::system::raise_contrast(cx);
        let theme = Theme::global(cx);
        let ink = if theme.is_dark() {
            gpui_kit::white()
        } else {
            gpui_kit::black()
        };
        assert_eq!(theme.foreground, ink);
        assert_eq!(theme.ring, ink);
        assert_eq!(theme.border, ink.opacity(0.8));
    });
}

#[gpui_kit::test]
fn dock_menu_changes_the_dock_position(cx: &mut TestAppContext) {
    use crate::settings::DockPosition;
    use gpui_kit::test::TestWindowExt as _;

    let (view, cx) = open_app(cx);
    open(&view, sample("sample.jpg"), cx);
    cx.update(|window, cx| window.click("dock-menu", cx));
    settle(cx);
    // 첫 항목(위쪽)을 고른다. 이 kit 버전에서는 아래 키 두 번이 첫 항목이다.
    for key in ["down", "down", "enter"] {
        cx.simulate_keystrokes(key);
        settle(cx);
    }
    cx.update(|_, cx| {
        assert_eq!(
            SettingsStore::global(cx).settings.dock_position,
            DockPosition::Top
        );
    });
    // 도크가 위로 옮겨져도 읽기 영역과 이동은 그대로 동작한다.
    let index = view.read_with(cx, |this, _| this.index);
    cx.update(|window, cx| window.click("dock-next", cx));
    settle(cx);
    assert_eq!(view.read_with(cx, |this, _| this.index), index + 1);
}

/// 제어 브리지: 실행 중인 창의 상태를 돌려주고, 명령을 실제 입력 경로로 넣는다.
#[gpui_kit::test]
fn control_bridge_reports_state_and_drives_input(cx: &mut TestAppContext) {
    let (view, cx) = open_app(cx);
    let home = send_control(&view, serde_json::json!({ "cmd": "state" }), cx);
    assert_eq!(home["ok"], true);
    assert_eq!(home["state"]["kind"], "home");
    assert_eq!(home["state"]["count"], 0u64);

    // 알 수 없는 명령은 이유만 돌려주고 창은 그대로 둔다.
    let unknown = send_control(&view, serde_json::json!({ "cmd": "nope" }), cx);
    assert_eq!(unknown["ok"], false);
    assert!(unknown["error"].is_string());

    open(&view, sample("sample.jpg"), cx);
    let start = view.read_with(cx, |this, _| this.index);
    let opened = send_control(&view, serde_json::json!({ "cmd": "state" }), cx);
    assert_eq!(opened["state"]["kind"], "image");
    assert_eq!(opened["state"]["index"], start as u64);
    assert!(
        opened["state"]["name"]
            .as_str()
            .is_some_and(|name| name.ends_with("sample.jpg"))
    );
    assert!(opened["state"]["viewport"]["width"].as_f64().unwrap_or(0.0) > 0.0);

    // 키 입력이 단축키 처리 경로를 지나 다음 화면으로 넘어간다.
    send_control(
        &view,
        serde_json::json!({ "cmd": "key", "spec": "right" }),
        cx,
    );
    settle(cx);
    assert_eq!(view.read_with(cx, |this, _| this.index), start + 1);

    // 수식키가 실린 휠은 창 메시지로 만들 수 없는 입력이다.
    let zoom = view.read_with(cx, |this, _| this.zoom);
    let center = viewport_center(&view, cx);
    let (x, y) = (f32::from(center.x), f32::from(center.y));
    send_control(
        &view,
        serde_json::json!({ "cmd": "mouse", "kind": "wheel", "lines": 1.0,
            "modifiers": ["ctrl"], "x": x, "y": y, "space": "logical" }),
        cx,
    );
    settle(cx);
    view.read_with(cx, |this, _| {
        assert!((this.zoom - zoom * 1.25).abs() < 1e-4);
        assert!(!this.fit_locked);
    });

    // 수식키 없는 휠은 다음 화면으로 넘긴다.
    send_control(
        &view,
        serde_json::json!({ "cmd": "mouse", "kind": "wheel", "lines": -1.0,
            "x": x, "y": y, "space": "logical" }),
        cx,
    );
    settle(cx);
    assert_eq!(view.read_with(cx, |this, _| this.index), start + 2);

    // 글자는 입력 상자 경로로 들어간다(명령 팔레트에서 "zoom out").
    send_control(
        &view,
        serde_json::json!({ "cmd": "action", "id": "togglePalette" }),
        cx,
    );
    assert!(cx.update(|window, cx| window.has_active_dialog(cx)));
    send_control(
        &view,
        serde_json::json!({ "cmd": "text", "value": "zoom out" }),
        cx,
    );
    settle(cx);
    send_control(
        &view,
        serde_json::json!({ "cmd": "key", "spec": "enter" }),
        cx,
    );
    settle(cx);
    assert!(!cx.update(|window, cx| window.has_active_dialog(cx)));
    view.read_with(cx, |this, _| assert!((this.zoom - zoom).abs() < 1e-4));

    // 파일을 여는 것도 제어 통로로 할 수 있다.
    let second = send_control(
        &view,
        serde_json::json!({ "cmd": "open", "path": sample("sample.bmp") }),
        cx,
    );
    settle(cx);
    assert_eq!(second["ok"], true);
    view.read_with(cx, |this, _| {
        assert!(this.list[this.index].ends_with("sample.bmp"));
    });

    // 끌어다 놓기도 같은 통로로 확인할 수 있다.
    let dropped = send_control(
        &view,
        serde_json::json!({ "cmd": "drop", "paths": [sample("sample.png")],
            "x": 200.0, "y": 200.0, "space": "logical" }),
        cx,
    );
    // 놓기는 다음 프레임에 보낸다. 테스트에는 플랫폼 프레임 루프가 없어 직접 돌린다.
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    settle(cx);
    assert_eq!(dropped["ok"], true);
    view.read_with(cx, |this, _| {
        assert!(this.list[this.index].ends_with("sample.png"));
    });
}
