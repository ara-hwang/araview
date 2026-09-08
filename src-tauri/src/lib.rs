pub mod archive;
pub mod commands;
pub mod file_assoc;
pub mod heif;
pub mod image;
pub mod process_temp;

use commands::{
    get_archive_images, get_directory_images, get_exif_data, get_file_associations,
    load_archive_image, load_image, open_default_apps_settings, resolve_dropped_path,
    set_all_file_associations, set_file_association,
};
use tauri::Emitter;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            load_image,
            get_directory_images,
            get_exif_data,
            resolve_dropped_path,
            get_archive_images,
            load_archive_image,
            get_file_associations,
            set_file_association,
            set_all_file_associations,
            open_default_apps_settings
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
