use std::path::{Path, PathBuf};

use serde::Serialize;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE};
use winreg::{RegKey, HKEY};

use crate::image::SUPPORTED_EXTENSIONS;

const BUNDLE_ID: &str = "com.tauri-image-viewer.app";
const APP_NAME: &str = "Image Viewer";
const APP_DESCRIPTION: &str = "Image Viewer";
const CAPABILITIES_PATH: &str = r"Software\Image Viewer\Capabilities";

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct FileAssociation {
    pub extension: String,
    pub associated: bool,
    pub current_prog_id: Option<String>,
    pub needs_os_confirmation: bool,
}

pub fn list_associations() -> Result<Vec<FileAssociation>, String> {
    let exe = current_exe()?;
    ensure_application_registration(&exe)?;
    SUPPORTED_EXTENSIONS
        .iter()
        .map(|ext| status_for(ext, &exe))
        .collect()
}

pub fn set_association(extension: &str, associate: bool) -> Result<FileAssociation, String> {
    let ext = parse_extension(extension)?;
    let exe = current_exe()?;
    ensure_application_registration(&exe)?;
    apply_association(&ext, associate, &exe)?;
    notify_assoc_changed();
    status_for(&ext, &exe)
}

pub fn set_all_associations(associate: bool) -> Result<Vec<FileAssociation>, String> {
    let exe = current_exe()?;
    ensure_application_registration(&exe)?;
    for ext in SUPPORTED_EXTENSIONS {
        apply_association(ext, associate, &exe)?;
    }
    notify_assoc_changed();
    SUPPORTED_EXTENSIONS
        .iter()
        .map(|ext| status_for(ext, &exe))
        .collect()
}

pub fn open_default_apps_settings() -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg("ms-settings:defaultapps")
        .spawn()
        .map_err(|e| format!("Failed to open Windows default apps settings: {e}"))?;
    Ok(())
}

pub fn prog_id_for(ext: &str) -> String {
    format!("{BUNDLE_ID}.{ext}")
}

pub fn parse_extension(raw: &str) -> Result<String, String> {
    let ext = raw.trim().trim_start_matches('.').to_lowercase();
    if ext.is_empty() {
        return Err("Extension is empty".to_string());
    }
    if !SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(format!("Unsupported extension: {raw}"));
    }
    Ok(ext)
}

pub fn is_our_prog_id(prog_id: &str) -> bool {
    let prefix = format!("{BUNDLE_ID}.");
    prog_id.eq_ignore_ascii_case(BUNDLE_ID) || starts_with_ignore_ascii_case(prog_id, &prefix)
}

pub fn extract_exe_from_command(command: &str) -> Option<String> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }
    if let Some(rest) = command.strip_prefix('"') {
        return rest.split('"').next().map(ToOwned::to_owned);
    }
    command.split_whitespace().next().map(ToOwned::to_owned)
}

fn current_exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("Failed to get executable path: {e}"))
}

fn status_for(ext: &str, exe: &Path) -> Result<FileAssociation, String> {
    let user_choice = read_user_choice(ext);
    let classes_default = read_classes_default(HKEY_CURRENT_USER, ext)
        .or_else(|| read_classes_default(HKEY_LOCAL_MACHINE, ext));
    let current_prog_id = user_choice.clone().or(classes_default);
    let associated = current_prog_id
        .as_deref()
        .map(|prog_id| is_handled_by_us(prog_id, exe))
        .unwrap_or(false);
    let needs_os_confirmation = user_choice.is_some() && !associated;

    Ok(FileAssociation {
        extension: ext.to_string(),
        associated,
        current_prog_id,
        needs_os_confirmation,
    })
}

fn is_handled_by_us(prog_id: &str, exe: &Path) -> bool {
    if is_our_prog_id(prog_id) {
        return true;
    }
    read_prog_id_command(HKEY_CURRENT_USER, prog_id)
        .or_else(|| read_prog_id_command(HKEY_LOCAL_MACHINE, prog_id))
        .and_then(|command| extract_exe_from_command(&command))
        .is_some_and(|command_exe| paths_equal(&command_exe, exe))
}

fn apply_association(ext: &str, associate: bool, exe: &Path) -> Result<(), String> {
    if associate {
        associate_extension(ext, exe)
    } else {
        unassociate_extension(ext, exe)
    }
}

fn associate_extension(ext: &str, exe: &Path) -> Result<(), String> {
    let prog_id = prog_id_for(ext);
    write_prog_id(&prog_id, ext, exe)?;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (ext_key, _) = hkcu.create_subkey(classes_ext_path(ext)).map_err(reg_err)?;

    if let Ok(current) = ext_key.get_value::<String, _>("") {
        if !current.is_empty() && !is_our_prog_id(&current) {
            let backup_name = format!("{prog_id}_backup");
            let _ = ext_key.set_value(&backup_name, &current);
        }
    }

    ext_key.set_value("", &prog_id).map_err(reg_err)?;

    let (open_with, _) = ext_key.create_subkey("OpenWithProgids").map_err(reg_err)?;
    open_with.set_value(&prog_id, &"").map_err(reg_err)?;

    let (file_exts, _) = hkcu
        .create_subkey(file_exts_open_with_path(ext))
        .map_err(reg_err)?;
    file_exts.set_value(&prog_id, &"").map_err(reg_err)?;

    Ok(())
}

fn unassociate_extension(ext: &str, exe: &Path) -> Result<(), String> {
    let prog_id = prog_id_for(ext);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(ext_key) = hkcu.open_subkey_with_flags(classes_ext_path(ext), KEY_READ | KEY_SET_VALUE)
    else {
        return Ok(());
    };

    if let Ok(current) = ext_key.get_value::<String, _>("") {
        if is_handled_by_us(&current, exe) {
            let backup_name = format!("{current}_backup");
            if let Ok(previous) = ext_key.get_value::<String, _>(&backup_name) {
                ext_key.set_value("", &previous).map_err(reg_err)?;
            } else {
                let _ = ext_key.delete_value("");
            }
        }
    }

    if let Ok(open_with) = ext_key.open_subkey_with_flags("OpenWithProgids", KEY_SET_VALUE) {
        let _ = open_with.delete_value(&prog_id);
    }

    if let Ok(file_exts) = hkcu.open_subkey_with_flags(file_exts_open_with_path(ext), KEY_SET_VALUE)
    {
        let _ = file_exts.delete_value(&prog_id);
    }

    Ok(())
}

fn ensure_application_registration(exe: &Path) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (capabilities, _) = hkcu.create_subkey(CAPABILITIES_PATH).map_err(reg_err)?;
    capabilities
        .set_value("ApplicationName", &APP_NAME)
        .map_err(reg_err)?;
    capabilities
        .set_value("ApplicationDescription", &APP_DESCRIPTION)
        .map_err(reg_err)?;
    capabilities
        .set_value("ApplicationIcon", &default_icon(exe))
        .map_err(reg_err)?;

    let (file_associations, _) = capabilities
        .create_subkey("FileAssociations")
        .map_err(reg_err)?;
    for ext in SUPPORTED_EXTENSIONS {
        file_associations
            .set_value(format!(".{ext}"), &prog_id_for(ext))
            .map_err(reg_err)?;
        write_prog_id(&prog_id_for(ext), ext, exe)?;
    }

    let (registered, _) = hkcu
        .create_subkey(r"Software\RegisteredApplications")
        .map_err(reg_err)?;
    registered
        .set_value(APP_NAME, &CAPABILITIES_PATH)
        .map_err(reg_err)?;
    Ok(())
}

fn write_prog_id(prog_id: &str, ext: &str, exe: &Path) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (prog_key, _) = hkcu
        .create_subkey(format!(r"Software\Classes\{prog_id}"))
        .map_err(reg_err)?;
    prog_key
        .set_value("", &format!("{APP_NAME} {ext} file"))
        .map_err(reg_err)?;

    let (icon_key, _) = prog_key.create_subkey("DefaultIcon").map_err(reg_err)?;
    icon_key
        .set_value("", &default_icon(exe))
        .map_err(reg_err)?;

    let (shell_key, _) = prog_key.create_subkey("shell").map_err(reg_err)?;
    shell_key.set_value("", &"open").map_err(reg_err)?;

    let (open_key, _) = shell_key.create_subkey("open").map_err(reg_err)?;
    open_key
        .set_value("", &format!("Open with {APP_NAME}"))
        .map_err(reg_err)?;

    let (command_key, _) = open_key.create_subkey("command").map_err(reg_err)?;
    command_key
        .set_value("", &open_command(exe))
        .map_err(reg_err)?;
    Ok(())
}

fn read_user_choice(ext: &str) -> Option<String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey(format!(
            r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.{ext}\UserChoice"
        ))
        .ok()?;
    nonempty_reg_string(key.get_value("ProgId").ok())
}

fn read_classes_default(hive: HKEY, ext: &str) -> Option<String> {
    let root = RegKey::predef(hive);
    let key = root.open_subkey(classes_ext_path(ext)).ok()?;
    nonempty_reg_string(key.get_value("").ok())
}

fn read_prog_id_command(hive: HKEY, prog_id: &str) -> Option<String> {
    let root = RegKey::predef(hive);
    let key = root
        .open_subkey(format!(r"Software\Classes\{prog_id}\shell\open\command"))
        .ok()?;
    nonempty_reg_string(key.get_value("").ok())
}

fn nonempty_reg_string(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}

fn classes_ext_path(ext: &str) -> String {
    format!(r"Software\Classes\.{ext}")
}

fn file_exts_open_with_path(ext: &str) -> String {
    format!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.{ext}\OpenWithProgids")
}

fn default_icon(exe: &Path) -> String {
    format!("\"{}\",0", exe.display())
}

fn open_command(exe: &Path) -> String {
    format!("\"{}\" \"%1\"", exe.display())
}

fn paths_equal(command_exe: &str, exe: &Path) -> bool {
    Path::new(command_exe)
        .to_string_lossy()
        .eq_ignore_ascii_case(&exe.to_string_lossy())
}

fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value.len() >= prefix.len()
        && value.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

fn reg_err(error: impl std::fmt::Display) -> String {
    format!("Registry error: {error}")
}

fn notify_assoc_changed() {
    const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
    const SHCNF_IDLIST: u32 = 0x0000;
    // SAFETY: SHCNE_ASSOCCHANGED with SHCNF_IDLIST and null items is the documented
    // shell notification for file association changes.
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
}

#[link(name = "shell32")]
extern "system" {
    fn SHChangeNotify(
        w_event_id: i32,
        u_flags: u32,
        dw_item1: *const std::ffi::c_void,
        dw_item2: *const std::ffi::c_void,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prog_id_uses_bundle_id_and_extension() {
        assert_eq!(prog_id_for("png"), "com.tauri-image-viewer.app.png");
        assert_eq!(prog_id_for("cbz"), "com.tauri-image-viewer.app.cbz");
    }

    #[test]
    fn parse_extension_normalizes_and_rejects_unknown() {
        assert_eq!(parse_extension(".PNG").unwrap(), "png");
        assert_eq!(parse_extension("Jpeg").unwrap(), "jpeg");
        assert!(parse_extension("").is_err());
        assert!(parse_extension("pdf").is_err());
    }

    #[test]
    fn is_our_prog_id_matches_bundle_prefix() {
        assert!(is_our_prog_id("com.tauri-image-viewer.app.png"));
        assert!(is_our_prog_id("COM.TAURI-IMAGE-VIEWER.APP.JPG"));
        assert!(!is_our_prog_id("Image"));
        assert!(!is_our_prog_id("jpegfile"));
        assert!(!is_our_prog_id("AppX123"));
    }

    #[test]
    fn extract_exe_from_command_handles_quoted_and_plain_paths() {
        assert_eq!(
            extract_exe_from_command(r#""C:\Program Files\Image Viewer\app.exe" "%1""#).as_deref(),
            Some(r"C:\Program Files\Image Viewer\app.exe")
        );
        assert_eq!(
            extract_exe_from_command(r"C:\Apps\viewer.exe %1").as_deref(),
            Some(r"C:\Apps\viewer.exe")
        );
        assert_eq!(extract_exe_from_command("   "), None);
    }

    #[test]
    fn open_command_quotes_executable_path() {
        let exe = Path::new(r"C:\Program Files\Image Viewer\app.exe");
        assert_eq!(
            open_command(exe),
            r#""C:\Program Files\Image Viewer\app.exe" "%1""#
        );
        assert_eq!(
            default_icon(exe),
            r#""C:\Program Files\Image Viewer\app.exe",0"#
        );
    }
}
