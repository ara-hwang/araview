pub mod commands;
pub mod image;

use commands::{get_directory_images, load_image};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![load_image, get_directory_images])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
