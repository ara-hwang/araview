// 릴리스 빌드는 Windows에서 콘솔 창을 띄우지 않는다.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// 상태 JSON(`bridge.rs`)이 중첩 객체를 담아 기본 매크로 재귀 한도를 넘는다.
#![recursion_limit = "256"]

mod app;
mod assets;
mod diag;
mod dialog;
mod geometry;
mod i18n;
mod keys;
mod layout;
mod logging;
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
pub(crate) const IDENTIFIER: &str = if cfg!(debug_assertions) {
    "com.araview.viewer.dev"
} else {
    "com.araview.viewer"
};

const DEFAULT_SIZE: (f32, f32) = (1024.0, 768.0);
/// 최소 너비 500은 Windows 11 Snap Layouts의 모든 배치에 창이 들어가기 위한 값이다.
const MIN_SIZE: (f32, f32) = (500.0, 400.0);

/// 환경 변수가 비어 있지 않을 때만 그 경로를 돌려준다.
fn env_dir(variable: &str) -> Option<PathBuf> {
    std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// 앱 전용 하위 폴더를 붙인다.
fn app_dir(base: Option<PathBuf>) -> Option<PathBuf> {
    base.map(|base| base.join(IDENTIFIER))
}

/// 설정 폴더. `%APPDATA%` 아래 앱 전용 하위 폴더다.
fn config_dir() -> Option<PathBuf> {
    app_dir(env_dir("APPDATA"))
}

/// 캐시 폴더. `%LOCALAPPDATA%` 아래 앱 전용 하위 폴더다.
pub(crate) fn cache_dir() -> Option<PathBuf> {
    app_dir(env_dir("LOCALAPPDATA"))
}

fn init_cache(mode: CacheStorageMode) {
    let mode = match mode {
        CacheStorageMode::Temporary => process_temp::CacheStorageMode::Temporary,
        CacheStorageMode::Persistent => process_temp::CacheStorageMode::Persistent,
    };
    let initialized = cache_dir()
        .ok_or_else(|| {
            araview_core::app_error::AppError::unknown("cache directory is not set".to_owned())
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
    // 콘솔과 파일(`logs/araview.log`)에 같은 기록을 남긴다(SPEC §17.1).
    logging::init();

    // 파일 연결이나 터미널로 실행하면 경로를 첫 인자로 받는다.
    let initial = std::env::args().nth(1);
    // 두 번째 실행은 새 창 대신 기존 창에 파일을 연다.
    let instances = match platform::single_instance(IDENTIFIER, initial.as_deref()) {
        Instance::Forwarded => return,
        Instance::Primary(receiver) => receiver,
    };

    let store = SettingsStore::load(config_dir());
    i18n::set_language(store.settings.language);
    init_cache(store.settings.cache_storage_mode);

    // 설정 토글(docs/hdr-display.md M4) 전까지는 환경 변수로만 켠다.
    platform::prepare_hdr_output(std::env::var_os("ARAVIEW_HDR_OUTPUT").is_some_and(|v| v == "1"));

    let application = gpui_kit::application().with_assets(assets::AppAssets);
    application.run(move |cx| {
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
