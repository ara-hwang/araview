pub mod app_error;
pub mod archive;
pub mod commands;
pub mod dir_cache;
pub mod file_assoc;
pub mod heif;
pub mod image;
pub mod image_info;
pub mod process_temp;
pub mod psd_sidecar;
pub mod save;
pub mod thumbnail;

use app_error::AppError;
use commands::{
    archive_prefetch, generate_archive_thumbnail, generate_thumbnail, generate_thumbnails_batch,
    get_archive_images, get_directory_images, get_exif_data, get_file_associations,
    get_image_details, get_image_histogram, load_archive_image, load_image,
    open_default_apps_settings, rename_file, resolve_dropped_path, set_all_file_associations,
    set_file_association, trash_file,
};
use save::save_image_edits;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

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
    let pending = state.path.lock().ok().and_then(|mut guard| guard.take());
    if let Some(path) = pending {
        app.emit("open-file", path).ok();
    }
    Ok(())
}

pub fn run() {
    let builder = tauri::Builder::default();
    // 개발용 MCP 브리지는 LAN에 노출되지 않도록 loopback에만 바인딩한다.
    // release CI는 --no-default-features로 dev-mcp를 끄고 컴파일한다.
    #[cfg(feature = "dev-mcp")]
    let builder = builder.plugin(tauri_plugin_mcp_bridge::init_with_config(
        tauri_plugin_mcp_bridge::Config::localhost_only(),
    ));
    builder
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
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
            generate_archive_thumbnail,
            resolve_dropped_path,
            get_archive_images,
            load_archive_image,
            archive_prefetch,
            get_file_associations,
            set_file_association,
            set_all_file_associations,
            open_default_apps_settings,
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
                    eprintln!("[asset-scope] failed to allow process temp dir: {e}");
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
