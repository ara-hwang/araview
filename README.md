# AraView

Tauri 2 + React 18 + TypeScript 기반의 Windows 데스크톱 이미지 뷰어입니다.

- 일반 이미지 포맷과 만화 아카이브(`.cbz`, `.cb7`, `.cbr`, `.cbt`) 및 일반 아카이브(`.zip`, `.7z`, `.rar`)를 지원합니다.
- 폴더 내 이미지 탐색, EXIF 표시, 슬라이드쇼, 멀티 페이지 보기 모드를 제공합니다.
- 로컬 파일 경로 기반 렌더링(Asset Protocol)으로 동작합니다.

## 주요 기능

- **지원 포맷**: `png`, `jpg`, `jpeg`, `gif`, `bmp`, `webp`, `svg`, `ico`, `tiff`, `tif`, `avif`, `heic`, `heif`, `cbz`, `cb7`, `cbr`, `rar`, `zip`, `7z`, `cbt`
- **파일 열기 방식**: 파일 선택, 드래그 앤 드롭(파일/폴더), OS 파일 연동으로 앱 실행
- **탐색**: 이전/다음 이동, 썸네일/슬라이더 이동, 루프 내비게이션
- **보기**: 확대/축소, 화면 맞춤(가로/세로/전체), 회전, 좌우/상하 반전
- **썸네일 그리드**(`G`): 폴더/아카이브 전체를 한눈에 보고 클릭 또는 키보드로 점프, 파일명 필터 지원
- **멀티 페이지 모드**: `single`, `left-to-right`, `right-to-left`, `webtoon`
- **부가 기능**: EXIF 패널, 최근 파일 목록, 즐겨찾기, 슬라이드쇼, 전체화면, 이미지 클립보드 복사(PNG 변환), 명령 팔레트(`Ctrl+K`)

## 기술 스택

- **Frontend**: React 18, TypeScript, Vite 6, Tailwind CSS 4, TanStack Router v1, Zustand 5
- **Backend**: Rust, Tauri 2
- **Tauri Plugins**: `dialog`, `fs`, `opener`, `store`, `window-state`, `updater`, `process`

## 요구 사항

- **OS**: Windows 10/11 (x64)
- **Node.js**: `>= 22`
- **Rust**: stable (`1.94` 이상 권장)

HEIC/HEIF를 쓰려면 [vcpkg](https://vcpkg.io/)로 `libheif`를 설치합니다.

```powershell
vcpkg install "libheif[core]:x64-windows"
$env:VCPKG_ROOT = "<vcpkg root>"
$env:Path += ";$env:VCPKG_ROOT\installed\x64-windows\bin"
```

`[core]`는 HEVC 디코더 `libde265`만 넣고, 인코더 `x265`는 빼는 설치입니다. 앱은 HEIC를 인코드하지 않습니다.

앱은 `libheif`를 동적 링크합니다. `embedded-libheif` Cargo 기능은 켜지 마세요. 설치본은 vcpkg DLL이 실행 파일 옆에 있어야 합니다. `VCPKG_ROOT`가 있으면 `src-tauri/build.rs`가 `heif.dll`과 `libde265.dll`을 복사합니다.

## 시작하기

```bash
git clone https://github.com/ara-hwang/araview.git
cd araview
npm install
npm run tauri dev
```

`npm run tauri dev` 실행 시 Vite(`http://localhost:1420`)와 Tauri 앱이 함께 실행됩니다.

Tauri MCP로 UI를 검증할 때는 `npm run dev:up`을 씁니다. 이 스크립트는 `src-tauri/tauri.dev.conf.json`을 함께 적용해 개발 빌드에서만 `withGlobalTauri`를 켭니다(프로덕션은 꺼져 있고 CSP가 적용됩니다). MCP 브리지도 이때 loopback(`127.0.0.1:9223`)에만 바인딩됩니다.

## 스크립트

```bash
# 개발
npm run tauri dev

# 프론트엔드 빌드 (tsc + vite)
npm run build

# 데스크톱 앱 빌드
npm run tauri build

# 테스트
npm test
npm run test:watch

# 타입 체크 / 포맷
npx tsc --noEmit
npx prettier --check "src/**/*.{ts,tsx}"
npx prettier --write "src/**/*.{ts,tsx}"

# 러스트 테스트 / 린트
cd src-tauri && cargo test
cd src-tauri && cargo clippy
```

빌드 결과물은 `src-tauri/target/release/bundle/` 아래에 생성됩니다.

## 단축키

- **파일/탐색**: `Ctrl+O`(열기), `Ctrl+Left`/`Ctrl+Right`(이전/다음), `S`(셔플 토글)
- **이동**: `Left`/`Right`/`Up`/`Down`(팬, 항상 동작)
- **줌**: `+`, `=`, `-`, `0`(초기화), `1`(가로 맞춤), `2`(세로 맞춤), `3`(화면 맞춤)
- **이미지 조작**: `R`(시계 회전), `Shift+R`(반시계 회전), `H`(좌우 반전), `V`(상하 반전), `Ctrl+S`(편집 저장)
- **기타**: `I`(EXIF), `G`(썸네일 그리드), `Space` 또는 `F5`(슬라이드쇼), `F11`(전체화면), `Ctrl+C`(이미지 복사), `D`(테마 전환), `B`(배경 순환), `F`(즐겨찾기 토글)
- **그리드 내부**: 화살표(이동), `Home`/`End`, `PageUp`/`PageDown`, `Enter`(열기), `Esc` 또는 `G`(닫기)
- **파일 관리**: `Del`(휴지통 이동), `Ctrl+Shift+E`(탐색기에서 보기), `Ctrl+Shift+O`(기본 앱으로 열기), `F2`(이름 변경), `Ctrl+Shift+C`(경로 복사)

## 설정 항목

설정은 헤더의 설정 버튼으로 여는 다이얼로그에서 관리합니다. 왼쪽 사이드바로 분류를 전환합니다.

- **일반 / Language**: 한국어 / English (OS 언어 자동 감지, 변경 즉시 적용)
- **일반 / Navigation**: 끝에서 정지 또는 루프 이동
- **일반 / Cache Mode**
  - `off`: 현재 이미지만 유지
  - `nearby`: 인접 이미지 중심 프리패치
  - `extended`: 더 넓은 범위 프리패치
  - `memory-1gb` / `memory-2gb`: 메모리 상한 기반 캐시
- **확장자 연결**: 연결을 바꾸면 해당 확장자의 Windows 기본 앱 선택 창이 열립니다.
- **View Mode**: `single`, `left-to-right`, `right-to-left`, `webtoon` (양면 모드는 2장씩 넘김)
- **Slideshow Interval**: 슬라이드쇼 간격(ms, 1~30초)
- **시작**: 홈 화면 표시 또는 마지막 파일 자동 열기

설정과 최근 파일 목록은 Tauri Store(`settings.json`)에 저장됩니다.

## 업데이트

앱은 시작 시 업데이트를 확인하지 않습니다. 설정 일반 탭의 `지금 확인` 버튼이나 명령 팔레트(`Ctrl+K`)의 `업데이트 확인`으로 직접 확인할 때만 네트워크를 씁니다. 새 버전이 있으면 토스트에서 다운로드 및 설치 후 다시 시작할 수 있습니다.

릴리스는 태그(`v*`) 푸시로 GitHub Releases에 발행되며, updater 아티팩트(`latest.json`, `.sig`)가 함께 첨부됩니다. 태그 푸시 시 릴리스 워크플로가 프런트/Rust 검사를 먼저 돌리고, 통과해야 빌드와 릴리스로 진행합니다. main 푸시와 PR에는 워크플로가 돌지 않으므로 검사는 로컬에서 먼저 실행하세요.

릴리스 생성 권한은 `GITHUB_TOKEN`(`contents: write`)을 씁니다. 저장소 Settings → Actions → General의 워크플로 권한이 `Read and write`여야 하며, `read`로 유지하려면 `contents: write` 권한의 fine-grained PAT를 `RELEASE_TOKEN` 시크릿으로 등록하세요(워크플로가 자동으로 그것을 사용).

### 업데이트 피드 (소스 저장소가 비공개인 동안)

앱의 updater는 인증 없이 엔드포인트를 받아야 하므로, 소스가 비공개인 동안에는 공개 저장소의 릴리스에 자산을 게시합니다.

```powershell
npm run release:local -- -Publish -UpdatesRepo ara-hwang/araview-updates
```

- 피드(`latest.json`)와 설치본이 모두 `ara-hwang/araview-updates` 릴리스에 있고, `plugins.updater.endpoints`는 `https://github.com/ara-hwang/araview-updates/releases/latest/download/latest.json`을 가리킵니다.
- 소스 저장소를 공개로 전환하면 `-UpdatesRepo` 없이 실행하고 endpoint를 원래 릴리스 URL로 되돌립니다. 엔드포인트는 앱에 포함되므로 그다음 릴리스부터 적용됩니다.

### 서명키 발급과 등록 (maintainer 1회)

```powershell
npm run tauri signer generate -- -w "$env:USERPROFILE\.tauri\araview.key"
```

- 키는 repo 밖(`$env:USERPROFILE\.tauri`)에 둡니다. 다른 폴더를 쓰려면 `$env:USERPROFILE` 대신 원하는 경로를 넣습니다.
- 공개키(`araview.key.pub` 내용)는 `src-tauri/tauri.conf.json`의 `plugins.updater.pubkey`에 넣습니다.
- 비밀키와 비밀번호는 repo Secrets에 등록합니다. CI에는 키 파일이 없으므로 비밀키는 파일 내용을 넣습니다.

```powershell
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo ara-hwang/araview --body ((Get-Content -Raw "$env:USERPROFILE\.tauri\araview.key").Trim())
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo ara-hwang/araview
```

값 끝에 개행이 붙으면 base64 디코드가 실패하므로 `.Trim()`으로 공백을 제거합니다.

### 로컬 서명 빌드

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = "$env:USERPROFILE\.tauri\araview.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "<키 비밀번호>"
npm run tauri build
```

- `tauri build`에서 `TAURI_SIGNING_PRIVATE_KEY` 값이 존재하는 파일 경로면 그 파일을 읽고, 아니면 값을 키 내용으로 봅니다. 경로와 내용 둘 다 됩니다.
- `TAURI_SIGNING_PRIVATE_KEY_PATH`는 `tauri signer sign` 전용이라 `tauri build`에서는 무시됩니다.
- 비밀번호를 설정하지 않으면 대화형 프롬프트가 뜨므로 비대화형 환경에서는 반드시 설정합니다. (`--ci` 또는 `CI` 환경이면 빈 문자열로 처리)
- 성공하면 `src-tauri/target/release/bundle/nsis/` 아래에 설치본과 `.sig`가 함께 생성됩니다.

### 서명 없이 설치본만 만들기

```powershell
npm run tauri build -- --no-sign
```

updater 서명을 건너뜁니다. `.sig`가 없으므로 릴리스 배포에는 쓸 수 없습니다.

### CI 없이 로컬에서 릴리스

CI 빌드가 오래 걸리거나 워크플로를 쓸 수 없을 때, 이 PC에서 빌드해 바로 릴리스할 수 있습니다.

```powershell
# .env.example을 .env.local로 복사해 키와 비밀번호를 채우면 환경변수 없이 동작합니다.
npm run release:local                 # 초안 릴리스 생성
npm run release:local -- -Publish     # 바로 공개
```

- 키와 비밀번호는 `.env.local`(gitignored)에 두는 걸 권장합니다. `.env.example`을 복사해 값을 채우면 환경변수를 하나도 설정하지 않아도 됩니다. 이미 설정된 환경변수가 `.env.local`보다 우선합니다.
- 서명 키 탐색 순서: `-KeyPath` → `TAURI_SIGNING_PRIVATE_KEY`(`.env.local` 포함) → `araview.key`(저장소 루트) → `~/.tauri/araview.key`. 저장소에 복사본을 두려면 루트에 `araview.key`를 놓으면 되고, `*.key`가 `.gitignore`에 있어 커밋되지 않습니다.
- 비밀번호는 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`(`.env.local` 포함)로 전달합니다. 미설정이면 빌드가 프롬프트로 멈춥니다.
- `.env.local`은 Vite도 읽지만 Vite는 `VITE_` 접두사만 클라이언트로 노출하므로 `TAURI_` 값은 프런트엔드 번들에 포함되지 않습니다.

- `scripts/Publish-LocalRelease.ps1`이 `tauri.conf.json`의 버전으로 태그(`vX.Y.Z`)를 확인하고, 서명 빌드, `latest.json` 생성, `gh release create`(기존 릴리스에는 업로드)까지 수행합니다.
- 산출물(설치본, `.sig`, `latest.json`)은 저장소 안 `release/vX.Y.Z/`에 모입니다. `.gitignore`에 포함되어 커밋되지 않고, `-OutputDir`로 위치를 바꿀 수 있습니다.
- 작업 트리가 지저분하거나 태그가 HEAD와 다른 커밋을 가리키면 중단합니다.
- `.sig`의 키 ID가 `plugins.updater.pubkey`와 다르면 중단합니다.
- 빌드를 건너뛰고 이미 만든 산출물을 올리려면 `-SkipBuild`, GitHub를 건드리지 않고 결과만 확인하려면 `-DryRun`을 씁니다.
- 같은 태그에 CI가 만든 릴리스가 이미 있으면 그 릴리스의 자산을 덮어씁니다(`--clobber`). CI와 로컬 중 한쪽만 쓰세요.

## 동작 구조 요약

- 프론트엔드는 Tauri IPC로 백엔드 명령을 호출합니다.
  - `load_image(file_path)`
  - `get_directory_images(file_path)`
  - `generate_thumbnail(file_path, max_side?)`
  - `generate_archive_thumbnail(archive_path, entry_name, max_side?)`
  - `resolve_dropped_path(path)`
  - `get_exif_data(file_path)`
  - `get_archive_images(file_path)`
  - `load_archive_image(archive_path, entry_name)`
  - `get_file_associations()`
  - `set_file_association(extension, associate)`
  - `set_all_file_associations(associate)`
  - `open_default_apps_settings()`
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
  src/commands.rs    # Tauri command
  src/file_assoc.rs  # Windows 확장자 연결(레지스트리)
  src/image.rs       # MIME/확장자 판별, load_viewable
  src/heif.rs        # HEIC/HEIF 디코드 및 JPEG sidecar
  src/process_temp.rs # 프로세스 수명 임시 디렉터리
  src/archive.rs     # 아카이브 목록/추출 처리 (cbz/zip, cb7/7z, cbr/rar, cbt)
  src/lib.rs         # Tauri 앱 설정 및 command 등록
```
