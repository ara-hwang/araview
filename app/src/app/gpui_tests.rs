//! araview 자체 회귀 시나리오. GPUI의 결정적 executor/test API만 사용한다.
//! API 참고: Zed crates/gpui/src/app/test_context.rs (Apache-2.0),
//! e5be9608ddd4cb0f2f179e1c0014f0a3a0337260. Zed 앱 테스트는 복사하지 않았다.

use std::sync::Arc;

use gpui::{AppContext as _, Entity, TestAppContext, VisualTestContext};
use gpui_kit as gpui;
use gpui_kit::component::WindowExt as _;

use super::AraView;
use super::thumbnail_state::ThumbQueue;
use crate::settings::SettingsStore;

#[gpui::test(seeds(1, 7, 42))]
fn stale_thumbnail_completion_cannot_release_replacement(cx: &mut TestAppContext) {
    let queue = cx.new(|_| ThumbQueue::default());
    let old = queue.update(cx, |queue, _| {
        queue.enqueue([Arc::from("same-page")]);
        queue.start_next().unwrap()
    });
    let background = cx.background_spawn(async move { old });
    queue.update(cx, |queue, cx| {
        cx.spawn(async move |queue, cx| {
            let old = background.await;
            queue
                .update(cx, |queue, _| assert!(!queue.finish(&old)))
                .unwrap();
        })
        .detach();
        queue.reset(1);
        queue.enqueue([Arc::from("same-page")]);
    });
    let replacement = queue.update(cx, |queue, _| queue.start_next().unwrap());
    cx.run_until_parked();
    queue.update(cx, |queue, _| {
        queue.enqueue([Arc::from("same-page")]);
        assert!(
            queue.start_next().is_none(),
            "late completion must not enable a duplicate"
        );
        assert!(queue.finish(&replacement));
    });
}

#[gpui::test(seeds(1, 7, 42))]
fn releasing_queue_entity_cancels_background_generation(cx: &mut TestAppContext) {
    let queue = cx.new(|_| ThumbQueue::default());
    let token = queue.read_with(cx, |queue, _| queue.cancellation());
    let weak = queue.downgrade();
    drop(queue);
    cx.run_until_parked();
    assert!(token.is_cancelled());
    assert!(weak.update(cx, |_, _| ()).is_err());
}

fn viewer(cx: &mut TestAppContext) -> (VisualTestContext, Entity<AraView>) {
    let mut settings = SettingsStore::load(None);
    settings.settings.auto_check_updates = false;
    settings.settings.record_recent_files = false;
    cx.set_global(settings);
    let (window, view) = cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            cx.new(|cx| AraView::new(None, None, window, cx))
        })
        .unwrap()
    });
    (VisualTestContext::from_window(window, cx), view)
}

#[gpui::test(seeds(1, 7, 42))]
fn late_open_result_cannot_replace_newer_request(cx: &mut TestAppContext) {
    let (mut cx, view) = viewer(cx);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.open_seq = 2;
            view.finish_open_result(
                2,
                "second.png".into(),
                Err(super::AppError::not_found("synthetic")),
                window,
                cx,
            );
        })
    });
    cx.update(|window, cx| view.update(cx, |view, cx| {
        view.finish_open_result(1, "first.png".into(), Err(super::AppError::not_found("old")), window, cx);
        assert!(matches!(&view.error, Some(super::ViewError::Open { path, .. }) if path == "second.png"));
    }));
}

#[gpui::test]
fn modal_owns_keys_before_open_grid_in_real_root(cx: &mut TestAppContext) {
    let (mut cx, view) = viewer(cx);
    // Open the actual grid, then remove synthetic entries before drawing so no disk I/O runs.
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.list = Arc::new(vec![Arc::from("a"), Arc::from("b")]);
            view.toggle_grid(window, cx);
            view.list = Arc::new(Vec::new());
            window.open_dialog(cx, |dialog, _, _| dialog.title("test modal"));
        })
    });
    // Exercise production root on_key directly: modal UI may consume its own key first.
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let event = gpui::KeyDownEvent {
                keystroke: gpui::Keystroke::parse("g").unwrap(),
                is_held: false,
                prefer_character_input: false,
            };
            view.on_key(&event, window, cx);
            assert!(view.grid_open(), "modal must prevent G closing the grid");
        })
    });
    cx.update(|window, cx| window.close_all_dialogs(cx));
}
