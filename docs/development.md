# 개발 안내

개발자용 안내입니다. 기능/기술 규격 진실은 `../SPEC.md`, 제품 정의는 `../PRODUCT.md`를 따릅니다. 코드 작성 규칙은 `../CODING_STANDARDS.md`, 작업별 절차(포맷 추가, 코어 연산 추가, 런타임 확인, 라이선스 데이터 갱신, 설치 프로그램 시험)는 `playbooks.md`를 봅니다.

## 기술 스택

- **앱**: Rust, [GPUI Kit](https://github.com/longbridge/gpui-kit) 0.9(`app`)
- **공용 코어**: `crates/araview-core`(디코드, 아카이브, 캐시, 썸네일, 파일 작업)
- **탐색기 썸네일**: `crates/araview-thumb`(COM DLL, Windows 전용)
- **배포 묶음**: Windows NSIS 설치 프로그램(`app/installer/araview.nsi`)
- 지원 OS: Windows 11 x64. 창 틀은 GPUI Kit의 타이틀바가 그립니다.
- 루트 `Cargo.toml`이 세 크레이트를 한 워크스페이스로 묶고 `Cargo.lock`도 하나입니다.

## 요구 사항

- **OS**: Windows 11 (x64)
- **Rust**: stable (`rust-toolchain.toml` 채널, 최소 버전은 `app/Cargo.toml`의 `rust-version = "1.100"`)
- **Node.js**: `>= 24` (`release`, `docs:check`, `licenses` 스크립트용. 의존성 설치는 없습니다)
- **NSIS**: Windows 설치 프로그램을 만들 때만 필요합니다(`makensis`가 PATH에 있거나 `Program Files (x86)\NSIS`에 있어야 합니다)

HEIC/HEIF/AVIF를 쓰려면 `libheif` 1.21 이상이 필요합니다. `vcpkg.json`은 `libheif`의 `default-features`를 끄고(`hevc` 인코더 `x265` 제외) `aom`만 켭니다. HEVC 디코더 `libde265`는 하드 의존성이라 항상 들어갑니다. 버전은 baseline의 1.21.2와 1.23.1이 모두 Windows에서 DLL 로드 시 초기화 실패(`0xC0000142`)를 일으켜 `cargo test`가 실행조차 되지 않으므로, `overrides`로 1.23.2를 고정합니다(`libheif-rs 3.0`의 sys 바인딩도 1.23 기준). `aom` feature는 AVIF(AV1) 디코드용이며, 없으면 AVIF 표시와 썸네일, 히스토그램이 실패합니다. 움직이는 AVIF를 디코드하려면 1.21 이상이 필요합니다(`libheif-rs`의 `v1_21` feature). 앱은 HEIC/AVIF를 인코드하지 않습니다.

### Windows

[vcpkg](https://vcpkg.io/) 매니페스트로 네이티브 의존성(`libheif`)을 설치합니다. 저장소 루트에서 실행해야 `vcpkg.json`이 적용됩니다.

```powershell
cd <araview>
vcpkg install --triplet x64-windows --x-install-root="<vcpkg root>/installed"
$env:VCPKG_ROOT = "<vcpkg root>"
$env:Path += ";$env:VCPKG_ROOT\installed\x64-windows\bin"
```

`--x-install-root`는 pin은 그대로 두고 설치 위치만 vcpkg 루트로 옮긴다.
`app/build.rs`와 pkg-config 설정이 그 위치를 보기 때문이다(그냥 `vcpkg install`만
하면 저장소 아래 `vcpkg_installed/`에 깔려 빌드가 찾지 못한다).

앱은 `libheif`를 동적 링크합니다. `app/build.rs`가 `VCPKG_ROOT`(또는 `VCPKG_INSTALLATION_ROOT`) 아래 `installed/x64-windows/bin`에서 `heif.dll`, `libde265.dll`, `aom.dll`을 실행 파일 옆(`target/<profile>/`)으로 복사하고, vcpkg의 `libheif`, `libde265`, `aom` `copyright`를 같은 위치의 `licenses/`로 모읍니다. 설치 프로그램은 이 파일들을 그대로 동봉합니다. CI에서는 `VCPKGRS_DYNAMIC=1`과 `PKG_CONFIG_PATH`를 함께 둡니다(워크플로 참조). `aom.dll`은 AVIF 이미지 디코드에 필요합니다.

## 시작하기

```bash
git clone https://github.com/ara-hwang/araview.git
cd araview
cargo run -p araview -- "C:/path/to/image.png"
```

개발(디버그) 빌드는 식별자 `com.araview.viewer.dev`를 씁니다. 설치본(`com.araview.viewer`)과 동시에 실행할 수 있고, 설정과 캐시도 따로 저장됩니다.

| 저장   | 위치                             |
| ------ | -------------------------------- |
| 설정   | `%APPDATA%\<식별자>\`            |
| 캐시   | `%LOCALAPPDATA%\<식별자>\`       |

Windows 개발 빌드는 확장자 연결이 설치 버전과 섞이지 않도록 `AraView (Dev)` 이름으로 등록됩니다. 기본 앱 목록에서 `AraView`(설치 버전)와 구분해 선택하세요. 확장자 연결 설정과 탐색기 썸네일은 Windows 셸 연동입니다.

### UI 검사 도구

GPUI Kit 기본 아이콘은 디버그 빌드에도 포함합니다(`rust-embed/debug-embed`). 크로스 빌드한 실행 파일을 빌드 머신의 소스 경로 없이 실행해도 창 버튼과 설정 컴포넌트 아이콘을 읽을 수 있습니다.

`cargo run -p araview --features devtools`로 실행하면 [GPUI DevTools](https://github.com/joris-gallot/gpui-devtools)를 사용할 수 있습니다. `Ctrl+Alt+I`로 검사 패널을 열고 닫습니다. 요소 선택, 크기·여백·스타일 확인과 임시 스타일 편집을 지원합니다. 초기화는 GPUI Kit 다음에 수행합니다.

`devtools`는 기본으로 꺼져 있습니다. 일반 개발 빌드와 기본 릴리스 빌드에는 포함되지 않으며, `--release --features devtools`를 명시하면 릴리스에도 포함됩니다. Keyboard 탭은 GPUI 액션을 보여 주므로, 자체 키 핸들러로 처리하는 뷰어 명령은 액션 목록에 모두 나타나지 않습니다.

### 개발 빌드 속도

루트 `Cargo.toml`의 `[profile.dev.package]`는 워크스페이스 밖 의존성(`"*"`, `vendor/` 포크 포함)의 디버그 정보를 `debug = 1`로 줄입니다. 줄 번호와 스택 추적은 유지하지만 의존성 내부의 지역 변수와 타입 디버깅 정보는 생략합니다. `app`, `araview-core`, `araview-thumb`는 전체 디버그 정보를 그대로 유지합니다. 이 설정은 dev를 상속하는 test에도 적용되며, 기존 GPUI·이미지 처리 의존성의 최적화 수준과 release 설정은 바꾸지 않습니다.

의존성 내부까지 전체 디버깅이 필요하면 설정 파일을 바꾸지 않고 다음처럼 한 번만 빌드합니다. 프로필이 달라진 의존성은 다시 컴파일됩니다.

```powershell
cargo build -p araview --config 'profile.dev.package."*".debug=2'
```

병목을 다시 측정할 때는 `cargo build -p araview --locked --timings`로 `target/cargo-timings/` 보고서를 봅니다. 변경 없는 빌드(캐시 확인), 앱만 다시 빌드, 의존성까지 다시 빌드는 구분해서 비교합니다. 이 설정의 근거는 의존성이 준비된 상태에서 `cargo rustc -p gpui-component --lib --locked --timings -- -C debuginfo=2 -C metadata=araview_build_bench`와 `debuginfo=1`을 비교한 Windows 측정입니다. 각각 37.10초와 25.28초였으며, 이는 해당 크레이트의 측정이지 앱 전체 빌드의 단축률은 아닙니다.

## 스크립트

```bash
# 개발 실행
cargo run -p araview

# 검사
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p araview --features devtools   # 선택 feature는 위 clippy에 포함되지 않음
cargo test --workspace

# 실행 중인 창 조작(개발 빌드 전용, docs/playbooks.md "Runtime check")
pwsh scripts/araview-drive.ps1 launch -Path samples/sample.jpg
pwsh scripts/araview-drive.ps1 selftest
pwsh scripts/araview-drive.ps1 hittest  # Windows가 판정하는 캡션 영역 읽기(커서를 옮기지 않음)
pwsh scripts/araview-mcp.ps1   # 같은 도구를 MCP 서버(stdio)로

# 배포 묶음(릴리스 빌드 + 패키징). 서명과 업로드는 하지 않는다
pwsh scripts/Build-Installer.ps1     # Windows: NSIS 설치 프로그램

# 릴리스 시작 (버전 올리기, 커밋, 태그, 푸시)
npm run release -- <X.Y.Z|patch|minor|major>   # main

# 문서-코드 정합성 (버전, 확장자, 설정 키, plan)
npm run docs:check

# 디코드 출력 다이제스트 (디코드·축소 경로를 리팩터링할 때 전후로 뽑아 diff한다.
# 라인이 하나라도 다르면 출력이 바뀐 것이므로 의도한 변경인지 확인한다)
cargo run -p araview-core --example digest -- samples > before.txt
cargo run -p araview-core --example digest -- samples > after.txt
diff before.txt after.txt

# 서드파티 라이선스 자료 다시 만들기 (cargo-about 필요, 네트워크 사용)
npm run licenses
```

## 앱 구조 요약

- `app`는 UI와 입력만 맡고, 파일·이미지 작업은 `araview-core`의 동기 `ops/` 함수를 `cx.background_spawn`으로 호출합니다.
- 렌더링은 `display.rs`의 RGBA 프레임을 GPU 타일로 그리고, 큰 JPEG/PNG는 `preview.rs`의 축소 디코드/행 샘플링을 먼저 시도합니다. 큰 이미지의 타일·축소 단계 규칙은 `SPEC.md` §18을 따릅니다.
- 단축키는 GPUI 키맵 대신 `src/keys.rs`에서 설정 맵을 직접 찾습니다. 파일 열기 대화상자는 `rfd`를 쓰고, Win/Super 키는 사용하지 않습니다(§14.3).
- 고대비와 모션 최소화는 Windows 설정을 따릅니다(`src/platform/`, `system.rs`). 테마가 '시스템'이면 OS 밝기 변경을 즉시 반영합니다.
- 플랫폼 계층은 `app/src/platform/mod.rs`의 공개 함수 집합을 `windows.rs`가 구현합니다. 항상 위는 항상 지원됩니다(`ALWAYS_ON_TOP_SUPPORTED`).
- 설정은 `<설정 폴더>/<식별자>/settings.json`에 저장합니다. `settings`/`recentFiles`/`archiveProgress` 모양은 1.x(Tauri)와 같고 `theme`, `window`(창 위치·크기) 키가 더 있어 1.x 설치본의 설정과 최근 파일을 이어받습니다.
- 단일 인스턴스: 두 번째 실행은 첫 인자를 명명된 파이프(`\\.\pipe\<식별자>`)로 기존 창에 넘기고 종료합니다.
- 개발 빌드 제어: `scripts/araview-drive.ps1`(CLI)와 `scripts/araview-mcp.ps1`(MCP)이 `src/app/bridge.rs`의 제어 파이프를 씁니다. 입력은 `Window::dispatch_event`/`dispatch_keystroke`로 들어가 사용자의 마우스·키보드를 건드리지 않습니다. 릴리스 빌드에는 이 통로가 없습니다. 절차는 `docs/playbooks.md` "Runtime check".
- 업데이트: 동작 계약은 `SPEC.md` §20.1. 로컬 검증은 `cargo run -p araview --example local_update_feed -- <setup.exe> <출력 폴더> <피드 URL>`로 피드를 만들고, `ARAVIEW_UPDATE_FEED`/`ARAVIEW_UPDATE_PUBKEY`를 준 디버그 빌드에서 확인합니다. 릴리스 빌드는 두 환경 변수를 읽지 않습니다.
- 업데이트 TLS는 OS 신뢰 저장소를 사용합니다. 프록시·조직 CA는 OS에서 신뢰하도록 설치해야 하며, `UnknownIssuer`를 피하려고 검증을 끄지 않습니다. 실제 피드 연결 검사는 `cargo test -p araview live_update_feed_uses_trusted_roots -- --ignored --nocapture`로 명시적으로 실행합니다.
- 라이선스: `npm run licenses`가 `app/THIRD_PARTY_LICENSES.json`과 `.md` 2절을 만들고, `cargo deny check`가 라이선스·RustSec을 검사합니다. 예외는 `licenses-extra/`와 `EXTRA_LICENSES`, `about.toml`/`deny.toml`을 함께 고칩니다. 절차는 `docs/playbooks.md` "Update the third-party license data".
- 스크롤: `src/smooth.rs`의 `SmoothScroll`이 휠 입력을 목표 오프셋으로 쌓고 프레임마다 따라갑니다. 가상 목록은 핸들을 공유하고 기본 스크롤을 끄며, 웹툰은 `Glide`로 같은 감속을 씁니다. Tab 포커스는 `focus_reveal.rs`가 최소 거리로 즉시 스크롤합니다.
- 알림은 기본 토스트, 다이얼로그가 열려 있으면 경고창입니다(`src/toast.rs`). 파일을 여는 중(200ms 이상), 그림이 없을 때, 페이지 이동이 늦을 때(300ms 이상) 스피너를 보여줍니다.
- `cargo test -p araview`는 test-support 기능을 테스트 빌드에만 켜 GPUI의 결정적 executor·TestAppContext 회귀도 실행합니다. 이 검사는 실제 GPU·Win32·IME 검증을 대체하지 않습니다. 키·휠·드래그·드롭·우클릭·다이얼로그의 실제 Windows 동작은 개발 빌드와 제어 브리지로 확인합니다.
- 썸네일 요청 큐·목록 준비 캐시는 `app/src/app/thumbnail_state.rs`, 창 수명의 디코드 이미지 LRU는 `thumbnail_cache.rs`입니다. 동시 배치 2개·대기 256항목·이미지 LRU 512항목이며 바이트 한도를 보장하지 않습니다. 순수 정책 벤치는 `rustc --edition=2024 -O --test app/tests/thumbnail_state.rs -o <출력>` 후 `--ignored --nocapture`로 실행합니다.
- 설정 I/O는 `app/src/settings_writer.rs`의 단일 writer, 업데이트 I/O는 `app/src/app/update_io.rs`의 HTTP·시계·설치 경계, 안전한 진단 요약은 `diagnostic_summary.rs`, 로컬 panic 기록은 `panic_report.rs`가 맡습니다. 앱/core/thumb 크레이트 경계와 자체 updater는 유지합니다.

## 프로젝트 구조

```text
Cargo.toml                 # 워크스페이스(crates/*, app), lock 하나
app/                       # 표현·입력. build.rs는 아이콘, vcpkg DLL·라이선스, 링크 인자
crates/araview-core/       # UI 비의존 코어. ops/는 동기 연산, 나머지는 평면 모듈
crates/araview-thumb/      # 탐색기 썸네일 COM DLL(Windows 전용)
scripts/                   # check-docs, release, generate-license-data, bench, Build-Installer, araview-drive/mcp, test-*
vendor/                    # [patch.crates-io] 포크(gpui-pre-windows, gpui-component)
licenses-extra/            # 업스트림 LICENSE 사본
```

## 파생 이미지 캐시

- `settings.json`의 `settings.cacheStorageMode`를 읽어 시작 시 저장 루트를 선택합니다. 값이 없으면 `persistent`입니다.
- 영구 모드는 캐시 폴더(위 표)의 `<식별자>/cache-v2/`를 사용합니다. 식별자는 설치 빌드가 `com.araview.viewer`, 디버그 빌드가 `com.araview.viewer.dev`라 개발 빌드는 별도 루트를 씁니다.
- `temporary` 모드는 사용자 앱 캐시 디렉터리의 `session-v1/` 아래 프로세스별 `TempDir`를 사용합니다. 정상 종료 이벤트와 다음 시작 시 이전 세션 루트를 정리합니다. 모드 설정 변경은 다음 실행부터 적용되며, 영구 캐시를 끄면 다음 시작 때 이전 버전 루트를 정리합니다.
- `cache.rs`의 `get_cache_stats`와 `clear_cache`는 캐시 트리만 대상으로 합니다. 사용자 원본 파일 경로를 입력받아 삭제하지 않습니다.
- `process_temp::mark_in_use`로 보호된 파일과 `*.tmp-<pid>-...` 작성 중 파일은 삭제하지 않습니다. 영구 캐시 hit은 mtime을 갱신해 LRU 순서를 유지합니다.
- 파생 캐시 작성은 `sidecar::write_cache_atomic(dest, context, write)`으로 수행합니다. 호출자는 같은 확장자의 임시 경로에 쓰고 파일 핸들을 닫은 뒤 결과를 반환합니다. 모듈이 작성 중 보호, 발행, 오류·panic unwinding 정리를 맡습니다. 부모 폴더 생성·잠금·축출 정책은 호출자에 남고, 사용자 원본 저장에는 사용하지 않습니다.
- 상한: 하위 캐시별(썸네일·`paint/`·`scaled/`·`clipboard/` 500MB, 아카이브별 추출 디렉터리 1GiB)은 쓸 때 누적 바이트로, 루트 전체 2GiB(`MAX_TOTAL_CACHE_BYTES`)는 시작 시와 새 아카이브 첫 열기 때 `request_total_cap_scan`으로 강제합니다. 모두 넘으면 오래된 것부터 지우며, 추출을 중단하지 않습니다(`SPEC.md` §8, §9.2).

## 픽셀 보존 표시와 축소

- 픽셀 보존(`pixelated`)은 확대 배율에서만 적용됩니다. 축소 배율에서는 nearest 계열 보간이 스크린톤 같은 주기 패턴을 계단·무아레로 깨뜨리므로 설정과 무관하게 항상 smooth로 렌더합니다(`app/src/app/viewport.rs`의 배율 게이트).
- 자동 모드(`auto`)는 2x 이상 확대에서 pixelated로 렌더합니다(`AUTO_PIXELATED_ZOOM`). 보간 없이 원본 픽셀을 그대로 보여주기 위함입니다. 이미지 내용을 분석하는 픽셀 아트 감지는 없습니다.
- 표시 해상도 상한 축소 sidecar(`crates/araview-core/src/scaled.rs`)도 표시 정책과 무관하게 항상 보간 필터(`Triangle`)를 사용합니다. 축소는 픽셀을 버리는 연산이므로 픽셀 보존 판정이 의미가 없습니다.

## 탐색기 썸네일 구현 메모

Windows 전용입니다. 사용자 동작 계약은 `SPEC.md` §20.2를 따릅니다.

- `crates/araview-thumb/`의 In-Proc COM DLL(`araview_thumb.dll`)은 `IThumbnailProvider`와 `IInitializeWithStream`/`IInitializeWithFile`을 구현합니다. PSD·CBZ/ZIP·HEIC/HEIF/AVIF·EXR·지원 RAW를 `araview-core::ops::decode_shell_thumbnail`로 디코드합니다. PSB는 거부합니다.
- CLSID(발행 후 변경 금지): 릴리스 `{FD6BD976-2DF4-4656-94F2-1D166163EC59}`, 개발 `{BD277595-1702-4AC5-AA7C-A67965C3D570}`. 문자열과 등록 구현의 정본은 `crates/araview-core/src/thumb_shell.rs`이고 DLL도 재사용합니다. `crates/araview-thumb/src/com.rs`의 GUID도 같은 값이어야 합니다.
- 등록은 HKCU(`Software\Classes`)의 `CLSID\{CLSID}\InprocServer32`(DLL 경로, `ThreadingModel=Apartment`), 지원 확장자와 채널 ProgID의 `ShellEx\{E357FCCD-A995-4576-B01F-234630154E96}` 슬롯입니다. 이미지 확장자에만 누락된 `PerceivedType`/`Content Type`을 채웁니다. 기존 사용자별 핸들러는 슬롯의 채널별 `AraView.Previous.{CLSID}` 값에 보관하고 해제 시 복원합니다. 등록/해제 후 `SHChangeNotify`를 보냅니다.
- 스트림은 최대 1GiB를 청크로 임시 파일에 복사합니다. `Stat` 이름에서 지원 확장자만 가져오고, 이름이 없으면 시그니처로 구분합니다. 요청마다 처음으로 되감아 반복 요청도 지원합니다. 표지 추출은 기존 엔트리 크기·경로 보호를 따르며 임시 파일은 RAII로 삭제합니다. 앱의 영구 이미지 캐시는 사용하지 않습니다.
- `scripts/Build-Installer.ps1`은 DLL과 `heif.dll`, `libde265.dll`, `aom.dll`을 설치 프로그램의 `resources/`에 넣습니다. 기존 DLL 등록 경로를 유지하고 셸 호스트에서도 코덱을 찾게 하기 위함입니다. 앱은 exe 옆, `resources/` 순으로 DLL을 찾습니다.
- `cargo build -p araview-thumb` 후 `cargo run -p araview-thumb --example probe -- D:\araview\target\debug\araview_thumb.dll D:\araview\samples`는 레지스트리 변경 없이 실제 DLL의 COM 진입점, 파일·무명 스트림 초기화, 반복 요청과 오류 반환을 검사합니다. 경로는 현재 체크아웃에 맞게 바꿉니다.
