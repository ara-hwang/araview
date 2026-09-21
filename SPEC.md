# SPEC.md - AraView 기능/기술 명세

> 한국어 스펙 문서. 구현 진실(source of truth)은 코드이며, 본 문서는 현재 코드베이스의 동작을 요약한다.
> 관련 문서: `PRODUCT.md`(제품 정의), `DESIGN.md`(비주얼 시스템), `ROADMAP.md`(단계 계획), `README.md`(소개/문서 허브), `docs/usage.md`(사용법), `docs/development.md`(개발 안내), `docs/releasing.md`(릴리스/업데이트), `AGENTS.md`(AI 작업 지침).

## 0. 문서 규약

- 경로 별칭 `@/...`는 `src/` 기준이다.
- 백엔드 경로는 `src-tauri/src/` 기준이다.
- `invoke()` 최상위 인자는 Tauri 기본 변환을 따른다. 프론트는 camelCase(`filePath`, `archivePath`, `entryName`)로 보내고 Rust 파라미터는 snake_case(`file_path`, `archive_path`, `entry_name`)로 받는다. 중첩 옵션 구조체(`DirListOptions`, `SaveImageOptions`)만 `serde(rename_all = "camelCase")`이다. 응답 구조체(`ImageInfo`, `DirectoryImages`, `ThumbnailInfo`, `ImageDetails`, `Histogram`, `FileAssociation`)는 rename 없이 snake_case로 직렬화되며 TypeScript 타입과 1:1 대응한다.
- 에러는 항상 구조화 에러 `{ code, message }`를 우선하고, 문자열 에러는 레거시 매칭으로 분류한다.
- base64 이미지 페이로드는 사용하지 않는다. 렌더링은 항상 파일 경로 기반이다.

## 1. 개요

- Windows 10/11 x64 전용 오프라인 데스크톱 이미지/코믹 뷰어.
- 프론트: React 19, TypeScript, Vite 6, Tailwind CSS 4, TanStack Router v1, Zustand 5, i18next.
- 백엔드: Rust + Tauri 2. 플러그인: `store`, `window-state`, `opener`, `dialog`, `fs`, `single-instance`, `updater`, `process` (디버그 한정 `mcp-bridge`).
- 라이브러리 가져오기, 계정, 네트워크 없이 로컬 파일만 다룬다.
- 창은 프레임리스(`decorations: false`)이며 커스텀 타이틀바/툴바(`src/components/Header.tsx`)를 쓴다.

## 2. 지원 포맷

총 21개 확장자. 프론트 진실은 `src/constants/imageExtensions.ts`, 백엔드 진실은 `src-tauri/src/image.rs` (`SUPPORTED_EXTENSIONS`, `get_mime_type`).

### 2.1 순수 이미지 14종

| 확장자        | MIME                        | 비고                                                                                                                    |
| ------------- | --------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `png`         | `image/png`                 | 네이티브 렌더                                                                                                           |
| `jpg`, `jpeg` | `image/jpeg`                | 네이티브 렌더                                                                                                           |
| `gif`         | `image/gif`                 | 네이티브 렌더                                                                                                           |
| `bmp`         | `image/bmp`                 | 네이티브 렌더                                                                                                           |
| `webp`        | `image/webp`                | 네이티브 렌더                                                                                                           |
| `svg`         | `image/svg+xml`             | 네이티브 렌더. 치수는 `<svg>` 헤더(width/height/viewBox, 절대 단위) 파싱으로 복원하며 해석 불가분만 `width/height` 생략 |
| `ico`         | `image/x-icon`              | 네이티브 렌더                                                                                                           |
| `tiff`, `tif` | `image/tiff`                | 네이티브 렌더                                                                                                           |
| `avif`        | `image/avif`                | 네이티브 렌더                                                                                                           |
| `heic`        | `image/heic`                | JPEG sidecar 트랜스코드 후 렌더                                                                                         |
| `heif`        | `image/heif`                | JPEG sidecar 트랜스코드 후 렌더                                                                                         |
| `psd`         | `image/vnd.adobe.photoshop` | JPEG sidecar 트랜스코드 후 렌더(읽기 전용, 편집 저장 미지원)                                                            |

### 2.2 아카이브 7종

| 확장자 | MIME                            | 비고                                  |
| ------ | ------------------------------- | ------------------------------------- |
| `cbz`  | `application/vnd.comicbook+zip` | ZIP 기반 코믹                         |
| `zip`  | `application/zip`               | 일반 ZIP도 이미지 목록으로 열 수 있음 |
| `cb7`  | `application/x-7z-compressed`   | 7z 기반 코믹                          |
| `7z`   | `application/x-7z-compressed`   | 일반 7z도 지원                        |
| `cbr`  | `application/vnd.comicbook-rar` | RAR 기반 코믹                         |
| `rar`  | `application/x-rar-compressed`  | 일반 RAR도 지원                       |
| `cbt`  | `application/x-tar`             | TAR 기반 코믹                         |

백엔드 판별:

- `is_image_file`: MIME이 `application/`으로 시작하지 않는 지원 파일.
- `is_archive_file`: 위 7종 MIME 해당.
- `is_supported_file`: 둘 중 하나.

### 2.3 제외 포맷

- QOI, JXL, RAW(CR2/NEF/ARW 등), PSB는 제외 유지. 이유는 WebView2 네이티브 렌더 불가로 JPEG sidecar 전제가 필요하기 때문이다(`ROADMAP.md` 4.4). PSD는 `psd` 크레이트(순수 Rust) 합성 디코드 + JPEG sidecar로 읽기 전용 지원한다. PSB(`8BPB`)는 디코더가 없어 진입 차단한다.

## 3. 화면과 라우트

라우트 진실: `src/routes/__root.tsx`, `src/routes/index.tsx`, `src/routes/image.tsx`.

### 3.1 홈 `/`

- 빈 상태: 실제 다음 행동을 안내한다(파일 열기). 가짜 샘플을 만들지 않는다.
- `파일 열기` 버튼은 파일 피커를 연다.
- 최근 파일 섹션: `recordRecentFiles`가 true이고 목록이 있을 때만 표시. 개수 + 전체 삭제. 커스텀 `RecentFileAttachment` 가로형 목록으로 파일명, 형식/크기/치수, 상위 폴더를 표시한다.
- 상세 정보는 `useRecentFileDetails(visibleRecent)`로 `load_image` 메타와 썸네일 src를 확보한다. 아카이브는 아이콘으로, 실패 항목은 error 상태로 표시한다.
- 드래그 오버레이: 드래그 중 점선 테두리 + `home.drop` 문구.
- 시작 옵션: `autoOpenLastFile && recordRecentFiles`이고 최근 파일이 있으면 첫 항목을 1회 자동 로드한다.
- 이미지가 로드되면 `/image`로 이동한다. 창 제목은 이미지 파일명, 없으면 `app.title`이다.

### 3.2 뷰어 `/image`

- `beforeLoad`에서 `imageInfo`가 없으면 `/`로 리다이렉트한다.
- 구성: `ImageContainer`(읽기 영역) + 이미지 목록 도크(`ImageNavBar`, 상/하/좌/우) + 드래그 오버레이 + `ThumbnailGrid`(선택) + `RenameDialog` + `SaveEditsDialog`.
- 도크: 뷰어 가장자리 플로우에 붙으며 읽기 영역을 밀어낸다. 위치(`dockPosition`, 기본 `bottom`), 표시(`dockVisible`, 기본 펼침), 썸네일 크기(`dockThumbSize` S/M/L), 파일명(`dockShowName`)/번호(`dockShowIndex`)를 보기 설정에서 바꾼다. 도크의 `⋯` 버튼(`DropdownMenu`, 위치/크기/표시)에서도 같은 값을 즉시 바꿀 수 있다. 바 전체를 접으면 얇은 엣지 바로 복구한다. 수직 도크(좌/우)에서는 슬라이더를 생략하고 썸네일+버튼만 세로로 둔다. 이미지가 2장 이상일 때만 표시한다.
- `G`(기본, 재할당 가능)로 썸네일 그리드 오버레이를 연다. 열 때 현재 이미지를 중앙에 둔다.
- 그리드: 뷰포트 기반 가상화(고정 셀, overscan 2행), 클릭/`Enter`로 점프 후 닫기, `Esc`/`G`로 닫기, 파일명 필터, 실패 셀 배지와 재시도. 그리드가 열려 있는 동안 뷰어 단축키는 비활성이다.
- 우클릭은 설정(`mouse.rightClick`)에 따라 컨텍스트 메뉴 또는 다른 동작이다. 홈에서는 우클릭을 막는다.
- `Esc` 닫기: 이름 변경/저장 다이얼로그가 열려 있거나 입력 포커스 중이면 닫지 않는다.
- `autoHideUI`가 true일 때만 `useIdleHide`로 크롬(상단바, 이미지 목록 도크, 상태바)을 숨긴다. 도크는 숨김 중 언마운트되어 읽기 영역이 확장된다.
- `menuBarHidden`이 true이면 상단바를 숨기고, 상단 20px 호버 영역에서 peek 오버레이로 표시한다. 헤더 숨기기 버튼과 보기 설정 스위치로 토글한다.
- 로드 실패 시 에러 카드에 재시도/홈 복구 경로를 제공한다. 실패한 적은 토스트로 알린다.

### 3.3 상태 규칙

- 모든 데이터 뷰는 empty, loading, error 상태를 가진다.
- 모든 컨트롤은 키보드로 도달/조작 가능하며 포커스 표시가 보인다.
- UI 카피에 em dash(`—`)를 쓰지 않는다. 쉼표, 마침표, 콜론, 괄호를 쓴다.

## 4. 파일 열기 흐름

진실: `src/hooks/useImageLoader.ts`, `src/hooks/useOpenFileListener.ts`, `src-tauri/src/lib.rs`.

### 4.1 파일 피커

- `open({ multiple: false, filters: [{ name: picker.images, extensions: SUPPORTED_IMAGE_EXTENSIONS }] })`.
- 선택된 1개 경로를 `loadImage`로 연다.

### 4.2 드래그 앤 드롭

- 파일/폴더를 여러 개 드롭할 수 있다.
- 아카이브 경로는 그대로 유지한다.
- 그 외 경로는 `resolve_dropped_path`로 해석한다:
  - 파일이면 그대로 반환.
  - 폴더면 내부 지원 파일 중 이름순(소문자 기준) 첫 이미지를 반환.
  - 이미지 없음/경로 없음/지원 불가면 에러.
- 해석 실패 항목은 건너뛴다. 전부 실패하면 `toast.drop.fail`.
- 성공 목록은 소문자 이름순 정렬 후 첫 항목을 열고, 2개 이상이면 `toast.drop.firstOf`로 알린다.
- 드래그 깊이 카운터로 오버레이 깜빡임을 방지한다.

### 4.3 OS 파일 연결 실행

- Windows가 CLI 인자(`args[1]`)로 파일 경로를 넘긴다.
- 백엔드는 경로를 `PendingOpenFile` 상태에 보관하고, 프론트 루트가 리스너 등록을 마친 뒤 호출하는 `frontend_ready`에서 emit한다. 고정 지연 emit은 프론트 로드 전에 유실될 수 있어 쓰지 않는다.
- 두 번째 실행은 `single-instance`가 기존 창에 전달한다. 프론트가 준비 전이면 같은 상태에 보관됐다가 flush되고, 준비 후면 즉시 emit한다.
- 프론트 루트(`__root.tsx`)의 `useOpenFileBridge`가 이벤트를 받아 현재 라우트가 등록한 로더로 전달한다. 라우트 전환 중이라 로더가 없으면 보류했다가 다음 등록 시 전달한다.
- 홈은 `useOpenFileListener(loadImage)`, 이미지 뷰어는 `useOpenFileListener(loadImageAndReset)`로 받는다.

## 5. 디렉토리 목록과 탐색

진실: `src-tauri/src/commands.rs`, `src-tauri/src/dir_cache.rs`, `src/hooks/useDirectoryNavigation.ts`, `src/utils/directoryOptions.ts`.

### 5.1 `get_directory_images`

- 입력: `file_path`, `options?: DirListOptions`.
- `DirListOptions` (camelCase):
  - `sortKey: name | date | size` (기본 `name`)
  - `descending: boolean` (기본 false)
  - `recursive: boolean` (기본 false, 설정 `includeSubfolders`와 대응)
- 부모 폴더 기준으로 정렬 목록을 만들고, `current_index`는 요청 경로의 위치(없으면 0)이다.
- 재귀가 켜지면 하위 폴더 이미지를 포함한다.
- 디렉토리 캐시는 폴더 mtime 지문(일반 모드)과 워처 기반 무효화(재귀 모드)를 쓰며 상한 128개이다.
- 응답 `availability`는 `images`와 같은 순서의 `local` | `cloud_only` | `unknown` 배열이다. Windows Files On-Demand placeholder는 메타데이터만으로 `cloud_only`로 표시한다(썸네일 스트립/그리드는 미리 hydration하지 않음). 아카이브 목록은 빈 배열이다.

### 5.2 목록 새로고침

- 설정 변경 후 `refreshDirectoryListing()`으로 다시 읽고 이전 위치를 복원한다.
- 복원 규칙(`resolveRefreshedIndex`): 이전 경로가 있으면 그 위치, 없으면 범위 내 clamp.
- HEIC/PSD sidecar처럼 `imageInfo.file_path`가 원본과 다를 수 있어 `dirImages` 원본 경로를 우선한다.
- 아카이브 모드에서는 폴더 새로고침을 하지 않는다.

### 5.3 이전/다음

- `viewMode`가 `left-to-right`/`right-to-left`이면 2장씩, 나머지는 1장씩 이동한다.
- `loopNavigation=false`: 끝에서 멈춘다. 단, 마지막 장은 clamp로 볼 수 있게 한다.
- `loopNavigation=true`: wrap한다.
- 아카이브 모드면 `loadArchiveImageByIndex` + 인덱스 갱신, 일반 모드면 `loadImage(..., refreshDirectory: false)` + 인덱스 갱신이다.
- 목록이 1개 이하면 이전/다음을 수행하지 않는다.

### 5.4 점프

- `PageUp/PageDown`: 10장 점프(`navigateByOffset(±10)`).
- `Home/End`: 처음/마지막(`navigateToIndex`).
- 루프 설정에 따라 wrap/clamp한다.
- 웹툰 모드에서는 같은 동작이 연속 스크롤 이동(`scrollWebtoonTo`)으로 바뀐다.

### 5.5 손상 파일 건너뛰기

- `skipBrokenFiles`가 true면 로드 실패 시 `failedPaths`에 기록하고 다음 후보(`findSkipTarget`, 최대 `MAX_SKIP_ATTEMPTS`)로 자동 이동한다.
- 아카이브/일반 모두 동일 규칙이며, 건너뛰면 `toast.load.skipped`를 표시한다.

## 6. 뷰 모드

진실: `src/store/settingsStore.ts`, `src/hooks/useMultiPageImages.ts`, `src/components/WebtoonContinuousView.tsx`, `src/routes/image.tsx`.

값: `single | left-to-right | right-to-left | webtoon`. 기본 `single`.

| 모드            | 렌더                                | 넘김 단위   | 프리페치 성향 |
| --------------- | ----------------------------------- | ----------- | ------------- |
| `single`        | 현재 1장                            | 1장         | 기본 거리     |
| `left-to-right` | 현재 + 다음, 좌에서 우              | 2장         | 기본 + 1      |
| `right-to-left` | 현재 + 다음, 우에서 좌              | 2장         | 기본 + 1      |
| `webtoon`       | 전 구간 연속 수직 스크롤, 지연 로드 | 스크롤 이동 | 기본 x 2      |

- 양면 모드 페이지는 `[current, next]`이며 루프가 켜지면 wrap한다. 로드 실패 페이지는 제외한다.
- 웹툰 모드에서 `ArrowLeft/ArrowRight`는 이전/다음 이미지 스크롤 이동이다.
- 웹툰 모드에서 `ArrowUp/ArrowDown`은 연속 스크롤 컨테이너(`aria-label="webtoon-scroll"`)를 240px씩 스크롤한다.
- 웹툰 중앙 이미지 변경은 전체 reload 없이 인덱스 동기화 + `imageInfo` 교체 + 아카이브 진행 저장 + 앞 5장 메타 예열로 처리한다.

## 7. 뷰어 조작

진실: `src/store/appStore.ts`, `src/utils/zoomPanUtils.ts`, `src/hooks/useZoomPan.ts`, `src/hooks/useWheelNavigation.ts`.

### 7.1 줌

- `zoomIn`: `min(zoom * 1.25, max)`. 상한은 SVG 40, 그 외 10.
- `zoomOut`: 동적 최소값까지 `zoom / 1.25`.
- `0`: 자동 맞춤으로 리셋(`resetZoomPan`, `fitMode: auto` 저장 + `getZoomForFitMode("auto")` + 위치 0 + 회전/반전 초기화).
- `1/2/3`: 가로 맞춤 / 세로 맞춤 / 화면 맞춤(`setZoomToFit`, `fitMode` 저장).
- `fitMode`: `width | height | screen | auto`. 기본 `auto`. `1/2/3`과 헤더 버튼으로 바꾸면 `settings.json`에 저장되고, 이미지 전환(`setImageInfoAndResetView`, `applyImageNaturalSize`, `applyRememberedFit`)마다 다시 적용된다. single 모드에서 핏 잠금(`isFitLocked`)이 유지 중이면 윈도우 리사이즈(컨테이너 변화) 시에도 다시 적용되며, 수동 줌(`zoomInBy`/`zoomOutBy`)은 잠금을 풀어 리사이즈해도 줌을 유지한다. `auto`는 큰 이미지만 맞추고 작은 이미지는 100%로 두며, `width/height/screen`은 작은 이미지도 확대한다. 헤더 맞춤 버튼은 현재 `fitMode`를 `aria-pressed`와 하이라이트로 표시한다.
- 맞춤 계산은 회전된 치수(90/270도면 가로세로 교환) 기준이다.

### 7.2 팬

- 키보드 팬 1회 48px(`PAN_STEP_PX`), 항상 동작하며 컨테이너 경계로 clamp한다.
- 마우스 왼쪽 드래그는 기본 팬이다(`mouse.leftDrag`, `pan` 또는 `none`만 허용).
- rAF 기반 팬 스로틀과 `translate3d` + `will-change`를 쓴다(Single 기준).

### 7.3 회전/반전

- `rotation`: `0 | 90 | 180 | 270`.
- `R`: 시계 90도, `Shift+R`: 반시계 90도.
- `H`: 좌우 반전, `V`: 상하 반전.
- 이미지 변경 시 기억된 맞춤 모드로 초기화된다(`setImageInfoAndResetView`, `applyRememberedFit`).
- 저장 시 순서는 회전 먼저, 반전 나중이며 화면 CSS 합성과 일치한다.

### 7.4 배경/표시

- `viewerBackground`: `theme | black | white | checker`. 기본 `theme`.
- `B`: `theme → black → white → checker` 순환.
- `T`: 항상 위 토글(`useAlwaysOnTop`).
- `F11`: 전체화면 토글. 더블클릭 기본도 전체화면이다.
- `autoHideUI`: true일 때 읽기 중 크롬 자동 숨김.

## 8. 아카이브

진실: `src-tauri/src/archive.rs`, `src-tauri/src/commands.rs`, `src/hooks/useImageLoader.ts`, `src/store/archiveProgressStore.ts`.

- `get_archive_images(filePath → file_path)`: 내부 이미지 엔트리 목록 + `current_index: 0`. 비어 있으면 `not_found`.
- `load_archive_image(archivePath → archive_path, entryName → entry_name)`: `process_temp/archive-<hash>/`(canonical 경로+mtime+크기 해시)에 추출 후 `load_viewable`로 반환한다.
- `archive_prefetch(archivePath → archive_path, entryNames → entry_names)`: 이웃 선추출. 항상 `Ok`를 돌려주는 fire-and-forget용이다.
- 아카이브 선추출 거리는 `min(max(base + bonus, 1), 2)`이며, `bonus`는 웹툰=base, 양면=1, single=0이다.
- 이어보기: `archiveProgressStore`가 `archivePath → entryName`을 최대 100개 LRU로 저장한다. 목록에 저장된 항목이 있으면 거기서 시작한다.
- `appStore.archivePath`가 null이 아니면 아카이브 모드이다.
- 아카이브 모드 제한: 휴지통 이동, 이름 변경, 편집 저장은 안내 토스트와 함께 차단된다.
- 탐색/썸네일은 엔트리 목록 기준으로 동일하게 동작한다.

## 9. 캐시/썸네일/프리페치

진실: `src/utils/cacheConfig.ts`, `src/hooks/useImageCache.ts`, `src-tauri/src/thumbnail.rs`.

### 9.1 캐시 모드

기본 `nearby`.

| 모드         | 항목 상한 | 바이트 상한 | 프리페치 거리 |
| ------------ | --------- | ----------- | ------------- |
| `off`        | 1         | 무제한      | 0             |
| `nearby`     | 24        | 무제한      | 1             |
| `extended`   | 64        | 무제한      | 3             |
| `memory-1gb` | 매우 큼   | 1GB         | 2             |
| `memory-2gb` | 매우 큼   | 2GB         | 3             |

- 일반 파일 프리페치 거리 보정: 웹툰은 기본 x 2, 양면은 기본 + 1, single은 기본.
- 캐시 키는 파일 경로 기준이며, 픽셀 데이터를 메모리에 오래 두지 않고 브라우저 이미지 캐시에 위임한다.

### 9.2 썸네일

- `generate_thumbnail(file_path, max_side?)`: 기본 256, 허용 32~1024.
- `process_temp/thumbs/`에 JPEG로 원자적 저장 후 재사용한다. 상한 500MB를 넘기면 오래된 것부터 제거한다.
- HEIC/HEIF/PSD는 썸네일용 JPEG sidecar 경로를 쓴다.
- `image` 크레이트가 디코드 불가한 입력(SVG 등)이나 아카이브 엔트리명은 에러를 내고, 프론트는 원본으로 폴백한다.
- `generate_thumbnails_batch(file_paths, max_side?)`: 항목별 성공/실패를 함께 반환하는 배치 API이다.
- `generate_archive_thumbnail(archive_path, entry_name, max_side?)`: 아카이브 엔트리를 추출해 축소 JPEG를 만든다. 추출물(존재 시 재사용)과 썸네일 캐시를 함께 재사용하므로 그리드에서 풀사이즈 로드를 피한다.
- 그리드 썸네일은 256px로 요청하고, 보이는 창의 경로만 요청한다. 아카이브는 동시 4개로 추출을 제한한다.

## 10. EXIF/파일 정보

진실: `src-tauri/src/commands.rs`, `src-tauri/src/image_info.rs`, `src/hooks/useExifLoader.ts`, `src/components/ExifPanel.tsx`, `src/components/HistogramChart.tsx`.

- `get_exif_data(file_path)`는 `HashMap<String, String>`을 반환한다.
- 파일 없음이면 `not_found`, EXIF 없으면 `unsupported("No EXIF data found: ...")`.
- `I`로 패널 토글. EXIF 패널 내부 포커스에서는 `I` 닫기를 허용한다.
- 표시 범주는 Camera, Exposure, Image, Lens, DateTime, GPS, Software 계열이다.
- HEIC는 원본 경로 기준 EXIF를 읽으므로 비어 있는 경우가 많다.

### 10.1 히스토그램 `get_image_histogram`

- 입력 `file_path`, 반환 `Histogram`(`r/g/b` 256빈 + `sampled_pixels`).
- HEIC/HEIF/PSD는 전용 디코더, 그 외는 `image` 크레이트로 디코드 후 최대 변 256px로 다운샘플해 집계한다.
- 디코드 불가 포맷(SVG, AVIF 등)은 `unsupported`/`corrupt` 에러를 내고, 프론트는 차트 대신 안내 문구를 표시한다(`histogram.unavailable`, SVG는 벡터 안내 `histogram.vectorUnavailable`).
- 렌더는 신규 의존성 없이 SVG 영역 차트(`HistogramChart`, 채널별 3경로 + 범례, `role="img"` + 제목)이다.

### 10.2 파일 상세 `get_image_details`

- 입력 `file_path`, 반환 `ImageDetails`(camelCase): `filePath`, `fileSize`, `width`/`height`(렌더 바이트 기준, 미지원분 null), `colorMode`(`rgb|rgba|grayscale|grayscale-alpha|unknown`, 디코드 표현 기준), `bitsPerChannel`, `createdUnix`/`modifiedUnix`(유닉스 초, FS 미지원 시 null), `dpiX`/`dpiY`(EXIF X/YResolution + ResolutionUnit 환산, 없으면 null), `iccStatus`(`present|absent|unchecked`) + `iccName`/`iccBytes`.
- ICC는 JPEG APP2(`ICC_PROFILE`)와 PNG iCCP 청크만 스캔한다. 그 외 포맷은 `unchecked`이다.
- EXIF가 없어도 명령은 성공한다. 디코드 실패 파일도 상세를 반환한다(색상 `unknown`, 치수 null).
- 패널 표시: 경로(아카이브 모드면 `아카이브경로 › 엔트리명`), 크기, 치수 + 픽셀 수, 생성/수정 시각(아카이브 모드 숨김), 색상 + 비트/채널, DPI, 색상 프로파일.
- 로딩: `useExifLoader.loadExif`가 EXIF와 병렬로 조회한다. 셋 다 실패해도 패널은 열리고 섹션별 안내 문구를 표시한다.

## 11. 파일 작업

진실: `src/hooks/useFileOperations.ts`, `src-tauri/src/commands.rs`, `src-tauri/src/save.rs`, `src/hooks/useCopyImage.ts`.

공통: 아카이브 모드면 원본 아카이브 경로를 대상으로 삼는다(`getEffectivePath`). 단, 휴지통/이름 변경/편집 저장은 아카이브에서 차단된다.

### 11.1 휴지통 이동 `Delete`

- `trash_file(file_path)`: OS 휴지통으로 이동(영구 삭제 아님). 디렉토리 불가.
- 확인 다이얼로그(warning) 후 실행한다.
- 성공 시 최근 파일에서 제거하고, 남은 목록에서 `min(currentIndex, remaining-1)`을 연다. 남은 게 없으면 홈으로 돌아간다.

### 11.2 이름 변경 `F2`

- `rename_file(old_path, new_name)` 후 새 `ImageInfo`를 반환한다.
- 검증(`validate_new_file_name`):
  - trim 후 비어 있으면 거부.
  - `/`, `\` 포함 거부.
  - `< > : " | ? *` 포함 거부.
  - 끝이 공백/마침표면 거부.
- 이동 전 지원 확장자 검사, 중복 이름이면 `already_exists`.
- 대소문자만 바꾸는 동일 파일은 이동을 생략한다.
- 성공 시 디렉토리 목록과 최근 파일 경로를 교체한다.

### 11.3 편집 저장 `Ctrl+S`

- PSD/SVG/AVIF는 저장 불가라 저장 진입(단축키/팔레트/컨텍스트 메뉴)에서 다이얼로그를 열지 않고 `toast.save.noPsd`/`noSvg`/`noAvif`로 안내한다. 백엔드도 `unsupported`로 2중 차단한다. 그 외 지원 포맷(png/jpg/jpeg/webp/gif/bmp/tiff/tif/ico/heic/heif)은 저장 가능하다.
- `save_image_edits(filePath → file_path, options)`:
  - `rotationCw: 0 | 90 | 180 | 270`
  - `flipH`, `flipV`
  - `format: "png" | "jpg" | "jpeg" | "webp"` 또는 null(원본 유지)
  - `overwrite`, `newFileName`
- 출력 포맷 결정: 요청값 > 원본 유지 가능값(png/jpg/webp) > HEIC/HEIF는 JPG > 그 외는 PNG.
- 지원 출력은 png/jpg/webp만. 그 외는 `invalid_input`.
- `overwrite=true`는 포맷이 바뀌지 않을 때만 같은 경로에 쓴다. 그 외는 `{stem}-edited.{ext}`이며 중복 시 `-2`, `-3`을 붙인다. 확장자는 선택 포맷으로 강제한다.
- 디코딩: HEIC/HEIF는 전용 디코더, 그 외는 `image::open`. 투명은 흰 배경에 합성한다.
- 회전 범위 오류, SVG 등 디코드 불가 입력은 원본을 건드리지 않고 실패한다.
- 변경 없음(회전 0 + 반전 없음 + 포맷 유지)은 저장하지 않고 안내한다.
- 덮어쓰기는 확인 다이얼로그 후 실행한다.
- 성공 시 최근 파일에 추가하고 새 경로로 다시 로드한다(회전 상태 초기화 포함).

### 11.4 클립보드 복사 `Ctrl+C`

- Asset URL fetch → Blob → `createImageBitmap` → Canvas → PNG Blob → `ClipboardItem("image/png")`.
- 이미지가 없거나 캔버스/PNG 변환 실패 시 에러 토스트이다.

### 11.5 경로/외부 열기

- `Ctrl+Shift+C`: 유효 경로를 텍스트로 복사한다.
- `Ctrl+Shift+E`: `revealItemInDir`로 탐색기에 표시한다.
- `Ctrl+Shift+O`: `openPath`로 기본 앱으로 연다.

## 12. 최근 파일/영속화

진실: `src/store/recentFilesStore.ts`, `src/store/archiveProgressStore.ts`, `src/store/settingsStore.ts`.

`settings.json`(Tauri Store) 키:

| 키                | 내용                            | 상한     |
| ----------------- | ------------------------------- | -------- |
| `settings`        | `SettingsState` 전체            | 1개 객체 |
| `recentFiles`     | 최근 경로 배열(최신 먼저)       | 20       |
| `archiveProgress` | 아카이브 경로 → 이어보기 엔트리 | 100      |

- 최근 파일은 중복 제거 후 맨 앞에 넣고 자른다. `recordRecentFiles=false`면 목록을 숨기고 자동 열기도 막는다.
- 설정 저장은 손상값도 `sanitizeSettings`로 복원한다. 언어는 저장값이 없으면 시스템 언어를 쓴다.
- 저장 실패는 토스트로 알리되 UI 동작을 막지 않는다.

## 13. 설정

진실: `src/store/settingsStore.ts`.

| 설정                | 값                                                      | 기본값                             |
| ------------------- | ------------------------------------------------------- | ---------------------------------- |
| `language`          | `ko \| en`                                              | `ko`(초기 로드는 시스템 감지 우선) |
| `loopNavigation`    | 끝에서 루프 여부                                        | `false`                            |
| `cacheMode`         | `off \| nearby \| extended \| memory-1gb \| memory-2gb` | `nearby`                           |
| `viewMode`          | `single \| left-to-right \| right-to-left \| webtoon`   | `single`                           |
| `autoOpenLastFile`  | 시작 시 마지막 파일 자동 열기                           | `false`                            |
| `recordRecentFiles` | 최근 기록 유지                                          | `true`                             |
| `viewerBackground`  | `theme \| black \| white \| checker`                    | `theme`                            |
| `autoHideUI`        | 읽기 중 크롬 자동 숨김                                  | `false`                            |
| `menuBarHidden`     | 상단바 수동 숨김 (상단 호버 시 peek 오버레이로 표시)    | `false`                            |
| `alwaysOnTop`       | 항상 위                                                 | `false`                            |
| `sortKey`           | `name \| date \| size`                                  | `name`                             |
| `sortDescending`    | 내림차순                                                | `false`                            |
| `includeSubfolders` | 하위 폴더 포함(재귀)                                    | `false`                            |
| `skipBrokenFiles`   | 손상 파일 자동 건너뛰기                                 | `false`                            |
| `fitMode`           | 맞춤 기억 `width \| height \| screen \| auto`           | `auto`                             |
| `dockPosition`      | 이미지 목록 위치 `top \| bottom \| left \| right`       | `bottom`                           |
| `dockVisible`       | 이미지 목록 표시                                        | `true`                             |
| `dockThumbSize`     | 썸네일 크기 `s \| m \| l`                               | `s`                                |
| `dockShowName`      | 썸네일 파일명 표시                                      | `false`                            |
| `dockShowIndex`     | 썸네일 번호 표시                                        | `false`                            |
| `shortcuts`         | 단축키 맵                                               | 아래 기본표                        |
| `wheel`             | 휠 맵                                                   | 아래 기본표                        |
| `mouse`             | 마우스 맵                                               | 아래 기본표                        |

설정 항목에 연결된 단축키가 있으면 항목 옆에 현재 할당된 단축키를 배지로 표시한다: 배경 `cycleBackground`, 창 `toggleAlwaysOnTop`. 재할당하거나 해제하면 배지도 즉시 따라간다.

## 14. 단축키/휠/마우스/명령 팔레트

진실: `src/constants/shortcuts.ts`, `src/constants/commands.ts`, `src/hooks/useImageViewerHotkeys.ts`.

### 14.1 기본 단축키

| 동작             | 기본값            |
| ---------------- | ----------------- |
| 이전             | `Ctrl+ArrowLeft`  |
| 다음             | `Ctrl+ArrowRight` |
| 왼쪽 팬          | `ArrowLeft`       |
| 오른쪽 팬        | `ArrowRight`      |
| 위 팬            | `ArrowUp`         |
| 아래 팬          | `ArrowDown`       |
| 확대             | `=`               |
| 축소             | `-`               |
| 보기 초기화      | `0`               |
| 가로 맞춤        | `1`               |
| 세로 맞춤        | `2`               |
| 화면 맞춤        | `3`               |
| 파일 열기        | `Ctrl+O`          |
| 이미지 닫기      | `Escape`          |
| EXIF             | `I`               |
| 시계 회전        | `R`               |
| 반시계 회전      | `Shift+R`         |
| 좌우 반전        | `H`               |
| 상하 반전        | `V`               |
| 전체화면         | `F11`             |
| 항상 위          | `T`               |
| 이미지 복사      | `Ctrl+C`          |
| 휴지통           | `Delete`          |
| 탐색기에 표시    | `Ctrl+Shift+E`    |
| 기본 앱으로 열기 | `Ctrl+Shift+O`    |
| 배경 순환        | `B`               |
| 이름 변경        | `F2`              |
| 경로 복사        | `Ctrl+Shift+C`    |
| 편집 저장        | `Ctrl+S`          |
| 명령 팔레트      | `Ctrl+K`          |
| 10장 이전        | `PageUp`          |
| 10장 다음        | `PageDown`        |
| 처음             | `Home`            |
| 마지막           | `End`             |
| 썸네일 그리드    | `G`               |

그리드 내부: 화살표(선택 이동), `Home`/`End`, `PageUp`/`PageDown`, `Enter`(점프), `Esc`/`G`(닫기). 그리드가 열려 있는 동안 다른 뷰어 단축키는 동작하지 않는다.

### 14.2 휠/마우스 기본값

- 휠 위: 이전, 휠 아래: 다음.
- `Ctrl+휠`: 확대/축소. `Shift/Alt+휠`: 없음.
- 왼쪽 드래그: 팬. 가운데: 없음. 더블클릭: 전체화면. 우클릭: 컨텍스트 메뉴.

### 14.3 커스텀 규칙

- `Ctrl/Shift/Alt`만 허용하며 `Meta`는 거부한다.
- `Tab`은 예약어로 금지한다.
- 영문자는 대문자로 정규화한다. 문자 아닌 키의 `Shift`는 제거한다.
- 빈 문자열은 바인딩 해제로 취급한다.
- 중복 바인딩은 `findBindingConflict`로 검출한다.
- 구버전 저장값에 팬 4종이 없으면 탐색/팬 6종은 새 기본값으로 이전한다.
- 왼쪽 드래그는 `pan` 또는 `none`만 허용한다.
- 다이얼로그/입력 포커스에서는 뷰어 단축키를 막는다(EXIF 패널의 `I` 닫기 예외 제외).

### 14.4 명령 팔레트 `Ctrl+K`

- 그룹 순서: `file → navigate → view → display → system`.
- `requiresImage`는 이미지 있을 때만, `requiresNavigation`은 이동 가능할 때만 활성화된다.
- `system` 그룹은 항상 활성이다: 설정 열기(`openSettings`), 업데이트 확인(`checkForUpdates`).
- 공백 분리 토큰 AND 매칭이며 점수는 라벨 시작 3점, 라벨 포함 2점, ID/영문 별칭 1점이다.
- 한국어 UI에서도 영문 별칭(`open`, `copy`, `rotate`, `grid` 등)으로 검색된다.

## 15. 백엔드 IPC 계약

진실: `src-tauri/src/lib.rs` `invoke_handler`, `src-tauri/src/commands.rs`, `src-tauri/src/save.rs`.

| 명령                         | 입력 (JS camelCase → Rust snake_case)                                                | 반환                           |
| ---------------------------- | ------------------------------------------------------------------------------------ | ------------------------------ |
| `load_image`                 | `filePath` → `file_path`                                                             | `ImageInfo`                    |
| `get_directory_images`       | `filePath` → `file_path`, `options?`                                                 | `DirectoryImages`              |
| `resolve_dropped_path`       | `path`                                                                               | 해석된 파일 경로 `string`      |
| `get_exif_data`              | `filePath` → `file_path`                                                             | `Record<string, string>`       |
| `get_image_histogram`        | `filePath` → `file_path`                                                             | `Histogram`                    |
| `get_image_details`          | `filePath` → `file_path`                                                             | `ImageDetails`                 |
| `get_archive_images`         | `filePath` → `file_path`                                                             | `DirectoryImages`(엔트리 목록) |
| `load_archive_image`         | `archivePath` → `archive_path`, `entryName` → `entry_name`                           | `ImageInfo`                    |
| `archive_prefetch`           | `archivePath` → `archive_path`, `entryNames` → `entry_names`                         | 추출 개수 `number`             |
| `generate_thumbnail`         | `filePath` → `file_path`, `maxSide?` → `max_side?`                                   | `ThumbnailInfo`                |
| `generate_thumbnails_batch`  | `filePaths` → `file_paths`, `maxSide?` → `max_side?`                                 | `BatchThumb[]`                 |
| `generate_archive_thumbnail` | `archivePath` → `archive_path`, `entryName` → `entry_name`, `maxSide?` → `max_side?` | `ThumbnailInfo`                |
| `get_file_associations`      | 없음                                                                                 | `FileAssociation[]`            |
| `set_file_association`       | `extension`, `associate`                                                             | `FileAssociation`              |
| `set_all_file_associations`  | `associate`                                                                          | `FileAssociation[]`            |
| `open_default_apps_settings` | 없음                                                                                 | 없음                           |
| `get_psd_thumbnail_status`   | 없음                                                                                 | `PsdThumbStatus`               |
| `register_psd_thumbnail`     | 없음                                                                                 | `PsdThumbStatus`               |
| `unregister_psd_thumbnail`   | 없음                                                                                 | `PsdThumbStatus`               |
| `trash_file`                 | `filePath` → `file_path`                                                             | 없음                           |
| `rename_file`                | `oldPath` → `old_path`, `newName` → `new_name`                                       | `ImageInfo`                    |
| `save_image_edits`           | `filePath` → `file_path`, `options`                                                  | `ImageInfo`                    |
| `frontend_ready`             | 없음                                                                                 | 없음 (`PendingOpenFile` flush) |

최상위 인자 키는 Tauri 기본 camelCase 변환을 사용하므로 JS 호출 키와 Rust 파라미터명이 위와 같이 대응한다. 중첩 `options` 페이로드(`DirListOptions`, `SaveImageOptions`)는 `serde(rename_all = "camelCase")`라 JS와 Rust 필드명이 같다(`sortKey`, `rotationCw` 등). 응답은 모두 snake_case이다(16절).

파일 연결 주의: 설정에서 연결 변경은 해당 확장자의 Windows 기본 앱 선택 창을 연다. 조용한 UserChoice 레지스트리 쓰기는 할 수 없다.

개발 빌드(디버그)는 설치 버전과 확장자 연결이 서로 덮어쓰지 않도록 `AraView (Dev)` 이름과 별도 레지스트리 키(`Software\AraView (Dev)\Capabilities`), 별도 ProgID(`com.araview.viewer.dev.<ext>`)를 사용한다. 창 제목도 `(Dev)`가 붙는다.

## 16. 데이터 모델

진실: `src/types/index.ts`, `src-tauri/src/image.rs`, `src-tauri/src/thumbnail.rs`, `src-tauri/src/file_assoc.rs`.

### 16.1 `ImageInfo`

Rust와 TypeScript는 같은 모양을 유지한다.

- `file_path: string`: WebView가 디코드할 경로. HEIC/HEIF/PSD는 JPEG sidecar 경로일 수 있다.
- `mime_type: string`
- `file_name: string`
- `file_size: number`
- `width: number | null`, `height: number | null`: 렌더 바이트 기준 치수. SVG는 헤더 파싱으로 복원하며 해석 불가분만 null.

### 16.2 기타

- `DirectoryImages`: `{ images: string[], current_index: number, availability: ("local"|"cloud_only"|"unknown")[] }`.
- `ThumbnailInfo`: `{ file_path: string, width: number, height: number }`.
- `ExifData`: `Record<string, string>`.
- `Histogram`: `{ r: number[256], g: number[256], b: number[256], sampled_pixels: number }`.
- `ImageDetails`: `{ file_path, file_size, width, height, color_mode, bits_per_channel?, created_unix?, modified_unix?, dpi_x?, dpi_y?, icc_status: "present"|"absent"|"unchecked", icc_name?, icc_bytes? }`. 백엔드 출력은 snake_case를 유지한다(`ImageInfo`·`DirectoryImages` 선례).
- `ArchiveState`: `{ archivePath: string | null }`.
- `FileAssociation`: `{ extension, associated, current_prog_id, needs_os_confirmation }`.
- `PsdThumbStatus`: `{ registered, clsid, dll_path, dll_exists }`. 탐색기 썸네일 등록 상태(20.2절). 백엔드 출력은 snake_case를 유지한다.
- `SaveImageOptions`: camelCase `{ rotationCw, flipH, flipV, format?, overwrite, newFileName? }`.
- `DirListOptions`: camelCase `{ sortKey, descending, recursive }`.

## 17. 에러 모델

진실: `src-tauri/src/app_error.rs`, `src/utils/appError.ts`.

백엔드 `ErrorCode` 8종(snake_case 직렬화):

`not_found`, `permission`, `unsupported`, `too_large`, `corrupt`, `invalid_input`, `already_exists`, `unknown`.

프론트 분류 5종:

- `not-found`: 파일 이동/삭제, 빈 폴더/아카이브.
- `permission`: 권한/잠금.
- `unsupported`: 미지원 포맷, EXIF 없음 포함.
- `corrupt`: 디코드/읽기 실패, 손상.
- `unknown`: 그 외.

규칙:

- 모든 command는 `Result<T, AppError>`이다.
- `io` 에러는 NotFound/Permission을 정확히 매핑하고 나머지는 호출자가 고른 fallback을 쓴다.
- 새 백엔드 에러 문구를 추가하면 프론트 `EXACT_KIND`/`PREFIX_KIND` 표에도 행을 추가한다.
- 표시는 `titleKey`/`hintKey`를 번역한다.

## 18. 렌더링 경로

- 백엔드는 디코드용 파일 경로를 돌려주고, 프론트는 `convertFileSrc(...)`로 변환해 `<img>`에 넣는다. single 모드의 SVG는 `scale(zoom)` 대신 레이아웃 크기(`원본 x 줌`)로 확대해 브라우저가 표시 크기에서 재래스터하게 한다.
- HEIC/HEIF/PSD만 JPEG sidecar를 만든다. sidecar는 프로세스 임시 디렉터리 아래에 있다.
- 아카이브 추출물도 같은 임시 디렉터리 아래 `archive-<hash>/`에 둔다. hash는 아카이브 canonical 경로+mtime+크기라 동명 아카이브가 캐시를 공유하지 않는다.
- 썸네일은 `process_temp/thumbs/` 아래 JPEG 캐시를 쓴다.
- `assetProtocol.enable=true`이고 정적 `scope`는 비어 있다. 프로세스 임시 디렉터리만 setup에서 재귀 허용하고, 사용자가 여는 파일/폴더는 `load_image`, `load_archive_image`, `rename_file`, `save_image_edits`가 런타임에 `asset_protocol_scope().allow_file/allow_directory`로 허용한다.
- CSP는 `default-src 'self'` 기반이며 `img-src`에 `asset:`/`http://asset.localhost`, `connect-src`에 `ipc: http://ipc.localhost`와 `http://asset.localhost`를 허용한다. 프로덕션은 `withGlobalTauri=false`이고, MCP 검증용 dev 실행만 `src-tauri/tauri.dev.conf.json`으로 `withGlobalTauri=true`를 덮어쓴다.

## 19. 다국어

진실: `src/i18n/index.ts`, `src/i18n/locales/ko.json`, `src/i18n/locales/en.json`.

- 지원 언어: `ko`, `en`.
- 시스템 감지: 브라우저 언어가 `ko`로 시작하면 `ko`, 아니면 `en`.
- 폴백: `ko`.
- 설정 변경 즉시 적용된다.

## 20. 윈도우/배포

진실: `src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs`, `docs/releasing.md`, `.github/workflows/release.yml`.

- 창: 제목 기본값 `AraView`(런타임에는 이미지가 열리면 파일명, 아니면 현재 로케일의 앱 이름), 1024x768, 최소 600x400, 프레임리스, 시작 시 숨김(`visible: false`).
- `window-state` 플러그인으로 창 상태를 유지한다.
- 번들: `nsis`만 빌드한다(릴리즈 빌드 시간 단축을 위해 MSI 제외). 결과물은 `src-tauri/target/release/bundle/` 아래에 생성된다.
- 릴리스 파이프라인: `.github/workflows/release.yml`만 있으며 태그(`v*`) 푸시에서만 돈다. 릴리스 러너 한 대에서 프런트 검사(`npm test`, `tsc --noEmit`, `oxfmt --check`)와 Rust 검사(`cargo fmt --check`, `cargo test --no-default-features`, `cargo clippy --no-default-features -- -D warnings`)를 먼저 수행하고, 하나라도 실패하면 빌드와 릴리스로 진행하지 않는다.
- 릴리스 생성 권한: `GITHUB_TOKEN`(`contents: write`)을 쓰며, 저장소 기본 워크플로 권한이 `read`면 릴리스 생성이 403(`Resource not accessible by integration`)으로 실패한다. 기본 권한을 `read`로 유지하려면 `contents: write` fine-grained PAT를 `RELEASE_TOKEN` 시크릿으로 등록한다(워크플로가 `secrets.RELEASE_TOKEN || secrets.GITHUB_TOKEN`으로 선택).
- 파일 연결 3그룹:
  - Image 14종: png, jpg, jpeg, gif, bmp, webp, svg, ico, tiff, tif, avif, heic, heif, psd.
  - Comic 4종: cbz, cb7, cbr, cbt.
  - Archive 3종: rar, zip, 7z.
- HEIC/HEIF는 vcpkg `libheif[core]` 동적 링크 + `libde265`만 사용한다. `embedded-libheif`를 켜지 않고 `x265`를 넣지 않는다. `VCPKG_ROOT`가 있으면 빌드 시 `heif.dll`, `libde265.dll`을 복사한다.

### 20.1 자동 업데이트 (tauri-plugin-updater)

수동 확인만 제공한다. 시작 시 자동 확인이나 백그라운드 폴링은 없다(오프라인 우선).

- 진실: `src/hooks/useUpdater.ts`(확인/설치 흐름), `src/components/settings/GeneralTabPanel.tsx`(업데이트 섹션), `src/constants/commands.ts`(`checkForUpdates`), `src-tauri/tauri.conf.json`(`plugins.updater`), `src-tauri/capabilities/default.json`(`updater:default`, `process:default`), `.github/workflows/release.yml`(서명).
- 진입점: 설정 일반 탭의 `지금 확인` 버튼, 명령 팔레트(`Ctrl+K`)의 `업데이트 확인`. 팔레트 실행은 `tiv:check-updates` 이벤트를 보내고, 루트(`__root.tsx`)의 `useUpdateCheckRequestListener`가 받아 `checkForUpdatesNow()`를 실행한다.
- 흐름: `check()` → 없으면 `toast.update.latest`, 있으면 `toast.update.availableTitle` + `다운로드 및 설치` 액션 → `downloadAndInstall` 진행률 토스트 → 완료 시 `toast.update.installed` + `다시 시작` 액션(`relaunch`). 중복 확인은 `busy`로 무시한다.
- 설정: `bundle.createUpdaterArtifacts: true`, `plugins.updater.endpoints`는 공개 저장소의 릴리스 피드(`https://github.com/ara-hwang/araview-updates/releases/latest/download/latest.json`)다. 소스 저장소가 비공개라 자산을 익명으로 받을 수 없어, 소스가 공개되기 전까지의 임시 구성이다. `pubkey`는 서명 공개키이며 maintainer 로컬 키 `araview.key.pub` 내용과 같다.
- 업데이트 피드(임시): 피드와 설치본 모두 공개 저장소 `ara-hwang/araview-updates`의 릴리스에 게시한다. 이것이 `scripts/Publish-LocalRelease.ps1`의 기본 대상이며 `-UpdatesRepo`로 덮어쓸 수 있다. 저장소가 달라질 때 대상 저장소에 태그를 생성한다. 소스 저장소를 공개로 전환하면 기본값과 endpoint를 원래 릴리스 URL로 되돌린다.
- 서명키 발급(maintainer 1회): `npm run tauri signer generate -- -w "$env:USERPROFILE\.tauri\araview.key"`. 공개키는 `tauri.conf.json`의 `plugins.updater.pubkey`에, 비밀키 내용과 비밀번호는 repo Secrets `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`에 등록한다. 릴리스 워크플로가 서명하고 `latest.json` + `.sig`를 태그 릴리스에 첨부한다.
- 로컬 서명 빌드: `TAURI_SIGNING_PRIVATE_KEY`에 키 경로 또는 키 내용을, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`에 비밀번호를 넣는다. 번들러(`crates/tauri-cli/src/bundle.rs`의 `sign_updaters`)는 `TAURI_SIGNING_PRIVATE_KEY_PATH`를 읽지 않고, 값이 존재하는 경로면 파일 내용을 읽는다. 비밀번호가 없으면 대화형 프롬프트가 뜨고, `--ci` 또는 `CI` 환경이면 빈 문자열로 처리한다.
- 서명 생략: `npm run tauri build -- --no-sign`은 updater 서명을 건너뛴다. 로컬 확인용이며 `.sig`가 없으므로 배포에 쓰지 않는다.
- 로컬 릴리스(CI 대체): `scripts/Publish-LocalRelease.ps1`(`npm run release:local`). `tauri.conf.json` 버전으로 태그를 확인하고, 서명 빌드, `.sig`와 `plugins.updater.pubkey`의 키 ID 대조, `latest.json` 생성, 태그 푸시, `gh release create`/`upload`를 수행한다. 기본은 바로 공개이며 `-Publish:$false`로 draft 유지, `-SkipBuild`로 기존 산출물 재사용, `-DryRun`으로 GitHub 접촉 없이 점검한다. 산출물(설치본, `.sig`, `latest.json`)은 저장소 안 `release/vX.Y.Z/`에 모으고 `.gitignore`로 제외하며 `-OutputDir`로 바꿀 수 있다. 서명 키는 `-KeyPath`, `TAURI_SIGNING_PRIVATE_KEY`, 저장소 루트 `araview.key`, `~/.tauri/araview.key` 순서로 찾고, 비밀번호는 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`로 받는다. 둘 다 `.env.local`(gitignored, `.env.example` 참고)에서 읽을 수 있고 이미 설정된 환경변수가 우선한다. `.env.local`은 Vite도 읽지만 `VITE_` 접두사만 클라이언트로 노출되므로 키가 프런트엔드로 새지 않는다.

### 20.2 PSD 탐색기 썸네일 (IThumbnailProvider)

Windows 파일 탐색기에서 `.psd` 축소판을 표시한다. 미리보기 창(`Alt+P`)은 범위 밖이며 썸네일만 제공한다.

- 진실: `src-tauri/crates/araview-thumb/`(COM DLL), `src-tauri/src/thumb_shell.rs`(HKCU 등록), `src/hooks/usePsdThumbnail.ts` + `src/components/settings/ExtensionSettingsPanel.tsx`(설정 UI).
- 핸들러는 `araview_thumb.dll` 안의 In-Proc COM 서버이다. `IThumbnailProvider` + `IInitializeWithStream`(격리 surrogate 주 경로)/`IInitializeWithFile`을 구현하고, `psd` 크레이트 합성 디코드 + 흰 배경 합성(앱 JPEG sidecar와 동일 규칙)으로 32bpp DIB를 돌려준다. PSB(`8BPB`)는 거부하고 탐색기가 기본 아이콘으로 폴백한다. `DisableProcessIsolation`은 두지 않는다.
- CLSID(발행 후 변경 금지): 릴리스 `{FD6BD976-2DF4-4656-94F2-1D166163EC59}`, 개발 `{BD277595-1702-4AC5-AA7C-A67965C3D570}`. 문자열은 DLL 크레이트(`registry.rs`)와 앱(`thumb_shell.rs`)에 중복 정의되어 있으며 함께 바꿔야 한다(앱은 `windows` 크레이트 의존을 피한다).
- 등록은 전부 HKCU(`Software\Classes`)라 관리자 권한이 필요 없다: `CLSID\{CLSID}\InprocServer32`(DLL 경로 + `ThreadingModel=Apartment`), `.psd`와 채널 ProgID(`com.araview.viewer[.dev].psd`)의 `ShellEx\{E357FCCD-A995-4576-B01F-234630154E96}` 슬롯, `.psd`의 `PerceivedType=image`/`Content Type` 빈값 채우기. 등록/해제 후 `SHChangeNotify(SHCNE_ASSOCCHANGED)`를 보낸다.
- DLL 전달: 워크스페이스 멤버(`src-tauri` `[workspace]`)로 함께 빌드한다. 개발 DLL은 공유 target dir라 dev exe 옆에 놓이고, 릴리스는 `scripts/Build-ThumbDll.ps1`(`npm run build:thumb`)로 빌드해 `src-tauri/resources/`에 스테이징하면 NSIS 번들이 `$INSTDIR\resources\`에 싣는다. Windows 리소스 목록의 진실은 `src-tauri/tauri.windows.conf.json`이며, 플랫폼 설정이 `tauri.conf.json`의 `bundle.resources`를 통째로 교체(JSON Merge Patch)하므로 Windows용 리소스는 전부 이 파일에 반복해야 한다(base에만 두면 설치본에서 빠진다). 이 파일은 빌드 스크립트(`tauri-build`)가 strict JSON으로 파싱하므로 주석(`//`)을 넣으면 `cargo test`/`tauri dev`가 실패한다. 앱은 exe 옆, `resources/` 순으로 DLL을 찾는다.
- 설정 UI(확장자 탭 하단): 상태 조회(`get_psd_thumbnail_status`), 켜기(`register_psd_thumbnail`)/끄기(`unregister_psd_thumbnail`). DLL이 없으면 등록을 거부하고 안내한다. 적용 뒤 탐색기 재시작/썸네일 캐시 정리가 필요할 수 있음을 안내한다.
- 제한: 고무결성 Explorer(Windows Sandbox 등)는 HKCU COM을 무시하므로 dev 채널 썸네일이 동작하지 않는다. 삭제 시 HKCU 키는 남을 수 있으며 다시 설치 후 설정에서 다시 켜면 복구된다.

## 21. 비목표와 제약

- 클라우드, 공유, 라이브러리 가져오기는 범위 밖이다.
- 브랜드명, 로고, 수치, 후기, 컴플라이언스 문구를 만들지 않는다.
- 사용자 수, 성능 수치, 리뷰 같은 근거 없는 주장을 UI/문서에 넣지 않는다.
- 빈 상태는 가짜 샘플 대신 실제 다음 행동을 안내한다.
- 파일 연결은 OS 확인 없이 바꿀 수 없다.

## 22. 변경 시 동기화 체크리스트

### 22.1 포맷 추가

1. `src-tauri/src/image.rs` MIME 매핑 + 테스트.
2. `src/constants/imageExtensions.ts`.
3. `src-tauri/tauri.conf.json` 파일 연결(필요 시).
4. `samples/` 파일로 `load_image` 확인, EXIF 포맷은 `get_exif_data`도 확인.
5. `README.md`, `docs/`(해당 안내), 본 문서(`SPEC.md`), 필요 시 `AGENTS.md` 범위 갱신.

### 22.2 백엔드 명령 추가

1. `src-tauri/src/commands.rs`에 `#[tauri::command]` 구현.
2. `src-tauri/src/lib.rs` `invoke_handler` 등록.
3. 프론트 `invoke(...)`(보통 훅)에서 호출.
4. 타입 변경 시 TypeScript/Rust 양쪽 갱신.
5. 본 문서 15절 IPC 표와 16절 데이터 모델 갱신.

### 22.3 설정/단축키 추가

1. `src/store/settingsStore.ts` 기본값 + sanitize.
2. `src/constants/shortcuts.ts` ID/기본값(필요 시).
3. 본 문서 13~14절 갱신.
4. 영속화 키 변경 시 12절도 갱신한다.
