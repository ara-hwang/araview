pub mod app_error;
pub mod archive;
pub mod comic_info;
pub mod commands;
pub mod dir_cache;
pub mod file_assoc;
pub mod file_availability;
pub mod heif;
pub mod image;
pub mod image_info;
pub mod orientation;
pub mod process_temp;
pub mod psd_sidecar;
pub mod save;
pub mod sidecar;
pub mod stable_hash;
pub mod thumb_shell;
pub mod thumbnail;

use app_error::AppError;
use commands::{
    archive_prefetch, generate_archive_file_thumbnail, generate_archive_thumbnail,
    generate_thumbnail, generate_thumbnails_batch, get_archive_images, get_cached_thumbnail,
    get_comic_info, get_directory_images, get_exif_data, get_file_associations, get_image_details,
    get_image_histogram, load_archive_image, load_image, open_default_apps_settings, rename_file,
    resolve_dropped_path, set_all_file_associations, set_file_association, trash_file,
};
use save::save_image_edits;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use thumb_shell::{get_psd_thumbnail_status, register_psd_thumbnail, unregister_psd_thumbnail};

/// 시작 시/두 번째 실행에서 받은 파일 경로를 프론트 준비 전까지 보관한다.
#[derive(Default)]
struct PendingOpenFile {
    path: Mutex<Option<String>>,
    ready: AtomicBool,
}

/// 프론트가 준비됐으면 바로 emit하고, 아니면 보관했다가 `frontend_ready`에서 보낸다.
/// (앱 시작 직후에는 프론트 리스너가 아직 등록되기 전이라 고정 지연 emit은 유실될 수 있다.)
fn deliver_open_file(app: &tauri::AppHandle, path: String) {
    let state = app.state::<PendingOpenFile>();
    if state.ready.load(Ordering::SeqCst) {
        app.emit("open-file", path).ok();
    } else if let Ok(mut pending) = state.path.lock() {
        *pending = Some(path);
    }
}

/// 프론트 루트가 리스너 등록을 마친 뒤 1회 호출한다. 보관된 경로를 flush한다.
#[tauri::command]
fn frontend_ready(app: tauri::AppHandle) -> Result<(), AppError> {
    let state = app.state::<PendingOpenFile>();
    state.ready.store(true, Ordering::SeqCst);
    let pending = state
        .path
        .lock()
        .map_err(|_| AppError::lock_poisoned("pending open file"))?
        .take();
    if let Some(path) = pending {
        app.emit("open-file", path).ok();
    }
    Ok(())
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
        .plugin(tauri_plugin_process::init())
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
            get_directory_images,
            get_exif_data,
            get_image_histogram,
            get_image_details,
            generate_thumbnail,
            generate_thumbnails_batch,
            get_cached_thumbnail,
            generate_archive_thumbnail,
            generate_archive_file_thumbnail,
            resolve_dropped_path,
            get_comic_info,
            get_archive_images,
            load_archive_image,
            archive_prefetch,
            get_file_associations,
            set_file_association,
            set_all_file_associations,
            open_default_apps_settings,
            get_psd_thumbnail_status,
            register_psd_thumbnail,
            unregister_psd_thumbnail,
            trash_file,
            rename_file,
            save_image_edits,
            frontend_ready
        ])
        .setup(|app| {
            app.manage(PendingOpenFile::default());
            // asset 프로토콜 scope는 설정에서 비워 두고, 렌더링이 필요한 경로만
            // 런타임에 허용한다. 사이드카/썸네일/아카이브 추출물은 프로세스
            // temp 아래에 생성되므로 여기서 통째로 허용한다.
            // scope 매칭은 canonicalize된 요청 경로 기준이라 여기서도 canonicalize한다.
            if let Ok(temp) = process_temp::process_temp_dir() {
                let canonical = std::fs::canonicalize(&temp).unwrap_or(temp);
                if let Err(e) = app.asset_protocol_scope().allow_directory(&canonical, true) {
                    log::error!("[asset-scope] failed to allow process temp dir: {e}");
                }
            }
            // Windows passes the file path as a CLI argument
            // when the app is launched via a file association.
            // 프론트 준비 전이면 보관했다가 frontend_ready에서 emit한다.
            let args: Vec<String> = std::env::args().collect();
            if let Some(file_path) = args.get(1).cloned() {
                deliver_open_file(app.handle(), file_path);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
