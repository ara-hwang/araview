//! 수동 업데이트 확인·설치와 서드파티 라이선스 화면(SPEC §20.1).

use std::collections::HashMap;
use std::io::Read as _;
use std::path::PathBuf;
use std::time::Duration;

use crate::smooth::SmoothScroll;
use crate::toast::{Toast, WindowToast};
use base64::Engine as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::AraView;
use crate::i18n::{t, t_with};

/// 확인 요청은 30초, 다운로드 요청은 10분이 전체 기한이다.
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);
/// GPUI 앱의 릴리스 피드. Tauri 앱의 `latest.json`과 같은 모양이고 같은 키로 서명한다.
const UPDATE_FEED: &str =
    "https://github.com/ara-hwang/araview/releases/latest/download/latest-gpui.json";
const RELEASE_PAGE: &str = "https://github.com/ara-hwang/araview/releases/latest";
const PLATFORM: &str = "windows-x86_64";
/// 설치 프로그램 크기 상한. 넘으면 받지 않는다.
const MAX_INSTALLER_BYTES: u64 = 512 * 1024 * 1024;
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

/// `1.2.10` 같은 버전을 숫자 조각으로 비교한다. 앞의 `v`와 뒤의 접미사는 무시한다.
fn version_parts(version: &str) -> Vec<u64> {
    version
        .trim()
        .trim_start_matches('v')
        .split(['.', '-', '+'])
        .map_while(|part| part.parse().ok())
        .collect()
}

fn is_newer(candidate: &str, current: &str) -> bool {
    version_parts(candidate) > version_parts(current)
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
fn feed_url() -> String {
    if cfg!(debug_assertions)
        && let Ok(url) = std::env::var("ARAVIEW_UPDATE_FEED")
    {
        return url;
    }
    UPDATE_FEED.to_owned()
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

/// 설치 프로그램을 내려받아 서명을 확인하고 임시 폴더에 저장한다.
fn download_installer(asset: &Asset, public_key: &str) -> Result<PathBuf, String> {
    let mut response = agent(DOWNLOAD_TIMEOUT)
        .get(&asset.url)
        .call()
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_INSTALLER_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_INSTALLER_BYTES {
        return Err("installer is larger than expected".to_owned());
    }
    verify_signature(&bytes, &asset.signature, public_key)?;
    let name = asset
        .url
        .rsplit('/')
        .next()
        .filter(|name| name.to_ascii_lowercase().ends_with(".exe"))
        .unwrap_or("araview-update-setup.exe");
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
    Ok(path)
}

impl AraView {
    /// 수동 업데이트 확인. 시작 시 자동 확인이나 백그라운드 폴링은 없다.
    pub(super) fn check_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.update_checking {
            // 중복 확인은 무시한다.
            return;
        }
        self.update_checking = true;
        let task = cx.background_spawn(async { fetch_feed(&feed_url()) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.update_checking = false;
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

    fn show_update(&mut self, feed: UpdateFeed, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let asset = feed.platforms.get(PLATFORM).cloned();
        let version = feed.version;
        let notes = feed.notes;
        let notes_scroll = SmoothScroll::default();
        window.open_dialog(cx, move |dialog, _, cx| {
            let notes = if notes.trim().is_empty() {
                t("dialog.update.noNotes").to_string()
            } else {
                notes.clone()
            };
            let view = view.clone();
            let asset = asset.clone();
            let download_version = version.clone();
            dialog
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
                                    div().pr_3().text_sm().whitespace_normal().child(notes),
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
        self.update_checking = true;
        window.toast(
            Toast::info(t_with(
                "dialog.update.downloadingDesc",
                &[("version", &version)],
            ))
            .title(t("dialog.update.downloadingTitle")),
            cx,
        );
        let task =
            cx.background_spawn(async move { download_installer(&asset, &release_public_key()?) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.update_checking = false;
                let launched = result.and_then(|installer| {
                    std::process::Command::new(&installer)
                        .spawn()
                        .map_err(|error| error.to_string())
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
        let task = cx.background_spawn(async { fetch_feed(&feed_url()) });
        cx.spawn_in(window, async move |this, cx| {
            let feed = task.await.ok().flatten()?;
            let asset = feed.platforms.get(PLATFORM).cloned()?;
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
