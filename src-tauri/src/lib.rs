pub mod commands;
pub mod image;
#[cfg(target_os = "windows")]
pub mod context_menu;

use commands::{get_directory_images, load_image};
use tauri::Emitter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // On Windows, handle the `--unregister-context-menu` flag so that NSIS
    // uninstall hooks can clean up the registry entries without launching the
    // full GUI.
    #[cfg(target_os = "windows")]
    {
        let args: Vec<String> = std::env::args().collect();
        if args.iter().any(|a| a == "--unregister-context-menu") {
            context_menu::unregister_context_menu();
            return;
        }
    }

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init());

    builder
        .invoke_handler(tauri::generate_handler![load_image, get_directory_images])
        .setup(|app| {
            // Register the Windows 11 File Explorer context menu entry so that
            // "Open with Image Viewer" appears when right-clicking image files.
            // The entry is written to HKCU (no administrator rights required).
            #[cfg(target_os = "windows")]
            context_menu::register_context_menu();

            // On Windows and Linux, the OS passes the file path as a CLI argument
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
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| {
            // On macOS, the OS uses applicationOpenURLs to open files in a running app.
            // Tauri surfaces this as RunEvent::Opened.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = &event {
                for url in urls {
                    if url.scheme() == "file" {
                        if let Ok(path) = url.to_file_path() {
                            let path_str = path.to_string_lossy().to_string();
                            app_handle.emit("open-file", &path_str).ok();
                        }
                    }
                }
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app_handle, event);
        });
}
