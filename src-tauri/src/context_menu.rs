use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

// Base registry path for all image types (Windows 10 and 11).
// Windows 11's new context menu also checks SystemFileAssociations\image\shell,
// so placing the entry here makes it appear in both the classic and new menus.
const SHELL_KEY: &str =
    r"Software\Classes\SystemFileAssociations\image\shell\ImageViewer";
const COMMAND_KEY: &str =
    r"Software\Classes\SystemFileAssociations\image\shell\ImageViewer\command";

// Supported image extensions for per-extension registration.
// Windows 11's new context menu additionally checks each extension's own
// `shell` key (HKCU\Software\Classes\.<ext>\shell\...), so registering here
// ensures the entry appears reliably in the Windows 11 top-level context menu.
const EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif",
    "avif", "heic", "heif",
];

/// Registers a "Open with Image Viewer" context menu entry for all image files
/// under HKCU so that no administrator privileges are required.
///
/// # Windows 11 new context menu compatibility
///
/// Windows 11 displays a new-style context menu (with large icon buttons at the
/// top) that is separate from the classic cascading menu. To appear in the new
/// menu, an entry must satisfy one or both of the following conditions:
///
/// 1. Be registered under `HKCU\Software\Classes\SystemFileAssociations\image\shell\`
///    (already the case for this app) **with a `Position` value** set to `"Top"`.
/// 2. Be registered under `HKCU\Software\Classes\.<ext>\shell\` for each
///    individual file extension the app wants to handle, also with `Position = "Top"`.
///
/// This function fulfils both conditions. The `Position = "Top"` value tells the
/// shell to place the entry near the top of the new context menu rather than
/// burying it in the "Show more options" overflow. No `Extended` value is set, so
/// the entry is visible on a plain right-click (not just Shift+right-click).
pub fn register_context_menu() {
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    let exe_str = exe_path.to_string_lossy();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    // --- 1. SystemFileAssociations\image (Windows 10 + 11) ---
    if let Ok((shell_key, _)) = hkcu.create_subkey(SHELL_KEY) {
        let _ = shell_key.set_value("", &"Open with Image Viewer");
        let _ = shell_key.set_value("Icon", &format!("{},0", exe_str));
        // "Top" causes the entry to appear near the top of the Windows 11
        // new-style context menu instead of in the overflow submenu.
        let _ = shell_key.set_value("Position", &"Top");
    }

    if let Ok((cmd_key, _)) = hkcu.create_subkey(COMMAND_KEY) {
        let _ = cmd_key.set_value("", &format!("\"{}\" \"%1\"", exe_str));
    }

    // --- 2. Per-extension registration (Windows 11 new menu) ---
    // Windows 11 also checks each extension's own shell key, so registering
    // here provides an additional signal that makes the entry appear reliably
    // in the new top-level context menu.
    for ext in EXTENSIONS {
        let shell_path =
            format!(r"Software\Classes\.{}\shell\ImageViewer", ext);
        let cmd_path =
            format!(r"Software\Classes\.{}\shell\ImageViewer\command", ext);

        if let Ok((shell_key, _)) = hkcu.create_subkey(&shell_path) {
            let _ = shell_key.set_value("", &"Open with Image Viewer");
            let _ = shell_key.set_value("Icon", &format!("{},0", exe_str));
            let _ = shell_key.set_value("Position", &"Top");
        }

        if let Ok((cmd_key, _)) = hkcu.create_subkey(&cmd_path) {
            let _ = cmd_key.set_value("", &format!("\"{}\" \"%1\"", exe_str));
        }
    }
}

/// Removes the context menu entries registered by [`register_context_menu`].
/// Called on uninstall if an NSIS/WiX uninstall hook invokes the binary with
/// `--unregister-context-menu`.
pub fn unregister_context_menu() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    // Remove the SystemFileAssociations\image entry.
    // delete_subkey_all removes the key and all its children recursively.
    let _ = hkcu.delete_subkey_all(SHELL_KEY);

    // Remove per-extension entries.
    for ext in EXTENSIONS {
        let shell_path =
            format!(r"Software\Classes\.{}\shell\ImageViewer", ext);
        let _ = hkcu.delete_subkey_all(&shell_path);
    }
}
