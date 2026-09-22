# SPEC.md - AraView 기능/기술 명세

> 한국어 스펙 문서. 구현 진실(source of truth)은 코드이며, 본 문서는 현재 코드베이스의 동작을 요약한다.
> 관련 문서: `PRODUCT.md`(제품 정의), `DESIGN.md`(비주얼 시스템), `ROADMAP.md`(단계 계획), `README.md`(소개/문서 허브), `docs/usage.md`(사용법), `docs/development.md`(개발 안내), `docs/releasing.md`(릴리스/업데이트), `AGENTS.md`(AI 작업 지침).

## 0. 문서 규약

- 경로 별칭 `@/...`는 `src/` 기준, 백엔드 경로는 `src-tauri/src/` 기준이다.
- `invoke()` 인자 변환(최상위 camelCase, 중첩 옵션 camelCase, 응답 snake_case) 상세는 15~16절을 따른다.
- 에러는 구조화 에러 `{ code, message }`를 우선한다.
- base64 이미지 페이로드는 사용하지 않는다. 렌더링은 항상 파일 경로 기반이다.

## 1. 개요

- Windows 10/11 x64 전용 오프라인 데스크톱 이미지/코믹 뷰어. 로컬 파일만 다루며 라이브러리 가져오기, 계정, 네트워크를 쓰지 않는다(수동 업데이트 확인 제외).
- 창은 프레임리스(`decorations: false`)이며 커스텀 타이틀바/툴바(`src/components/Header.tsx`)를 쓴다. Windows 11에서는 최대화 버튼 호버로 OS Snap Layouts 플라이아웃이 뜬다(§20).
- 기술 스택과 플러그인 목록은 `docs/development.md`와 `AGENTS.md`를 따른다.

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

- QOI, JXL, RAW(CR2/NEF/ARW 등), PSB는 제외 유지. 선정 이유와 후보 검토는 `ROADMAP.md` 4.4를 따른다. PSD는 읽기 전용 미리보기만 지원하며 PSB(`8BPB`)는 디코더가 없어 진입 차단한다.

## 3. 화면과 라우트

라우트 진실: `src/routes/__root.tsx`, `src/routes/index.tsx`, `src/routes/image.tsx`.

### 3.1 홈 `/`

- 빈 상태: 실제 다음 행동(파일 열기)을 안내한다. 가짜 샘플을 만들지 않는다.
- `파일 열기` 버튼은 파일 피커를 연다.
- 최근 파일 섹션: `recordRecentFiles`가 true이고 목록이 있을 때만 표시하며 개수와 전체 삭제를 제공한다. 파일명, 형식/크기/치수, 상위 폴더를 표시하고 아카이브는 읽기 진도를 함께 보여준다.
- 아카이브는 아이콘으로, 로드 실패 항목은 error 상태로 표시한다.
- 드래그 중에는 드래그 오버레이를 표시한다.
- 시작 옵션: `autoOpenLastFile && recordRecentFiles`이고 최근 파일이 있으면 첫 항목을 1회 자동 로드한다.
- 이미지가 로드되면 `/image`로 이동한다. 창 제목은 이미지 파일명, 없으면 앱 이름이다.

### 3.2 뷰어 `/image`

- `beforeLoad`에서 `imageInfo`가 없으면 `/`로 리다이렉트한다.
- 구성: `ImageContainer`(읽기 영역) + 이미지 목록 도크(`ImageNavBar`, 상/하/좌/우) + 드래그 오버레이 + `ThumbnailGrid`(선택) + `RenameDialog` + `SaveEditsDialog`.
- 도크: 뷰어 가장자리 플로우에 붙으며 읽기 영역을 밀어낸다. 위치(`dockPosition`, 기본 `bottom`), 표시(`dockVisible`, 기본 펼침), 썸네일 크기(`dockThumbSize` S/M/L), 파일명(`dockShowName`)/번호(`dockShowIndex`)를 보기 설정에서 바꾼다. 도크의 `⋯` 버튼에서도 같은 값을 즉시 바꿀 수 있다. 바 전체를 접으면 얇은 엣지 바로 복구한다. 접힘·자동숨김 동안에도 목록 스크롤 위치와 로드한 썸네일이 유지된다. 이미지가 2장 이상일 때만 표시한다.
- 도크 구성: 한 줄에 이전/다음 버튼, 썸네일 목록(가상화), 그리드 토글, `⋯` 옵션, 접기 버튼을 둔다. 슬라이더는 없다. 목록은 폴더/아카이브의 모든 항목을 같은 순서로 유지하며, 보이는 창을 먼저 채운 뒤 나머지 썸네일을 이어서 로드한다.
- 도크 위 휠은 설정된 휠 동작(`wheel`)을 따른다(기본: 위=이전, 아래=다음, Ctrl+휠=확대/축소). 동작이 `none`이면 목록 스크롤로 넘긴다. 웹툰 모드에서도 도크 휠은 이동으로 동작한다.
- `G`(기본, 재할당 가능)로 썸네일 그리드 오버레이를 연다. 열 때 현재 이미지를 중앙에 둔다.
- 그리드: 뷰포트 기반 가상화, 클릭/`Enter`로 점프 후 닫기, `Esc`/`G`로 닫기, 파일명 필터, 실패 셀 배지와 재시도. 그리드가 열려 있는 동안 뷰어 단축키는 비활성이다.
- 우클릭은 설정(`mouse.rightClick`)에 따라 컨텍스트 메뉴 또는 다른 동작이다. 홈에서는 우클릭을 막는다.
- `Esc` 닫기: 이름 변경/저장 다이얼로그가 열려 있거나 입력 포커스 중이면 닫지 않는다.
- `autoHideUI`가 true일 때만 읽기 중 크롬(상단바, 이미지 목록 도크, 상태바)을 숨기고 읽기 영역을 확장한다. 도크 상태(스크롤 위치, 로드한 썸네일)는 유지된다.
- `menuBarHidden`이 true이면 상단바를 숨기고, 상단 호버 영역에서 peek 오버레이로 표시한다. 헤더 숨기기 버튼과 보기 설정 스위치로 토글한다. peek 시에는 읽기 영역 위로 겹쳐 내려오며, 포커스 이탈 시에는 즉시 닫힌다. 모션 토큰과 timing은 `DESIGN.md`를 따른다. `Esc`로는 닫히지 않는다(뷰어의 이미지 닫기와 충돌 방지).
- 로드 실패 시 에러 카드에 재시도/홈 복구 경로를 제공한다. 실패한 적은 토스트로 알린다.

### 3.3 상태 규칙

- `PRODUCT.md` 원칙과 `DESIGN.md` 시스템을 따른다: 모든 데이터 뷰는 empty/loading/error 상태, 모든 컨트롤은 키보드 조작과 포커스 표시, UI 카피 금칙(문서 참조)을 지킨다.

## 4. 파일 열기 흐름

진실: `src/hooks/useImageLoader.ts`, `src/hooks/useOpenFileListener.ts`, `src-tauri/src/lib.rs`.

### 4.1 파일 피커

- 이미지 필터로 단일 선택 후 `loadImage`로 연다.

### 4.2 드래그 앤 드롭

- 파일/폴더를 여러 개 드롭할 수 있다.
- 아카이브 경로는 그대로 유지한다.
- 그 외 경로는 `resolve_dropped_path`로 해석한다:
  - 파일이면 그대로 반환.
  - 폴더면 내부 지원 파일 중 이름순(소문자 기준) 첫 이미지를 반환.
  - 이미지 없음/경로 없음/지원 불가면 에러.
- 해석 실패 항목은 건너뛴다. 전부 실패하면 실패 안내를 표시한다.
- 성공 목록은 소문자 이름순 정렬 후 첫 항목을 열고, 2개 이상이면 첫 항목 열림을 알린다.

### 4.3 OS 파일 연결 실행

- Windows가 CLI 인자(`args[1]`)로 파일 경로를 넘긴다.
- 백엔드는 경로를 `PendingOpenFile` 상태에 보관하고, 프론트가 준비된 뒤 호출하는 `frontend_ready`에서 emit한다.
- 두 번째 실행은 `single-instance`가 기존 창에 전달한다. 프론트가 준비 전이면 보관됐다가 flush되고, 준비 후면 즉시 emit한다.
- 프론트 루트의 브리지가 이벤트를 받아 현재 라우트가 등록한 로더로 전달한다. 라우트 전환 중이라 로더가 없으면 보류했다가 다음 등록 시 전달한다.

## 5. 디렉토리 목록과 탐색

진실: `src-tauri/src/commands.rs`, `src-tauri/src/dir_cache.rs`, `src/hooks/useDirectoryNavigation.ts`, `src/utils/directoryOptions.ts`.

### 5.1 `get_directory_images`

- 입력: `file_path`, `options?: DirListOptions`(`sortKey: name | date | size`, `descending`, `recursive`).
- 부모 폴더 기준으로 정렬 목록을 만들고, `current_index`는 요청 경로의 위치(없으면 0)이다.
- 재귀가 켜지면 하위 폴더 이미지를 포함한다.
- 디렉토리 목록은 캐시되며 상한이 있다.
- 응답 `availability`는 `images`와 같은 순서의 `local` | `cloud_only` | `unknown` 배열이다. Windows Files On-Demand placeholder는 `cloud_only`로 표시한다. 아카이브 목록은 빈 배열이다.

### 5.2 목록 새로고침

- 설정 변경 후 다시 읽고 이전 위치를 복원한다.
- 복원 규칙: 이전 경로가 있으면 그 위치, 없으면 범위 내 clamp.
- sidecar처럼 표시 경로가 원본과 다를 수 있어 원본 경로를 우선한다.
- 아카이브 모드에서는 폴더 새로고침을 하지 않는다.

### 5.3 이전/다음

- `viewMode`가 `left-to-right`/`right-to-left`이면 2장씩, 나머지는 1장씩 이동한다.
- `loopNavigation=false`: 끝에서 멈춘다. 단, 마지막 장은 clamp로 볼 수 있게 한다.
- `loopNavigation=true`: wrap한다.
- 아카이브/일반 모드에 맞는 로더로 인덱스를 갱신한다.
- 목록이 1개 이하면 이전/다음을 수행하지 않는다.

### 5.4 점프

- `PageUp/PageDown`: 10장 점프.
- `Home/End`: 처음/마지막.
- 루프 설정에 따라 wrap/clamp한다.
- 웹툰 모드에서는 같은 동작이 연속 스크롤 이동으로 바뀐다.

### 5.5 손상 파일 건너뛰기

- `skipBrokenFiles`가 true면 로드 실패 시 다음 후보로 자동 이동한다.
- 아카이브/일반 모두 동일 규칙이며, 건너뛰면 안내를 표시한다.

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
- 웹툰 모드에서 `ArrowUp/ArrowDown`은 연속 스크롤 컨테이너를 일정량씩 스크롤한다.
- 웹툰 중앙 이미지 변경은 전체 reload 없이 인덱스 동기화와 정보 교체로 처리한다.

## 7. 뷰어 조작

진실: `src/store/appStore.ts`, `src/utils/zoomPanUtils.ts`, `src/hooks/useZoomPan.ts`, `src/hooks/useWheelNavigation.ts`.

### 7.1 줌

- `zoomIn`: `min(zoom * 1.25, max)`. 상한은 SVG 40, 그 외 10.
- `zoomOut`: 동적 최소값까지 `zoom / 1.25`.
- `0`: 자동 맞춤으로 리셋(위치 0 + 회전/반전 초기화).
- `1/2/3`: 가로 맞춤 / 세로 맞춤 / 화면 맞춤.
- `fitMode`: `width | height | screen | auto`. 기본 `auto`. 헤더 버튼과 단축키로 바꾸면 저장되고 이미지 전환마다 다시 적용된다. 수동 줌을 하면 잠금이 풀려 리사이즈해도 줌을 유지한다. `auto`는 큰 이미지만 맞추고 작은 이미지는 100%로 두며, `width/height/screen`은 작은 이미지도 확대한다.
- 맞춤 계산은 회전된 치수(90/270도면 가로세로 교환) 기준이다.

### 7.2 팬

- 키보드 팬은 1회 48px이며 컨테이너 경계로 clamp한다.
- 마우스 왼쪽 드래그는 기본 팬이다(`pan` 또는 `none`만 허용).

### 7.3 회전/반전

- `rotation`: `0 | 90 | 180 | 270`.
- `R`: 시계 90도, `Shift+R`: 반시계 90도.
- `H`: 좌우 반전, `V`: 상하 반전.
- 이미지 변경 시 기억된 맞춤 모드로 초기화된다.
- 저장 시 순서는 회전 먼저, 반전 나중이며 화면 표시와 일치한다.

### 7.4 배경/표시

- `viewerBackground`: `theme | black | white | checker`. 기본 `theme`.
- `B`: `theme → black → white → checker` 순환.
- `T`: 항상 위 토글.
- `F11`: 전체화면 토글. 더블클릭 기본도 전체화면이다.
- `autoHideUI`: true일 때 읽기 중 크롬 자동 숨김.

## 8. 아카이브

진실: `src-tauri/src/archive.rs`, `src-tauri/src/commands.rs`, `src/hooks/useImageLoader.ts`, `src/store/archiveProgressStore.ts`.

- `get_archive_images`: 내부 이미지 엔트리 목록 + `current_index: 0`. 비어 있으면 `not_found`.
- `load_archive_image`: 임시 디렉터리에 추출 후 표시 가능한 경로로 반환한다.
- `archive_prefetch`: 이웃 선추출용 fire-and-forget 명령이다.
- 선추출 거리는 뷰 모드에 따라 보정되며 상한이 있다.
- 이어보기: 아카이브 경로별 마지막 엔트리와 위치(엔트리명/인덱스/전체 페이지)를 최대 100개 LRU로 저장한다. `resumeReading`이 true이고 목록에 저장된 항목이 있으면 거기서 시작하며, 저장 위치에서 시작할 때 "이어보기" 토스트와 "처음부터" 동작을 함께 제공한다. 설정이 false면 항상 첫 페이지에서 열고 열기만으로 저장 위치를 0페이지로 덮지 않는다.
- 아카이브 모드 제한: 휴지통 이동, 이름 변경, 편집 저장은 안내와 함께 차단된다.
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

- `generate_thumbnail`: 기본 256px, JPEG 캐시 후 재사용한다. 상한(500MB)을 넘기면 오래된 것부터 제거한다.
- HEIC/HEIF/PSD는 썸네일용 JPEG sidecar 경로를 쓴다.
- 디코드 불가 입력(SVG 등)이나 아카이브 엔트리명은 에러를 내고, 프론트는 원본으로 폴백한다.
- 배치 조회와 아카이브 엔트리용 썸네일 API를 별도로 제공한다. 아카이브 썸네일은 추출물과 캐시를 재사용해 풀사이즈 로드를 피한다.
- 그리드는 보이는 창의 경로만 썸네일을 요청한다.

## 10. EXIF/파일 정보

진실: `src-tauri/src/commands.rs`, `src-tauri/src/image_info.rs`, `src/hooks/useExifLoader.ts`, `src/components/ExifPanel.tsx`, `src/components/HistogramChart.tsx`.

- `get_exif_data`는 문자열 맵을 반환한다.
- 파일 없음이면 `not_found`, EXIF 없으면 `unsupported`.
- `I`로 패널 토글. 패널 내부 포커스에서는 `I` 닫기를 허용한다.
- 표시 범주는 Camera, Exposure, Image, Lens, DateTime, GPS, Software 계열이다.
- HEIC는 원본 경로 기준 EXIF를 읽으므로 비어 있는 경우가 많다.

### 10.1 히스토그램 `get_image_histogram`

- 입력 `file_path`, 반환 `Histogram`(16절).
- 디코드 불가 포맷(SVG, AVIF 등)은 에러를 내고, 프론트는 차트 대신 안내 문구를 표시한다.
- 렌더는 신규 의존성 없이 SVG 영역 차트이다.

### 10.2 파일 상세 `get_image_details`

- 입력 `file_path`, 반환 `ImageDetails`(16절).
- EXIF가 없어도 명령은 성공한다. 디코드 실패 파일도 상세를 반환한다.
- 패널 표시: 경로(아카이브 모드면 아카이브 경로와 엔트리명 함께), 크기, 치수 + 픽셀 수, 생성/수정 시각(아카이브 모드 숨김), 색상 + 비트/채널, DPI, 색상 프로파일.
- 조회 실패가 있어도 패널은 열리고 섹션별 안내 문구를 표시한다.

## 11. 파일 작업

진실: `src/hooks/useFileOperations.ts`, `src-tauri/src/commands.rs`, `src-tauri/src/save.rs`, `src/hooks/useCopyImage.ts`.

공통: 아카이브 모드면 원본 아카이브 경로를 대상으로 삼는다. 단, 휴지통/이름 변경/편집 저장은 아카이브에서 차단된다.

### 11.1 휴지통 이동 `Delete`

- `trash_file`: OS 휴지통으로 이동(영구 삭제 아님). 디렉토리 불가.
- 확인 다이얼로그 후 실행한다.
- 성공 시 최근 파일에서 제거하고, 남은 목록에서 다음 위치를 연다. 남은 게 없으면 홈으로 돌아간다.

### 11.2 이름 변경 `F2`

- `rename_file` 후 새 `ImageInfo`를 반환한다.
- 검증:
  - trim 후 비어 있으면 거부.
  - `/`, `\` 포함 거부.
  - `< > : " | ? *` 포함 거부.
  - 끝이 공백/마침표면 거부.
- 이동 전 지원 확장자 검사, 중복 이름이면 `already_exists`.
- 대소문자만 바꾸는 동일 파일은 이동을 생략한다.
- 성공 시 디렉토리 목록과 최근 파일 경로를 교체한다.

### 11.3 편집 저장 `Ctrl+S`

- PSD/SVG/AVIF는 저장 불가라 진입 시 다이얼로그를 열지 않고 안내한다. 백엔드도 `unsupported`로 2중 차단한다. 그 외 지원 포맷은 저장 가능하다.
- `save_image_edits` 옵션: 회전(`0 | 90 | 180 | 270`), 좌우/상하 반전, 출력 포맷(`png | jpg | jpeg | webp` 또는 원본 유지), 덮어쓰기 여부와 새 파일명.
- 출력 포맷 결정: 요청값 > 원본 유지 가능값(png/jpg/webp) > HEIC/HEIF는 JPG > 그 외는 PNG.
- 지원 출력은 png/jpg/webp만. 그 외는 `invalid_input`.
- `overwrite=true`는 포맷이 바뀌지 않을 때만 같은 경로에 쓴다. 그 외는 새 파일명으로 쓰며 중복 시 번호를 붙인다. 확장자는 선택 포맷으로 강제한다.
- 회전 범위 오류, 디코드 불가 입력은 원본을 건드리지 않고 실패한다.
- 변경 없음(회전 0 + 반전 없음 + 포맷 유지)은 저장하지 않고 안내한다.
- 덮어쓰기는 확인 다이얼로그 후 실행한다.
- 성공 시 최근 파일에 추가하고 새 경로로 다시 로드한다(회전 상태 초기화 포함).

### 11.4 클립보드 복사 `Ctrl+C`

- 현재 이미지를 PNG로 클립보드에 복사한다.
- 이미지가 없거나 변환 실패 시 에러 안내를 표시한다.

### 11.5 경로/외부 열기

- `Ctrl+Shift+C`: 유효 경로를 텍스트로 복사한다.
- `Ctrl+Shift+E`: 탐색기에 표시한다.
- `Ctrl+Shift+O`: 기본 앱으로 연다.

## 12. 최근 파일/영속화

진실: `src/store/recentFilesStore.ts`, `src/store/archiveProgressStore.ts`, `src/store/settingsStore.ts`.

`settings.json`(Tauri Store) 키:

| 키                | 내용                                                   | 상한     |
| ----------------- | ------------------------------------------------------ | -------- |
| `settings`        | `SettingsState` 전체                                   | 1개 객체 |
| `recentFiles`     | 최근 경로 배열(최신 먼저)                              | 20       |
| `archiveProgress` | 아카이브 경로 → `{ entry, index, total }` 이어보기 기록 | 100      |

- 최근 파일은 중복 제거 후 맨 앞에 넣고 자른다. `recordRecentFiles=false`면 목록을 숨기고 자동 열기도 막는다.
- 최근 파일 카드의 아카이브 항목은 읽기 진도(`index+1/total`)를 함께 표시한다.
- 설정 저장은 손상값도 복원한다. 언어는 저장값이 없으면 시스템 언어를 쓴다.

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
| `resumeReading`     | 아카이브 재진입 시 이어보기                             | `true`                             |
| `fitMode`           | 맞춤 기억 `width \| height \| screen \| auto`           | `auto`                             |
| `dockPosition`      | 이미지 목록 위치 `top \| bottom \| left \| right`       | `bottom`                           |
| `dockVisible`       | 이미지 목록 표시                                        | `true`                             |
| `dockThumbSize`     | 썸네일 크기 `s \| m \| l`                               | `s`                                |
| `dockShowName`      | 썸네일 파일명 표시                                      | `false`                            |
| `dockShowIndex`     | 썸네일 번호 표시                                        | `false`                            |
| `shortcuts`         | 단축키 맵                                               | 아래 기본표                        |
| `wheel`             | 휠 맵                                                   | 아래 기본표                        |
| `mouse`             | 마우스 맵                                               | 아래 기본표                        |

설정 항목에 연결된 단축키가 있으면 항목 옆에 현재 할당된 단축키를 배지로 표시한다. 재할당하거나 해제하면 배지도 즉시 따라간다.

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

- 허용 수식키는 `Ctrl/Shift/Alt`이며 `Meta`와 `Tab`은 쓸 수 없다.
- 빈 값은 바인딩 해제로 취급하고, 중복 바인딩은 검출한다.
- 왼쪽 드래그는 `pan` 또는 `none`만 허용한다.
- 다이얼로그/입력 포커스에서는 뷰어 단축키를 막는다(EXIF 패널의 `I` 닫기 예외 제외).

### 14.4 명령 팔레트 `Ctrl+K`

- 그룹 순서: `file → navigate → view → display → system`.
- 이미지가 있을 때만, 이동 가능할 때만 활성화되는 명령이 있다. `system` 그룹은 항상 활성이다.
- 공백 분리 토큰 AND 매칭이며 라벨 앞부분 일치를 우선한다.
- 한국어 UI에서도 영문 별칭으로 검색된다.

## 15. 백엔드 IPC 계약

진실: `src-tauri/src/lib.rs` `invoke_handler`, `src-tauri/src/commands.rs`, `src-tauri/src/save.rs`, `src-tauri/src/thumb_shell.rs`.

| 명령                              | 입력 (JS camelCase)                    | 반환                                   |
| --------------------------------- | -------------------------------------- | -------------------------------------- |
| `load_image`                      | `filePath`                             | `ImageInfo`                            |
| `get_directory_images`            | `filePath`, `options?`                 | `DirectoryImages`                      |
| `resolve_dropped_path`            | `path`                                 | 해석된 파일 경로 `string`              |
| `get_exif_data`                   | `filePath`                             | `Record<string, string>`               |
| `get_image_histogram`             | `filePath`                             | `Histogram`                            |
| `get_image_details`               | `filePath`                             | `ImageDetails`                         |
| `get_archive_images`              | `filePath`                             | `DirectoryImages`(엔트리 목록)         |
| `load_archive_image`              | `archivePath`, `entryName`             | `ImageInfo`                            |
| `archive_prefetch`                | `archivePath`, `entryNames`            | 추출 개수 `number`                     |
| `generate_thumbnail`              | `filePath`, `maxSide?`                 | `ThumbnailInfo`                        |
| `generate_thumbnails_batch`       | `filePaths`, `maxSide?`                | `BatchThumb[]`                         |
| `generate_archive_thumbnail`      | `archivePath`, `entryName`, `maxSide?` | `ThumbnailInfo`                        |
| `generate_archive_file_thumbnail` | `archivePath`, `maxSide?`              | `ThumbnailInfo`(첫 이미지 엔트리 기준) |
| `get_file_associations`           | 없음                                   | `FileAssociation[]`                    |
| `set_file_association`            | `extension`, `associate`               | `FileAssociation`                      |
| `set_all_file_associations`       | `associate`                            | `FileAssociation[]`                    |
| `open_default_apps_settings`      | 없음                                   | 없음                                   |
| `get_psd_thumbnail_status`        | 없음                                   | `PsdThumbStatus`                       |
| `register_psd_thumbnail`          | 없음                                   | `PsdThumbStatus`                       |
| `unregister_psd_thumbnail`        | 없음                                   | `PsdThumbStatus`                       |
| `trash_file`                      | `filePath`                             | 없음                                   |
| `rename_file`                     | `oldPath`, `newName`                   | `ImageInfo`                            |
| `save_image_edits`                | `filePath`, `options`                  | `ImageInfo`                            |
| `frontend_ready`                  | 없음                                   | 없음 (`PendingOpenFile` flush)         |

인자 변환과 직렬화 규칙은 0절을 따른다. 응답은 snake_case이며 TypeScript 타입과 1:1 대응한다.

파일 연결 주의: 설정에서 연결 변경은 해당 확장자의 Windows 기본 앱 선택 창을 연다. 조용한 UserChoice 레지스트리 쓰기는 할 수 없다.

개발 빌드의 별도 등록(`AraView (Dev)`)은 `docs/development.md`를 따른다.

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
- `ImageDetails`: 16.2 모양 그대로. 색상 모드, 비트/채널, 생성/수정 시각, DPI, ICC 상태를 포함한다.
- `ArchiveState`: `{ archivePath: string | null }`.
- `FileAssociation`: `{ extension, associated, current_prog_id, needs_os_confirmation }`.
- `PsdThumbStatus`: 탐색기 썸네일 등록 상태(20.2절).
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
- 새 백엔드 에러 문구를 추가하면 `src/utils/appError.ts` 매핑표도 함께 갱신한다.
- 표시는 번역 키로 보여준다.

## 18. 렌더링 경로

- 백엔드는 디코드용 파일 경로를 돌려주고, 프론트는 `convertFileSrc(...)`로 변환해 `<img>`에 넣는다.
- HEIC/HEIF/PSD만 JPEG sidecar를 만든다. sidecar와 아카이브 추출물, 썸네일 캐시는 프로세스 임시 디렉터리 아래에 둔다.
- 사용자가 여는 파일/폴더는 명령 실행 시 런타임에 asset scope로 허용한다.
- CSP는 `default-src 'self'` 기반이며 `asset:`/`ipc:` 접근을 허용한다. dev 전용 설정은 `docs/development.md`를 따른다.

## 19. 다국어

진실: `src/i18n/index.ts`, `src/i18n/locales/ko.json`, `src/i18n/locales/en.json`.

- 지원 언어: `ko`, `en`.
- 시스템 감지: 브라우저 언어가 `ko`로 시작하면 `ko`, 아니면 `en`.
- 폴백: `ko`.
- 설정 변경 즉시 적용된다.

## 20. 윈도우/배포

진실: `src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs`, `docs/releasing.md`, `.github/workflows/release.yml`.

- 창: 제목 기본값 `AraView`(이미지가 열리면 파일명, 아니면 앱 이름), 1024x768, 최소 500x400, 프레임리스, 시작 시 숨김. 최소 너비 500은 Windows 11 Snap Layouts의 모든 배치에 창이 들어가기 위한 상한이다(Microsoft 권장 ≤500epx).
- `window-state` 플러그인으로 창 상태를 유지한다.
- Windows 11 Snap Layouts: 커스텀 최대화 버튼(`id=caption-maximize`) 위에 `WM_NCHITTEST`에 `HTMAXBUTTON`으로 응답하는 투명 네이티브 오버레이를 띄운다(`tauri-plugin-snap-layout`, 비-Windows no-op). 오버레이가 마우스를 가로채므로 버튼의 hover 배경/툴팁은 플러그인 이벤트(`tauri-snap://snap/mouseenter|mouseleave`)로 미러링하고, 클릭 최대화/복원은 네이티브가, 키보드(Enter/Space)는 기존 onClick이 담당한다. 헤더가 완전히 가려지는 동안(auto-hide, 메뉴바 숨김; peek 제외) 오버레이를 떼어낸다(`useSnapLayout` 훅).
- 번들: `nsis`만 빌드한다. 결과물은 `src-tauri/target/release/bundle/` 아래에 생성된다.
- 릴리스 파이프라인: 태그(`v*`) 푸시에서만 돌며 검사 실패 시 빌드와 릴리스로 진행하지 않는다. 권한과 절차 상세는 `docs/releasing.md`를 따른다.
- 파일 연결 3그룹:
  - Image 14종: png, jpg, jpeg, gif, bmp, webp, svg, ico, tiff, tif, avif, heic, heif, psd.
  - Comic 4종: cbz, cb7, cbr, cbt.
  - Archive 3종: rar, zip, 7z.
- HEIC/HEIF는 vcpkg `libheif[core]` 동적 링크 + `libde265`만 사용한다. 설치와 DLL 복사는 `docs/development.md`를 따른다.

### 20.1 자동 업데이트 (tauri-plugin-updater)

수동 확인만 제공한다. 시작 시 자동 확인이나 백그라운드 폴링은 없다(오프라인 우선).

- 진입점: 설정 일반 탭의 `지금 확인` 버튼, 명령 팔레트의 `업데이트 확인`.
- 흐름: 업데이트 확인 → 없으면 최신 안내, 있으면 다운로드 및 설치 → 완료 시 다시 시작. 중복 확인은 무시한다.
- 설정: updater 아티팩트 생성과 릴리스 피드 endpoint, 서명 공개키를 둔다. 소스 저장소가 비공개라 당분간 공개 저장소 피드를 쓰는 임시 구성이다.
- 키 발급, 서명 빌드, 로컬 릴리스 절차는 `docs/releasing.md`를 따른다.

### 20.2 PSD 탐색기 썸네일 (IThumbnailProvider)

Windows 파일 탐색기에서 `.psd` 축소판을 표시한다. 미리보기 창(`Alt+P`)은 범위 밖이며 썸네일만 제공한다.

- 설정 UI(확장자 탭 하단)에서 켜고 끌 수 있다. DLL이 없으면 등록을 거부하고 안내한다. 적용 뒤 탐색기 재시작/썸네일 캐시 정리가 필요할 수 있다.
- 등록은 HKCU라 관리자 권한이 필요 없다. PSB는 거부하고 탐색기가 기본 아이콘으로 폴백한다.
- 고무결성 Explorer(Windows Sandbox 등)에서는 HKCU COM이 무시될 수 있다. 삭제 시 키가 남을 수 있으며 재설치 후 다시 켜면 복구된다.
- CLSID, 레지스트리 슬롯, DLL 스테이징 같은 구현 상세는 `docs/development.md`를 따른다.

## 21. 비목표와 제약

- 클라우드, 공유, 라이브러리 가져오기는 범위 밖이다(`PRODUCT.md`).
- 근거 없는 브랜드/수치/후기 주장을 UI/문서에 넣지 않는다(`PRODUCT.md`, `DESIGN.md`).
- 빈 상태는 가짜 샘플 대신 실제 다음 행동을 안내한다.
- 파일 연결은 OS 확인 없이 바꿀 수 없다.

## 22. 변경 시 동기화 체크리스트

작업 절차는 `AGENTS.md`를 따른다. 변경 후 아래 SPEC 절을 갱신한다.

- 포맷 추가: 2절 + 필요 시 15/16절.
- 백엔드 명령 추가: 15절 IPC 표 + 16절 데이터 모델.
- 설정/단축키 추가: 12~14절.
