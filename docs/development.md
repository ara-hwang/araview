# 개발 안내

개발자용 안내입니다. 기능/기술 규격 진실은 `../SPEC.md`, 제품 정의는 `../PRODUCT.md`를 따릅니다. 코드 작성 규칙은 `../CODING_STANDARDS.md`, 작업별 절차(포맷 추가, 코어 연산 추가, 런타임 확인, 라이선스 데이터 갱신, 설치 프로그램 시험)는 `playbooks.md`를 봅니다.

## 기술 스택

- **앱**: Rust, [GPUI Kit](https://github.com/longbridge/gpui-kit) 0.8(`app`)
- **공용 코어**: `crates/araview-core`(디코드, 아카이브, 캐시, 썸네일, 파일 작업)
- **PSD 탐색기 썸네일**: `crates/araview-thumb`(COM DLL, Windows 전용. 다른 OS에서는 빈 크레이트로 빌드됩니다)
- **배포 묶음**: Windows는 NSIS 설치 프로그램(`app/installer/araview.nsi`), Linux는 tar.gz(`scripts/package-linux.sh`), macOS는 dmg(`scripts/package-macos.sh`)
- 지원 OS: Windows 11 x64(정식), macOS(Apple silicon 기준)와 Linux x64(X11/Wayland)는 실험적입니다(CI 빌드만 확인). 창 틀은 GPUI Kit의 타이틀바가 OS별로 그립니다.
- 루트 `Cargo.toml`이 세 크레이트를 한 워크스페이스로 묶고 `Cargo.lock`도 하나입니다.

## 요구 사항

- **OS**: Windows 11 (x64), macOS 11 이상(실험적), Linux x64(실험적)
- **Rust**: stable (`rust-toolchain.toml` 채널, 최소 버전은 `app/Cargo.toml`의 `rust-version = "1.97"`)
- **Node.js**: `>= 24` (`release`, `docs:check`, `licenses` 스크립트용. 의존성 설치는 없습니다)
- **NSIS**: Windows 설치 프로그램을 만들 때만 필요합니다(`makensis`가 PATH에 있거나 `Program Files (x86)\NSIS`에 있어야 합니다)

모든 OS에서 HEIC/HEIF/AVIF를 쓰려면 `libheif` 1.21 이상이 필요합니다. `vcpkg.json`은 `libheif`의 `default-features`를 끄고(`hevc` 인코더 `x265` 제외) `aom`만 켭니다. HEVC 디코더 `libde265`는 하드 의존성이라 항상 들어갑니다. 버전은 baseline의 1.21.2가 아니라 `overrides`로 1.23.1을 고정합니다. 1.21.2의 Windows DLL이 로드 시 초기화 실패(`0xC0000142`)를 일으켜 `cargo test`가 실행조차 되지 않기 때문입니다(`libheif-rs 3.0`의 sys 바인딩도 1.23 기준). `aom` feature는 AVIF(AV1) 디코드용이며, 없으면 AVIF 표시와 썸네일, 히스토그램이 실패합니다. 움직이는 AVIF를 디코드하려면 1.21 이상이 필요합니다(`libheif-rs`의 `v1_21` feature). 앱은 HEIC/AVIF를 인코드하지 않습니다.

동영상(mp4/mov/webm/mkv/avi)은 FFmpeg 8.x가 필요합니다. `rsmpeg`의 `ffmpeg8` feature와 메이저가 맞아야 컴파일되므로, 저장소 루트의 `vcpkg.json` 매니페스트가 `builtin-baseline`으로 FFmpeg 8.0.1(vcpkg 2026-03-23)을 고정합니다. `rsmpeg 0.18`이 FFmpeg 8.0 헤더 기준이라 8.1(구조체 필드 개명)에서는 컴파일되지 않으므로, 8.0.x를 유지합니다. classic 명령(`vcpkg install ffmpeg...`)은 pin을 무시하고 최신(9.x, 미지원)을 가져오므로 쓰지 않습니다. FFmpeg feature는 LGPL 호환 기본값 + AV1 디코드용 `dav1d`만 켭니다(GPL 계열 `gpl`/`x264`/`x265` 미포함). 시스템 FFmpeg(배포판 패키지)는 메이저가 다르면 빌드가 깨지므로 개발·CI 모두 vcpkg 제공 FFmpeg를 씁니다.

### Windows

[vcpkg](https://vcpkg.io/) 매니페스트로 네이티브 의존성(`libheif`, FFmpeg)을 설치합니다. 저장소 루트에서 실행해야 `vcpkg.json`이 적용됩니다.

```powershell
cd <araview>
vcpkg install --triplet x64-windows --x-install-root="<vcpkg root>/installed"
$env:VCPKG_ROOT = "<vcpkg root>"
$env:FFMPEG_LIBS_DIR = "<vcpkg root>\installed\x64-windows\lib"
$env:Path += ";$env:VCPKG_ROOT\installed\x64-windows\bin"
```

`--x-install-root`는 pin은 그대로 두고 설치 위치만 vcpkg 루트로 옮긴다.
`app/build.rs`와 pkg-config 설정이 그 위치를 보기 때문이다(그냥 `vcpkg install`만
하면 저장소 아래 `vcpkg_installed/`에 깔려 빌드가 찾지 못한다). Windows에서
`FFMPEG_LIBS_DIR`는 rsmpeg의 prebuilt 바인딩(`FFMPEG_BINDING_PATH`,
`.cargo/config.toml` 고정)과 짝을 이룬다. 바인딩을 로컬 생성하지 않으므로
Windows에서는 libclang이 필요 없다.

앱은 `libheif`와 FFmpeg를 동적 링크합니다. `app/build.rs`가 `VCPKG_ROOT`(또는 `VCPKG_INSTALLATION_ROOT`) 아래 `installed/x64-windows/bin`에서 `heif.dll`, `libde265.dll`, `aom.dll`, `avcodec-62.dll`, `avformat-62.dll`, `avutil-60.dll`, `avfilter-11.dll`, `avdevice-62.dll`, `swscale-9.dll`, `swresample-6.dll`, `dav1d.dll`을 실행 파일 옆(`target/<profile>/`)으로 복사하고, vcpkg의 `libheif`, `libde265`, `aom`, `ffmpeg`, `dav1d` `copyright`를 같은 위치의 `licenses/`로 모읍니다. 설치 프로그램은 이 파일들을 그대로 동봉합니다. CI에서는 `VCPKGRS_DYNAMIC=1`과 `PKG_CONFIG_PATH`를 함께 둡니다(워크플로 참조). FFmpeg 메이저가 바뀌면 DLL 이름(SONAME)도 바뀌므로 `app/build.rs`와 `scripts/Build-Installer.ps1`을 함께 고칩니다.

### macOS

Xcode 명령줄 도구와 `pkg-config`가 필요합니다. 네이티브 의존성은 Windows와 같은 vcpkg 매니페스트로 설치합니다(Homebrew의 `libheif`/FFmpeg는 메이저가 맞지 않으면 빌드가 깨집니다).

```bash
brew install pkgconf
# vcpkg가 없으면 https://vcpkg.io/ 절차로 설치
cd <araview>
vcpkg install --triplet arm64-osx --x-install-root="<vcpkg root>/installed"  # Intel은 x64-osx
export VCPKG_ROOT="<vcpkg root>"
export PKG_CONFIG_PATH="<vcpkg root>/installed/arm64-osx/lib/pkgconfig"  # Intel은 x64-osx
export PKG_CONFIG_ALL_STATIC=1
cargo run -p araview -- ~/Pictures/image.png
```

배포 묶음은 vcpkg 정적 빌드(`arm64-osx`, Intel은 `x64-osx`)로 `libheif`와 FFmpeg를 실행 파일에 넣어 dylib를 동봉하지 않습니다. CI의 `.github/actions/setup-rust-native`가 이 환경(`VCPKG_ROOT`, `PKG_CONFIG_PATH`, `PKG_CONFIG_ALL_STATIC=1`)을 만듭니다. Windows는 여기에 `FFMPEG_LIBS_DIR`가 더해집니다.

### Linux

GPUI의 X11/Wayland 연동에 필요한 개발 패키지를 설치합니다(Debian/Ubuntu 기준). 배포판의 `libheif`/FFmpeg 패키지는 쓰지 않습니다(메이저가 맞지 않으면 `rsmpeg` 빌드가 깨집니다). 네이티브 의존성은 macOS와 같은 vcpkg 매니페스트 정적 빌드(`x64-linux`)를 씁니다.

```bash
sudo apt-get install pkg-config cmake nasm clang libclang-dev libasound2-dev \
  libxkbcommon-dev libxkbcommon-x11-dev libxcb1-dev libxcb-xkb-dev libx11-xcb-dev \
  libwayland-dev libfontconfig-dev libfreetype-dev
cd <araview>
vcpkg install --triplet x64-linux --x-install-root="<vcpkg root>/installed"
export VCPKG_ROOT="<vcpkg root>"
export PKG_CONFIG_PATH="<vcpkg root>/installed/x64-linux/lib/pkgconfig"
export PKG_CONFIG_ALL_STATIC=1
```

실행에는 X11 또는 Wayland 세션, Vulkan 드라이버, `libxkbcommon`, `libasound2`(동영상 오디오 출력용)가 필요합니다. 파일 열기 대화상자는 `xdg-desktop-portal`을 씁니다. 디스플레이가 없는 환경에서는 `Xvfb`와 소프트웨어 Vulkan(`mesa-vulkan-drivers`)으로 실제 창을 띄워 확인할 수 있습니다.

## 시작하기

```bash
git clone https://github.com/ara-hwang/araview.git
cd araview
cargo run -p araview -- "C:/path/to/image.png"
```

개발(디버그) 빌드는 식별자 `com.araview.viewer.dev`를 씁니다. 설치본(`com.araview.viewer`)과 동시에 실행할 수 있고, 설정과 캐시도 따로 저장됩니다. 저장 위치는 OS별로 다릅니다.

| OS      | 설정(`settings.json`)                        | 캐시                               |
| ------- | -------------------------------------------- | ---------------------------------- |
| Windows | `%APPDATA%\<식별자>\`                        | `%LOCALAPPDATA%\<식별자>\`         |
| macOS   | `~/Library/Application Support/<식별자>/`    | `~/Library/Caches/<식별자>/`       |
| Linux   | `$XDG_CONFIG_HOME`(없으면 `~/.config`)`/<식별자>/` | `$XDG_CACHE_HOME`(없으면 `~/.cache`)`/<식별자>/` |

Windows 개발 빌드는 확장자 연결이 설치 버전과 섞이지 않도록 `AraView (Dev)` 이름으로 등록됩니다. 기본 앱 목록에서 `AraView`(설치 버전)와 구분해 선택하세요. 확장자 연결 설정과 PSD 탐색기 썸네일은 Windows에만 있습니다. macOS는 앱 번들의 문서 형식 선언(`Info.plist`), Linux는 `.desktop` 파일의 `MimeType`이 "다음으로 열기" 목록을 채웁니다.

## 스크립트

```bash
# 개발 실행
cargo run -p araview

# 검사
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 실행 중인 창 조작(개발 빌드 전용, docs/playbooks.md "Runtime check"). 드라이버 스크립트는 Windows 전용이다.
# macOS와 Linux는 제어 소켓에 한 줄 JSON을 보낸다(playbooks.md의 "Runtime check on macOS and Linux")
pwsh scripts/araview-drive.ps1 launch -Path samples/sample.jpg
pwsh scripts/araview-drive.ps1 selftest
pwsh scripts/araview-drive.ps1 hittest  # Windows가 판정하는 캡션 영역 읽기(커서를 옮기지 않음)
pwsh scripts/araview-mcp.ps1   # 같은 도구를 MCP 서버(stdio)로

# 배포 묶음(릴리스 빌드 + 패키징). 서명과 업로드는 하지 않는다
pwsh scripts/Build-Installer.ps1     # Windows: NSIS 설치 프로그램
scripts/package-linux.sh             # Linux: target/AraView-<버전>-linux-<arch>.tar.gz
scripts/package-macos.sh             # macOS: target/AraView-<버전>-macos-<arch>.dmg

# 릴리스 시작 (버전 올리기, 커밋, 태그, 푸시)
npm run release -- <X.Y.Z|patch|minor|major>

# 문서-코드 정합성 (버전, 확장자, 설정 키, plan)
npm run docs:check

# 서드파티 라이선스 자료 다시 만들기 (cargo-about 필요, 네트워크 사용)
npm run licenses
```

## 앱 구조 요약

- `app`는 UI와 입력만 맡고, 파일과 이미지 작업은 `araview-core`의 동기 함수(`ops/`)를 `cx.background_spawn`으로 호출합니다.
- 렌더링: `araview-core`의 `display.rs`가 디코드한 RGBA 프레임을 GPU 타일로 그립니다. GPUI 텍스처는 한 변 16384px 제한과 밉맵 없는 선형 보간뿐이라, 큰 이미지는 타일로 나누고 축소 단계를 CPU에서 만듭니다. 픽셀 보존 표시는 보이는 영역만 최근접 확대한 래스터로 그립니다.
- 단축키: GPUI 키맵 대신 뷰어 루트의 키 입력에서 설정 맵을 직접 찾습니다(`src/keys.rs`).
- 파일 열기 대화상자: GPUI의 경로 선택 창에는 형식 필터가 없어 `rfd`의 네이티브 대화상자를 씁니다.
- 고대비: Windows 대비 테마 여부를 시작할 때, 테마를 바꿀 때, 창이 다시 활성화될 때 읽어 글자·테두리 대비를 올립니다(다른 OS는 항상 꺼짐). 모션 최소화는 Windows "애니메이션 효과", macOS "동작 줄이기", GNOME `enable-animations`를 따릅니다(`src/platform/`).
- 플랫폼 계층: `app/src/platform/mod.rs`가 공개 함수 집합을 정의하고 `windows.rs`(Win32 직접 호출), `unix.rs`(Linux와 macOS)가 구현합니다. 항상 위(`ALWAYS_ON_TOP_SUPPORTED`)는 Windows에서만 켜지고, 다른 OS에서는 헤더 버튼, 설정 행, 명령 팔레트 항목이 숨겨집니다. 문구 중 OS마다 다른 것(탐색기/Finder/파일 관리자)은 `locales/*.json`의 `platform.<windows|macos|linux>` 아래에 같은 키로 덮어씁니다.
- 단축키: macOS에서는 Cmd를 Ctrl 자리에 씁니다(`Ctrl+O`는 Cmd+O로도 눌립니다). Windows와 Linux의 Win/Super 키는 쓰지 않습니다.
- 설정은 `<설정 폴더>/<식별자>/settings.json`에 저장합니다(위 표). `settings`/`recentFiles`/`archiveProgress` 모양은 1.x(Tauri)와 같고 `theme`, `window`(창 위치·크기) 키가 더 있습니다. 그래서 1.x 설치본 위에 설치하면 설정과 최근 파일을 이어받습니다.
- 단일 인스턴스: 두 번째 실행은 첫 인자를 기존 창에 넘기고 종료합니다. Windows는 명명된 파이프(`\\.\pipe\<식별자>`), Linux와 macOS는 유닉스 도메인 소켓(`$XDG_RUNTIME_DIR` 또는 임시 폴더의 `<식별자>.sock`, 경로가 100바이트를 넘으면 `/tmp/araview-<uid>-<해시>.sock`)입니다. macOS에서 Finder로 파일을 열면 `open-document` 이벤트가 같은 경로로 전달됩니다.
- 런타임 확인: 디버그 빌드는 실행 중인 창에 `araview action:<동작 ID>`(Windows는 `araview.exe`)로 동작을 실행시킬 수 있습니다(`src/app/menu.rs`의 `run_action`, 단축키 동작 ID와 `openSettings`, `viewLtr` 등). 릴리스 빌드에는 이 통로가 없습니다.
- 제어 브리지(개발 빌드 전용): 디버그 빌드는 `\\.\pipe\<식별자>-control`(Linux와 macOS는 유닉스 소켓 `<식별자>-control.sock`)을 열고 한 줄 JSON 요청에 한 줄 JSON 응답을 돌려줍니다(`src/app/bridge.rs`, 통로는 `src/platform/`). 입력은 창 메시지가 아니라 GPUI 입력 경로(`Window::dispatch_event`, `Window::dispatch_keystroke`)로 들어가므로 수식키 조합(ctrl-휠, ctrl-shift-c)과 파일 드롭이 그대로 살고 사용자의 마우스·키보드를 건드리지 않습니다. 상태(열린 파일, 인덱스, 보기 모드, 배율, 위치, 읽기 영역, 패널, 전체화면, 에러)를 함께 돌려줍니다. `scripts/araview-drive.ps1`(CLI), `scripts/araview-mcp.ps1`(MCP 서버), `scripts/AraViewControl.psm1`(캡처와 통로 클라이언트)이 이 통로를 씁니다. 릴리스 빌드에는 이 통로가 없습니다(The control channel is compiled out of release builds).
- 업데이트: 동작 계약은 `SPEC.md` §20.1을 따른다(피드 `update-feed.json`, 공개키 `app/update-pubkey.txt`, `PLATFORM` 분기). 피드가 없으면(404) 확인 결과는 "최신"이다.
- 업데이트 로컬 검증: `cargo run -p araview --example local_update_feed -- <setup.exe> <출력 폴더> http://127.0.0.1:8765`가 임시 키로 서명한 피드를 만듭니다. 그 폴더를 `python -m http.server 8765 --bind 127.0.0.1`로 띄우고, 디버그 빌드를 `ARAVIEW_UPDATE_FEED`(피드 주소)와 `ARAVIEW_UPDATE_PUBKEY`(`pubkey.txt` 내용)를 준 채 실행한 뒤 `action:checkUpdates`나 `action:installUpdate`를 보냅니다. 릴리스 빌드는 두 환경 변수를 읽지 않습니다.
- 서드파티 라이선스: `npm run licenses`가 Rust 의존성을 `app/THIRD_PARTY_LICENSES.json`에 모으고, 앱이 이 파일을 컴파일 시점에 포함합니다. 같은 스크립트가 `THIRD_PARTY_LICENSES.md`의 2절 표도 다시 씁니다. 1절(네이티브 라이브러리)은 직접 고칩니다. 의존성을 바꾸면 다시 생성합니다. 허용하지 않은 라이선스는 `cargo deny check licenses bans sources`(설정 `deny.toml`, `cargo install cargo-deny --locked`)로 검사하며 CI와 릴리스 워크플로도 같은 검사를 합니다. 패키지에 LICENSE 파일이 없어 표준 템플릿("Copyright (c) <year> <owner>")이 나오는 크레이트는 스크립트가 경고로 알려 주며, 업스트림 LICENSE를 `licenses-extra/`에 복사하고 스크립트의 `EXTRA_LICENSES`에 추가합니다. 새 라이선스가 나오면 `about.toml`의 `accepted`와 `deny.toml`의 `allow`를 같이 고칩니다.
- 스크롤: GPUI에는 휠 스크롤 감속이 없어 `src/smooth.rs`의 `SmoothScroll`이 홈, 설정, 정보 패널, 라이선스 창(원문과 목록), 썸네일 그리드의 휠 입력을 목표 오프셋으로 쌓아 프레임마다 따라가게 합니다. 가상 목록(그리드, 라이선스 목록, 도크)은 목록이 추적하는 핸들을 공유하고 기본 스크롤을 끈 뒤(`overflow_hidden`) 바깥에서 휠을 처리합니다. 웹툰 보기는 `src/app/viewport.rs`가 같은 방식으로 휠과 위·아래 키 스크롤을 나눠 반영합니다.
- 알림: GPUI 알림 레이어는 다이얼로그보다 아래에 그려지므로, 다이얼로그가 열려 있으면 `src/toast.rs`가 같은 내용을 경고창으로 띄웁니다.
- 로딩 표시: 파일을 여는 중(200ms 이상)에는 홈 위에, 그림이 아직 없을 때는 가운데에, 그림이 떠 있는 채 새 페이지나 웹툰 페이지가 늦어지면(페이지 이동은 300ms 이상) 오른쪽 위에 스피너를 보입니다.
- UI 통합 테스트: 헤드리스 UI 테스트는 없습니다. `cargo test`는 단위 테스트만 돌립니다. 키, 휠, 드래그, 드롭, 우클릭, 다이얼로그는 개발 빌드를 띄워 제어 브리지로 확인합니다(`docs/playbooks.md`의 "Runtime check").

## 프로젝트 구조

```text
Cargo.toml  # 워크스페이스(crates/*, app). Cargo.lock도 하나
crates/araview-core/src/  # UI 비의존 코어. 경계: app=표현, ops/**=동기 연산
  ops/  # 도메인별 동기 함수(directory, file_ops, archive, load, metadata, thumbnail, ocr, system)
  decode/  # display, image(resolve_mime), sniff, heif, psd_sidecar, raster_sidecar, raw, jxl, svg_*, transcode
  cache/  # process_temp, cache, sidecar, scaled, thumbnail, archive_index, comic_info
  meta/  # image_info, orientation, natural_sort, ocr, dir_cache, file_availability, app_error, stable_hash, clipboard_png, file_assoc
  araview-thumb/  # PSD 탐색기 썸네일 COM DLL(Windows 전용)
app/  # 표현 계층. 상세는 코드 읽기
  src/app/  # 화면: pages, viewport, dock, grid, menu, info_panel, file_ops, ocr, settings_panel, system, update, licenses, bridge
  src/  # layout, picture, geometry, settings, smooth, toast, keys, dialog, i18n, assets, platform/, build.rs
  locales/ update-pubkey.txt THIRD_PARTY_LICENSES.json(생성물) installer/ packaging/ resources/
scripts/  # check-docs/release/licenses(.mjs), package-*.sh, Build-Installer.ps1, araview-drive/mcp(.ps1)
vendor/  # [patch.crates-io] 포크(HDR 실험용 gpui-pre-windows)
licenses-extra/  # 업스트림 LICENSE 사본
```

## 파생 이미지 캐시

- `settings.json`의 `settings.cacheStorageMode`를 읽어 시작 시 저장 루트를 선택합니다. 값이 없으면 `persistent`입니다.
- 영구 모드는 캐시 폴더(위 표)의 `<식별자>/cache-v2/`를 사용합니다. 식별자는 설치 빌드가 `com.araview.viewer`, 디버그 빌드가 `com.araview.viewer.dev`라 개발 빌드는 별도 루트를 씁니다.
- `temporary` 모드는 사용자 앱 캐시 디렉터리의 `session-v1/` 아래 프로세스별 `TempDir`를 사용합니다. 정상 종료 이벤트와 다음 시작 시 이전 세션 루트를 정리합니다. 모드 설정 변경은 다음 실행부터 적용되며, 영구 캐시를 끄면 다음 시작 때 이전 버전 루트를 정리합니다.
- `cache.rs`의 `get_cache_stats`와 `clear_cache`는 캐시 트리만 대상으로 합니다. 사용자 원본 파일 경로를 입력받아 삭제하지 않습니다.
- `process_temp::mark_in_use`로 보호된 파일과 `*.tmp-<pid>-...` 작성 중 파일은 삭제하지 않습니다. 영구 캐시 hit은 mtime을 갱신해 LRU 순서를 유지합니다.

## 픽셀 보존 표시와 축소

- 픽셀 보존(`pixelated`)은 확대 배율에서만 적용됩니다. 축소 배율에서는 nearest 계열 보간이 스크린톤 같은 주기 패턴을 계단·무아레로 깨뜨리므로 설정과 무관하게 항상 smooth로 렌더합니다(`app/src/app/viewport.rs`의 배율 게이트).
- 자동 모드(`auto`)는 2x 이상 확대에서 pixelated로 렌더합니다(`AUTO_PIXELATED_ZOOM`). 보간 없이 원본 픽셀을 그대로 보여주기 위함입니다. 이미지 내용을 분석하는 픽셀 아트 감지는 없습니다.
- 표시 해상도 상한 축소 sidecar(`crates/araview-core/src/scaled.rs`)도 표시 정책과 무관하게 항상 보간 필터(`Triangle`)를 사용합니다. 축소는 픽셀을 버리는 연산이므로 픽셀 보존 판정이 의미가 없습니다.

## PSD 탐색기 썸네일 구현 메모

Windows 전용입니다. 사용자 동작 계약은 `SPEC.md` §20.2를 따릅니다. 아래는 개발용 구현 메모입니다.

- 핸들러는 `crates/araview-thumb/`의 In-Proc COM DLL(`araview_thumb.dll`)이며 `IThumbnailProvider` + `IInitializeWithStream`/`IInitializeWithFile`을 구현합니다. PSB(`8BPS` version 2)는 거부합니다.
- CLSID(발행 후 변경 금지): 릴리스 `{FD6BD976-2DF4-4656-94F2-1D166163EC59}`, 개발 `{BD277595-1702-4AC5-AA7C-A67965C3D570}`. `crates/araview-thumb/src/registry.rs`, `crates/araview-core/src/thumb_shell.rs`, `crates/araview-thumb/src/com.rs` 세 곳에 중복 정의되어 함께 바꿔야 합니다.
- 등록은 전부 HKCU(`Software\Classes`)입니다: `CLSID\{CLSID}\InprocServer32`(DLL 경로 + `ThreadingModel=Apartment`), `.psd`와 채널 ProgID의 `ShellEx\{E357FCCD-A995-4576-B01F-234630154E96}` 슬롯, `.psd`의 `PerceivedType`/`Content Type` 채우기. 등록/해제 후 `SHChangeNotify`를 보냅니다.
- DLL 전달: 워크스페이스 멤버로 함께 빌드합니다. `scripts/Build-Installer.ps1`이 릴리스 DLL을 설치 프로그램의 `resources/` 폴더에 넣습니다(1.x 설치본과 같은 위치라 기존 탐색기 등록이 그대로 유효합니다). 앱은 exe 옆, `resources/` 순으로 DLL을 찾습니다.
