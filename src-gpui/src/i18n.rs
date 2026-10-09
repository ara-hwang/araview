//! UI 문구. 번역 원본은 Tauri 프런트와 같은 `src/i18n/locales/*.json`이다.

use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU8, Ordering};

use gpui_kit::SharedString;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Ko,
    En,
}

type Catalog = HashMap<String, String>;

static KO: LazyLock<Catalog> =
    LazyLock::new(|| flatten(include_str!("../../src/i18n/locales/ko.json")));
static EN: LazyLock<Catalog> =
    LazyLock::new(|| flatten(include_str!("../../src/i18n/locales/en.json")));

static CURRENT: AtomicU8 = AtomicU8::new(0);

fn flatten(source: &str) -> Catalog {
    fn walk(prefix: &str, value: &serde_json::Value, out: &mut Catalog) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, child) in map {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    walk(&path, child, out);
                }
            }
            serde_json::Value::String(text) => {
                out.insert(prefix.to_owned(), text.clone());
            }
            _ => {}
        }
    }
    let mut out = Catalog::new();
    // 번들에 포함된 고정 파일이라 파싱 실패는 빌드 결함이다. 키를 그대로 보여주는
    // 쪽으로 떨어지게 빈 카탈로그를 쓴다.
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(source) {
        walk("", &value, &mut out);
    }
    out
}

pub fn set_language(language: Language) {
    CURRENT.store(language as u8, Ordering::Relaxed);
}

pub fn language() -> Language {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Language::En,
        _ => Language::Ko,
    }
}

/// 시스템 UI 언어가 한국어면 `ko`, 아니면 `en`.
pub fn detect_system_language() -> Language {
    let locale = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LANG"))
        .ok()
        .or_else(windows_ui_locale)
        .unwrap_or_default();
    if locale.to_ascii_lowercase().starts_with("ko") {
        Language::Ko
    } else {
        Language::En
    }
}

fn windows_ui_locale() -> Option<String> {
    const LOCALE_NAME_MAX_LENGTH: usize = 85;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultLocaleName(locale_name: *mut u16, cch: i32) -> i32;
    }
    let mut buffer = [0u16; LOCALE_NAME_MAX_LENGTH];
    // SAFETY: 버퍼 길이를 그대로 넘기며, 함수는 그 길이 안에서만 NUL 종료 문자열을 쓴다.
    let written =
        unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), LOCALE_NAME_MAX_LENGTH as i32) };
    if written <= 1 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..written as usize - 1]))
}

fn lookup(key: &str) -> Option<&'static str> {
    let primary: &'static Catalog = match language() {
        Language::Ko => &KO,
        Language::En => &EN,
    };
    primary.get(key).or_else(|| KO.get(key)).map(String::as_str)
}

/// 번역 문구. 키가 없으면 키를 그대로 돌려줘 누락이 화면에 드러나게 한다.
pub fn t(key: &str) -> SharedString {
    match lookup(key) {
        Some(text) => SharedString::new_static(text),
        None => SharedString::from(key.to_owned()),
    }
}

/// 언어 설정과 무관한 영문 문구. 명령 팔레트의 영문 별칭 검색에 쓴다.
pub fn t_en(key: &str) -> SharedString {
    match EN.get(key) {
        Some(text) => SharedString::new_static(text.as_str()),
        None => SharedString::from(key.to_owned()),
    }
}

/// `{{name}}` 자리표시자를 채운 번역 문구.
pub fn t_with(key: &str, args: &[(&str, &dyn std::fmt::Display)]) -> SharedString {
    let Some(template) = lookup(key) else {
        return SharedString::from(key.to_owned());
    };
    let mut text = template.to_owned();
    for (name, value) in args {
        text = text.replace(&format!("{{{{{name}}}}}"), &value.to_string());
    }
    SharedString::from(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogs_share_every_key() {
        let mut ko: Vec<_> = KO.keys().collect();
        let mut en: Vec<_> = EN.keys().collect();
        ko.sort();
        en.sort();
        assert!(!ko.is_empty());
        assert_eq!(ko, en);
    }

    #[test]
    fn placeholders_are_filled() {
        let text = t_with("viewer.grid.count", &[("current", &3), ("total", &10)]);
        assert_eq!(text.as_ref(), "3/10");
    }

    #[test]
    fn missing_key_is_returned_verbatim() {
        assert_eq!(t("no.such.key").as_ref(), "no.such.key");
    }
}
