//! 단축키 표기와 매칭(SPEC §14).
//!
//! 뷰어 단축키는 GPUI 키맵에 바인딩하지 않고 뷰어 루트의 키 입력에서 직접 찾는다.
//! 설정에서 재할당하면 바로 반영되고, 다이얼로그나 입력에 포커스가 있을 때는 키
//! 입력이 뷰어 루트로 오지 않아 자연히 막힌다.

use gpui_kit::Keystroke;

use crate::settings::ShortcutMap;

/// 동작 ID와 그 이름의 번역 키.
pub const ACTION_LABELS: &[(&str, &str)] = &[
    ("navigatePrev", "menu.prev"),
    ("navigateNext", "menu.next"),
    ("panLeft", "menu.panLeft"),
    ("panRight", "menu.panRight"),
    ("panUp", "menu.panUp"),
    ("panDown", "menu.panDown"),
    ("zoomIn", "menu.zoomIn"),
    ("zoomOut", "menu.zoomOut"),
    ("resetView", "menu.actualSize"),
    ("fitWidth", "menu.fitWidth"),
    ("fitHeight", "menu.fitHeight"),
    ("fitScreen", "menu.fitScreen"),
    ("openFile", "menu.open"),
    ("closeImage", "menu.closeImage"),
    ("toggleExif", "menu.toggleExif"),
    ("rotateCW", "menu.rotateCw"),
    ("rotateCCW", "menu.rotateCcw"),
    ("flipH", "menu.flipH"),
    ("flipV", "menu.flipV"),
    ("toggleFullscreen", "menu.toggleFullscreen"),
    ("toggleAlwaysOnTop", "menu.toggleAlwaysOnTop"),
    ("copyImage", "menu.copyImage"),
    ("trashFile", "menu.trash"),
    ("revealInExplorer", "menu.reveal"),
    ("openExternal", "menu.openExternal"),
    ("cycleBackground", "menu.cycleBg"),
    ("renameFile", "menu.rename"),
    ("copyPath", "menu.copyPath"),
    ("togglePalette", "palette.open"),
    ("jumpPrev10", "menu.jumpPrev10"),
    ("jumpNext10", "menu.jumpNext10"),
    ("jumpFirst", "menu.jumpFirst"),
    ("jumpLast", "menu.jumpLast"),
    ("toggleGrid", "menu.toggleGrid"),
    ("toggleGifPlayback", "menu.gifPlayPause"),
    ("gifPrevFrame", "menu.gifPrevFrame"),
    ("gifNextFrame", "menu.gifNextFrame"),
];

pub fn label_key(action: &str) -> Option<&'static str> {
    ACTION_LABELS
        .iter()
        .find(|(id, _)| *id == action)
        .map(|(_, key)| *key)
}

/// GPUI 키 이름과 설정 표기(`ArrowLeft` 등)의 대응.
const KEY_NAMES: &[(&str, &str)] = &[
    ("left", "ArrowLeft"),
    ("right", "ArrowRight"),
    ("up", "ArrowUp"),
    ("down", "ArrowDown"),
    ("escape", "Escape"),
    ("delete", "Delete"),
    ("backspace", "Backspace"),
    ("enter", "Enter"),
    ("pageup", "PageUp"),
    ("pagedown", "PageDown"),
    ("home", "Home"),
    ("end", "End"),
    ("insert", "Insert"),
    ("space", "Space"),
];

/// 키 입력을 설정 표기(`Ctrl+Shift+E`)로 바꾼다. 수식키만 눌렀거나 허용하지 않는
/// 조합(`Meta`, `Tab`, §14.3)이면 `None`이다.
pub fn spec_from_keystroke(keystroke: &Keystroke) -> Option<String> {
    let modifiers = keystroke.modifiers;
    if modifiers.platform || modifiers.function {
        return None;
    }
    let key = keystroke.key.as_str();
    if key.is_empty() || matches!(key, "tab" | "control" | "shift" | "alt" | "capslock") {
        return None;
    }
    let name = match KEY_NAMES.iter().find(|(gpui, _)| *gpui == key) {
        Some((_, display)) => (*display).to_owned(),
        None if key.len() > 1 && key.starts_with('f') && key[1..].parse::<u8>().is_ok() => {
            key.to_ascii_uppercase()
        }
        None if key.chars().count() == 1 => key.to_uppercase(),
        None => return None,
    };
    let mut spec = String::new();
    if modifiers.control {
        spec.push_str("Ctrl+");
    }
    if modifiers.shift {
        spec.push_str("Shift+");
    }
    if modifiers.alt {
        spec.push_str("Alt+");
    }
    spec.push_str(&name);
    Some(spec)
}

/// 비교용 정규형: 수식키 순서를 고정하고 대소문자를 무시한다.
fn canonical(spec: &str) -> Option<String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }
    let mut parts: Vec<&str> = spec.split('+').collect();
    // 키 자체가 `+`면 마지막 조각이 비어 있다.
    let key = match parts.pop()? {
        "" => "+".to_owned(),
        key => key.trim().to_ascii_lowercase(),
    };
    let (mut ctrl, mut shift, mut alt) = (false, false, false);
    for part in parts {
        match part.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" => alt = true,
            _ => return None,
        }
    }
    if matches!(key.as_str(), "tab" | "meta") {
        return None;
    }
    Some(format!(
        "{}{}{}{key}",
        if ctrl { "ctrl+" } else { "" },
        if shift { "shift+" } else { "" },
        if alt { "alt+" } else { "" },
    ))
}

/// 설정에 쓸 수 있는 표기인지(§14.3).
pub fn is_valid_spec(spec: &str) -> bool {
    canonical(spec).is_some()
}

/// 키 입력에 할당된 동작 ID.
pub fn action_for<'a>(shortcuts: &'a ShortcutMap, keystroke: &Keystroke) -> Option<&'a str> {
    let pressed = canonical(&spec_from_keystroke(keystroke)?)?;
    shortcuts
        .iter()
        .find(|(_, spec)| canonical(spec).as_deref() == Some(pressed.as_str()))
        .map(|(id, _)| id.as_str())
}

/// `spec`을 이미 쓰고 있는 다른 동작.
pub fn conflict<'a>(shortcuts: &'a ShortcutMap, action: &str, spec: &str) -> Option<&'a str> {
    let wanted = canonical(spec)?;
    shortcuts
        .iter()
        .find(|(id, other)| {
            id.as_str() != action && canonical(other).as_deref() == Some(wanted.as_str())
        })
        .map(|(id, _)| id.as_str())
}
