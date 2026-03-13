use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

const SHELL_KEY: &str =
    r"Software\Classes\SystemFileAssociations\image\shell\ImageViewer";
const COMMAND_KEY: &str =
    r"Software\Classes\SystemFileAssociations\image\shell\ImageViewer\command";

/// Registers a "Open with Image Viewer" context menu entry for all image files
/// under HKCU so that no administrator privileges are required.
pub fn register_context_menu() {
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    let exe_str = exe_path.to_string_lossy();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    if let Ok((shell_key, _)) = hkcu.create_subkey(SHELL_KEY) {
        let _ = shell_key.set_value("", &"Open with Image Viewer");
        let _ = shell_key.set_value("Icon", &format!("{},0", exe_str));
    }

    if let Ok((cmd_key, _)) = hkcu.create_subkey(COMMAND_KEY) {
        let _ = cmd_key.set_value("", &format!("\"{}\" \"%1\"", exe_str));
    }
}

/// Removes the context menu entry registered by [`register_context_menu`].
/// Called on uninstall if an NSIS/WiX uninstall hook invokes the binary with
/// `--unregister-context-menu`.
pub fn unregister_context_menu() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    // delete_subkey_all removes the key and all its children recursively.
    let _ = hkcu.delete_subkey_all(SHELL_KEY);
}
