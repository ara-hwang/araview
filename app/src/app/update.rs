//! 업데이트 확인·설치와 서드파티 라이선스 화면(SPEC §20.1).

use std::collections::HashMap;
use std::io::{Read as _, Write as _};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use crate::smooth::SmoothScroll;
use crate::toast::{Toast, WindowToast};
use base64::Engine as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::AraView;
use crate::dialog::{DOWNLOAD_HEIGHT, UPDATE_HEIGHT, centered};
use crate::i18n::{t, t_with};
use crate::settings::{SettingsStore, UpdateChannel};

/// 확인 요청은 30초, 다운로드 요청은 10분이 전체 기한이다.
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);
/// 시작 후 자동 확인까지 기다리는 시간. 창과 첫 화면이 자리 잡은 뒤 조용히 확인한다.
const AUTO_CHECK_DELAY: Duration = Duration::from_secs(5);
/// 안정 채널 피드. `releases/latest`는 프리릴리스를 제외하므로 베타 릴리스가 있어도
/// 정식 릴리스만 가리킨다.
const UPDATE_FEED_STABLE: &str =
    "https://github.com/ara-hwang/araview/releases/latest/download/update-feed.json";
/// 베타 채널 피드. 베타 릴리스마다 롤링 `beta` 릴리스의 이 자산을 덮어쓴다.
const UPDATE_FEED_BETA: &str =
    "https://github.com/ara-hwang/araview/releases/download/beta/update-feed-beta.json";
const RELEASE_PAGE: &str = "https://github.com/ara-hwang/araview/releases";
/// 앱 안에서 내려받아 설치할 수 있는 플랫폼 키. 설치 프로그램 실행은 Windows만 지원하므로
/// macOS와 Linux는 업데이트 확인 후 릴리스 페이지로 안내한다.
const PLATFORM: Option<&str> = if cfg!(windows) {
    Some("windows-x86_64")
} else {
    None
};
/// 설치 프로그램 크기 상한. 넘으면 받지 않는다.
const MAX_INSTALLER_BYTES: u64 = 512 * 1024 * 1024;
/// 본문을 읽는 조각 크기. 조각마다 받은 양을 기록하고 취소를 확인한다.
const DOWNLOAD_CHUNK: usize = 64 * 1024;
/// 다운로드 대화상자의 진행 표시를 다시 그리는 간격.
const PROGRESS_TICK: Duration = Duration::from_millis(100);
/// 업데이트 서명 공개키(minisign, base64). 기존 1.x 릴리스와 같은 키를 쓴다.
const UPDATE_PUBKEY: &str = include_str!("../../update-pubkey.txt");

#[derive(Clone, serde::Deserialize)]
struct Asset {
    url: String,
    signature: String,
}

#[derive(serde::Deserialize)]
struct UpdateFeed {
    version: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    platforms: HashMap<String, Asset>,
}

/// 다운로드 작업과 진행 대화상자가 함께 보는 상태.
#[derive(Default)]
struct DownloadProgress {
    received: AtomicU64,
    /// 전체 크기. 서버가 알려주지 않으면 0이다.
    total: AtomicU64,
    cancelled: AtomicBool,
}

impl DownloadProgress {
    /// 받은 비율(0~100). 전체 크기를 모르면 None이다.
    fn percent(&self) -> Option<f32> {
        let total = self.total.load(Ordering::Relaxed);
        let received = self.received.load(Ordering::Relaxed);
        (total > 0).then(|| (received as f32 / total as f32 * 100.).min(100.))
    }

    fn label(&self) -> SharedString {
        let received = super::format_bytes(self.received.load(Ordering::Relaxed));
        match self.percent() {
            Some(percent) => t_with(
                "dialog.update.downloadedOfTotal",
                &[
                    ("downloaded", &received),
                    (
                        "total",
                        &super::format_bytes(self.total.load(Ordering::Relaxed)),
                    ),
                    ("pct", &format!("{percent:.0}")),
                ],
            ),
            None => t_with("dialog.update.downloadedBytes", &[("size", &received)]),
        }
    }
}

/// 비교 가능한 버전. `1.2.10`, `2.7.0-beta.2` 같은 값을 다룬다.
/// 앞의 `v`와 뒤의 빌드 메타데이터(`+...`)는 무시한다.
#[derive(Debug, PartialEq, Eq)]
struct Version<'a> {
    core: [u64; 3],
    pre: Option<Vec<&'a str>>,
}

fn parse_version(version: &str) -> Option<Version<'_>> {
    let version = version.trim();
    let version = version.strip_prefix('v').unwrap_or(version);
    let version = version.split('+').next().unwrap_or(version);
    let (core, pre): (&str, Option<Vec<&str>>) = match version.split_once('-') {
        Some((core, pre)) if !pre.is_empty() => (core, Some(pre.split('.').collect())),
        Some(_) => return None,
        None => (version, None),
    };
    let mut parts = core.split('.');
    let core = [
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ];
    if parts.next().is_some() {
        return None;
    }
    if pre
        .as_ref()
        .is_some_and(|parts| parts.iter().any(|part| part.is_empty()))
    {
        return None;
    }
    Some(Version { core, pre })
}

/// 세미버전 규칙으로 프리릴리스 식별자를 비교한다. 숫자 식별자는 수로, 문자 식별자는
/// 사전순으로 비교하고 숫자가 항상 문자보다 작다. 앞부분이 같으면 짧은 쪽이 작다.
fn compare_identifier(a: &str, b: &str) -> std::cmp::Ordering {
    match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(a), Ok(b)) => a.cmp(&b),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => a.cmp(b),
    }
}

fn compare_versions(candidate: &str, current: &str) -> Option<std::cmp::Ordering> {
    let candidate = parse_version(candidate)?;
    let current = parse_version(current)?;
    let order = candidate.core.cmp(&current.core);
    if order != std::cmp::Ordering::Equal {
        return Some(order);
    }
    Some(match (candidate.pre.as_deref(), current.pre.as_deref()) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(a), Some(b)) => {
            for (a, b) in a.iter().zip(b) {
                let order = compare_identifier(a, b);
                if order != std::cmp::Ordering::Equal {
                    return Some(order);
                }
            }
            a.len().cmp(&b.len())
        }
    })
}

fn is_newer(candidate: &str, current: &str) -> bool {
    compare_versions(candidate, current) == Some(std::cmp::Ordering::Greater)
}

fn decode_base64_text(value: &str) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value.trim())
        .map_err(|error| error.to_string())?;
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

/// 내려받은 설치 프로그램이 릴리스 키로 서명됐는지 확인한다. 피드의 서명과 설정의
/// 공개키는 각각 minisign 파일 내용을 base64로 감싼 값이다.
fn verify_signature(bytes: &[u8], signature: &str, public_key: &str) -> Result<(), String> {
    let key = minisign_verify::PublicKey::decode(&decode_base64_text(public_key)?)
        .map_err(|error| error.to_string())?;
    let signature = minisign_verify::Signature::decode(&decode_base64_text(signature)?)
        .map_err(|error| error.to_string())?;
    key.verify(bytes, &signature, true)
        .map_err(|error| error.to_string())
}

/// 확인할 피드 주소. 디버그 빌드는 로컬 검증용으로 환경 변수로 바꿀 수 있다.
fn feed_url(channel: UpdateChannel) -> String {
    if cfg!(debug_assertions)
        && let Ok(url) = std::env::var("ARAVIEW_UPDATE_FEED")
    {
        return url;
    }
    match channel {
        UpdateChannel::Stable => UPDATE_FEED_STABLE,
        UpdateChannel::Beta => UPDATE_FEED_BETA,
    }
    .to_owned()
}

fn release_public_key() -> Result<String, String> {
    // 디버그 빌드는 로컬 검증용 키를 환경 변수로 받을 수 있다. 릴리스 빌드는 항상 릴리스 키다.
    if cfg!(debug_assertions)
        && let Ok(key) = std::env::var("ARAVIEW_UPDATE_PUBKEY")
    {
        return Ok(key);
    }
    let key = UPDATE_PUBKEY.trim();
    if key.is_empty() {
        return Err("updater public key is missing".to_owned());
    }
    Ok(key.to_owned())
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into()
}

/// 피드를 읽는다. 아직 GPUI 릴리스가 없어 피드가 없으면(404) 최신으로 본다.
fn fetch_feed(url: &str) -> Result<Option<UpdateFeed>, String> {
    let mut response = match agent(CHECK_TIMEOUT).get(url).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&body)
        .map(Some)
        .map_err(|error| error.to_string())
}

/// 본문을 조각 단위로 읽으며 받은 양을 기록한다. 도중에 취소되면 None이다.
fn read_with_progress(
    mut reader: impl std::io::Read,
    progress: &DownloadProgress,
) -> Result<Option<Vec<u8>>, String> {
    let mut bytes = Vec::new();
    let mut chunk = vec![0; DOWNLOAD_CHUNK];
    loop {
        if progress.cancelled.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let read = match reader.read(&mut chunk) {
            Ok(0) => return Ok(Some(bytes)),
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.to_string()),
        };
        bytes.extend_from_slice(&chunk[..read]);
        progress
            .received
            .store(bytes.len() as u64, Ordering::Relaxed);
    }
}

/// 설치 프로그램은 HTTPS에서만 받는다. 디버그 빌드는 로컬 피드 검증을 위해
/// HTTP도 허용한다.
fn is_allowed_asset_url(url: &str) -> bool {
    url.starts_with("https://") || (cfg!(debug_assertions) && url.starts_with("http://"))
}

/// 설치 프로그램을 내려받아 서명을 확인하고 임시 폴더에 저장한다. 취소되면 None이다.
fn download_installer(
    asset: &Asset,
    public_key: &str,
    progress: &DownloadProgress,
) -> Result<Option<PathBuf>, String> {
    if !is_allowed_asset_url(&asset.url) {
        return Err("update URL must use HTTPS".to_owned());
    }
    let mut response = agent(DOWNLOAD_TIMEOUT)
        .get(&asset.url)
        .call()
        .map_err(|error| error.to_string())?;
    let total = response.body().content_length().unwrap_or(0);
    progress.total.store(total, Ordering::Relaxed);
    let reader = response
        .body_mut()
        .as_reader()
        .take(MAX_INSTALLER_BYTES + 1);
    let Some(bytes) = read_with_progress(reader, progress)? else {
        return Ok(None);
    };
    if bytes.len() as u64 > MAX_INSTALLER_BYTES {
        return Err("installer is larger than expected".to_owned());
    }
    verify_signature(&bytes, &asset.signature, public_key)?;
    // URL에서 파일명을 가져오지 않는다. 고유한 임시 이름을 쓰면 경로 조작과
    // 사전 배치(pre-planting)를 피할 수 있다. `.exe` 접미사는 유지한다.
    let mut file = tempfile::Builder::new()
        .prefix("araview-update-")
        .suffix(".exe")
        .tempfile_in(std::env::temp_dir())
        .map_err(|error| error.to_string())?;
    file.write_all(&bytes).map_err(|error| error.to_string())?;
    let (_file, path) = file.keep().map_err(|error| error.error.to_string())?;
    Ok(Some(path))
}

impl AraView {
    /// 수동 업데이트 확인. 설정에서 자동 확인을 끄더라도 버튼과 팔레트는 항상 동작한다.
    pub(super) fn check_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.update_checking {
            // 중복 확인은 무시한다.
            return;
        }
        self.update_checking = true;
        let channel = SettingsStore::global(cx).settings.update_channel;
        let task = cx.background_spawn(async move { fetch_feed(&feed_url(channel)) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.update_checking = false;
                // 열린 설정 대화상자가 있으면 확인 버튼을 원래대로 되돌린다.
                window.refresh();
                match result {
                    Ok(Some(feed)) if is_newer(&feed.version, env!("CARGO_PKG_VERSION")) => {
                        this.show_update(feed, window, cx)
                    }
                    Ok(_) => window.toast(Toast::success(t("toast.update.latest")), cx),
                    Err(error) => {
                        window.toast(Toast::error(error).title(t("toast.update.checkFail")), cx)
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// 시작 시 1회 자동 확인. 설정이 켜져 있을 때만 창이 자리 잡은 뒤 조용히 확인하고,
    /// 새 버전이 있을 때만 대화상자를 띄운다. 최신이거나 실패하면 알리지 않는다.
    pub(super) fn maybe_auto_check_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !SettingsStore::global(cx).settings.auto_check_updates {
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(AUTO_CHECK_DELAY).await;
            let channel = this
                .update_in(cx, |this, _, cx| {
                    if !SettingsStore::global(cx).settings.auto_check_updates
                        || this.update_checking
                    {
                        return None;
                    }
                    this.update_checking = true;
                    Some(SettingsStore::global(cx).settings.update_channel)
                })
                .ok()
                .flatten();
            let Some(channel) = channel else {
                return;
            };
            let result = cx
                .background_spawn(async move { fetch_feed(&feed_url(channel)) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.update_checking = false;
                // 자동 확인 중 설정을 열었다면 확인 버튼을 원래대로 되돌린다.
                window.refresh();
                match result {
                    Ok(Some(feed)) if is_newer(&feed.version, env!("CARGO_PKG_VERSION")) => {
                        this.show_update(feed, window, cx)
                    }
                    Ok(_) => {}
                    Err(error) => {
                        log::warn!("[update] automatic check failed: {error}");
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn show_update(&mut self, feed: UpdateFeed, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let asset = PLATFORM
            .and_then(|platform| feed.platforms.get(platform))
            .cloned();
        let version = feed.version;
        let notes = feed.notes;
        let notes_scroll = SmoothScroll::default();
        window.open_dialog(cx, move |dialog, window, cx| {
            let notes = if notes.trim().is_empty() {
                t("dialog.update.noNotes").to_string()
            } else {
                notes.clone()
            };
            let view = view.clone();
            let asset = asset.clone();
            let download_version = version.clone();
            centered(dialog, window, UPDATE_HEIGHT)
                .title(t_with(
                    "dialog.update.availableTitle",
                    &[("version", &version)],
                ))
                .child(
                    v_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(t("dialog.update.availableDesc")),
                        )
                        .child(
                            notes_scroll
                                .area(
                                    "update-notes",
                                    div().pr_3().text_sm().child(
                                        TextView::markdown("update-notes-text", notes.clone())
                                            .selectable(true)
                                            .on_link_click(|url, _, _, cx| cx.open_url(url)),
                                    ),
                                )
                                .h(px(220.)),
                        ),
                )
                .footer(
                    DialogFooter::new()
                        .gap_2()
                        .child(
                            Button::new("update-later")
                                .outline()
                                .label(t("dialog.update.later"))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("update-page")
                                .outline()
                                .label(t("dialog.update.releasePage"))
                                .on_click(|_, _, cx| cx.open_url(RELEASE_PAGE)),
                        )
                        .when_some(asset, |footer, asset| {
                            footer.child(
                                Button::new("update-download")
                                    .primary()
                                    .label(t("dialog.update.download"))
                                    .on_click(move |_, window, cx| {
                                        window.close_dialog(cx);
                                        view.update(cx, |this, cx| {
                                            this.download_update(
                                                asset.clone(),
                                                download_version.clone(),
                                                window,
                                                cx,
                                            )
                                        });
                                    }),
                            )
                        }),
                )
        });
    }

    /// 다운로드가 끝날 때까지 떠 있는 진행 대화상자. 닫는 길은 취소 버튼뿐이고,
    /// 다운로드가 끝나면 `download_update`가 닫는다.
    fn show_download(
        progress: Arc<DownloadProgress>,
        version: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.open_dialog(cx, move |dialog, window, cx| {
            let percent = progress.percent();
            let cancel = progress.clone();
            centered(dialog, window, DOWNLOAD_HEIGHT)
                .close_button(false)
                .keyboard(false)
                .title(t("dialog.update.downloading"))
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(t_with(
                                    "dialog.update.downloadingDesc",
                                    &[("version", &version)],
                                )),
                        )
                        .child(
                            Progress::new("update-progress")
                                .accessibility_label(t("dialog.update.downloading"))
                                .loading(percent.is_none())
                                .value(percent.unwrap_or(0.)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(progress.label()),
                        ),
                )
                .footer(
                    DialogFooter::new().child(
                        Button::new("update-cancel")
                            .outline()
                            .label(t("dialog.cancel"))
                            .on_click(move |_, window, cx| {
                                cancel.cancelled.store(true, Ordering::Relaxed);
                                window.close_dialog(cx);
                            }),
                    ),
                )
        });
    }

    /// 설치 프로그램을 내려받아 실행하고 앱을 종료한다. 설치와 새 버전 재실행은
    /// 설치 프로그램이 맡는다.
    fn download_update(
        &mut self,
        asset: Asset,
        version: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.update_checking {
            return;
        }
        let public_key = match release_public_key() {
            Ok(key) => key,
            Err(error) => {
                window.toast(Toast::error(error).title(t("toast.update.checkFail")), cx);
                return;
            }
        };
        self.update_checking = true;
        let signature = asset.signature.clone();
        let progress = Arc::new(DownloadProgress::default());
        Self::show_download(progress.clone(), version, window, cx);
        // 받은 양은 배경 스레드가 기록하므로, 대화상자는 일정 간격으로 다시 그린다.
        let refresh = cx.spawn_in(window, async move |_, cx| {
            loop {
                cx.background_executor().timer(PROGRESS_TICK).await;
                if cx.update(|window, _| window.refresh()).is_err() {
                    break;
                }
            }
        });
        let task = cx.background_spawn({
            let progress = progress.clone();
            let public_key = public_key.clone();
            async move { download_installer(&asset, &public_key, &progress) }
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            drop(refresh);
            this.update_in(cx, |this, window, cx| {
                this.update_checking = false;
                // 취소 버튼은 대화상자를 직접 닫았다. 그때는 아래 다이얼로그를 건드리지 않는다.
                if progress.cancelled.load(Ordering::Relaxed) {
                    return;
                }
                window.close_dialog(cx);
                let launched = result.and_then(|installer| {
                    let installer = installer.ok_or("download was cancelled")?;
                    // 파일을 다시 읽어 실행 직전에 서명을 재검증한다. 다운로드와
                    // 실행 사이에 임시 파일이 바뀌는 창을 닫는다.
                    let bytes = std::fs::read(&installer).map_err(|error| error.to_string())?;
                    if let Err(error) = verify_signature(&bytes, &signature, &public_key) {
                        let _ = std::fs::remove_file(&installer);
                        return Err(error);
                    }
                    std::process::Command::new(&installer)
                        .spawn()
                        .map_err(|error| {
                            let _ = std::fs::remove_file(&installer);
                            error.to_string()
                        })
                });
                match launched {
                    Ok(_) => {
                        window.toast(
                            Toast::info(t("dialog.update.installingDesc"))
                                .title(t("dialog.update.installingTitle")),
                            cx,
                        );
                        crate::settings::SettingsStore::flush(cx);
                        cx.quit();
                    }
                    Err(error) => window.toast(
                        Toast::error(error).title(t("dialog.update.downloadFail")),
                        cx,
                    ),
                }
            })
            .ok();
        })
        .detach();
    }

    /// 대화상자 없이 최신 버전을 바로 내려받아 설치를 시작한다. 디버그 빌드의
    /// 런타임 검증 통로(`action:installUpdate`)에서만 쓴다.
    pub(super) fn install_latest_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let channel = SettingsStore::global(cx).settings.update_channel;
        let task = cx.background_spawn(async move { fetch_feed(&feed_url(channel)) });
        cx.spawn_in(window, async move |this, cx| {
            let feed = task.await.ok().flatten()?;
            let asset = PLATFORM
                .and_then(|platform| feed.platforms.get(platform))
                .cloned()?;
            this.update_in(cx, |this, window, cx| {
                if is_newer(&feed.version, env!("CARGO_PKG_VERSION")) {
                    this.download_update(asset, feed.version, window, cx);
                }
            })
            .ok()
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    // `super::*`는 gpui의 `test` 매크로까지 끌어와 `#[test]`를 가린다.
    use super::{
        DOWNLOAD_CHUNK, DownloadProgress, Ordering, compare_versions, is_allowed_asset_url,
        is_newer, parse_version, read_with_progress,
    };

    #[test]
    fn asset_urls_must_be_https() {
        assert!(is_allowed_asset_url("https://example.com/araview.exe"));
        assert!(!is_allowed_asset_url("file:///C:/araview.exe"));
        assert!(!is_allowed_asset_url("ftp://example.com/araview.exe"));
    }

    #[test]
    fn read_records_received_bytes() {
        let progress = DownloadProgress::default();
        let body = vec![7u8; DOWNLOAD_CHUNK * 2 + 5];
        progress.total.store(body.len() as u64, Ordering::Relaxed);
        let bytes = read_with_progress(body.as_slice(), &progress).unwrap();
        assert_eq!(bytes, Some(body.clone()));
        assert_eq!(progress.received.load(Ordering::Relaxed), body.len() as u64);
        assert_eq!(progress.percent(), Some(100.));
    }

    #[test]
    fn read_stops_when_cancelled() {
        let progress = DownloadProgress::default();
        progress.cancelled.store(true, Ordering::Relaxed);
        assert_eq!(read_with_progress([1u8, 2].as_slice(), &progress), Ok(None));
        assert_eq!(progress.received.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn percent_needs_a_known_total() {
        let progress = DownloadProgress::default();
        progress.received.store(50, Ordering::Relaxed);
        assert_eq!(progress.percent(), None);
        progress.total.store(200, Ordering::Relaxed);
        assert_eq!(progress.percent(), Some(25.));
    }

    #[test]
    fn numeric_version_parts_compare_as_numbers() {
        assert!(is_newer("1.2.10", "1.2.9"));
        assert!(is_newer("2.7.0", "2.6.99"));
        assert!(!is_newer("2.6.1", "2.6.1"));
        assert!(!is_newer("2.5.9", "2.6.0"));
    }

    #[test]
    fn beta_releases_compare_in_order() {
        assert!(is_newer("2.7.0-beta.2", "2.7.0-beta.1"));
        assert!(is_newer("2.7.0-beta.10", "2.7.0-beta.9"));
        assert!(!is_newer("2.7.0-beta.1", "2.7.0-beta.1"));
        assert!(!is_newer("2.7.0-beta.1", "2.7.0"));
        assert!(is_newer("2.7.0", "2.7.0-beta.1"));
        assert!(is_newer("2.8.0-beta.1", "2.7.0"));
    }

    #[test]
    fn build_metadata_and_v_prefix_are_ignored() {
        assert_eq!(
            compare_versions("v2.7.0+build.7", "2.7.0"),
            Some(std::cmp::Ordering::Equal)
        );
        assert!(is_newer("v2.7.0", "v2.6.1"));
    }

    #[test]
    fn malformed_versions_are_never_newer() {
        assert_eq!(parse_version("2.7"), None);
        assert_eq!(parse_version("2.7.0-"), None);
        assert_eq!(parse_version("2.7.0-beta."), None);
        assert!(!is_newer("not-a-version", "2.6.1"));
        assert!(!is_newer("2.6.1", "not-a-version"));
    }
}
