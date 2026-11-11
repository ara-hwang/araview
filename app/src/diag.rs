//! 진단 정보(SPEC §17.1).
//!
//! 지원 요청에 붙일 환경 요약을 만든다. 사진 경로와 파일 이름, 환경 변수 값은 담지
//! 않고, 홈 폴더는 `<home>`으로 가린다. 클립보드로만 나가며 네트워크로 보내지 않는다.
//! 항목을 늘릴 때는 `docs/usage.md`의 "진단 정보와 로그"도 함께 고친다.

use std::path::{Path, PathBuf};

use crate::logging;
use crate::settings::SettingsStore;

/// Windows 배포본이 실행 파일 옆에 두는 이미지 코덱 DLL(`scripts/Build-Installer.ps1`).
const IMAGE_CODEC_DLLS: &[&str] = &["heif.dll", "libde265.dll", "aom.dll"];

/// 복사할 진단 정보 본문.
pub(crate) fn collect(store: &SettingsStore) -> String {
    let settings = &store.settings;
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let mut lines = vec![
        format!("AraView {} ({profile})", env!("CARGO_PKG_VERSION")),
        format!("OS: {}", os_line()),
        format!("Arch: {} {}", std::env::consts::OS, std::env::consts::ARCH),
        format!(
            "Locale: {:?}, theme: {:?}, view: {:?}, cache: {:?}/{:?}, resolution: {:?}",
            settings.language,
            store.theme,
            settings.view_mode,
            settings.cache_mode,
            settings.cache_max,
            settings.max_resolution
        ),
        format!(
            "Image codec libraries: {}",
            codec_summary(exe_dir().as_deref())
        ),
        format!("Cache: {}", cache_line(store)),
        format!("Log: {}", log_line()),
    ];
    let recent = logging::recent_problems();
    if recent.is_empty() {
        lines.push("Recent problems: none".to_owned());
    } else {
        lines.push("Recent problems:".to_owned());
        lines.extend(
            recent
                .iter()
                .map(|line| format!("  {}", line.trim_end_matches('\n'))),
        );
    }
    // 경로에 섞인 사용자 이름을 지운다. 로그 줄에 섞인 경로도 같은 규칙을 따른다.
    format!("{}\n", mask_home(&lines.join("\n"), home_dir().as_deref()))
}

/// 실행 파일이 있는 폴더. Windows 배포본의 코덱 DLL도 여기에 있다.
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
}

fn os_line() -> String {
    platform_os_version().unwrap_or_else(|| "Windows (version unknown)".to_owned())
}

fn platform_os_version() -> Option<String> {
    crate::platform::os_version()
}

/// Windows 빌드 번호로 표시 이름을 정한다. Windows 11은 빌드 22000부터다.
pub(crate) fn describe_windows_version(major: u32, minor: u32, build: u32) -> String {
    let name = match (major, minor) {
        (10, _) if build >= 22_000 => "Windows 11",
        (10, _) => "Windows 10",
        (6, 3) => "Windows 8.1",
        (6, 2) => "Windows 8",
        (6, 1) => "Windows 7",
        _ => "Windows",
    };
    format!("{name} (build {build})")
}

/// 실행 파일 옆 코덱 라이브러리 상태.
fn codec_summary(install_dir: Option<&Path>) -> String {
    let dlls = IMAGE_CODEC_DLLS;
    let total = dlls.len();
    let Some(dir) = install_dir else {
        return "install folder unknown".to_owned();
    };
    let missing: Vec<&str> = dlls
        .iter()
        .copied()
        .filter(|dll| !dir.join(dll).is_file())
        .collect();
    if missing.is_empty() {
        format!("{total}/{total} present")
    } else {
        format!(
            "{}/{total} present, missing {}",
            total - missing.len(),
            missing.join(", ")
        )
    }
}

/// 캐시 폴더와 보관 모드.
fn cache_line(store: &SettingsStore) -> String {
    let Some(dir) = crate::cache_dir() else {
        return "not set".to_owned();
    };
    let state = if dir.is_dir() { "exists" } else { "missing" };
    format!(
        "{} ({:?}, {state})",
        dir.display(),
        store.settings.cache_storage_mode
    )
}

/// 로그 파일 경로와 지금 크기.
fn log_line() -> String {
    let Some(path) = logging::log_path() else {
        return "not set".to_owned();
    };
    match std::fs::metadata(&path) {
        Ok(meta) => format!("{} ({})", path.display(), kib(meta.len())),
        Err(_) => format!("{} (not written yet)", path.display()),
    }
}

fn kib(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    }
}

/// 홈 폴더 경로를 `<home>`으로 가린다.
fn mask_home(text: &str, home: Option<&str>) -> String {
    let Some(home) = home.map(str::trim).filter(|home| !home.is_empty()) else {
        return text.to_owned();
    };
    text.replace(home, "<home>")
}

fn home_dir() -> Option<String> {
    std::env::var("USERPROFILE")
        .ok()
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn home_paths_are_masked() {
        assert_eq!(
            mask_home(
                "Cache: /home/ara/.cache/com.araview.viewer (exists)",
                Some("/home/ara")
            ),
            "Cache: <home>/.cache/com.araview.viewer (exists)"
        );
        assert_eq!(mask_home("Cache: /tmp/x", None), "Cache: /tmp/x");
        assert_eq!(mask_home("Cache: /tmp/x", Some("  ")), "Cache: /tmp/x");
    }

    #[test]
    fn windows_versions_follow_the_build_number() {
        assert_eq!(
            describe_windows_version(10, 0, 22_631),
            "Windows 11 (build 22631)"
        );
        assert_eq!(
            describe_windows_version(10, 0, 19_045),
            "Windows 10 (build 19045)"
        );
        assert_eq!(
            describe_windows_version(6, 1, 7601),
            "Windows 7 (build 7601)"
        );
    }

    #[test]
    fn codec_summary_reports_missing_libraries() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("heif.dll"), b"x").unwrap();
        assert_eq!(
            codec_summary(Some(dir.path())),
            "1/3 present, missing libde265.dll, aom.dll"
        );
        assert_eq!(codec_summary(None), "install folder unknown");
    }

    #[test]
    fn sizes_are_human_readable() {
        assert_eq!(kib(0), "0 B");
        assert_eq!(kib(900), "900 B");
        assert_eq!(kib(1536), "1.5 KiB");
    }
}
