// 릴리스 빌드는 콘솔 창을 띄우지 않는다.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assets;
mod dialog;
mod geometry;
mod i18n;
mod keys;
mod layout;
mod picture;
mod platform;
mod settings;
mod smooth;
mod toast;

use std::path::PathBuf;

use araview_core::process_temp;
use gpui_kit::component::TitleBar;
use gpui_kit::*;

use crate::app::AraView;
use crate::platform::Instance;
use crate::settings::{CacheStorageMode, SettingsStore, WindowState};

/// 개발 빌드와 설치 빌드가 설정과 캐시를 공유하지 않게 식별자를 나눈다.
/// 설치 빌드는 1.x(Tauri) 설치본과 같은 식별자라 기존 설정과 최근 파일을 그대로 이어받는다.
const IDENTIFIER: &str = if cfg!(debug_assertions) {
    "com.araview.viewer.dev"
} else {
    "com.araview.viewer"
};

const DEFAULT_SIZE: (f32, f32) = (1024.0, 768.0);
/// 최소 너비 500은 Windows 11 Snap Layouts의 모든 배치에 창이 들어가기 위한 값이다.
const MIN_SIZE: (f32, f32) = (500.0, 400.0);

fn app_dir(variable: &str) -> Option<PathBuf> {
    std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(|base| PathBuf::from(base).join(IDENTIFIER))
}

fn init_cache(mode: CacheStorageMode) {
    let mode = match mode {
        CacheStorageMode::Temporary => process_temp::CacheStorageMode::Temporary,
        CacheStorageMode::Persistent => process_temp::CacheStorageMode::Persistent,
    };
    let initialized = app_dir("LOCALAPPDATA")
        .ok_or_else(|| {
            araview_core::app_error::AppError::unknown("LOCALAPPDATA is not set".to_owned())
        })
        .and_then(|dir| process_temp::initialize(&dir, mode));
    if let Err(error) = initialized {
        log::warn!("[cache] persistent cache unavailable, using session cache: {error}");
        if let Err(error) = process_temp::initialize_temporary_fallback() {
            log::error!("[cache] no usable cache directory: {error}");
        }
    }
    process_temp::request_total_cap_scan();
}

/// 저장된 창 상태가 지금 연결된 화면 안에 들어올 때만 복원한다.
fn restored_bounds(saved: Option<WindowState>, cx: &App) -> Option<WindowBounds> {
    let saved = saved?;
    let bounds = Bounds::new(
        point(px(saved.x), px(saved.y)),
        size(
            px(saved.width.max(MIN_SIZE.0)),
            px(saved.height.max(MIN_SIZE.1)),
        ),
    );
    let visible = cx
        .displays()
        .iter()
        .any(|display| display.bounds().intersects(&bounds));
    if !visible {
        return None;
    }
    Some(if saved.maximized {
        WindowBounds::Maximized(bounds)
    } else {
        WindowBounds::Windowed(bounds)
    })
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .try_init();

    // Windows는 파일 연결로 실행할 때 경로를 첫 인자로 넘긴다.
    let initial = std::env::args().nth(1);
    // 두 번째 실행은 새 창 대신 기존 창에 파일을 연다.
    let instances = match platform::single_instance(IDENTIFIER, initial.as_deref()) {
        Instance::Forwarded => return,
        Instance::Primary(receiver) => receiver,
    };

    let store = SettingsStore::load(app_dir("APPDATA"));
    i18n::set_language(store.settings.language);
    init_cache(store.settings.cache_storage_mode);

    gpui_kit::application()
        .with_assets(assets::AppAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            let saved_window = store.window;
            cx.set_global(store);

            let bounds = restored_bounds(saved_window, cx).unwrap_or_else(|| {
                WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(DEFAULT_SIZE.0), px(DEFAULT_SIZE.1)),
                    cx,
                ))
            });
            let options = WindowOptions {
                window_bounds: Some(bounds),
                window_min_size: Some(size(px(MIN_SIZE.0), px(MIN_SIZE.1))),
                ..TitleBar::window_options()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                window.set_window_title(&i18n::t("app.title"));
                cx.new(|cx| AraView::new(initial, Some(instances), window, cx))
            })
            .expect("failed to open the main window");

            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    process_temp::cleanup_on_exit();
                    cx.quit();
                }
            })
            .detach();
        });
}
