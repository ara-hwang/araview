# SPEC.md - AraView 기능/기술 명세

> 한국어 스펙 문서. 구현 진실(source of truth)은 코드이며, 본 문서는 현재 코드베이스의 동작을 요약한다.
> 관련 문서: `PRODUCT.md`(제품 정의), `DESIGN.md`(비주얼 시스템), `README.md`(소개/문서 허브), `docs/usage.md`(사용법), `docs/development.md`(개발 안내), `docs/releasing.md`(릴리스/업데이트), `AGENTS.md`(AI 작업 지침).

## 0. 문서 규약

- 경로 별칭 `@/...`는 `src/` 기준, 백엔드 경로는 `src-tauri/src/` 기준이다.
- `invoke()` 인자 변환(최상위 camelCase, 중첩 옵션 camelCase, 응답 snake_case) 상세는 15~16절을 따른다.
- 에러는 구조화 에러 `{ code, message }`를 우선한다.
- base64 이미지 페이로드는 사용하지 않는다. 렌더링은 항상 파일 경로 기반이다.

## 1. 개요

- Windows 10/11 x64 전용 오프라인 데스크톱 이미지/코믹 뷰어. 로컬 파일만 다루며 라이브러리 가져오기, 계정, 네트워크를 쓰지 않는다(수동 업데이트 확인 제외).
- 창은 프레임리스(`decorations: false`)이며 커스텀 타이틀바/툴바(`src/components/Header.tsx`)를 쓴다. Windows 11에서는 최대화 버튼 호버로 OS Snap Layouts 플라이아웃이 뜬다(§20).
- 다이얼로그/시트가 열려 있어도 타이틀바(최소화/최대화/닫기)는 계속 동작한다. Dialog/Sheet는 `modal="trap-focus"`로 포커스만 가두고, 오버레이는 헤더 아래(`--header-height`)에서 시작한다. Base UI의 투명 전체화면 백드롭(`modal=true`일 때만 렌더)이 창 제어를 가로채는 것을 막기 위한 선택이다. dimmed 영역 클릭과 `Esc`로 닫히고, 키보드 포커스는 다이얼로그 안에 갇힌다.
- 기술 스택과 플러그인 목록은 `docs/development.md`와 `AGENTS.md`를 따른다.

## 2. 지원 포맷

총 19개 확장자. 프론트 진실은 `src/constants/imageExtensions.ts`, 백엔드 진실은 `src-tauri/src/image.rs` (`SUPPORTED_EXTENSIONS`, `get_mime_type`).

### 2.1 순수 이미지 12종

| 확장자        | MIME                        | 비고                                                                                                                    |
| ------------- | --------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `png`         | `image/png`                 | 네이티브 렌더                                                                                                           |
| `jpg`, `jpeg` | `image/jpeg`                | 네이티브 렌더. EXIF Orientation(1~8)을 치수·썸네일·편집 저장에 반영(WebView2 표시와 일치)                               |
| `gif`         | `image/gif`                 | 네이티브 렌더. 단일 보기에서 재생/정지·프레임 이동 지원(WebCodecs `ImageDecoder`, 미지원 시 네이티브 애니메이션)        |
| `bmp`         | `image/bmp`                 | 네이티브 렌더                                                                                                           |
| `webp`        | `image/webp`                | 네이티브 렌더                                                                                                           |
| `svg`         | `image/svg+xml`             | 네이티브 렌더. 치수는 `<svg>` 헤더(width/height/viewBox, 절대 단위) 파싱으로 복원하며 해석 불가분만 `width/height` 생략 |
| `ico`         | `image/x-icon`              | 네이티브 렌더                                                                                                           |
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

CBZ/ZIP의 `ComicInfo.xml`은 읽기 전용 메타데이터로만 지원한다(8절, 15절). CB7/CBR/CBT는 리더가 달라 읽지 않는다.

백엔드 판별:

- `is_image_file`: MIME이 `application/`으로 시작하지 않는 지원 파일.
- `is_archive_file`: 위 7종 MIME 해당.
- `is_supported_file`: 둘 중 하나.

### 2.3 제외 포맷

- QOI, JXL, RAW(CR2/NEF/ARW 등), PSB는 제외 유지.
  - QOI: 백엔드(`image` 크레이트) 디코드는 가능하지만 WebView2가 네이티브 렌더를 못 해 JPEG sidecar 전제가 필요하다.
  - JXL(`jxl-oxide`), RAW(`rawloader`): 디코더 크레이트는 있으나 같은 이유로 sidecar 파이프라인이 선행돼야 한다.
  - PSB: 실제 PSB는 `8BPS` + version 2인데 디코더가 없어 진입 차단한다(`psd` 크레이트는 PSD만 지원).
- PSD는 읽기 전용 미리보기(JPEG sidecar)만 지원하며 편집 저장은 불가하다.

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
- 그리드: 뷰포트 기반 가상화, 클릭/`Enter`로 점프 후 닫기, `Esc`/`G`로 닫기, 파일명 필터, 실패 셀 배지와 재시도. 그리드가 열려 있는 동안 뷰어 단축키는 비활성이다. 셀은 `aria-current`로 현재 페이지를, `aria-selected`로 키보드 선택(링)을 따로 표시한다.
- 우클릭은 설정(`mouse.rightClick`)에 따라 컨텍스트 메뉴 또는 다른 동작이다. 홈에서는 우클릭을 막는다.
- `Esc` 닫기: 이름 변경/저장 다이얼로그가 열려 있거나 입력 포커스 중이면 닫지 않는다.
- 이미지 표시 설정은 `auto | smooth | pixelated`를 제공한다. `auto`는 픽셀 아트 자동 감지가 켜져 있고 분석 결과가 `pixel_art`로 분류되면 `pixelated`, 그 외에는 `smooth`를 사용한다. 수동 모드는 자동 감지보다 우선한다.
- 픽셀 아트 자동 감지는 현재 이미지를 렌더한 뒤 별도 백그라운드 IPC로 분석한다. 분석 전/실패/불확실은 `smooth`로 대체하며 원본과 파생 이미지는 변경하지 않는다. 감지는 동시 2개로 제한하고 웹툰에서는 화면에서 벗어난 대기 요청을 취소한다.
- 단일 이미지의 픽셀 보존 모드는 transform 확대 대신 레이아웃 크기 확대를 사용해 Chromium 합성 단계의 재보간을 줄인다. GIF Canvas는 `imageSmoothingEnabled=false`를 사용한다. 양쪽/웹툰의 각 이미지는 같은 설정을 공유하되 판정은 이미지별이다.
- `autoHideUI`가 true일 때만 읽기 중 크롬(상단바, 이미지 목록 도크, 상태바)을 숨기고 읽기 영역을 확장한다. 도크 상태(스크롤 위치, 로드한 썸네일)는 유지된다.
- `menuBarHidden`이 true이면 상단바를 숨기고, 상단 호버 영역에서 peek 오버레이로 표시한다. 헤더 숨기기 버튼과 보기 설정 스위치로 토글한다. peek 시에는 읽기 영역 위로 겹쳐 내려오며, 포커스 이탈 시에는 즉시 닫힌다. 모션 토큰과 timing은 `DESIGN.md`를 따른다. `Esc`로는 닫히지 않는다(뷰어의 이미지 닫기와 충돌 방지).
- `menuBarHidden`은 이미지를 보고 있을 때만 적용한다. 이미지 없이 빈 화면(홈)에서는 설정값을 유지한 채 상단바를 항상 표시한다(열기/드래그 동선 유지). `autoHideUI`와 동일한 조건을 쓴다.
- 로드 실패 시 에러 카드에 재시도/홈 복구 경로를 제공한다. 실패한 적은 토스트로 알린다.

### 3.3 상태 규칙

- `PRODUCT.md` 원칙과 `DESIGN.md` 시스템을 따른다: 모든 데이터 뷰는 empty/loading/error 상태, 모든 컨트롤은 키보드 조작과 포커스 표시, UI 카피 금칙(문서 참조)을 지킨다.
- 고대비: OS 설정(`prefers-contrast: more`, Windows 대비 테마의 `forced-colors: active`)에 반응한다. 별도 토글은 두지 않으며, 강제 색상 모드에서도 시스템 Highlight 외곽선으로 포커스를 표시한다.
- 체감 로딩: 큰 이미지는 캐시된 저해상 썸네일을 먼저 깔고 원본이 디코드되면 페이드 인한다(9.2절). 모션 최소화 설정에서는 페이드를 끈다.

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
- `showCoverAlone`이 true인 양쪽 모드에서는 화면이 `[0], [1,2], [3,4], ...`가 되고 넘김도 화면 단위다. false면 `[0,1], [2,3], ...`다.
- `loopNavigation=false`: 끝에서 멈춘다. 단, 마지막 장은 clamp로 볼 수 있게 한다.
- `loopNavigation=true`: wrap한다.
- 아카이브/일반 모드에 맞는 로더로 인덱스를 갱신한다.
- 목록이 1개 이하면 이전/다음을 수행하지 않는다.

### 5.4 점프

- `PageUp/PageDown`: 10장 점프.
- `Home/End`: 처음/마지막.
- 루프 설정에 따라 wrap/clamp한다.
- 양쪽 모드에서 임의 인덱스로 점프하면 쌍 시작 인덱스로 스냅한다(예: 3번으로 점프하면 `[2,3]` 또는 표지 단독 모드에서 `[3,4]`). 두 화면이 페이지를 겹쳐 보여주지 않게 한다.
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
| `left-to-right` | 현재 + 다음, 좌에서 우              | 2장(화면)   | 기본 + 1      |
| `right-to-left` | 현재 + 다음, 우에서 좌              | 2장(화면)   | 기본 + 1      |
| `webtoon`       | 전 구간 연속 수직 스크롤, 지연 로드 | 스크롤 이동 | 기본 x 2      |

- 양쪽 모드 페이지는 `[current, next]`이며 루프가 켜지면 wrap한다. 로드 실패 페이지는 제외한다.
- 표지 단독(`showCoverAlone`, 기본 true): 표지를 혼자 보여주고 그 뒤부터 `[표지+1, 표지+2]` 쌍을 맞춘다. 표지 인덱스는 CBZ/ZIP ComicInfo의 `FrontCover`(8절)를 쓰고, 메타데이터가 없거나 범위를 벗어나면 0번이다. 표지가 0번이 아니면 표지 바로 앞에 남는 페이지도 단독 화면이 된다. 표지 화면에서는 다음 페이지를 로드하지 않는다. 마지막에 남은 한 장은 기존 단일 중앙 렌더를 재사용한다.
- 표지 단독을 끄면 표지 인덱스를 무시하고 `[0,1], [2,3], ...`로 넘긴다.
- 양쪽 모드의 점프(썸네일/도크/`Home`/`End`/`PageUp`/`PageDown`)와 아카이브 이어보기 진입은 쌍 시작으로 스냅한다(`src/utils/dirNavigation.ts`).
- 도크와 썸네일 그리드는 화면에 떠 있는 페이지를 모두 현재로 표시한다(`useCurrentPageIndices`). 양쪽 모드는 쌍 두 장이 함께 하이라이트되고, 단독 화면(표지, 표지 바로 앞 페이지, 마지막 홀수 장)이나 단일/웹툰 모드는 현재 장만 하이라이트한다. 쌍의 기준 장은 도크의 `data-dock-current`로 한 개만 표시해 그리드 닫기 시 포커스 복귀 지점을 유지한다. 계산은 `dualPageIndices`가 뷰어 로드 대상과 같은 목록을 돌려준다.
- 웹툰 모드에서 `ArrowLeft/ArrowRight`는 이전/다음 이미지 스크롤 이동이다.
- 웹툰 모드에서 `ArrowUp/ArrowDown`은 연속 스크롤 컨테이너를 일정량씩 스크롤한다.
- 웹툰 이미지 사이 간격(`webtoonImageGap`, 기본 8px)과 페이지 경계선(`webtoonPageBoundaries`)을 설정한다.
- `webtoonFitWidth`를 켜면 작은 이미지도 읽기 영역 너비까지 확대하고, 끄면 원본 크기를 유지한 채 너비만 제한한다.
- `webtoonShowProgress`를 켜면 읽기 영역에 현재 장 번호, 전체 장 수, 스크롤 진행률을 작은 표시로 보여준다.
- `webtoonThumbnailJump`를 켜면 읽기 영역의 썸네인 버튼으로 그리드를 열어 원하는 장으로 바로 이동할 수 있다. 현재 위치 표시를 꺼도 이 버튼은 유지된다.
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

### 7.5 GIF 재생 제어

- 단일 보기에서 GIF는 캔버스로 그리며 재생/정지(`P`), 프레임 이동(`,`/`.`), 프레임 카운터를 제공한다. WebCodecs `ImageDecoder` 미지원 환경이나 정지 GIF, 양쪽/웹툰 보기에서는 네이티브 `<img>` 애니메이션으로 폴백한다.
- 재생은 이미지 진입 시 자동 시작하며, OS 모션 최소화(`prefers-reduced-motion: reduce`)면 정지 상태로 시작한다.
- 단일/다중 프레임 판정과 반복 횟수는 디코더 트랙 정보를 따르고, 유한 반복 GIF는 마지막 회차에서 멈춘다.
- 헤더 컨트롤은 좁은 창(1024px 미만)에서 숨는다. 단축키와 명령 팔레트는 항상 쓸 수 있다.

## 8. 아카이브

진실: `src-tauri/src/archive.rs`, `src-tauri/src/archive_index.rs`, `src-tauri/src/commands.rs`, `src/hooks/useImageLoader.ts`, `src/store/archiveProgressStore.ts`.

- 아카이브 커맨드(`get_archive_images`, `load_archive_image`, `archive_prefetch`, `get_comic_info`, `generate_archive_thumbnail`, `generate_archive_file_thumbnail(s)_batch`)는 모두 비동기 커맨드로 `spawn_blocking`에서 실행되어 메인 스레드를 막지 않는다. `archive_prefetch`는 fire-and-forget 성격에 맞게 조인 실패도 흡수해 `Ok(0)`을 반환한다.
- 엔트리 인덱스 캐시(`archive_index.rs`): 아카이브별 이미지/전체 엔트리 목록을 canonical 경로 + mtime + size 검증으로 캐시한다(최대 64개 LRU). `get_archive_images`, `get_comic_info`, `generate_archive_file_thumbnail(s)_batch`가 공유해 아카이브당 전체 스캔이 1회로 수렴한다. `clear_cache` 시 함께 비워진다.
- `get_archive_images`: 내부 이미지 엔트리 목록 + `current_index: 0`. 비어 있으면 `not_found`.
- `load_archive_image`: 활성 파생 이미지 캐시 루트의 `archives/` 아래에 추출 후 표시 가능한 경로로 반환한다.
- `archive_prefetch`: 이웃 선추출용 fire-and-forget 명령이다.
- 선추출 거리는 뷰 모드에 따라 보정되며 상한이 있다.
- 이어보기: 아카이브 경로별 마지막 엔트리와 위치(엔트리명/인덱스/전체 페이지)를 최대 100개 LRU로 저장한다. `resumeReading`이 true이고 목록에 저장된 항목이 있으면 거기서 시작하며, 저장 위치에서 시작할 때 "이어보기" 토스트와 "처음부터" 동작을 함께 제공한다. 설정이 false면 항상 첫 페이지에서 열고 열기만으로 저장 위치를 0페이지로 덮지 않는다. 양쪽 모드에서는 저장 위치가 쌍 중간이면 쌍 시작으로 맞춰 연다.
- `get_comic_info`: CBZ/ZIP의 `ComicInfo.xml`(ComicRack/Komga/Kavita 스키마)을 읽기 전용 메타데이터로 반환한다. 탐색은 엔트리 basename이 `comicinfo.xml`인 항목(대소문자 무시)이며 루트를 우선하고 없으면 첫 중첩 경로를 쓴다. 상한 1 MiB, UTF-8(BOM 허용)과 UTF-16 LE/BE BOM을 지원한다. XML 부재나 CBZ/ZIP 이외 확장자는 `null`, 깨진 XML/디코딩 실패는 `corrupt`, 크기 초과는 `too_large`다. 필드 누락과 빈 값은 `null`로 정규화하고, `pages`는 `image` 오름차순으로 최대 1000개까지 담는다.
- 표지 판정: `pages`에서 `page_type`이 `FrontCover`(대소문자·공백 무시)인 첫 페이지를 양쪽 보기 표지 인덱스로 쓴다(`src/utils/comicCover.ts`). `image`가 목록 범위를 벗어나면(1 기반으로 적은 파일 등) 0번으로 폴백하고, 이어보기 진입도 표지 기준 쌍 시작으로 스냅한다.
- 열기 흐름: 아카이브를 열 때 `get_archive_images`와 `get_comic_info`를 병행 호출한다. 폴더 미리보기에서는 메타데이터를 읽지 않고, 이전 로드의 응답은 최신 로드 토큰이 아니면 커밋하지 않는다. 파싱 실패는 로드를 막지 않고 패널의 Comic 섹션에 에러로 표시한다.
- 아카이브 모드 제한: 휴지통 이동, 이름 변경, 편집 저장은 안내와 함께 차단된다.
- 추출 가드: 엔트리 1개당 200MB, solid 7z 블록 총량 2GB, 아카이브별 추출 디렉터리 1GB를 넘으면 `too_large`로 중단한다. RAR은 스트리밍 디코드에 bounded writer를 붙여 선언 크기를 위조한 헤더도 실제 할당 전에 막는다.
- 목록은 추출과 일치하도록 중복 엔트리 이름을 1회만 노출한다(zip `by_name`은 첫 항목만 돌려준다).
- 표시용 추출 경로는 추출 전에 `mark_in_use`로 보호하고, 선추출(prefetch)은 보호 슬롯을 소비하지 않는다. `load_archive_image`의 `protect`는 기본 `true`이며 프론트 선로딩은 `false`를 전달한다.
- 탐색/썸네일은 엔트리 목록 기준으로 동일하게 동작한다.

## 9. 캐시/썸네일/프리페치

진실: `src/utils/cacheConfig.ts`, `src/hooks/useImageCache.ts`, `src/store/cacheInvalidationStore.ts`, `src-tauri/src/process_temp.rs`, `src-tauri/src/cache.rs`, `src-tauri/src/thumbnail.rs`.

### 9.1 이미지 캐시 모드

기본 `nearby`. 이 설정은 이미지 미리 로드 범위와 프론트 메타데이터 캐시의 항목 한도를 설정한다.

| 모드         | 항목 상한 | 바이트 상한 | 프리페치 거리 |
| ------------ | --------- | ----------- | ------------- |
| `off`        | 1         | 무제한      | 0             |
| `nearby`     | 24        | 무제한      | 1             |
| `extended`   | 64        | 무제한      | 3             |
| `memory-1gb` | 매우 큼   | 1GB         | 2             |
| `memory-2gb` | 매우 큼   | 2GB         | 3             |

- 일반 파일 프리페치 거리 보정: 웹툰은 기본 x 2, 양쪽은 기본 + 1, single은 기본.
- 캐시 키는 파일 경로 기준이며, 픽셀 데이터를 메모리에 오래 두지 않고 브라우저 이미지 캐시에 위임한다.
- `memory-*`는 실제 프로세스 RAM 전체 사용량이 아니라 프론트 이미지 메타데이터 예산이다.

### 9.2 저장 방식

- `cacheStorageMode`는 `temporary | persistent`이며 기본값은 `persistent`다.
- `persistent`는 Tauri `app.path().app_cache_dir()` 아래 `cache-v2/`을 사용하며 앱 재실행 뒤에도 썸네일, sidecar, 축소본, 아카이브 추출물을 재사용한다.
- `temporary`는 사용자 앱 캐시 디렉터리의 버전된 `session-v1/` 아래 프로세스별 `TempDir`를 사용한다. 정상 종료 이벤트와 다음 시작 시 이전 세션 루트를 정리해 종료·충돌 후에도 임시 파일을 남기지 않는다. 저장 방식 변경은 다음 실행부터 적용된다.
- 영구 캐시는 OS가 지울 수 있는 best-effort 데이터다. 원본이나 렌더 결과의 진실 원본은 아니다.
- 일반 빌드와 개발 빌드는 Tauri identifier가 달라 캐시 루트가 분리된다.
- 시작 시 이전 `cache-v*` 버전 디렉터리와 이전 프로세스의 orphan temp 파일을 정리한다.

### 9.3 썸네일

- `generate_thumbnail`: 기본 256px, JPEG 캐시 후 재사용한다. 상한(500MB)을 넘기면 오래된 것부터 제거한다.
- HEIC/HEIF/PSD는 썸네일용 JPEG sidecar 경로를 쓴다.
- 디코드 불가 입력(SVG 등)이나 아카이브 엔트리명은 에러를 내고, 프론트는 원본으로 폴백한다.
- 배치 조회와 아카이브 엔트리용 썸네일 API를 별도로 제공한다. 아카이브 썸네일은 추출물과 캐시를 재사용해 풀사이즈 로드를 피한다.
- 그리드는 보이는 창의 경로만 썸네일을 요청한다.
- `get_cached_thumbnail`: 생성 없이 캐시에 있는 썸네일(256/128/96/72/48/32)만 반환한다. 큰 이미지(2MP 또는 1.5MB 이상)를 열 때 풀사이즈 디코드와 병행 조회해 첫 페인트 프리뷰로 쓰고, 원본 `onLoad`에서 걷는다. GIF는 제외한다.
- 생성된 썸네일·sidecar·축소본 캐시 hit은 파일 modification time을 갱신해 이후 LRU 정리에서 최근 사용 파일이 먼저 삭제되지 않게 한다.

### 9.4 표시 해상도 제한

진실: `src-tauri/src/scaled.rs`, `src/hooks/useImageCache.ts`, `src/hooks/useImageLoader.ts`, `src/utils/resolutionLimit.ts`.

- 설정 `maxResolution`(기본 `original`)이 켜져 있으면 긴 변이 상한(4k=3840px, 1080p=1920px)을 넘는 래스터를 `scaled/` sidecar로 한 번만 축소해 렌더한다. 원본 파일은 바뀌지 않는다.
- 적용 대상은 `image` 크레이트가 디코드할 수 있는 래스터(PNG/JPEG/BMP/ICO/정지 WebP)이며, EXIF Orientation(5~8)은 픽셀에 반영한 뒤 축소한다. 알파 채널이 있으면 투명도 보존을 위해 PNG로 저장한다.
- GIF, 움직이는 WebP, SVG, 디코드 불가 포맷(AVIF 등)은 원본 바이트를 그대로 렌더한다.
- sidecar는 활성 캐시 루트의 `scaled/`에 캐시되며 상한(500MB)을 넘기면 오래된 것부터 제거한다. 캐시 경로는 원본 식별 해시 + 상한 + 픽셀 아트 필터 알고리즘 revision 기준이라 설정이나 필터 알고리즘이 바뀌면 다른 사본을 만든다.
- sidecar 축소는 표시 정책과 무관하게 항상 보간 필터(`Triangle`)를 사용한다. Nearest 축소는 스크린톤 같은 주기 패턴을 계단·무아레로 깨뜨린다. `smooth`/`auto`는 JPEG로, `pixelated`는 lossless PNG로 저장한다. `auto`는 휴리스틱 분석을 실행해 결과를 후속 `detect_pixel_art` IPC가 재사용하지만, 분류가 `pixel_art`여도 축소 필터에는 영향을 주지 않는다(픽셀 보존 표시는 확대 배율에서만 적용되기 때문). 분석은 원본을 바꾸지 않는다.
- 새 상한으로 로드한 `ImageInfo.width/height`는 축소 사본 기준이고, `source_path`는 항상 원본 파일이다(11절). 히스토그램은 렌더 바이트(`file_path`), EXIF/파일 상세는 원본(`source_path`) 기준이다.
- 해상도 상한이나 이미지 표시 정책을 바꾸면 프론트는 메타 캐시와 픽셀 예열을 비우고 현재 이미지를 새 기준으로 다시 로드한다.

### 9.5 캐시 관리 화면

- 설정 > 성능 탭에서 현재 활성 저장 방식, 전체 사용량, 종류별 사용량, 파일 수, 보호된 파일 수를 확인한다.
- `get_cache_stats`는 썸네일, 변환 이미지, 축소본, 아카이브 추출물, 기타 임시 파일을 분류한다.
- `clear_cache(scope)`는 `all`, `thumbnails`, `converted`, `scaled`, `archives`, `other`를 지원한다.
- 현재 WebView에 전달되어 사용 중인 파일과 `mark_in_use` 보호 파일은 삭제하지 않는다. 보호된 용량은 결과 통계에 남는다.
- 진행 중인 `*.tmp-<pid>-...` 파일은 통계와 삭제 대상에서 제외한다.
- 삭제 성공 후 프론트는 이미지 메타데이터, 픽셀 preload, 썸네일 URL을 무효화한다. 다음 탐색에서 필요한 캐시를 다시 만든다.

## 10. EXIF/파일 정보

진실: `src-tauri/src/commands.rs`, `src-tauri/src/image_info.rs`, `src/hooks/useExifLoader.ts`, `src/components/ExifPanel.tsx`, `src/components/HistogramChart.tsx`.

- `get_exif_data`는 문자열 맵을 반환한다.
- 파일 없음이면 `not_found`, EXIF 없으면 `unsupported`.
- `I`로 패널 토글. 패널 내부 포커스에서는 `I` 닫기를 허용한다.
- 표시 범주는 Camera, Exposure, Image, Lens, DateTime, GPS, Software 계열이다.
- HEIC는 원본 경로 기준 EXIF를 읽으므로 비어 있는 경우가 많다.
- JPEG의 Orientation은 표시·썸네일·저장에 반영한다. 패널에는 EXIF 원문 설명을 그대로 보여준다.
- Comic 섹션: 아카이브 모드이고 `showComicInfo`가 켜져 있고(기본 켜짐, 설정 보기 탭 읽기) `comicInfo`가 있으면 파일/히스토그램/EXIF보다 위에 표시한다. 설정을 끄면 읽은 메타데이터는 유지한 채 섹션만 숨긴다(표지 판정은 계속 동작한다, 6절). 첫 줄은 `Series #Number`(있는 것만), 둘째 줄은 `Title`, 이어서 Writer, Penciller, Publisher, Genre, Tags, Volume, Count, PageCount, LanguageISO, AgeRating, CommunityRating, 마지막에 Summary를 줄바꿈 그대로 표시한다. 값이 없는 필드는 행을 만들지 않는다. 파싱 실패면 재시도 버튼 없이 에러 문구만 남기고(복구 수단은 아카이브 다시 열기), 메타데이터가 없으면 섹션을 숨긴다.

### 10.1 히스토그램 `get_image_histogram`

- 입력 `file_path`, 반환 `Histogram`(16절).
- 디코드 불가 포맷(SVG, AVIF 등)은 에러를 내고, 프론트는 차트 대신 안내 문구를 표시한다.
- 렌더는 신규 의존성 없이 SVG 영역 차트이다.

### 10.2 파일 상세 `get_image_details`

- 입력 `file_path`, 반환 `ImageDetails`(16절).
- EXIF가 없어도 명령은 성공한다. 디코드 실패 파일도 상세를 반환한다.
- 색상 모드는 픽셀을 전부 디코드하지 않고 디코더의 `color_type()`으로 판정한다.
- ICC 검사는 jpg/png만 한다. JPEG은 선두 1MiB(+최대 세그먼트 길이) prefix만, PNG는 청크를 스트리밍으로 걸어 iCCP 데이터만 읽는다(전체 파일 로드 없음).
- 패널 표시: 경로(아카이브 모드면 아카이브 경로와 엔트리명 함께), 크기, 치수 + 픽셀 수, 생성/수정 시각(아카이브 모드 숨김), 색상 + 비트/채널, DPI, 색상 프로파일.
- 조회 실패가 있어도 패널은 열리고 섹션별 안내 문구를 표시한다.

## 11. 파일 작업

진실: `src/hooks/useFileOperations.ts`, `src-tauri/src/commands.rs`, `src-tauri/src/save.rs`, `src/hooks/useCopyImage.ts`.

공통: 아카이브 모드(전체/미리보기)면 원본 아카이브 경로를 대상으로 삼는다. 단, 휴지통/이름 변경/편집 저장은 아카이브에서 차단된다. 파일 작업은 렌더 경로(`file_path`)가 아니라 사용자가 연 원본(`source_path`)을 대상으로 한다(9.4절).

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
- 새 파일명은 `create_new`로 원자적으로 예약한다. 인코딩 중 다른 저장이 같은 이름을 만들면 다음 번호로 넘어가며, 실패 시 예약한 빈 파일을 지운다.
- JPEG→JPEG 저장은 원본의 APP1(EXIF/XMP), APP2(ICC), APP13(IPTC), COM 세그먼트를 이식한다. 회전은 픽셀에 반영되므로 EXIF Orientation은 1로, PixelXDimension/PixelYDimension은 새 크기로 in-place 패치한다(구조가 예상 밖이면 손대지 않음). 메타데이터 총량이 16MB를 넘거나 SOS를 찾지 못하면 이식하지 않는다.
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

| 키                | 내용                                                    | 상한     |
| ----------------- | ------------------------------------------------------- | -------- |
| `settings`        | `SettingsState` 전체                                    | 1개 객체 |
| `recentFiles`     | 최근 경로 배열(최신 먼저)                               | 20       |
| `archiveProgress` | 아카이브 경로 → `{ entry, index, total }` 이어보기 기록 | 100      |

- 최근 파일은 중복 제거 후 맨 앞에 넣고 자른다. `recordRecentFiles=false`면 목록을 숨기고 자동 열기도 막는다.
- 최근 파일 카드의 아카이브 항목은 읽기 진도(`index+1/total`)를 함께 표시한다.
- `cacheStorageMode`는 `temporary | persistent` 중 하나를 저장하며 기본값은 `persistent`다. 값이 손상됐거나 없으면 `persistent`로 복원한다.
- 설정 저장은 손상값도 복원한다. 언어는 저장값이 없으면 시스템 언어를 쓴다.
- 저장 방식 변경은 현재 프로세스의 캐시 루트를 즉시 바꾸지 않고 다음 실행부터 적용한다. 영구 캐시를 끄는 경우 다음 시작 시 이전 `cache-v*`을 정리한다.

## 13. 설정

진실: `src/store/settingsStore.ts`.

| 설정                    | 값                                                      | 기본값                             |
| ----------------------- | ------------------------------------------------------- | ---------------------------------- |
| `language`              | `ko \| en`                                              | `ko`(초기 로드는 시스템 감지 우선) |
| `loopNavigation`        | 끝에서 루프 여부                                        | `false`                            |
| `cacheMode`             | `off \| nearby \| extended \| memory-1gb \| memory-2gb` | `nearby`                           |
| `cacheStorageMode`      | `temporary \| persistent`                               | `persistent`                       |
| `maxResolution`         | `original \| 4k \| 1080p` (긴 변 상한, 9.4절)           | `original`                         |
| `imageScalingMode`      | `auto \| smooth \| pixelated` 이미지 보간 방식          | `auto`                             |
| `autoDetectPixelArt`    | 자동 모드에서 픽셀 아트 감지 사용                       | `true`                             |
| `viewMode`              | `single \| left-to-right \| right-to-left \| webtoon`   | `single`                           |
| `webtoonImageGap`       | 웹툰 이미지 사이 간격(px, 0~64)                         | `8`                                |
| `webtoonPageBoundaries` | 웹툰 페이지 경계선 표시                                 | `false`                            |
| `webtoonFitWidth`       | 웹툰 이미지를 읽기 영역 너비까지 확대                   | `false`                            |
| `webtoonShowProgress`   | 웹툰 현재 장 번호와 스크롤 진행률 표시                  | `true`                             |
| `webtoonThumbnailJump`  | 진행 표시에서 썸네인 그리드로 바로가기                  | `true`                             |
| `autoOpenLastFile`      | 시작 시 마지막 파일 자동 열기                           | `false`                            |
| `recordRecentFiles`     | 최근 기록 유지                                          | `true`                             |
| `viewerBackground`      | `theme \| black \| white \| checker`                    | `theme`                            |
| `autoHideUI`            | 읽기 중 크롬 자동 숨김                                  | `false`                            |
| `menuBarHidden`         | 상단바 수동 숨김 (이미지 보기 중만, 상단 호버 시 peek)  | `false`                            |
| `alwaysOnTop`           | 항상 위                                                 | `false`                            |
| `sortKey`               | `name \| date \| size`                                  | `name`                             |
| `sortDescending`        | 내림차순                                                | `false`                            |
| `includeSubfolders`     | 하위 폴더 포함(재귀)                                    | `false`                            |
| `skipBrokenFiles`       | 손상 파일 자동 건너뛰기                                 | `false`                            |
| `resumeReading`         | 아카이브 재진입 시 이어보기                             | `true`                             |
| `showCoverAlone`        | 양쪽 보기에서 첫 페이지(표지)를 단독 표시               | `true`                             |
| `showComicInfo`         | 정보 패널에 만화 정보(ComicInfo.xml) 섹션 표시          | `true`                             |
| `fitMode`               | 맞춤 기억 `width \| height \| screen \| auto`           | `auto`                             |
| `dockPosition`          | 이미지 목록 위치 `top \| bottom \| left \| right`       | `bottom`                           |
| `dockVisible`           | 이미지 목록 표시                                        | `true`                             |
| `dockThumbSize`         | 썸네일 크기 `s \| m \| l`                               | `s`                                |
| `dockShowName`          | 썸네일 파일명 표시                                      | `false`                            |
| `dockShowIndex`         | 썸네일 번호 표시                                        | `false`                            |
| `shortcuts`             | 단축키 맵                                               | 아래 기본표                        |
| `wheel`                 | 휠 맵                                                   | 아래 기본표                        |
| `mouse`                 | 마우스 맵                                               | 아래 기본표                        |

설정 항목에 연결된 단축키가 있으면 항목 옆에 현재 할당된 단축키를 배지로 표시한다. 재할당하거나 해제하면 배지도 즉시 따라간다.

## 14. 단축키/휠/마우스/명령 팔레트

진실: `src/constants/shortcuts.ts`, `src/constants/commands.ts`, `src/hooks/useImageViewerHotkeys.ts`.

### 14.1 기본 단축키

| 동작             | 기본값            |
| ---------------- | ----------------- |
| 이전             | `ArrowLeft`       |
| 다음             | `ArrowRight`      |
| 왼쪽 팬          | `Ctrl+ArrowLeft`  |
| 오른쪽 팬        | `Ctrl+ArrowRight` |
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
| GIF 재생/정지    | `P`               |
| 이전 GIF 프레임  | `,`               |
| 다음 GIF 프레임  | `.`               |

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
- 이미지가 있을 때만, 이동 가능할 때만, 제어 가능한 GIF일 때만 활성화되는 명령이 있다. `system` 그룹은 항상 활성이다.
- 공백 분리 토큰 AND 매칭이며 라벨 앞부분 일치를 우선한다.
- 한국어 UI에서도 영문 별칭으로 검색된다.

## 15. 백엔드 IPC 계약

진실: `src-tauri/src/lib.rs` `invoke_handler`, `src-tauri/src/commands.rs`, `src-tauri/src/save.rs`, `src-tauri/src/thumb_shell.rs`.

| 명령                                     | 입력 (JS camelCase)                                                                            | 반환                                   |
| ---------------------------------------- | ---------------------------------------------------------------------------------------------- | -------------------------------------- |
| `load_image`                             | `filePath`, `maxSide?`, `imageScalingMode?`, `autoDetectPixelArt?`                             | `ImageInfo`                            |
| `detect_pixel_art`                       | `filePath`                                                                                     | `PixelArtDetection`                    |
| `get_directory_images`                   | `filePath`, `options?`                                                                         | `DirectoryImages`                      |
| `resolve_dropped_path`                   | `path`                                                                                         | 해석된 파일 경로 `string`              |
| `get_exif_data`                          | `filePath`                                                                                     | `Record<string, string>`               |
| `get_image_histogram`                    | `filePath`                                                                                     | `Histogram`                            |
| `get_image_details`                      | `filePath`                                                                                     | `ImageDetails`                         |
| `get_archive_images`                     | `filePath`                                                                                     | `DirectoryImages`(엔트리 목록)         |
| `get_comic_info`                         | `filePath`                                                                                     | `ComicInfo \| null`(CBZ/ZIP 전용)      |
| `load_archive_image`                     | `archivePath`, `entryName`, `maxSide?`, `protect?`, `imageScalingMode?`, `autoDetectPixelArt?` | `ImageInfo`                            |
| `archive_prefetch`                       | `archivePath`, `entryNames`                                                                    | 추출 개수 `number`                     |
| `generate_thumbnail`                     | `filePath`, `maxSide?`                                                                         | `ThumbnailInfo`                        |
| `generate_thumbnails_batch`              | `filePaths`, `maxSide?`                                                                        | `BatchThumb[]`                         |
| `get_cached_thumbnail`                   | `filePath`                                                                                     | `ThumbnailInfo \| null`(캐시 히트만)   |
| `get_cache_stats`                        | 없음                                                                                           | `CacheStats`                           |
| `clear_cache`                            | `scope`                                                                                        | `CacheClearResult`                     |
| `generate_archive_thumbnail`             | `archivePath`, `entryName`, `maxSide?`                                                         | `ThumbnailInfo`                        |
| `generate_archive_file_thumbnail`        | `archivePath`, `maxSide?`                                                                      | `ThumbnailInfo`(첫 이미지 엔트리 기준) |
| `generate_archive_file_thumbnails_batch` | `archivePaths`, `maxSide?`                                                                     | `BatchThumb[]`(source=아카이브 경로)   |
| `get_file_associations`                  | 없음                                                                                           | `FileAssociation[]`                    |
| `set_file_association`                   | `extension`, `associate`                                                                       | `FileAssociation`                      |
| `open_default_apps_settings`             | 없음                                                                                           | 없음                                   |
| `get_license_bundle`                     | 없음                                                                                           | `LicenseBundle`                        |
| `get_psd_thumbnail_status`               | 없음                                                                                           | `PsdThumbStatus`                       |
| `register_psd_thumbnail`                 | 없음                                                                                           | `PsdThumbStatus`                       |
| `unregister_psd_thumbnail`               | 없음                                                                                           | `PsdThumbStatus`                       |
| `trash_file`                             | `filePath`                                                                                     | 없음                                   |
| `rename_file`                            | `oldPath`, `newName`, `maxSide?`, `imageScalingMode?`, `autoDetectPixelArt?`                   | `ImageInfo`                            |
| `save_image_edits`                       | `filePath`, `options`                                                                          | `ImageInfo`                            |
| `frontend_ready`                         | 없음                                                                                           | 없음 (`PendingOpenFile` flush)         |

`load_image`, `load_archive_image`, `rename_file`은 표시 정책에 따라 선택적으로 `imageScalingMode`(`auto | smooth | pixelated`)와 `autoDetectPixelArt`를 받는다.

인자 변환과 직렬화 규칙은 0절을 따른다. 응답은 snake_case이며 TypeScript 타입과 1:1 대응한다.

파일 연결 주의: 설정에서 연결 변경은 해당 확장자의 Windows 기본 앱 선택 창을 연다. 조용한 UserChoice 레지스트리 쓰기는 할 수 없다.

개발 빌드의 별도 등록(`AraView (Dev)`)은 `docs/development.md`를 따른다.

## 16. 데이터 모델

진실: `src/types/index.ts`, `src-tauri/src/image.rs`, `src-tauri/src/thumbnail.rs`, `src-tauri/src/file_assoc.rs`.

### 16.1 `ImageInfo`

Rust와 TypeScript는 같은 모양을 유지한다.

- `file_path: string`: WebView가 디코드할 경로. HEIC/HEIF/PSD는 JPEG sidecar 경로, 표시 해상도 제한이 걸린 큰 래스터는 `scaled/` 사본 경로일 수 있다(9.4절).
- `source_path: string`: 사용자가 연 원본 파일 경로. sidecar가 아니며, 파일 작업·EXIF·파일 상세가 이 경로를 쓴다. 아카이브 엔트리는 활성 파생 이미지 캐시에서 추출된 경로다.
- `mime_type: string`
- `file_name: string`
- `file_size: number`
- `width: number | null`, `height: number | null`: 렌더 바이트 기준 치수(축소 사본이면 사본 치수). SVG는 헤더 파싱으로 복원하며 해석 불가분만 null.

- `PixelArtDetection`: `{ classification: "pixel_art" | "continuous" | "uncertain", confidence: number, pixel_scale: number | null, method: "runs" | "edges" | "hybrid" | "unsupported" }`. `pixel_scale`은 픽셀 아트로 분류된 경우의 추정 격자 주기이며, 분석은 표시 힌트일 뿐 원본 파일을 변경하지 않는다.

### 16.2 기타

- `DirectoryImages`: `{ images: string[], current_index: number, availability: ("local"|"cloud_only"|"unknown")[] }`.
- `ThumbnailInfo`: `{ file_path: string, width: number, height: number }`.
- `ExifData`: `Record<string, string>`.
- `Histogram`: `{ r: number[256], g: number[256], b: number[256], sampled_pixels: number }`.
- `ImageDetails`: 16.2 모양 그대로. 색상 모드, 비트/채널, 생성/수정 시각, DPI, ICC 상태를 포함한다.
- `ArchiveState`: `{ archivePath: string | null }`.
- `ComicInfo`: CBZ/ZIP의 `ComicInfo.xml`(8절). `{ title, series, number, summary, writer, penciller, publisher, genre, tags, language_iso, age_rating, community_rating: string | null, count, volume, page_count: number | null, pages: ComicPage[] | null }`. `number`와 `community_rating`은 소수 값을 보존하려고 문자열이다.
- `ComicPage`: `{ image: number, page_type: string | null }`. `image`는 ComicRack 스키마대로 0 기반 페이지 인덱스이고 `page_type`은 `FrontCover` 같은 값이다. 양쪽 표지 판정(6절)의 근거다.
- `FileAssociation`: `{ extension, associated, current_prog_id, needs_os_confirmation }`.
- `PsdThumbStatus`: 탐색기 썸네일 등록 상태(20.2절).
- `SaveImageOptions`: camelCase `{ rotationCw, flipH, flipV, format?, overwrite, newFileName? }`.
- `DirListOptions`: camelCase `{ sortKey, descending, recursive }`.
- `CacheStats`: `{ storage_mode: "temporary"|"persistent", persistent_available, total_bytes, file_count, protected_bytes, protected_file_count, total_limit_bytes, categories }`.
- `CacheCategoryStats`: `{ key: "thumbnails"|"converted"|"scaled"|"archives"|"other", bytes, file_count, protected_bytes, protected_file_count, limit_bytes }`.
- `CacheClearResult`: `{ removed_bytes, removed_file_count, failed_file_count, stats }`.

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
- HEIC/HEIF/PSD는 JPEG sidecar를, 표시 해상도 제한이 걸린 큰 래스터는 `scaled/` 사본(9.4절)을 만든다. sidecar와 아카이브 추출물, 썸네일 캐시는 `cacheStorageMode`가 `persistent`면 Tauri 사용자 캐시 디렉터리의 버전된 루트에, `temporary`면 프로세스 수명 TempDir에 둔다.
- JPEG의 EXIF Orientation(1~8)은 WebView2 `<img>`가 자동 적용한다. 백엔드는 같은 기준을 따르도록 치수(`ImageInfo.width/height`, `get_image_details`), 썸네일, 편집 저장(`save_image_edits`)에 회전을 명시 적용한다(SVG/WebP/PNG/HEIC는 대상 아님).
- `detect_pixel_art`는 표시 바이트 경로를 제한된 분석 이미지로 읽고, 작은 색상 팔레트·평탄도·동일 색상 run·주기적 경계 신호를 결합한다. ML 모델이나 네트워크를 사용하지 않으며, 분석 제한 초과·디코드 실패·불확실 결과는 안전하게 부드러운 표시로 대체한다.
- `image-rendering`은 `smooth`와 `pixelated` 값을 사용한다. `smooth`는 브라우저의 고품질 보간 선호이며 특정 Bilinear 구현을 보장하지 않는다. `pixelated`는 확대 시 최근접 계열 보간을 요청한다. 픽셀 보존 판정(pixelated 모드와 확신 있는 자동 감지 포함)은 표시 배율이 1x 이상(확대)일 때만 적용되며, 축소 배율에서는 설정·감지와 무관하게 항상 `smooth`로 강제된다. nearest 축소는 스크린톤 같은 주기 패턴을 계단·무아레로 깨뜨린다. 단일 보기 배율은 `imageSize`에 대한 `zoom`이고, 웹툰/양쪽 보기는 렌더된 `<img>`에서 실측한다(측정 전에는 기존 판정 유지).
- 사용자 원본 파일은 필터링하지 않는다. 표시 해상도 상한 sidecar와 썸네일은 기존 파생 이미지 파이프라인을 유지하며, 픽셀 보존 판정은 메인 이미지 표시 힌트로만 사용한다.
- 사용자가 여는 파일/폴더는 명령 실행 시 런타임에 asset scope로 허용한다.
- CSP는 `default-src 'self'` 기반이며 `asset:`/`ipc:` 접근을 허용한다. dev 전용 설정은 `docs/development.md`를 따른다.

## 19. 다국어

진실: `src/i18n/index.ts`, `src/i18n/locales/ko.json`, `src/i18n/locales/en.json`.

- 지원 언어: `ko`, `en`.
- 시스템 감지: 브라우저 언어가 `ko`로 시작하면 `ko`, 아니면 `en`.
- 폴백: `ko`.
- 설정 변경 즉시 적용된다.

## 20. 윈도우/배포

진실: `src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs`, `docs/releasing.md`, `.github/workflows/release.yml`, `.github/workflows/ci.yml`.

- 창: 제목 기본값 `AraView`(이미지가 열리면 파일명, 아니면 앱 이름), 1024x768, 최소 500x400, 프레임리스, 시작 시 숨김. 최소 너비 500은 Windows 11 Snap Layouts의 모든 배치에 창이 들어가기 위한 상한이다(Microsoft 권장 ≤500epx).
- `window-state` 플러그인으로 창 상태를 유지한다.
- Windows 11 Snap Layouts: 커스텀 최대화 버튼(`id=caption-maximize`) 위에 `WM_NCHITTEST`에 `HTMAXBUTTON`으로 응답하는 투명 네이티브 오버레이를 띄운다(`tauri-plugin-snap-layout`, 비-Windows no-op). 오버레이가 마우스를 가로채므로 버튼의 hover 배경/툴팁은 플러그인 이벤트(`tauri-snap://snap/mouseenter|mouseleave`)로 미러링하고, 클릭 최대화/복원은 네이티브가, 키보드(Enter/Space)는 기존 onClick이 담당한다. 헤더가 완전히 가려지는 동안(auto-hide, 메뉴바 숨김; peek 제외) 오버레이를 떼어낸다(`useSnapLayout` 훅).
- 번들: `nsis`만 빌드한다. 결과물은 `src-tauri/target/release/bundle/` 아래에 생성된다.
- 릴리스 파이프라인: `npm run release -- <버전>`(`scripts/release.mjs`)이 버전 파일 5곳을 올려 커밋하고 `vX.Y.Z` 태그와 함께 푸시한다. 태그 푸시를 `.github/workflows/release.yml`이 받아 태그와 버전 파일의 일치, 서명 키 Secrets를 확인한 뒤 검증(테스트, 타입, 포맷, cargo test/clippy, npm/cargo 보안 감사, 라이선스 검사), 서명 빌드, `latest.json` 생성, 원본 저장소 `ara-hwang/araview` 릴리스 생성까지 자동으로 수행한다. `ci.yml`은 PR과 main 푸시에서 자동으로 돌지 않고, Actions에서 수동 실행할 때만 같은 검사를 수행한다. 권한과 절차 상세는 `docs/releasing.md`를 따른다.
- 파일 연결 3그룹:
  - Image 12종: png, jpg, jpeg, gif, bmp, webp, svg, ico, avif, heic, heif, psd.
  - Comic 4종: cbz, cb7, cbr, cbt.
  - Archive 3종: rar, zip, 7z.
- HEIC/HEIF는 vcpkg `libheif[core,aom]` 동적 링크 + `libde265`(HEVC), `aom`(AV1, AVIF 썸네일/히스토그램)만 사용한다. 설치와 DLL 복사는 `docs/development.md`를 따른다.

### 20.1 자동 업데이트 (tauri-plugin-updater)

수동 확인만 제공한다. 시작 시 자동 확인이나 백그라운드 폴링은 없다(오프라인 우선).

- 진입점: 설정 일반 탭의 `지금 확인` 버튼, 명령 팔레트의 `업데이트 확인`.
- 흐름: 업데이트 확인 → 없으면 최신 안내, 있으면 다운로드 및 설치. 다운로드가 끝나면 Dialog가 설치 인계를 안내하고, NSIS 설치 관리자를 실행한 뒤 앱이 종료된다. 설치와 새 버전 재실행은 NSIS가 담당하므로 수동 재시작 UI는 없다. 중복 확인은 무시한다.
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

- 포맷 추가: 2절 + 필요 시 15/16절, `src-tauri/tauri.conf.json` 파일 연결, `samples/` 검증.
- 백엔드 명령 추가: 15절 IPC 표 + 16절 데이터 모델.
- 설정/단축키 추가: 12~14절.
- 파생 이미지 캐시 변경: 9절 + 12~16절 + `docs/development.md`.
- 문서 정합성 검사: `npm run docs:check`(확장자, IPC, 설정, 플러그인, 버전, plan, i18n 잔재). CI 수동 실행에도 포함된다.
