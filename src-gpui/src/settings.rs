//! 설정, 최근 파일, 아카이브 이어보기 기록과 그 영속화(SPEC §12, §13).
//!
//! 파일 모양은 Tauri 앱의 `settings.json`과 같다: `settings`, `recentFiles`,
//! `archiveProgress` 세 키. 저장 위치만 식별자가 달라 두 앱이 서로 덮어쓰지 않는다.

use std::collections::BTreeMap;
use std::path::PathBuf;

use gpui_kit::{App, Global};
use serde::{Deserialize, Serialize};

use crate::i18n::Language;

pub const RECENT_FILES_LIMIT: usize = 20;
pub const ARCHIVE_PROGRESS_LIMIT: usize = 100;
pub const PALETTE_MRU_LIMIT: usize = 5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheMode {
    #[serde(rename = "off")]
    Off,
    #[default]
    #[serde(rename = "nearby")]
    Nearby,
    #[serde(rename = "extended")]
    Extended,
    #[serde(rename = "memory-1gb")]
    Memory1Gb,
    #[serde(rename = "memory-2gb")]
    Memory2Gb,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheStorageMode {
    Temporary,
    #[default]
    Persistent,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaxResolution {
    #[default]
    #[serde(rename = "original")]
    Original,
    #[serde(rename = "4k")]
    FourK,
    #[serde(rename = "1080p")]
    FullHd,
}

impl MaxResolution {
    /// 긴 변 상한(px). 원본이면 `None`.
    pub fn max_side(self) -> Option<u32> {
        match self {
            Self::Original => None,
            Self::FourK => Some(3840),
            Self::FullHd => Some(1920),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageScalingMode {
    #[default]
    Auto,
    Smooth,
    Pixelated,
}

impl ImageScalingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Smooth => "smooth",
            Self::Pixelated => "pixelated",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewMode {
    #[default]
    Single,
    LeftToRight,
    RightToLeft,
    Webtoon,
}

impl ViewMode {
    pub fn is_dual(self) -> bool {
        matches!(self, Self::LeftToRight | Self::RightToLeft)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewerBackground {
    #[default]
    Theme,
    Black,
    White,
    Checker,
}

impl ViewerBackground {
    /// `B` 단축키 순환: theme → black → white → checker.
    pub fn next(self) -> Self {
        match self {
            Self::Theme => Self::Black,
            Self::Black => Self::White,
            Self::White => Self::Checker,
            Self::Checker => Self::Theme,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortKey {
    #[default]
    Name,
    Date,
    Size,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FitMode {
    Width,
    Height,
    Screen,
    #[default]
    Auto,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DockPosition {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}

impl DockPosition {
    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
}

/// 앱 밝기. 설정 객체가 아니라 저장소의 별도 키로 둔다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

/// 마지막 창 위치와 크기(논리 픽셀).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DockThumbSize {
    #[default]
    S,
    M,
    L,
}

impl DockThumbSize {
    /// 썸네일 한 변(px).
    pub fn side(self) -> f32 {
        match self {
            Self::S => 56.0,
            Self::M => 80.0,
            Self::L => 112.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WheelAction {
    Prev,
    Next,
    ZoomIn,
    ZoomOut,
    #[default]
    None,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MouseAction {
    Pan,
    Prev,
    Next,
    ZoomIn,
    ZoomOut,
    ToggleFullscreen,
    ContextMenu,
    #[default]
    None,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WheelMap {
    #[serde(rename = "wheelUp")]
    pub wheel_up: WheelAction,
    #[serde(rename = "wheelDown")]
    pub wheel_down: WheelAction,
    #[serde(rename = "ctrl+wheelUp")]
    pub ctrl_wheel_up: WheelAction,
    #[serde(rename = "ctrl+wheelDown")]
    pub ctrl_wheel_down: WheelAction,
    #[serde(rename = "shift+wheelUp")]
    pub shift_wheel_up: WheelAction,
    #[serde(rename = "shift+wheelDown")]
    pub shift_wheel_down: WheelAction,
    #[serde(rename = "alt+wheelUp")]
    pub alt_wheel_up: WheelAction,
    #[serde(rename = "alt+wheelDown")]
    pub alt_wheel_down: WheelAction,
}

impl Default for WheelMap {
    fn default() -> Self {
        Self {
            wheel_up: WheelAction::Prev,
            wheel_down: WheelAction::Next,
            ctrl_wheel_up: WheelAction::ZoomIn,
            ctrl_wheel_down: WheelAction::ZoomOut,
            shift_wheel_up: WheelAction::None,
            shift_wheel_down: WheelAction::None,
            alt_wheel_up: WheelAction::None,
            alt_wheel_down: WheelAction::None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MouseMap {
    pub left_drag: MouseAction,
    pub middle_click: MouseAction,
    pub double_click: MouseAction,
    pub right_click: MouseAction,
}

impl Default for MouseMap {
    fn default() -> Self {
        Self {
            left_drag: MouseAction::Pan,
            middle_click: MouseAction::None,
            double_click: MouseAction::ToggleFullscreen,
            right_click: MouseAction::ContextMenu,
        }
    }
}

/// 단축키 동작 ID와 기본 키(SPEC §14.1). 키 표기는 프런트와 같은
/// `Ctrl+Shift+E` 형식이다.
pub const DEFAULT_SHORTCUTS: &[(&str, &str)] = &[
    ("navigatePrev", "ArrowLeft"),
    ("navigateNext", "ArrowRight"),
    ("panLeft", "Ctrl+ArrowLeft"),
    ("panRight", "Ctrl+ArrowRight"),
    ("panUp", "ArrowUp"),
    ("panDown", "ArrowDown"),
    ("zoomIn", "="),
    ("zoomOut", "-"),
    ("resetView", "0"),
    ("fitWidth", "1"),
    ("fitHeight", "2"),
    ("fitScreen", "3"),
    ("openFile", "Ctrl+O"),
    ("closeImage", "Escape"),
    ("toggleExif", "I"),
    ("rotateCW", "R"),
    ("rotateCCW", "Shift+R"),
    ("flipH", "H"),
    ("flipV", "V"),
    ("toggleFullscreen", "F11"),
    ("toggleAlwaysOnTop", "T"),
    ("copyImage", "Ctrl+C"),
    ("trashFile", "Delete"),
    ("revealInExplorer", "Ctrl+Shift+E"),
    ("openExternal", "Ctrl+Shift+O"),
    ("cycleBackground", "B"),
    ("renameFile", "F2"),
    ("copyPath", "Ctrl+Shift+C"),
    ("togglePalette", "Ctrl+K"),
    ("jumpPrev10", "PageUp"),
    ("jumpNext10", "PageDown"),
    ("jumpFirst", "Home"),
    ("jumpLast", "End"),
    ("toggleGrid", "G"),
    ("toggleGifPlayback", "P"),
    ("gifPrevFrame", ","),
    ("gifNextFrame", "."),
];

pub type ShortcutMap = BTreeMap<String, String>;

pub fn default_shortcuts() -> ShortcutMap {
    DEFAULT_SHORTCUTS
        .iter()
        .map(|(id, key)| ((*id).to_owned(), (*key).to_owned()))
        .collect()
}

/// 알 수 없는 동작은 버리고 빠진 동작은 기본값으로 채운다. 빈 문자열은 사용자가
/// 해제한 바인딩이라 그대로 둔다.
fn sanitize_shortcuts(saved: ShortcutMap) -> ShortcutMap {
    let mut map = default_shortcuts();
    for (id, key) in saved {
        // 쓸 수 없는 조합(Tab, Meta 등)이 저장돼 있으면 기본값을 유지한다.
        if let Some(slot) = map.get_mut(&id)
            && (key.is_empty() || crate::keys::is_valid_spec(&key))
        {
            *slot = key;
        }
    }
    map
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub language: Language,
    pub loop_navigation: bool,
    pub cache_mode: CacheMode,
    pub cache_storage_mode: CacheStorageMode,
    pub max_resolution: MaxResolution,
    pub image_scaling_mode: ImageScalingMode,
    pub auto_detect_pixel_art: bool,
    pub view_mode: ViewMode,
    pub webtoon_image_gap: u32,
    pub webtoon_page_boundaries: bool,
    pub webtoon_fit_width: bool,
    pub webtoon_show_progress: bool,
    pub webtoon_thumbnail_jump: bool,
    pub auto_open_last_file: bool,
    pub record_recent_files: bool,
    pub viewer_background: ViewerBackground,
    #[serde(rename = "autoHideUI")]
    pub auto_hide_ui: bool,
    pub menu_bar_hidden: bool,
    pub always_on_top: bool,
    pub sort_key: SortKey,
    pub sort_descending: bool,
    pub include_subfolders: bool,
    pub skip_broken_files: bool,
    pub resume_reading: bool,
    pub show_cover_alone: bool,
    pub show_wide_page_alone: bool,
    pub show_comic_info: bool,
    pub comic_auto_dual_view: bool,
    pub fit_mode: FitMode,
    pub dock_position: DockPosition,
    pub dock_visible: bool,
    pub dock_thumb_size: DockThumbSize,
    pub dock_show_name: bool,
    pub dock_show_index: bool,
    pub shortcuts: ShortcutMap,
    pub wheel: WheelMap,
    pub mouse: MouseMap,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: Language::Ko,
            loop_navigation: false,
            cache_mode: CacheMode::Nearby,
            cache_storage_mode: CacheStorageMode::Persistent,
            max_resolution: MaxResolution::Original,
            image_scaling_mode: ImageScalingMode::Auto,
            auto_detect_pixel_art: true,
            view_mode: ViewMode::Single,
            webtoon_image_gap: 8,
            webtoon_page_boundaries: false,
            webtoon_fit_width: false,
            webtoon_show_progress: true,
            webtoon_thumbnail_jump: true,
            auto_open_last_file: false,
            record_recent_files: true,
            viewer_background: ViewerBackground::Theme,
            auto_hide_ui: false,
            menu_bar_hidden: false,
            always_on_top: false,
            sort_key: SortKey::Name,
            sort_descending: false,
            include_subfolders: false,
            skip_broken_files: false,
            resume_reading: true,
            show_cover_alone: true,
            show_wide_page_alone: true,
            show_comic_info: true,
            comic_auto_dual_view: true,
            fit_mode: FitMode::Auto,
            dock_position: DockPosition::Bottom,
            dock_visible: true,
            dock_thumb_size: DockThumbSize::S,
            dock_show_name: false,
            dock_show_index: false,
            shortcuts: default_shortcuts(),
            wheel: WheelMap::default(),
            mouse: MouseMap::default(),
        }
    }
}

impl Settings {
    fn sanitized(mut self) -> Self {
        self.webtoon_image_gap = self.webtoon_image_gap.min(64);
        self.shortcuts = sanitize_shortcuts(std::mem::take(&mut self.shortcuts));
        // 왼쪽 드래그는 pan 또는 none만 허용한다(SPEC §14.3).
        if !matches!(self.mouse.left_drag, MouseAction::Pan | MouseAction::None) {
            self.mouse.left_drag = MouseAction::Pan;
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveProgress {
    pub entry: String,
    pub index: usize,
    pub total: usize,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct StoreFile {
    settings: serde_json::Value,
    recent_files: Vec<String>,
    /// 삽입 순서를 LRU로 쓰므로 (경로, 기록) 쌍 목록으로 들고, 파일에는 객체로 쓴다.
    #[serde(with = "progress_map")]
    archive_progress: Vec<(String, ArchiveProgress)>,
    theme: serde_json::Value,
    window: serde_json::Value,
    palette_mru: Vec<String>,
}

mod progress_map {
    use super::ArchiveProgress;
    use serde::ser::SerializeMap as _;
    use serde::{Deserialize as _, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        items: &[(String, ArchiveProgress)],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(items.len()))?;
        for (path, progress) in items {
            map.serialize_entry(path, progress)?;
        }
        map.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<(String, ArchiveProgress)>, D::Error> {
        // serde_json의 preserve_order 없이도 파일 순서를 지키려고 직접 순회한다.
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Vec<(String, ArchiveProgress)>;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("archive progress map")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut access: A,
            ) -> Result<Self::Value, A::Error> {
                let mut out = Vec::new();
                while let Some(key) = access.next_key::<String>()? {
                    // 한 항목이 깨져도 나머지 기록은 살린다.
                    let value = access.next_value::<serde_json::Value>()?;
                    if let Ok(progress) = ArchiveProgress::deserialize(value) {
                        out.push((key, progress));
                    }
                }
                Ok(out)
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}

/// 앱 전역 설정 저장소.
pub struct SettingsStore {
    pub settings: Settings,
    pub recent_files: Vec<String>,
    pub theme: ThemeChoice,
    pub window: Option<WindowState>,
    /// 명령 팔레트에서 최근에 실행한 명령 ID(최신 먼저).
    pub palette_mru: Vec<String>,
    archive_progress: Vec<(String, ArchiveProgress)>,
    path: Option<PathBuf>,
}

impl Global for SettingsStore {}

impl SettingsStore {
    /// `dir/settings.json`을 읽는다. 파일이 없거나 깨졌으면 기본값으로 시작하고,
    /// 필드 단위로 손상된 값만 기본값으로 되돌린다.
    pub fn load(dir: Option<PathBuf>) -> Self {
        let path = dir.map(|dir| dir.join("settings.json"));
        let file = path
            .as_ref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<StoreFile>(&bytes).ok())
            .unwrap_or_default();
        let has_language = file.settings.get("language").is_some();
        let mut settings = restore_settings(file.settings).sanitized();
        if !has_language {
            settings.language = crate::i18n::detect_system_language();
        }
        let mut recent_files = file.recent_files;
        recent_files.truncate(RECENT_FILES_LIMIT);
        Self {
            settings,
            recent_files,
            // 손상된 값은 기본값으로 되돌린다.
            theme: serde_json::from_value(file.theme).unwrap_or_default(),
            window: serde_json::from_value(file.window).ok(),
            palette_mru: file.palette_mru,
            archive_progress: file.archive_progress,
            path,
        }
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// 설정을 고치고 저장한다.
    pub fn update(cx: &mut App, edit: impl FnOnce(&mut Settings)) {
        let store = cx.global_mut::<Self>();
        edit(&mut store.settings);
        store.save();
    }

    pub fn set_theme(cx: &mut App, theme: ThemeChoice) {
        let store = cx.global_mut::<Self>();
        store.theme = theme;
        store.save();
    }

    /// 창 상태는 리사이즈마다 바뀌므로 메모리에만 두고 `flush`에서 쓴다.
    pub fn set_window(cx: &mut App, window: WindowState) {
        cx.global_mut::<Self>().window = Some(window);
    }

    pub fn flush(cx: &App) {
        cx.global::<Self>().save();
    }

    pub fn push_palette_mru(cx: &mut App, command: &str) {
        let store = cx.global_mut::<Self>();
        store.palette_mru.retain(|item| item != command);
        store.palette_mru.insert(0, command.to_owned());
        store.palette_mru.truncate(PALETTE_MRU_LIMIT);
        store.save();
    }

    pub fn push_recent(cx: &mut App, path: &str) {
        let store = cx.global_mut::<Self>();
        store.recent_files.retain(|item| item != path);
        store.recent_files.insert(0, path.to_owned());
        store.recent_files.truncate(RECENT_FILES_LIMIT);
        store.save();
    }

    pub fn remove_recent(cx: &mut App, path: &str) {
        let store = cx.global_mut::<Self>();
        store.recent_files.retain(|item| item != path);
        store.save();
    }

    pub fn replace_recent(cx: &mut App, old: &str, new: &str) {
        let store = cx.global_mut::<Self>();
        for item in &mut store.recent_files {
            if item == old {
                *item = new.to_owned();
            }
        }
        store.save();
    }

    pub fn clear_recent(cx: &mut App) {
        let store = cx.global_mut::<Self>();
        store.recent_files.clear();
        store.save();
    }

    pub fn archive_progress(&self, archive: &str) -> Option<&ArchiveProgress> {
        self.archive_progress
            .iter()
            .find(|(path, _)| path == archive)
            .map(|(_, progress)| progress)
    }

    pub fn set_archive_progress(cx: &mut App, archive: &str, progress: ArchiveProgress) {
        let store = cx.global_mut::<Self>();
        store.archive_progress.retain(|(path, _)| path != archive);
        store.archive_progress.push((archive.to_owned(), progress));
        let overflow = store
            .archive_progress
            .len()
            .saturating_sub(ARCHIVE_PROGRESS_LIMIT);
        store.archive_progress.drain(..overflow);
        store.save();
    }

    fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let file = StoreFile {
            settings: serde_json::to_value(&self.settings).unwrap_or_default(),
            recent_files: self.recent_files.clone(),
            archive_progress: self.archive_progress.clone(),
            theme: serde_json::to_value(self.theme).unwrap_or_default(),
            window: serde_json::to_value(self.window).unwrap_or_default(),
            palette_mru: self.palette_mru.clone(),
        };
        let result = serde_json::to_vec_pretty(&file)
            .map_err(std::io::Error::other)
            .and_then(|bytes| {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                // 쓰는 도중 종료돼도 기존 파일이 깨지지 않게 임시 파일로 쓴 뒤 바꾼다.
                let tmp = path.with_extension("json.tmp");
                std::fs::write(&tmp, bytes)?;
                std::fs::rename(&tmp, path)
            });
        if let Err(error) = result {
            log::warn!("[settings] failed to save {}: {error}", path.display());
        }
    }
}

/// 저장된 객체에서 읽을 수 있는 필드만 살리고 나머지는 기본값으로 둔다.
fn restore_settings(saved: serde_json::Value) -> Settings {
    if let Ok(settings) = serde_json::from_value::<Settings>(saved.clone()) {
        return settings;
    }
    let serde_json::Value::Object(saved) = saved else {
        return Settings::default();
    };
    let mut merged = match serde_json::to_value(Settings::default()) {
        Ok(serde_json::Value::Object(map)) => map,
        _ => return Settings::default(),
    };
    for (key, value) in saved {
        let Some(slot) = merged.get_mut(&key) else {
            continue;
        };
        let previous = std::mem::replace(slot, value);
        let candidate = serde_json::Value::Object(merged.clone());
        if serde_json::from_value::<Settings>(candidate).is_err() {
            merged.insert(key, previous);
        }
    }
    serde_json::from_value(serde_json::Value::Object(merged)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip_with_frontend_key_names() {
        let value = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(value["viewMode"], "single");
        assert_eq!(value["cacheMode"], "nearby");
        assert_eq!(value["maxResolution"], "original");
        assert_eq!(value["autoHideUI"], false);
        assert_eq!(value["wheel"]["ctrl+wheelUp"], "zoomIn");
        assert_eq!(value["mouse"]["doubleClick"], "toggleFullscreen");
        assert_eq!(value["shortcuts"]["rotateCCW"], "Shift+R");
        let back: Settings = serde_json::from_value(value).unwrap();
        assert_eq!(back, Settings::default());
    }

    #[test]
    fn corrupt_field_falls_back_without_losing_the_rest() {
        let saved = serde_json::json!({
            "viewMode": "webtoon",
            "cacheStorageMode": "nonsense",
            "webtoonImageGap": 24,
        });
        let settings = restore_settings(saved);
        assert_eq!(settings.view_mode, ViewMode::Webtoon);
        assert_eq!(settings.cache_storage_mode, CacheStorageMode::Persistent);
        assert_eq!(settings.webtoon_image_gap, 24);
    }

    #[test]
    fn sanitize_fills_missing_shortcuts_and_keeps_unbound() {
        let settings = Settings {
            shortcuts: ShortcutMap::from([
                ("zoomIn".to_owned(), String::new()),
                ("bogus".to_owned(), "Q".to_owned()),
            ]),
            mouse: MouseMap {
                left_drag: MouseAction::Next,
                ..MouseMap::default()
            },
            ..Settings::default()
        };
        let settings = settings.sanitized();
        assert_eq!(settings.shortcuts["zoomIn"], "");
        assert_eq!(settings.shortcuts["navigateNext"], "ArrowRight");
        assert!(!settings.shortcuts.contains_key("bogus"));
        assert_eq!(settings.mouse.left_drag, MouseAction::Pan);
    }

    #[test]
    fn store_persists_recent_files_and_progress_order() {
        let dir =
            std::env::temp_dir().join(format!("araview-gpui-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut store = SettingsStore::load(Some(dir.clone()));
        store.recent_files = vec!["b.png".into(), "a.png".into()];
        store.archive_progress = vec![
            (
                "z.cbz".into(),
                ArchiveProgress {
                    entry: "1.png".into(),
                    index: 0,
                    total: 3,
                },
            ),
            (
                "a.cbz".into(),
                ArchiveProgress {
                    entry: "2.png".into(),
                    index: 1,
                    total: 3,
                },
            ),
        ];
        store.save();
        let loaded = SettingsStore::load(Some(dir.clone()));
        assert_eq!(loaded.recent_files, ["b.png", "a.png"]);
        let order: Vec<_> = loaded
            .archive_progress
            .iter()
            .map(|(path, _)| path.as_str())
            .collect();
        assert_eq!(order, ["z.cbz", "a.cbz"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
