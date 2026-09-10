pub mod app_error;
pub mod archive;
pub mod commands;
pub mod dir_cache;
pub mod file_assoc;
pub mod heif;
pub mod image;
pub mod process_temp;
pub mod save;
pub mod thumbnail;

use commands::{
    archive_prefetch, generate_thumbnail, generate_thumbnails_batch, get_archive_images,
    get_directory_images, get_exif_data, get_file_associations, load_archive_image, load_image,
    open_default_apps_settings, rename_file, resolve_dropped_path, set_all_file_associations,
    set_file_association, trash_file,
};
use save::save_image_edits;
use tauri::{Emitter, Manager};

pub fn run() {
    let mut builder = tauri::Builder::default();
    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(tauri_plugin_mcp_bridge::init());
    }
    builder
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        // 두 번째 실행(파일 더블클릭 등)은 새 창 대신 기존 창에 파일을 연다.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(file_path) = args.get(1).cloned() {
                let handle = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    handle.emit("open-file", file_path).ok();
                });
                if let Some(window) = app.get_webview_window("main") {
                    window.set_focus().ok();
                }
            }
        }))
        .invoke_handler(tauri::generate_handler![
            load_image,
            get_directory_images,
            get_exif_data,
            generate_thumbnail,
            generate_thumbnails_batch,
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
            save_image_edits
        ])
        .setup(|app| {
            // Windows passes the file path as a CLI argument
            // when the app is launched via a file association.
            let args: Vec<String> = std::env::args().collect();
            if let Some(file_path) = args.get(1).cloned() {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    // Wait briefly for the webview to finish loading before emitting
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    handle.emit("open-file", file_path).ok();
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
