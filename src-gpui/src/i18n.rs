//! UI 문구. 번역 원본은 `src-gpui/locales/*.json`이다.

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

static KO: LazyLock<Catalog> = LazyLock::new(|| flatten(include_str!("../locales/ko.json")));
static EN: LazyLock<Catalog> = LazyLock::new(|| flatten(include_str!("../locales/en.json")));

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
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::*;

    /// 번역 키가 아닌데 카탈로그 최상위 이름으로 시작하는 문자열 리터럴.
    const NON_KEYS: &[&str] = &["settings.json"];

    fn placeholders(text: &str) -> BTreeSet<&str> {
        text.match_indices("{{")
            .filter_map(|(start, _)| {
                let rest = &text[start + 2..];
                rest.find("}}").map(|end| &rest[..end])
            })
            .collect()
    }

    fn string_literals(source: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut chars = source.chars();
        while let Some(ch) = chars.next() {
            if ch != '"' {
                continue;
            }
            let mut literal = String::new();
            while let Some(next) = chars.next() {
                match next {
                    '\\' => {
                        chars.next();
                    }
                    '"' => break,
                    _ => literal.push(next),
                }
            }
            out.push(literal);
        }
        out
    }

    fn rust_sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("소스 디렉터리") {
            let path = entry.expect("디렉터리 항목").path();
            if path.is_dir() {
                rust_sources(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }

    fn is_key_shaped(text: &str) -> bool {
        let mut segments = 0;
        for segment in text.split('.') {
            segments += 1;
            let valid = !segment.is_empty()
                && segment
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
            if !valid {
                return false;
            }
        }
        segments >= 2
    }

    #[test]
    fn catalogs_have_the_same_keys() {
        let ko: BTreeSet<_> = KO.keys().collect();
        let en: BTreeSet<_> = EN.keys().collect();
        assert!(!ko.is_empty(), "ko.json을 읽지 못했다");
        assert_eq!(
            ko.difference(&en).collect::<Vec<_>>(),
            Vec::<&&String>::new(),
            "en.json에 없는 키"
        );
        assert_eq!(
            en.difference(&ko).collect::<Vec<_>>(),
            Vec::<&&String>::new(),
            "ko.json에 없는 키"
        );
    }

    #[test]
    fn catalogs_have_the_same_placeholders() {
        for (key, ko) in KO.iter() {
            let en = EN.get(key).expect("키 집합은 위 테스트가 검증한다");
            assert_eq!(
                placeholders(ko),
                placeholders(en),
                "자리표시자가 다르다: {key}"
            );
        }
    }

    #[test]
    fn catalog_values_are_not_blank() {
        for (name, catalog) in [("ko", &*KO), ("en", &*EN)] {
            for (key, text) in catalog {
                assert!(!text.trim().is_empty(), "{name}: 빈 문구 {key}");
            }
        }
    }

    #[test]
    fn every_key_used_in_source_exists_in_catalogs() {
        let roots: BTreeSet<&str> = KO.keys().filter_map(|key| key.split('.').next()).collect();
        let mut files = Vec::new();
        rust_sources(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        assert!(!files.is_empty(), "소스 파일을 찾지 못했다");

        let mut missing = BTreeSet::new();
        for file in files {
            let source = std::fs::read_to_string(&file).expect("소스 읽기");
            for literal in string_literals(&source) {
                let known_root = literal
                    .split('.')
                    .next()
                    .is_some_and(|root| roots.contains(root));
                if known_root
                    && is_key_shaped(&literal)
                    && !NON_KEYS.contains(&literal.as_str())
                    && !(KO.contains_key(&literal) && EN.contains_key(&literal))
                {
                    missing.insert(format!(
                        "{literal} ({})",
                        file.file_name().unwrap_or_default().to_string_lossy()
                    ));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "카탈로그에 없는 번역 키:\n{}",
            missing.into_iter().collect::<Vec<_>>().join("\n")
        );
    }

    #[test]
    fn error_screen_keys_exist_for_every_kind() {
        // `app.rs`의 `render_error`가 `error.{kind}.title|hint`로 조립하는 키.
        for kind in [
            "notFound",
            "permission",
            "unsupported",
            "corrupt",
            "unknown",
        ] {
            for part in ["title", "hint"] {
                let key = format!("error.{kind}.{part}");
                assert!(KO.contains_key(&key) && EN.contains_key(&key), "{key}");
            }
        }
    }
}
