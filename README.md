# Tauri Image Viewer

Tauri 2 + React 18 + TypeScript 기반의 데스크톱 이미지 뷰어입니다.

- 일반 이미지 포맷과 만화 아카이브(`.cbz`)를 지원합니다.
- 폴더 내 이미지 탐색, EXIF 표시, 슬라이드쇼, 멀티 페이지 보기 모드를 제공합니다.
- 로컬 파일 경로 기반 렌더링(Asset Protocol)으로 동작합니다.

## 주요 기능

- **지원 포맷**: `png`, `jpg`, `jpeg`, `gif`, `bmp`, `webp`, `svg`, `ico`, `tiff`, `tif`, `avif`, `heic`, `heif`, `cbz`
- **파일 열기 방식**: 파일 선택, 드래그 앤 드롭(파일/폴더), OS 파일 연동으로 앱 실행
- **탐색**: 이전/다음 이동, 썸네일/슬라이더 이동, 루프 내비게이션
- **보기**: 확대/축소, 화면 맞춤(가로/세로/전체), 회전, 좌우/상하 반전
- **멀티 페이지 모드**: `single`, `left-to-right`, `right-to-left`, `webtoon`
- **부가 기능**: EXIF 패널, 최근 파일 목록, 슬라이드쇼, 전체화면, 이미지 클립보드 복사(PNG 변환)

## 기술 스택

- **Frontend**: React 18, TypeScript, Vite 6, Tailwind CSS 4, TanStack Router v1, Zustand 5
- **Backend**: Rust, Tauri 2
- **Tauri Plugins**: `dialog`, `fs`, `opener`, `store`, `window-state`

## 요구 사항

- **Node.js**: `>= 22`
- **Rust**: stable (`1.94` 이상 권장)

### Linux 시스템 라이브러리 (Ubuntu/Debian)

```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libheif-dev
```

macOS:

```bash
brew install libheif
```

Windows: install [vcpkg](https://vcpkg.io/), then:

```powershell
vcpkg install "libheif[core]:x64-windows"
$env:VCPKG_ROOT = "<vcpkg root>"
$env:Path += ";$env:VCPKG_ROOT\installed\x64-windows\bin"
```

`[core]`는 HEVC 디코더 `libde265`만 넣고, 인코더 `x265`는 빼는 설치입니다. 앱은 HEIC를 인코드하지 않습니다.

앱은 `libheif`를 동적 링크합니다. `embedded-libheif` Cargo 기능은 켜지 마세요. Windows 설치본은 vcpkg DLL이 실행 파일 옆에 있어야 합니다. `VCPKG_ROOT`가 있으면 `src-tauri/build.rs`가 `heif.dll`과 `libde265.dll`을 복사합니다.

## 시작하기

```bash
git clone https://github.com/ara-hwang/tauri-image-viewer.git
cd tauri-image-viewer
npm install
npm run tauri dev
```

`npm run tauri dev` 실행 시 Vite(`http://localhost:1420`)와 Tauri 앱이 함께 실행됩니다.

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

- **파일/탐색**: `Ctrl/Cmd+O`(열기), `Left`/`Right`(이전/다음)
- **줌**: `+`, `=`, `-`, `0`(초기화), `1`(가로 맞춤), `2`(세로 맞춤), `3`(화면 맞춤)
- **이미지 조작**: `R`(시계 회전), `Shift+R`(반시계 회전), `H`(좌우 반전), `V`(상하 반전)
- **기타**: `I`(EXIF), `Space` 또는 `F5`(슬라이드쇼), `F11`(전체화면), `Ctrl/Cmd+C`(이미지 복사), `D`(테마 전환)

## 설정 항목

- **Navigation**: 끝에서 정지 또는 루프 이동
- **Cache Mode**
  - `off`: 현재 이미지만 유지
  - `nearby`: 인접 이미지 중심 프리패치
  - `extended`: 더 넓은 범위 프리패치
  - `memory-1gb` / `memory-2gb`: 메모리 상한 기반 캐시
- **View Mode**: `single`, `left-to-right`, `right-to-left`, `webtoon`
- **Slideshow Interval**: 슬라이드쇼 간격(ms)

설정과 최근 파일 목록은 Tauri Store(`settings.json`)에 저장됩니다.

## 동작 구조 요약

- 프론트엔드는 Tauri IPC로 백엔드 명령을 호출합니다.
  - `load_image(file_path)`
  - `get_directory_images(file_path)`
  - `resolve_dropped_path(path)`
  - `get_exif_data(file_path)`
  - `get_archive_images(file_path)`
  - `load_archive_image(archive_path, entry_name)`
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
  src/image.rs       # MIME/확장자 판별, load_viewable
  src/heif.rs        # HEIC/HEIF 디코드 및 JPEG sidecar
  src/process_temp.rs # 프로세스 수명 임시 디렉터리
  src/archive.rs     # CBZ 목록/추출 처리
  src/lib.rs         # Tauri 앱 설정 및 command 등록
```

## 참고

- 앱 창은 프레임리스(`decorations: false`)로 동작합니다.
- 파일 연결은 `src-tauri/tauri.conf.json`의 `bundle.fileAssociations`에서 관리합니다.
