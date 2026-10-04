pub mod app_error;
pub mod archive;
pub mod archive_index;
pub mod cache;
pub mod comic_info;
pub mod commands;
pub mod dir_cache;
pub mod file_assoc;
pub mod file_availability;
pub mod heif;
pub mod image;
pub mod image_info;
pub mod orientation;
pub mod pixel_art;
pub mod process_temp;
pub mod psd_sidecar;
pub mod raster_sidecar;
pub mod scaled;
pub mod sidecar;
pub mod sniff;
pub mod stable_hash;
pub mod svg_raster;
pub mod svg_size;
pub mod thumb_shell;
pub mod thumbnail;
pub mod transcode;

use app_error::AppError;
use commands::{
    archive_prefetch, clear_cache, detect_pixel_art, generate_archive_file_thumbnail,
    generate_archive_file_thumbnails_batch, generate_archive_thumbnail, generate_thumbnail,
    generate_thumbnails_batch, get_archive_images, get_cache_stats, get_cached_thumbnail,
    get_comic_info, get_directory_images, get_exif_data, get_file_associations, get_image_details,
    get_image_histogram, get_license_bundle, load_archive_image, load_image,
    open_default_apps_settings, rename_file, resolve_dropped_path, set_comic_cover_pages,
    set_file_association, trash_file,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use tauri_plugin_store::StoreExt;
use thumb_shell::{get_psd_thumbnail_status, register_psd_thumbnail, unregister_psd_thumbnail};

/// 시작 시/두 번째 실행에서 받은 파일 경로를 프론트 준비 전까지 보관한다.
#[derive(Default)]
struct PendingOpenFile {
    path: Mutex<Option<String>>,
    ready: AtomicBool,
}

/// 프론트가 준비됐으면 바로 emit하고, 아니면 보관했다가 `frontend_ready`에서 보낸다.
/// (앱 시작 직후에는 프론트 리스너가 아직 등록되기 전이라 고정 지연 emit은 유실될 수 있다.)
/// `ready` 확인은 `path` 락 안에서만 한다: 바깥에서 읽으면 ready 전환과 보관 사이에
/// 끼어든 이벤트가 emit도 저장도 못 하고 유실된다.
fn deliver_open_file(app: &tauri::AppHandle, path: String) {
    let state = app.state::<PendingOpenFile>();
    let Ok(mut pending) = state.path.lock() else {
        return;
    };
    if state.ready.load(Ordering::SeqCst) {
        drop(pending);
        app.emit("open-file", path).ok();
    } else {
        *pending = Some(path);
    }
}

/// 프론트 루트가 리스너 등록을 마친 뒤 1회 호출한다. 보관된 경로를 flush한다.
#[tauri::command]
fn frontend_ready(app: tauri::AppHandle) -> Result<(), AppError> {
    let state = app.state::<PendingOpenFile>();
    let pending = {
        let mut guard = state
            .path
            .lock()
            .map_err(|_| AppError::lock_poisoned("pending open file"))?;
        state.ready.store(true, Ordering::SeqCst);
        guard.take()
    };
    if let Some(path) = pending {
        app.emit("open-file", path).ok();
    }
    Ok(())
}

fn requested_cache_storage_mode(app: &tauri::AppHandle) -> process_temp::CacheStorageMode {
    let mode = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("settings"))
        .and_then(|settings| settings.get("cacheStorageMode").cloned())
        .and_then(|value| value.as_str().map(str::to_owned));
    match mode.as_deref() {
        Some("temporary") => process_temp::CacheStorageMode::Temporary,
        Some("persistent") | None => process_temp::CacheStorageMode::Persistent,
        Some(other) => {
            log::warn!("[cache] ignoring invalid cacheStorageMode {other:?}");
            process_temp::CacheStorageMode::Persistent
        }
    }
}

pub fn run() {
    // env_logger의 기본 필터는 error다. 기존 eprintln! 진단이 조용히 사라지지
    // 않도록 warn을 기본값으로 두고, RUST_LOG로 덮어쓸 수 있게 둔다.
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .try_init();
    let builder = tauri::Builder::default();
    // 개발용 MCP 브리지는 LAN에 노출되지 않도록 loopback에만 바인딩한다.
    // release CI는 --no-default-features로 dev-mcp를 끄고 컴파일한다.
    // base port는 플러그인 기본값(9223)을 쓰지 않는다. 9223을 다른 브리지가
    // 선점하면 dev 브리지가 조용히 다음 포트로 밀리고, MCP 클라이언트는 9223에
    // 붙어 엉뚱한 앱(설치본 등)을 검증하게 된다. dev 전용 포트를 고정해
    // 소유자를 명확히 한다. 클라이언트는 `--port 9323` 또는
    // MCP_BRIDGE_PORT=9323을 쓴다(.opencode/opencode.json 참고).
    #[cfg(feature = "dev-mcp")]
    let builder = builder.plugin(
        tauri_plugin_mcp_bridge::Builder::new()
            .bind_address("127.0.0.1")
            .base_port(9323)
            .build(),
    );
    builder
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        // Windows 11 Snap Layouts: 커스텀 최대화 버튼 위에 WM_NCHITTEST에
        // HTMAXBUTTON으로 응답하는 투명 네이티브 오버레이를 띄워 OS 플라이아웃을
        // 띄운다. 비-Windows에서는 no-op. 대상 버튼 id는 Header.tsx의 캡션 버튼과
        // 일치해야 한다.
        .plugin(
            tauri_plugin_snap_layout::init()
                .button_id("caption-maximize")
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        // 두 번째 실행(파일 더블클릭 등)은 새 창 대신 기존 창에 파일을 연다.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(file_path) = args.get(1).cloned() {
                deliver_open_file(app, file_path);
                if let Some(window) = app.get_webview_window("main") {
                    window.set_focus().ok();
                }
            }
        }))
        .invoke_handler(tauri::generate_handler![
            load_image,
            detect_pixel_art,
            get_directory_images,
            get_exif_data,
            get_image_histogram,
            get_image_details,
            generate_thumbnail,
            generate_thumbnails_batch,
            get_cache_stats,
            clear_cache,
            get_cached_thumbnail,
            generate_archive_thumbnail,
            generate_archive_file_thumbnail,
            generate_archive_file_thumbnails_batch,
            resolve_dropped_path,
            get_comic_info,
            set_comic_cover_pages,
            get_archive_images,
            load_archive_image,
            archive_prefetch,
            get_file_associations,
            set_file_association,
            open_default_apps_settings,
            get_license_bundle,
            get_psd_thumbnail_status,
            register_psd_thumbnail,
            unregister_psd_thumbnail,
            trash_file,
            rename_file,
            frontend_ready
        ])
        .setup(|app| {
            app.manage(PendingOpenFile::default());
            let requested_mode = requested_cache_storage_mode(app.handle());
            let cache_root = match process_temp::initialize(app.handle(), requested_mode) {
                Ok(root) => root,
                Err(e) => {
                    log::warn!("[cache] persistent cache unavailable, using session cache: {e}");
                    process_temp::initialize_temporary_fallback()?
                }
            };
            // 캐시 루트는 이미지/썸네일/아카이브 asset 경로로만 허용한다.
            // scope 매칭은 canonicalize된 요청 경로 기준이라 여기서도 canonicalize한다.
            let canonical = std::fs::canonicalize(&cache_root).unwrap_or(cache_root);
            if let Err(e) = app.asset_protocol_scope().allow_directory(&canonical, true) {
                log::error!("[asset-scope] failed to allow cache dir: {e}");
            }
            process_temp::request_total_cap_scan();
            // Windows passes the file path as a CLI argument
            // when the app is launched via a file association.
            // 프론트 준비 전이면 보관했다가 frontend_ready에서 emit한다.
            let args: Vec<String> = std::env::args().collect();
            if let Some(file_path) = args.get(1).cloned() {
                deliver_open_file(app.handle(), file_path);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                process_temp::cleanup_on_exit();
            }
        });
}
