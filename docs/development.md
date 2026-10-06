# 개발 안내

개발자용 안내입니다. 기능/기술 규격 진실은 `../SPEC.md`, 제품 정의는 `../PRODUCT.md`를 따릅니다.

## 기술 스택

- **Frontend**: React 19, TypeScript, Vite 6, Tailwind CSS 4, TanStack Router v1, Zustand 5
- **Backend**: Rust, Tauri 2
- **Tauri Plugins**: `dialog`, `opener`, `store`, `window-state`, `single-instance`, `updater`, `snap-layout` (+ dev 전용 `mcp-bridge`)

## 요구 사항

- **OS**: Windows 11 (x64)
- **Node.js**: `>= 24`
- **npm**: `>= 11.7` (11.6 이하는 `npm install`만으로 `package-lock.json`의 번들 항목을 지워 `npm ci`가 실패합니다. `preinstall` 가드 `scripts/check-npm-version.mjs`가 낮은 버전의 설치를 실패시킵니다. 이때 락파일은 이미 바뀌었을 수 있으니 `git checkout package-lock.json`으로 되돌립니다. `npm install -g npm@11`로 올립니다)
- **Rust**: stable (`1.97` 이상 권장, 릴리스 워크플로 기준)

HEIC/HEIF를 쓰려면 [vcpkg](https://vcpkg.io/)로 `libheif`를 설치합니다.

```powershell
vcpkg install "libheif[core,aom]:x64-windows"
$env:VCPKG_ROOT = "<vcpkg root>"
$env:Path += ";$env:VCPKG_ROOT\installed\x64-windows\bin"
```

`[core]`는 HEVC 디코더 `libde265`만 넣고, 인코더 `x265`는 빼는 설치입니다. `aom` feature는 AVIF(AV1) 디코드용이며, 없으면 AVIF 썸네일과 히스토그램이 실패합니다. 앱은 HEIC/AVIF를 인코드하지 않습니다.

앱은 `libheif`를 동적 링크합니다(`libheif-rs`, `default-features = false`, 정적 포함 없음). 번들용 DLL은 `src-tauri/build.rs`가 `VCPKG_ROOT`(또는 `VCPKG_INSTALLATION_ROOT`) 아래 `installed/x64-windows/bin`에서 `heif.dll`, `libde265.dll`, `aom.dll`을 모아 `generated/libheif-dlls/`에 넣고, `src-tauri/tauri.windows.conf.json` 경유로 번들에 실립니다. CI에서는 `VCPKGRS_DYNAMIC=1`과 `PKG_CONFIG_PATH`를 함께 둡니다(워크플로 참조).

라이선스 원문도 같은 빌드 스크립트가 `generated/licenses/`에 모아 설치 프로그램의 `licenses/` 폴더로 동봉합니다. vcpkg의 `libheif`, `libde265`, `aom` `copyright`와 저장소의 `THIRD_PARTY_LICENSES.md`, 패키지별 라이선스 전문 `THIRD_PARTY_LICENSES.json`이 들어갑니다. 환경설정의 라이선스 화면은 이 자료를 읽어 보여줍니다. 의존성을 바꾸면 `cargo install cargo-about --locked --features cli`를 한 번 설치한 뒤 `node scripts/generate-license-data.mjs`를 실행합니다. Rust 본문은 `src-tauri/about.toml` 설정의 cargo-about이 모으고(네트워크 사용), 스크립트가 `THIRD_PARTY_LICENSES.json`과 `THIRD_PARTY_LICENSES.md`의 2, 3절 표를 다시 씁니다. 1절(네이티브 라이브러리)은 직접 고칩니다. 허용하지 않은 라이선스는 `cd src-tauri && cargo deny check licenses bans sources`(설정 `src-tauri/deny.toml`, `cargo install cargo-deny --locked`)로 검사하며 CI와 릴리스 워크플로도 같은 검사를 합니다. 패키지에 LICENSE 파일이 없어 표준 템플릿("Copyright (c) <year> <owner>")이 나오는 크레이트는 스크립트가 경고로 알려 주며, 업스트림 LICENSE를 `src-tauri/licenses-extra/`에 복사하고 스크립트의 `EXTRA_LICENSES`에 추가합니다. 새 라이선스가 나오면 `about.toml`의 `accepted`와 `deny.toml`의 `allow`를 같이 고칩니다.

## 시작하기

```bash
git clone https://github.com/ara-hwang/araview.git
cd araview
npm install
npm run tauri dev
```

`npm run tauri dev` 실행 시 Vite(`http://localhost:1420`)와 Tauri 앱이 함께 실행됩니다.

Tauri MCP로 UI를 검증할 때는 `npm run dev:up`을 씁니다. 이 스크립트는 `src-tauri/tauri.dev.conf.json`을 함께 적용해 개발 빌드에서만 `withGlobalTauri`를 켜고 식별자를 `com.araview.viewer.dev`로 분리합니다(프로덕션은 꺼져 있고 CSP가 적용됩니다). 덕분에 설치본과 dev 앱이 동시에 실행될 수 있고, 설정과 최근 파일은 dev 전용 저장소에 따로 저장됩니다. MCP 브리지도 이때 loopback(`127.0.0.1:9323`)에만 바인딩됩니다. 포트 9323은 dev 전용 고정값이며, 설치본이나 다른 Tauri 앱이 기본값 9223을 선점해도 dev 검증이 엉뚱한 앱에 붙지 않게 합니다.

개발(디버그) 빌드는 확장자 연결이 설치 버전과 섞이지 않도록 `AraView (Dev)` 이름으로 등록됩니다. 기본 앱 목록에서 `AraView`(설치 버전)와 구분해 선택하세요.

## 스크립트

```bash
# 개발
npm run tauri dev

# 프론트엔드 빌드 (tsc + vite)
npm run build

# 데스크톱 앱 빌드
npm run tauri build

# PSD 탐색기 썸네일 DLL 빌드
npm run build:thumb

# Tauri MCP 검증용 dev 실행 (Vite :1420 + 브리지 :9323 대기)
npm run dev:up

# 릴리스 시작 (버전 올리기, 커밋, 태그, 푸시)
npm run release -- <X.Y.Z|patch|minor|major>

# 테스트
npm test
npm run test:watch

# 린트
npm run lint
npm run lint:fix

# 타입 체크 / 포맷
npx tsc --noEmit
npm run format:check
npm run format

# 문서-코드 정합성 (확장자, IPC, 설정, 플러그인, 버전, plan, i18n 잔재)
npm run docs:check

# 러스트 테스트 / 린트
cd src-tauri && cargo test
cd src-tauri && cargo clippy
```

빌드 결과물은 `src-tauri/target/release/bundle/` 아래에 생성됩니다.

## 동작 구조 요약

- 프론트엔드는 Tauri IPC로 백엔드 명령을 호출합니다. 전체 계약은 `SPEC.md` §15이 진실입니다.
- 백엔드는 파일 경로/메타데이터를 반환하고, 프론트는 `convertFileSrc`로 렌더링합니다.
- 파일 연동으로 앱이 실행되면 `open-file` 이벤트를 통해 대상 파일을 자동 오픈합니다.

## 프로젝트 구조

```text
src/
  components/        # UI 컴포넌트
  hooks/             # 비즈니스 로직 훅
  routes/            # TanStack Router 라우트
  store/             # Zustand 상태 저장소
  constants/         # 지원 확장자 등 상수
  types/             # 공용 타입

src-tauri/
  src/commands/      # Tauri command (도메인별 모듈, mod.rs에서 재내보내기)
  src/file_assoc.rs  # Windows 확장자 연결(레지스트리)
  src/image.rs       # MIME/확장자 판별(resolve_mime), load_viewable
  src/sniff.rs       # 파일 선두 시그니처 기반 포맷 판별
  src/heif.rs        # HEIC/HEIF 디코드 및 JPEG sidecar
  src/psd_sidecar.rs # PSD 합성 디코드(`psd` 크레이트) 및 JPEG sidecar (읽기 전용)
  src/raster_sidecar.rs # TGA/DDS/EXR/QOI 디코드(`image` 크레이트) 및 JPEG sidecar (읽기 전용)
  src/transcode.rs # 포맷(resolve_mime)별 sidecar 디코더 디스패치
  src/process_temp.rs # 임시/영구 파생 이미지 캐시 루트, 보호, 상한, 시작 정리
  src/cache.rs        # 캐시 통계 및 종류별/전체 삭제
  src/archive.rs      # 아카이브 목록/추출 처리 (cbz/zip)
  src/archive_index.rs # 아카이브 엔트리 목록 캐시 (mtime+size 검증 LRU)
  src/comic_info.rs  # CBZ/ZIP ComicInfo.xml 파싱, 표지 지정 쓰기
  src/thumbnail.rs   # 썸네일 생성/캐시
  src/svg_raster.rs  # SVG 래스터화(resvg, 썸네일/히스토그램 전용)
  src/svg_size.rs    # SVG 헤더(width/height/viewBox) 치수 파서
  src/sidecar.rs     # sidecar/썸네일/추출물 공용 헬퍼(해시, 락, 원자 발행)
  src/scaled.rs      # 표시 해상도 제한 축소본
  src/thumb_shell.rs # PSD 탐색기 썸네일 셸 연동 명령
  src/app_error.rs   # 구조화 에러 코드 매핑
  src/dir_cache.rs    # 디렉토리 목록 캐시
  src/file_availability.rs # Files On-Demand availability 판별
  src/image_info.rs  # 이미지 상세/파일 정보 조회
  src/stable_hash.rs # 영속 캐시 파일명용 안정 해시
  src/orientation.rs # EXIF Orientation 읽기/적용 (JPEG 표시·썸네일 정합)
  src/pixel_art.rs   # 표시용 픽셀 아트 휴리스틱 감지
  src/lib.rs         # Tauri 앱 설정 및 command 등록
```

## 파생 이미지 캐시

- `settings.json`의 `settings.cacheStorageMode`를 읽어 시작 시 저장 루트를 선택합니다. 값이 없으면 `persistent`입니다.
- 영구 모드는 Tauri `app_cache_dir()` 아래 `cache-v2/`을 사용하고, 개발 빌드는 `tauri.dev.conf.json`의 identifier 때문에 별도 루트를 사용합니다.
- `temporary` 모드는 사용자 앱 캐시 디렉터리의 `session-v1/` 아래 프로세스별 `TempDir`를 사용합니다. 정상 종료 이벤트와 다음 시작 시 이전 세션 루트를 정리합니다. 모드 설정 변경은 다음 실행부터 적용되며, 영구 캐시를 끄면 다음 시작 때 이전 버전 루트를 정리합니다.
- `cache.rs`의 `get_cache_stats`와 `clear_cache` 명령은 캐시 트리만 대상으로 합니다. 사용자 원본 파일 경로를 입력받아 삭제하지 않습니다.
- `process_temp::mark_in_use`로 보호된 파일과 `*.tmp-<pid>-...` 작성 중 파일은 삭제하지 않습니다. 영구 캐시 hit은 mtime을 갱신해 LRU 순서를 유지합니다.

## 픽셀 보존 표시와 축소

- 픽셀 보존(`pixelated`)은 확대 배율에서만 적용됩니다. 축소 배율에서는 nearest 계열 보간이 스크린톤 같은 주기 패턴을 계단·무아레로 깨뜨리므로 설정과 감지 결과와 무관하게 항상 smooth로 렌더합니다(`src/utils/imageRendering.ts`의 배율 게이트).
- 표시 해상도 상한 축소 sidecar(`src-tauri/src/scaled.rs`)도 감지 결과와 무관하게 항상 보간 필터(`Triangle`)를 사용합니다. 축소는 픽셀을 버리는 연산이므로 픽셀 보존 판정이 의미가 없습니다.

## 픽셀 아트 감지 참고 자료

`src-tauri/src/pixel_art.rs`의 감지는 AraView에서 작성한 경량 휴리스틱입니다. 외부 모델이나 네트워크를 사용하지 않으며, 다음 공개 자료의 개념을 참고했습니다.

- [unfake.js](https://github.com/jenissimo/unfake.js), MIT License: 동일 색상 run과 반복 길이 기반 스케일 탐지, Sobel 경계 프로파일과 주기성 분석의 실용적인 조합
- Johannes Kopf and Dani Lischinski, [Depixelizing Pixel Art](https://johanneskopf.de/publications/pixelart/paper/pixel.pdf): 작은 팔레트, 픽셀 단위 경계, 평탄한 색상 영역의 특징
- [MDN image-rendering](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/image-rendering): `smooth`와 `pixelated` 표시 의미 및 브라우저 지원

AraView는 참고 자료의 코드를 복사하거나 런타임 의존성으로 포함하지 않습니다. 외부 코드나 모델을 추가할 때는 이 절과 `THIRD_PARTY_LICENSES.md`를 함께 갱신합니다.

## PSD 탐색기 썸네일 구현 메모

사용자 동작 계약은 `SPEC.md` §20.2를 따릅니다. 아래는 개발용 구현 메모입니다.

- 핸들러는 `src-tauri/crates/araview-thumb/`의 In-Proc COM DLL(`araview_thumb.dll`)이며 `IThumbnailProvider` + `IInitializeWithStream`/`IInitializeWithFile`을 구현합니다. PSB(`8BPS` version 2)는 거부합니다.
- CLSID(발행 후 변경 금지): 릴리스 `{FD6BD976-2DF4-4656-94F2-1D166163EC59}`, 개발 `{BD277595-1702-4AC5-AA7C-A67965C3D570}`. `registry.rs`와 `thumb_shell.rs`, `com.rs` 세 곳(`crates/araview-thumb/src/registry.rs`, `src/thumb_shell.rs`, `crates/araview-thumb/src/com.rs`)에 중복 정의되어 함께 바꿔야 합니다.
- 등록은 전부 HKCU(`Software\Classes`)입니다: `CLSID\{CLSID}\InprocServer32`(DLL 경로 + `ThreadingModel=Apartment`), `.psd`와 채널 ProgID의 `ShellEx\{E357FCCD-A995-4576-B01F-234630154E96}` 슬롯, `.psd`의 `PerceivedType`/`Content Type` 채우기. 등록/해제 후 `SHChangeNotify`를 보냅니다.
- DLL 전달: 워크스페이스 멤버로 함께 빌드합니다. 릴리스는 `scripts/Build-ThumbDll.ps1`(`npm run build:thumb`)로 빌드해 `src-tauri/resources/`에 스테이징하면 NSIS 번들에 실립니다. Windows 리소스는 `src-tauri/tauri.windows.conf.json`에 반복해야 합니다(플랫폼 설정이 `bundle.resources`를 통째로 교체). 이 파일은 strict JSON이라 주석을 넣으면 빌드가 실패합니다. 앱은 exe 옆, `resources/` 순으로 DLL을 찾습니다.
