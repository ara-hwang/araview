// 릴리스 빌드는 Windows에서 콘솔 창을 띄우지 않는다.
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

fn home_dir() -> Option<PathBuf> {
    env_dir("HOME")
}

/// 앱 전용 하위 폴더를 붙인다.
fn app_dir(base: Option<PathBuf>) -> Option<PathBuf> {
    base.map(|base| base.join(IDENTIFIER))
}

/// 설정 폴더. Windows는 `%APPDATA%`, macOS는 `~/Library/Application Support`,
/// 그 밖의 유닉스는 `$XDG_CONFIG_HOME`(없으면 `~/.config`) 아래다.
fn config_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        app_dir(env_dir("APPDATA"))
    } else if cfg!(target_os = "macos") {
        app_dir(home_dir().map(|home| home.join("Library/Application Support")))
    } else {
        app_dir(env_dir("XDG_CONFIG_HOME").or_else(|| home_dir().map(|home| home.join(".config"))))
    }
}

/// 캐시 폴더. Windows는 `%LOCALAPPDATA%`, macOS는 `~/Library/Caches`,
/// 그 밖의 유닉스는 `$XDG_CACHE_HOME`(없으면 `~/.cache`) 아래다.
fn cache_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        app_dir(env_dir("LOCALAPPDATA"))
    } else if cfg!(target_os = "macos") {
        app_dir(home_dir().map(|home| home.join("Library/Caches")))
    } else {
        app_dir(env_dir("XDG_CACHE_HOME").or_else(|| home_dir().map(|home| home.join(".cache"))))
    }
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
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .try_init();

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

    let application = gpui_kit::application().with_assets(assets::AppAssets);
    // Finder 등 운영체제가 실행 중인 앱에 파일 열기를 요청하는 경로(macOS의 `open-document`).
    #[cfg(unix)]
    application.on_open_urls(|urls| platform::open_urls(&urls));
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
            // Wayland와 X11이 `.desktop` 파일과 창을 이 값으로 짝짓는다(아이콘, 작업 표시줄 묶기).
            app_id: Some(IDENTIFIER.to_owned()),
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
