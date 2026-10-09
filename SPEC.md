# SPEC.md - AraView 기능/기술 명세

> 한국어 스펙 문서. 구현 진실(source of truth)은 코드이며, 본 문서는 현재 코드베이스의 동작을 요약한다.
> 관련 문서: `PRODUCT.md`(제품 정의), `DESIGN.md`(비주얼 시스템), `README.md`(소개/문서 허브), `docs/usage.md`(사용법), `docs/development.md`(개발 안내), `docs/releasing.md`(릴리스/업데이트), `AGENTS.md`(AI 작업 지침), `CODING_STANDARDS.md`(코드 작성 규칙), `docs/playbooks.md`(작업별 절차).

## 0. 문서 규약

- 구현 경로: UI와 입력은 `src-gpui/src/`, UI에 의존하지 않는 코어(`ops/` 연산 함수 포함)는 `crates/araview-core/src/`에 있다. 이 문서의 '프론트'는 UI(`src-gpui`), '백엔드'는 코어(`araview-core`)를 가리킨다.
- 코어 연산 입력은 Rust 타입이며, 이 문서의 표는 입력 이름을 camelCase로 적는다(설정 파일 키와 같은 표기). 응답 필드는 snake_case다. 상세는 15~16절을 따른다.
- 에러는 구조화 에러 `{ code, message }`를 우선한다.
- 렌더링은 파일 경로(또는 파생 이미지 경로) 기반이며 base64 페이로드를 쓰지 않는다. 코어가 디코드한 RGBA 프레임을 GPU 텍스처로 올려 그린다(18절).

## 1. 개요

- Windows 11 x64 전용 오프라인 데스크톱 이미지/코믹 뷰어. 로컬 파일만 다루며 라이브러리 가져오기, 계정, 네트워크를 쓰지 않는다(수동 업데이트 확인 제외).
- 창은 프레임리스이며 커스텀 타이틀바/툴바(`src-gpui/src/app.rs`의 헤더)를 쓴다. 헤더 버튼 묶음은 창 드래그 영역과 겹치지 않도록 마우스를 막아 클릭이 캡션 드래그로 처리되지 않게 한다. Windows 11에서는 최대화 버튼 호버로 OS Snap Layouts 플라이아웃이 뜬다(§20).
- 다이얼로그가 열려 있어도 타이틀바 컨트롤(최소화/최대화/닫기)은 동작해야 한다. 어두운 배경 클릭과 `Esc`로 닫히고, 키보드 포커스는 다이얼로그 안에 갇힌다. 다이얼로그는 알림 레이어 위에 그려지므로, 열려 있는 동안의 결과 알림은 경고창으로 보여준다(`src-gpui/src/toast.rs`). 다이얼로그와 명령 팔레트는 화면 가운데에 배치한다(추정 높이로 상단 오프셋을 계산, `src-gpui/src/dialog.rs`).
- 기술 스택과 플러그인 목록은 `docs/development.md`를 따른다.

## 2. 지원 포맷

총 19개 확장자(이미지 17종 + 아카이브 2종). 진실은 `crates/araview-core/src/image.rs` (`SUPPORTED_EXTENSIONS`, `get_mime_type`).

### 2.1 순수 이미지 17종

| 확장자        | MIME                        | 비고                                                                                                                       |
| ------------- | --------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `png`         | `image/png`                 | 직접 디코드                                                                                                              |
| `apng`        | `image/apng`                | 직접 디코드. PNG와 시그니처가 같아 확장자로 구분하며, 프레임을 디코드해 재생. 재생 제어는 GIF와 같다(7.5절) |
| `jpg`, `jpeg` | `image/jpeg`                | 직접 디코드. EXIF Orientation(1~8)을 치수·썸네일에 반영                                            |
| `gif`         | `image/gif`                 | 직접 디코드. 단일 보기에서 재생/정지·프레임 이동 지원                                                                  |
| `bmp`         | `image/bmp`                 | 직접 디코드                                                                                                              |
| `webp`        | `image/webp`                | 직접 디코드                                                                                                              |
| `svg`         | `image/svg+xml`             | 직접 디코드. 치수는 `<svg>` 헤더(width/height/viewBox, 절대 단위) 파싱으로 복원하며 해석 불가분만 `width/height` 생략    |
| `ico`         | `image/x-icon`              | 직접 디코드                                                                                                              |
| `avif`        | `image/avif`                | 직접 디코드                                                                                                              |
| `heic`        | `image/heic`                | JPEG sidecar 트랜스코드 후 렌더                                                                                            |
| `heif`        | `image/heif`                | JPEG sidecar 트랜스코드 후 렌더                                                                                            |
| `psd`         | `image/vnd.adobe.photoshop` | JPEG sidecar 트랜스코드 후 렌더(읽기 전용)                                                                                 |
| `tga`         | `image/x-tga`               | JPEG sidecar 트랜스코드 후 렌더(읽기 전용). `image` 크레이트 디코드, 투명은 흰 배경 합성                                   |
| `dds`         | `image/vnd.ms-dds`          | JPEG sidecar 트랜스코드 후 렌더(읽기 전용). DXT1/3/5(BC1~BC3)만 지원, 그 외는 `unsupported`                                |
| `exr`         | `image/x-exr`               | JPEG sidecar 트랜스코드 후 렌더(읽기 전용). 선형 값을 clamp 후 sRGB 감마로 8비트화(노출 보정 없음)                         |
| `qoi`         | `image/qoi`                 | JPEG sidecar 트랜스코드 후 렌더(읽기 전용). `image` 크레이트 디코드, 투명은 흰 배경 합성                                   |

### 2.2 아카이브 2종

| 확장자 | MIME                            | 비고                                  |
| ------ | ------------------------------- | ------------------------------------- |
| `cbz`  | `application/vnd.comicbook+zip` | ZIP 기반 코믹                         |
| `zip`  | `application/zip`               | 일반 ZIP도 이미지 목록으로 열 수 있음 |

CBZ/ZIP 안의 `ComicInfo.xml`은 메타데이터 읽기와 표지 지정(`FrontCover`) 쓰기를 지원한다(8절, 15절). CB7/7Z, CBR/RAR, CBT는 지원하지 않는다(디코더 의존성을 줄이려고 제거했다).

백엔드 판별:

- MIME은 확장자를 1차로 보되, 렌더 경로를 정하는 `resolve_mime`(`image.rs`)이 파일 선두 32바이트 시그니처(`sniff.rs`)로 보정한다. 이름이 바뀐 파일(HEIC를 `.jpg`로 저장 등)이나 확장자 없는 파일도 올바른 경로를 탄다. PNG 시그니처 파일은 확장자가 `apng`이거나 첫 `IDAT` 앞에 `acTL` 청크가 있으면 `image/apng`로 보고한다(확장자가 `.png`인 APNG 포함). 시그니처가 불확실한 SVG, 아카이브, TGA, HEIF/AVIF 공용 브랜드(`mif1`)는 확장자를 따른다. 폴더 목록은 확장자 기준 그대로다.

- `is_image_file`: MIME이 `application/`으로 시작하지 않는 지원 파일.
- `is_archive_file`: 위 2종 MIME 해당.
- `is_supported_file`: 둘 중 하나.

### 2.3 제외 포맷

- JXL, RAW(CR2/NEF/ARW 등), PSB는 제외 유지.
  - JXL(`jxl-oxide`), RAW(`rawloader`): 디코더 크레이트는 있으나 아직 포함하지 않았다. 추가 시 `transcode.rs`에 디코더를 등록해 같은 sidecar 파이프라인(`SidecarSpec`)에 얹는다.
  - PSB: 실제 PSB는 `8BPS` + version 2인데 디코더가 없어 진입 차단한다(`psd` 크레이트는 PSD만 지원).
- PSD, TGA, DDS, EXR, QOI는 JPEG sidecar 미리보기로만 표시한다(PSD는 합성 이미지, EXR은 8비트 변환). 앱은 어떤 포맷도 편집·저장하지 않는다.

## 3. 화면

화면 진실: `src-gpui/src/app.rs`(홈, 헤더, 상태바), `src-gpui/src/app/viewport.rs`, `src-gpui/src/app/dock.rs`, `src-gpui/src/app/grid.rs`.

### 3.1 홈

- 빈 상태: 실제 다음 행동(파일 열기)을 안내한다. 가짜 샘플을 만들지 않는다.
- `파일 열기` 버튼은 파일 피커를 연다.
- 최근 파일 섹션: `recordRecentFiles`가 true이고 목록이 있을 때만 표시하며 개수와 전체 삭제를 제공한다. 파일명, 형식/크기/치수, 상위 폴더를 표시하고 아카이브는 읽기 진도를 함께 보여준다.
- 아카이브는 아이콘으로, 로드 실패 항목은 error 상태로 표시한다.
- 드래그 중에는 드래그 오버레이를 표시한다.
- 시작 옵션: `autoOpenLastFile && recordRecentFiles`이고 최근 파일이 있으면 첫 항목을 1회 자동 로드한다.
- 이미지가 로드되면 뷰어 화면으로 전환한다. 창 제목은 이미지 파일명, 없으면 앱 이름이다.

### 3.2 뷰어

- 열린 이미지가 없으면 홈을 보여준다.
- 구성: 읽기 영역 + 이미지 목록 도크(상/하/좌/우) + 드래그 오버레이 + 썸네일 그리드(선택) + 이름 변경 다이얼로그.
- 도크: 뷰어 가장자리 플로우에 붙으며 읽기 영역을 밀어낸다. 위치(`dockPosition`, 기본 `bottom`), 표시(`dockVisible`, 기본 펼침), 썸네일 크기(`dockThumbSize` S/M/L), 파일명(`dockShowName`)/번호(`dockShowIndex`)를 보기 설정에서 바꾼다. 도크의 `⋯` 버튼에서도 같은 값을 즉시 바꿀 수 있다. 바 전체를 접으면 얇은 엣지 바로 복구한다. 접힘·자동숨김 동안에도 목록 스크롤 위치와 로드한 썸네일이 유지된다. 이미지가 2장 이상일 때만 표시한다.
- 도크 구성: 한 줄에 이전/다음 버튼, 썸네일 목록(가상화), 그리드 토글, `⋯` 옵션, 접기 버튼을 둔다. 슬라이더는 없다. 목록은 폴더/아카이브의 모든 항목을 같은 순서로 유지하며, 보이는 창을 먼저 채운 뒤 나머지 썸네일을 이어서 로드한다. 우→좌 양쪽 보기(`right-to-left`, 만화 자동 판정 포함)에서 가로 도크(상/하)는 읽는 방향을 따라 첫 장을 오른쪽 끝에 두고 왼쪽으로 늘어놓으며, 이전/다음 버튼도 자리를 바꾼다(다음이 왼쪽). 세로 도크(좌/우)는 위에서 아래 그대로다.
- 도크 위 휠은 설정된 휠 동작(`wheel`)을 따른다(기본: 위=이전, 아래=다음, Ctrl+휠=확대/축소). 동작이 `none`이면 목록 스크롤로 넘긴다. 웹툰 모드에서도 도크 휠은 이동으로 동작한다.
- `G`(기본, 재할당 가능)로 썸네일 그리드 오버레이를 연다. 열 때 현재 이미지를 중앙에 둔다.
- 그리드: 뷰포트 기반 가상화, 클릭/`Enter`로 점프 후 닫기, `Esc`/`G`로 닫기, 파일명 필터, 실패 셀 배지와 재시도. 그리드가 열려 있는 동안 뷰어 단축키는 비활성이다. 셀은 현재 페이지와 키보드 선택(링)을 따로 표시한다.
- 우클릭은 설정(`mouse.rightClick`)에 따라 컨텍스트 메뉴 또는 다른 동작이다. 홈에서는 우클릭을 막는다.
- `Esc` 닫기: 이름 변경 다이얼로그가 열려 있거나 입력 포커스 중이면 닫지 않는다.
- 이미지 표시 설정은 `auto | smooth | pixelated`를 제공한다. `auto`는 픽셀 아트 자동 감지가 켜져 있고 분석 결과가 `pixel_art`로 분류되면 `pixelated`를 사용한다. 추가로 자동 모드는 표시 배율이 2x 이상이면 감지 결과와 무관하게 `pixelated`를 사용한다(확대 시 원본 픽셀 표시). 그 외에는 `smooth`를 사용한다. 수동 모드는 자동 감지와 배율 기반 전환보다 우선한다.
- 픽셀 아트 자동 감지는 현재 이미지를 렌더한 뒤 백그라운드 작업(`detect_pixel_art`)으로 분석한다. 분석 전/실패/불확실은 `smooth`로 대체하며 원본과 파생 이미지는 변경하지 않는다.
- 분석은 표시 바이트를 백엔드에서 다시 디코드하므로, 결과가 쓰이는 표시에서만 요청한다. `auto`에서 분석 결과가 판정에 들어가는 구간은 1x~2x 확대이거나 배율 미상일 때이므로, 축소와 2x 이상 확대에서는 요청하지 않는다. 단일 보기는 치수를 모를 때도 요청하고, 웹툰/양쪽 보기는 렌더된 크기를 실측해 1x~2x 확대일 때만 요청한다. 축소에서 확대로 바뀌면 그때 요청한다.
- 픽셀 보존 표시는 GPU의 선형 보간 대신, 보이는 영역만 CPU에서 최근접 확대한 래스터를 그려 원본 픽셀을 그대로 보여준다(`src-gpui/src/app/viewport.rs`의 픽셀 뷰). 양쪽/웹툰의 각 이미지는 같은 설정을 공유하되 판정은 이미지별이다.
- `autoHideUI`가 true일 때만 읽기 중 크롬(상단바, 이미지 목록 도크, 상태바)을 숨기고 읽기 영역을 확장한다. 도크 상태(스크롤 위치, 로드한 썸네일)는 유지된다.
- `menuBarHidden`이 true이면 상단바를 숨기고, 상단 호버 영역에서 peek 오버레이로 표시한다. 헤더 숨기기 버튼과 보기 설정 스위치로 토글한다. peek 시에는 읽기 영역 위로 겹쳐 내려오며, 포커스 이탈 시에는 즉시 닫힌다. 모션 토큰과 timing은 `DESIGN.md`를 따른다. `Esc`로는 닫히지 않는다(뷰어의 이미지 닫기와 충돌 방지).
- `menuBarHidden`은 이미지를 보고 있을 때만 적용한다. 이미지 없이 빈 화면(홈)에서는 설정값을 유지한 채 상단바를 항상 표시한다(열기/드래그 동선 유지). `autoHideUI`와 동일한 조건을 쓴다.
- 로드 실패 시 에러 카드에 재시도/홈 복구 경로를 제공한다. 실패한 적은 토스트로 알린다.

### 3.3 상태 규칙

- `PRODUCT.md` 원칙과 `DESIGN.md` 시스템을 따른다: 모든 데이터 뷰는 empty/loading/error 상태, 모든 컨트롤은 키보드 조작과 포커스 표시, UI 카피 금칙(문서 참조)을 지킨다.
- 고대비: OS 설정(`prefers-contrast: more`, Windows 대비 테마의 `forced-colors: active`)에 반응한다. 별도 토글은 두지 않으며, 강제 색상 모드에서도 시스템 Highlight 외곽선으로 포커스를 표시한다.
- 체감 로딩: 큰 이미지는 캐시된 저해상 썸네일을 먼저 깔고 원본이 로드되면 페이드 없이 바로 교체한다(9.2절).
- 이미지 전환: 단일 보기에서 다른 이미지로 넘어갈 때 새 이미지가 디코드될 때까지(최대 300ms) 이전 이미지를 유지한 뒤 페이드 없이 교체한다. 예열된 이웃 이미지는 같은 프레임에 바뀐다. 양쪽 보기는 두 장이 모두 디코드될 때까지 이전 화면을 유지하고, 두 장이 모두 로드(또는 실패)한 뒤 함께 표시한다.

## 4. 파일 열기 흐름

진실: `src-gpui/src/app/pages.rs`(`open_path`), `src-gpui/src/app.rs`(`prompt_open`, `open_dropped`), `src-gpui/src/app/system.rs`(두 번째 실행 전달), `src-gpui/src/main.rs`.

### 4.1 파일 피커

- 이미지 필터로 단일 선택 후 `loadImage`로 연다.

### 4.2 드래그 앤 드롭

- 파일/폴더를 여러 개 드롭할 수 있다.
- 아카이브 경로는 그대로 유지한다.
- 그 외 경로는 `resolve_dropped_path`로 해석한다:
  - 파일이면 그대로 반환.
  - 폴더면 내부 지원 파일 중 이름순(자연 정렬, 5.1절) 첫 이미지를 반환.
  - 이미지 없음/경로 없음/지원 불가면 에러.
- 해석 실패 항목은 건너뛴다. 전부 실패하면 실패 안내를 표시한다.
- 성공 목록은 이름순(숫자는 값으로 비교, 대소문자 무시) 정렬 후 첫 항목을 열고, 2개 이상이면 첫 항목 열림을 알린다.

### 4.3 OS 파일 연결 실행

- Windows가 CLI 인자(`args[1]`)로 파일 경로를 넘긴다.
- 시작 인자로 받은 경로는 창이 준비되면 한 번 연다(`src-gpui/src/main.rs`, `open_path`).
- 두 번째 실행은 명명된 파이프(`\\.\pipe\<식별자>`)로 첫 인자를 기존 창에 넘기고 종료한다. 기존 창은 받은 경로를 바로 연다(`src-gpui/src/app/system.rs`).
- 연결 프로그램으로 실행하든 두 번째 실행이든 같은 `open_path` 흐름을 탄다.

## 5. 디렉토리 목록과 탐색

진실: `crates/araview-core/src/ops/directory.rs`, `crates/araview-core/src/dir_cache.rs`, `crates/araview-core/src/natural_sort.rs`, `src-gpui/src/layout.rs`, `src-gpui/src/app/pages.rs`.

### 5.1 `get_directory_images`

- 입력: `file_path`, `options?: DirListOptions`(`sortKey: name | date | size`, `descending`, `recursive`).
- 부모 폴더 기준으로 정렬 목록을 만들고, `current_index`는 요청 경로의 위치(없으면 0)이다.
- 이름순은 탐색기와 같은 자연 정렬이다(`crates/araview-core/src/natural_sort.rs`): 숫자 구간은 값으로 비교해 `2.jpg`가 `10.jpg`보다 앞이고, 대소문자는 무시하며, 기호와 비ASCII 문자는 사용자 로캘 규칙을 따른다. 경로는 폴더 단위로 나눠 비교한다. `date`/`size` 정렬의 동률과 아카이브 엔트리 순서에도 같은 규칙을 쓴다.
- 재귀가 켜지면 하위 폴더 이미지를 포함한다. 재귀 목록이 캐시에서 올 때만 항목별 존재를 다시 확인해(256개 이상은 병렬) 워처 이벤트가 오기 전에 지워진 파일을 뺀다. 방금 스캔한 목록은 다시 확인하지 않는다.
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
- 넓은 페이지나 폴더 안 아카이브처럼 단독 화면이 섞이면 그 화면은 1장씩 넘긴다(6절).
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

진실: `src-gpui/src/settings.rs`, `src-gpui/src/layout.rs`, `src-gpui/src/app/viewport.rs`, `src-gpui/src/app/pages.rs`.

값: `single | left-to-right | right-to-left | webtoon`. 기본 `single`.

| 모드            | 렌더                                | 넘김 단위   | 프리페치 성향 |
| --------------- | ----------------------------------- | ----------- | ------------- |
| `single`        | 현재 1장                            | 1장         | 기본 거리     |
| `left-to-right` | 현재 + 다음, 좌에서 우              | 2장(화면)   | 기본 + 1      |
| `right-to-left` | 현재 + 다음, 우에서 좌              | 2장(화면)   | 기본 + 1      |
| `webtoon`       | 전 구간 연속 수직 스크롤, 지연 로드 | 스크롤 이동 | 기본 x 2      |

- 양쪽 모드 페이지는 `[current, next]`이며 루프가 켜지면 wrap한다. 로드 실패 페이지는 제외한다.
- 표지 단독: 표지를 혼자 보여주고 그 뒤부터 `[표지+1, 표지+2]` 쌍을 맞춘다. 표지가 0번이 아니면 표지 바로 앞에 남는 페이지도 단독 화면이 된다. 표지 화면에서는 다음 페이지를 로드하지 않는다. 마지막에 남은 한 장은 기존 단일 중앙 렌더를 재사용한다.
- 표지 배치는 아카이브마다 정해지며 아카이브 지정이 전역 설정보다 우선한다. 열린 아카이브의 ComicInfo(8절)에 `FrontCover`가 있으면 `showCoverAlone`이 꺼져 있어도 그 구간을 단독으로 보여준다. `FrontCover`가 없고 0번 페이지에 다른 `Type`이 명시돼 있으면 "표지 없음"이라 `showCoverAlone`이 켜져 있어도 `[0,1], [2,3], ...`로 넘긴다. 아무 지정이 없거나 `FrontCover`가 목록 범위 밖이면 `showCoverAlone`(기본 true)대로 0번을 표지로 본다. 폴더 목록과 폴더 미리보기에는 ComicInfo를 적용하지 않는다.
- 표지는 연속된 여러 장일 수 있다(변형 표지 등). `FrontCover`가 여러 개면 가장 앞에서부터 끊기지 않고 이어지는 구간만 표지로 보고, 각 장이 단독 화면이 된다.
- 폴더 목록의 아카이브(CBZ/ZIP)는 이미지로 그릴 수 없으므로 양쪽 모드에서 단독 화면이 된다. 짝은 앞에서부터 두 장씩 맞추되 아카이브(표지 단독이면 표지 포함)와 그 바로 앞에 남는 페이지는 혼자 한 화면이므로, 넘김이 아카이브를 건너뛰지 않는다. 아카이브 화면에서는 첫 페이지 미리보기를 단일로 그리지만 이동·도크 하이라이트는 양쪽 배치(`useLayoutViewMode`)를 따른다. 아카이브 안(만화 모드)에는 적용하지 않는다.
- 넓은 페이지 단독(`showWidePageAlone`, 기본 true): 표시 치수의 가로가 세로보다 긴 페이지(펼침면을 한 장으로 스캔한 가로 스캔)는 양쪽 모드에서 혼자 한 화면을 쓴다. 반쪽 칸에 넣으면 절반 크기로 줄어들기 때문이다. 배치 규칙은 폴더 아카이브와 같아서 넓은 페이지 바로 앞에 남는 페이지도 단독 화면이 되고, 그 뒤부터 다시 두 장씩 짝을 맞춘다. 폴더와 아카이브 모두에 적용한다. 치수는 로드해야 알 수 있으므로 본 페이지와 예열한 이웃만 반영하며(목록 범위가 바뀌면 버리고 영속화하지 않는다), 아직 로드하지 않은 구간은 두 장씩으로 가정한다. 이전 화면으로 넘기거나 점프할 때는 도착할 자리의 페이지(이전은 앞의 두 장, 점프는 대상 한 장)를 먼저 로드해 확인하므로 넓은 페이지를 건너뛰지 않는다. 뒤늦게 알려진 앞쪽 넓은 페이지가 짝을 밀어도 지금 화면은 바뀌지 않는다. 현재 인덱스를 화면 시작으로 고정하고(`anchor`) 그 앞뒤로 배치를 이어 간다.
- 양쪽 모드의 점프(썸네일/도크/`Home`/`End`/`PageUp`/`PageDown`)와 아카이브 이어보기 진입은 쌍 시작으로 스냅한다.
- 만화 자동 양쪽 보기(`comicAutoDualView`, 기본 true): 아카이브를 만화 모드로 열 때 `viewMode`가 `webtoon`이 아니면 양쪽 보기로 연다. 방향은 ComicInfo `Manga`가 `YesAndRightToLeft`면 우→좌, `No`면 좌→우를 따르고, `Yes`/`Unknown`/메타데이터 없음이면 설정의 양쪽 방향(우→좌였다면 우→좌), 그 외에는 좌→우다. 결과는 영속화하지 않는 `appStore.comicViewMode`에 담고, 화면에 적용되는 모드는 `useEffectiveViewMode`(`comicViewMode ?? viewMode`, 아카이브일 때만)로 읽는다. 헤더나 설정에서 보기 모드를 직접 고르면(`applyManualViewMode`) 자동 결정은 해제되고, 아카이브를 다시 열면 다시 계산한다. 설정값 `viewMode`는 바뀌지 않으므로 일반 이미지는 영향이 없다.
- 도크와 썸네일 그리드는 화면에 떠 있는 페이지를 모두 현재로 표시한다(`useCurrentPageIndices`). 양쪽 모드는 쌍 두 장이 함께 하이라이트되고, 단독 화면(표지, 표지 바로 앞 페이지, 넓은 페이지, 마지막 홀수 장)이나 단일/웹툰 모드는 현재 장만 하이라이트한다. 쌍의 기준 장은 도크의 `data-dock-current`로 한 개만 표시해 그리드 닫기 시 포커스 복귀 지점을 유지한다. 계산은 `dualPageIndices`가 뷰어 로드 대상과 같은 목록을 돌려준다.
- 웹툰 모드에서 `ArrowLeft/ArrowRight`는 이전/다음 이미지 스크롤 이동이다.
- 웹툰 모드에서 `ArrowUp/ArrowDown`은 연속 스크롤 컨테이너를 일정량씩 스크롤한다.
- 웹툰 이미지는 간격 없이 이어 붙고, 페이지 경계선(`webtoonPageBoundaries`)을 설정한다.
- `webtoonFitWidth`를 켜면 작은 이미지도 읽기 영역 너비까지 확대하고, 끄면 원본 크기를 유지한 채 너비만 제한한다.
- `webtoonShowProgress`를 켜면 읽기 영역에 현재 장 번호, 전체 장 수, 스크롤 진행률을 작은 표시로 보여준다.
- `webtoonThumbnailJump`를 켜면 읽기 영역의 썸네일 버튼으로 그리드를 열어 원하는 장으로 바로 이동할 수 있다. 현재 위치 표시를 꺼도 이 버튼은 유지된다.
- 웹툰 중앙 이미지 변경은 전체 reload 없이 인덱스 동기화와 정보 교체로 처리한다.
- 웹툰 페이지는 뷰포트 위아래 2화면 안에 들어오면 로드하고, 4화면 밖으로 나가면 이미지를 내려놓는다. 내려놓은 페이지는 같은 너비와 종횡비의 자리 표시로 남아 스크롤 위치가 변하지 않고, 다시 가까워지면 로드한다. 현재 페이지 판정은 근처 페이지만 측정한다.

## 7. 뷰어 조작

진실: `src-gpui/src/geometry.rs`, `src-gpui/src/app/viewport.rs`.

### 7.1 줌

- `zoomIn`: `min(zoom * 1.25, max)`. 상한은 SVG 40, 그 외 10.
- `zoomOut`: 동적 최소값까지 `zoom / 1.25`.
- 줌 기준점: 단일 보기에서 휠 줌은 커서 아래의 이미지 지점을 고정한다. 커서가 이미지 컨테이너 밖(도크 등)이거나 키보드/헤더/메뉴/팔레트 줌이면 컨테이너 중심이 기준이다. 위치는 줌과 같은 갱신에서 옮기고 컨테이너 경계로 clamp하므로, 경계에 닿으면 기준점이 어긋날 수 있다.
- `0`: 자동 맞춤으로 리셋(위치 0 + 회전/반전 초기화).
- `1/2/3`: 가로 맞춤 / 세로 맞춤 / 화면 맞춤.
- `fitMode`: `width | height | screen | auto`. 기본 `auto`. 헤더 버튼과 단축키로 바꾸면 저장되고 이미지 전환마다 다시 적용된다. 수동 줌을 하면 잠금이 풀려 리사이즈해도 줌을 유지한다. `auto`는 큰 이미지만 맞추고 작은 이미지는 100%로 두며, `width/height/screen`은 작은 이미지도 확대한다.
- 맞춤 계산은 회전된 치수(90/270도면 가로세로 교환) 기준이다.

### 7.2 팬

- 키보드 팬은 1회 48px이며 컨테이너 경계로 clamp한다.
- 마우스 왼쪽 드래그는 기본 팬이다(`pan` 또는 `none`만 허용).
- 팬·스크롤 여유가 없으면 왼쪽 드래그가 창 이동이 된다. 단일·양쪽 보기에서 팬 한계가 0이거나 웹툰 전체 높이가 읽기 영역을 넘지 않을 때가 대상이며, 전체화면·최대화 상태에서는 창을 옮기지 않는다. 더블클릭 동작이 우선한다.

### 7.3 회전/반전

- `rotation`: `0 | 90 | 180 | 270`.
- `R`: 시계 90도, `Shift+R`: 반시계 90도.
- `H`: 좌우 반전, `V`: 상하 반전.
- 이미지 변경 시 기억된 맞춤 모드로 초기화된다.

### 7.4 배경/표시

- `viewerBackground`: `theme | black | white | checker`. 기본 `theme`.
- `B`: `theme → black → white → checker` 순환.
- `T`: 항상 위 토글.
- `F11`: 전체화면 토글. 더블클릭 기본도 전체화면이다.
- `autoHideUI`: true일 때 읽기 중 크롬 자동 숨김.

### 7.5 GIF/APNG 재생 제어

- 단일 보기에서 GIF와 APNG는 프레임을 디코드해 재생/정지(`P`), 프레임 이동(`,`/`.`), 프레임 카운터를 제공한다. 정지 GIF/PNG와 양쪽/웹툰 보기에서는 첫 프레임을 정지 상태로 보여준다.
- 재생은 이미지 진입 시 자동 시작하며, OS 모션 최소화(`prefers-reduced-motion: reduce`)면 정지 상태로 시작한다.
- 단일/다중 프레임 판정과 반복 횟수는 디코더 트랙 정보를 따르고, 유한 반복 GIF는 마지막 회차에서 멈춘다.
- 헤더 컨트롤은 좁은 창(1024px 미만)에서 숨는다. 단축키와 명령 팔레트는 항상 쓸 수 있다.

## 8. 아카이브

진실: `crates/araview-core/src/archive.rs`, `crates/araview-core/src/archive_index.rs`, `crates/araview-core/src/ops/archive.rs`, `crates/araview-core/src/comic_info.rs`, `src-gpui/src/app/pages.rs`.

- 아카이브 커맨드(`get_archive_images`, `load_archive_image`, `archive_prefetch`, `get_comic_info`, `set_comic_cover_pages`, `generate_archive_thumbnail`, `generate_archive_thumbnails_batch`, `generate_archive_file_thumbnail(s)_batch`)는 모두 비동기 커맨드로 `spawn_blocking`에서 실행되어 메인 스레드를 막지 않는다. `archive_prefetch`는 fire-and-forget 성격에 맞게 조인 실패도 흡수해 `Ok(0)`을 반환한다.
- 엔트리 인덱스 캐시(`archive_index.rs`): 아카이브별 이미지/전체 엔트리 목록을 canonical 경로 + mtime + size 검증으로 캐시한다(최대 64개 LRU). `get_archive_images`, `get_comic_info`, `generate_archive_file_thumbnail(s)_batch`가 공유해 아카이브당 전체 스캔이 1회로 수렴한다. `clear_cache` 시 함께 비워진다.
- `get_archive_images`: 내부 이미지 엔트리 목록 + `current_index: 0`. 비어 있으면 `not_found`.
- `load_archive_image`: 활성 파생 이미지 캐시 루트의 `archives/` 아래에 추출 후 표시 가능한 경로로 반환한다.
- `archive_prefetch`: 이웃 선추출용 fire-and-forget 명령이다. 엔트리마다 압축 해제가 독립이라 썸네일 배치와 같은 워커 수로 병렬 추출한다.
- ZIP 중앙 디렉터리 캐시(`archive.rs`): 최근에 연 아카이브 4개의 파싱 결과를 경로 + mtime + size가 같을 때만 재사용한다. 추출물이 없는 페이지를 열거나 배치 워커가 각자 핸들을 열 때 디렉터리를 다시 파싱하지 않는다. 파일 핸들은 캐시에 남기지 않는다.
- 선추출 거리는 뷰 모드에 따라 보정되며 상한이 있다.
- 이어보기: 아카이브 경로별 마지막 엔트리와 위치(엔트리명/인덱스/전체 페이지)를 최대 100개 LRU로 저장한다. `resumeReading`이 true이고 목록에 저장된 항목이 있으면 거기서 시작하며, 저장 위치에서 시작할 때 "이어보기" 토스트와 "처음부터" 동작을 함께 제공한다. 설정이 false면 항상 첫 페이지에서 열고 열기만으로 저장 위치를 0페이지로 덮지 않는다. 양쪽 모드에서는 저장 위치가 쌍 중간이면 쌍 시작으로 맞춰 연다.
- `get_comic_info`: CBZ/ZIP의 `ComicInfo.xml`(ComicRack/Komga/Kavita 스키마)을 메타데이터로 반환한다. 탐색은 엔트리 basename이 `comicinfo.xml`인 항목(대소문자 무시)이며 루트를 우선하고 없으면 첫 중첩 경로를 쓴다. 상한 1 MiB, UTF-8(BOM 허용)과 UTF-16 LE/BE BOM을 지원한다. XML 부재나 지원하지 않는 확장자는 `null`, 깨진 XML/디코딩 실패는 `corrupt`, 크기 초과는 `too_large`다. 필드 누락과 빈 값은 `null`로 정규화하고, 엔트리는 `read_archive_entry_bounded`(`archive.rs`)로 1 MiB 상한 아래에서 메모리로 읽는다. `pages`는 `image` 오름차순으로 최대 1000개까지 담는다.
- 표지 판정: `pages`에서 `page_type`이 `FrontCover`(대소문자·공백 무시)인 페이지를 양쪽 보기 표지로 쓴다(규칙은 6절). `image`가 목록 범위를 벗어난 항목(1 기반으로 적은 파일 등)은 버리고, 이어보기 진입도 표지 기준 쌍 시작으로 스냅한다.
- `set_comic_cover_pages`: 표지 페이지 집합을 `ComicInfo.xml`에 쓴다. ComicInfo에서 앱이 고쳐 쓰는 것은 `Pages`의 `Type`뿐이다. 집합의 페이지는 `FrontCover`가 되고 그 밖의 기존 `FrontCover`는 `Story`로 바뀌며, 집합이 비면 0번을 `Story`로 남겨 "표지 없음"을 명시한다. `Page` 요소가 없으면 추가하고 XML이 없으면 루트에 새로 만든다. 다른 요소와 속성은 이벤트 단위로 그대로 옮겨 보존하고 출력은 UTF-8이다. 인덱스가 페이지 수를 넘으면 `invalid_input`, CBZ/ZIP이 아니면 `unsupported`다.
- 아카이브 쓰기(`replace_archive_entry`, `archive.rs`): 나머지 엔트리는 재압축 없이 그대로 옮기고, 같은 폴더의 임시 파일(`.araview-part`)에 다 쓴 뒤 원본과 바꾼다. 백업은 남기지 않고 수정 시각은 쓴 시각이 된다. 실패하면 원본은 그대로 남고 임시 파일은 지운다. 파일이 바뀌면 인덱스 캐시와 추출 디렉터리 식별자(mtime + size)가 함께 바뀐다.
- 표지 지정 UI: 아카이브 안에서 뷰어 본문을 우클릭하면 컨텍스트 메뉴 끝에 `이 페이지를 표지로 지정`/`표지 지정 해제`가 나온다. 양쪽 보기와 웹툰은 클릭한 쪽 페이지가 대상이다. 명령 팔레트의 `현재 페이지 표지 지정/해제`는 현재 페이지에 같은 동작을 한다. 표지 구간에 붙은 페이지를 지정하면 구간이 늘고, 떨어진 페이지를 지정하면 그 페이지만 표지가 되며, 구간 중간을 해제하면 앞쪽 구간만 남는다. 전역 설정으로 표지인 0번을 해제하면 "표지 없음"이 기록된다. 저장 후에는 그 페이지의 쌍 시작으로 다시 맞추고, 실패는 토스트로 알린다.
- 열기 흐름: 아카이브를 열 때 `get_archive_images`와 `get_comic_info`를 병행 호출한다. 폴더 미리보기에서는 메타데이터를 읽지 않고, 이전 로드의 응답은 최신 로드 토큰이 아니면 커밋하지 않는다. 파싱 실패는 로드를 막지 않고 패널의 Comic 섹션에 에러로 표시한다.
- 아카이브 모드 제한: 휴지통 이동, 이름 변경은 안내와 함께 차단된다.
- 추출 가드: 엔트리 1개당 200MB, 아카이브별 추출 디렉터리 1GB를 넘으면 `too_large`로 중단한다.
- 목록은 추출과 일치하도록 중복 엔트리 이름을 1회만 노출한다(zip `by_name`은 첫 항목만 돌려준다).
- 표시용 추출 경로는 추출 전에 `mark_in_use`로 보호하고, 선추출(prefetch)은 보호 슬롯을 소비하지 않는다. `load_archive_image`의 `protect`는 기본 `true`이며 UI의 선로딩은 `false`를 전달한다.
- 탐색/썸네일은 엔트리 목록 기준으로 동일하게 동작한다.

## 9. 캐시/썸네일/프리페치

진실: `crates/araview-core/src/process_temp.rs`, `crates/araview-core/src/thumbnail.rs`, `crates/araview-core/src/cache.rs`, `src-gpui/src/app/pages.rs`, `src-gpui/src/app/dock.rs`.

### 9.1 이미지 캐시 모드

기본 `nearby`. 이 설정은 이미지 미리 로드 범위와 페이지 캐시의 항목 한도를 설정한다.

| 모드         | 항목 상한 | 바이트 상한 | 프리페치 거리 |
| ------------ | --------- | ----------- | ------------- |
| `off`        | 1         | 무제한      | 0             |
| `nearby`     | 24        | 무제한      | 1             |
| `extended`   | 64        | 무제한      | 3             |
| `memory-1gb` | 매우 큼   | 1GB         | 2             |
| `memory-2gb` | 매우 큼   | 2GB         | 3             |

- 일반 파일 프리페치 거리 보정: 웹툰은 기본 x 2, 양쪽은 기본 + 1, single은 기본.
- 캐시 키는 파일 경로 기준이며, 디코드한 페이지는 항목 한도 안에서만 메모리에 두고 한도 밖은 GPU 텍스처까지 내려놓는다.
- `memory-*`는 실제 프로세스 RAM 전체 사용량이 아니라 페이지 캐시 항목 예산이다.

### 9.2 저장 방식

- `cacheStorageMode`는 `temporary | persistent`이며 기본값은 `persistent`다.
- `persistent`는 `%LOCALAPPDATA%\<식별자>\cache-v2\`를 사용하며 앱 재실행 뒤에도 썸네일, sidecar, 축소본, 아카이브 추출물을 재사용한다.
- `temporary`는 사용자 앱 캐시 디렉터리의 버전된 `session-v1/` 아래 프로세스별 `TempDir`를 사용한다. 정상 종료 이벤트와 다음 시작 시 이전 세션 루트를 정리해 종료·충돌 후에도 임시 파일을 남기지 않는다. 저장 방식 변경은 다음 실행부터 적용된다.
- 영구 캐시는 OS가 지울 수 있는 best-effort 데이터다. 원본이나 렌더 결과의 진실 원본은 아니다.
- 설치 빌드(`com.araview.viewer`)와 개발 빌드(`com.araview.viewer.dev`)는 식별자가 달라 캐시 루트가 분리된다.
- 시작 시 이전 `cache-v*` 버전 디렉터리와 이전 프로세스의 orphan temp 파일을 정리한다.

### 9.3 썸네일

- `generate_thumbnail`: 기본 256px, JPEG 캐시 후 재사용한다. 상한(500MB)을 넘기면 오래된 것부터 상한의 90%까지 제거한다. 상한 초과는 새로 쓴 파일 크기를 누적해 판단하므로, 쓸 때마다 폴더 전체를 다시 훑지 않는다(HEIC/PSD sidecar의 `paint/`, 축소본의 `scaled/`, 클립보드 복사용 PNG의 `clipboard/`도 같다. 11.3절).
- HEIC/HEIF/AVIF/PSD/TGA/DDS/EXR/QOI는 썸네일용 JPEG sidecar 경로를 쓴다(AVIF는 표시만 네이티브이고 썸네일은 libheif로 디코드한다).
- 디코드 불가 입력(SVG 등)이나 아카이브 엔트리명은 에러를 내고, UI는 원본으로 폴백한다.
- 배치 조회와 아카이브 엔트리용 썸네일 API를 별도로 제공한다. 아카이브 썸네일은 추출물과 캐시를 재사용해 풀사이즈 로드를 피한다.
- 도크와 그리드의 아카이브 엔트리는 청크마다 `generate_archive_thumbnails_batch` 1회로 요청한다. 추출 디렉터리 식별은 배치당 한 번만 계산하고, 워커마다 아카이브를 한 번만 열어 자기 몫의 엔트리를 추출·축소한다. 실패한 엔트리와 배치 자체의 실패는 UI가 원본 로드로 폴백한다.
- JPEG는 DCT 단계에서 1/2, 1/4, 1/8로 줄여 디코드한다(요청 크기보다 작아지지 않는 가장 작은 배율). CMYK 등 이 경로가 다루지 않는 JPEG는 풀해상도 디코드로 처리한다.
- 배치 내부 워커 수는 논리 코어의 절반이며 4~8로 묶는다.
- 그리드는 보이는 창의 경로만 썸네일을 요청하고, 화면에 걸친 행을 미리 그려 두는 행보다 먼저 요청한다.
- 도크와 그리드는 보이는 창을 8개 청크로 지연 없이 요청해 끝난 청크부터 그린다. 창이 바뀌어 큐가 다시 시작돼도 이미 보낸 요청의 응답은 그대로 쓰고, 요청 중인 경로는 다시 요청하지 않는다.
- `get_cached_thumbnail`: 생성 없이 캐시에 있는 썸네일(256/128/96/72/48/32)만 반환한다. 큰 이미지(2MP 또는 1.5MB 이상)를 열 때 풀사이즈 디코드와 병행 조회해 첫 페인트 프리뷰로 쓰고, 원본 `onLoad`에서 걷는다. GIF는 제외한다.
- 생성된 썸네일·sidecar·축소본 캐시 hit은 파일 modification time을 갱신해 이후 LRU 정리에서 최근 사용 파일이 먼저 삭제되지 않게 한다. 갱신한 지 60초가 지나지 않은 파일은 다시 갱신하지 않는다.

### 9.4 표시 해상도 제한

진실: `crates/araview-core/src/process_temp.rs`, `crates/araview-core/src/thumbnail.rs`, `crates/araview-core/src/cache.rs`, `src-gpui/src/app/pages.rs`, `src-gpui/src/app/dock.rs`.

- 설정 `maxResolution`(기본 `original`)이 켜져 있으면 긴 변이 상한(4k=3840px, 1080p=1920px)을 넘는 래스터를 `scaled/` sidecar로 한 번만 축소해 렌더한다. 원본 파일은 바뀌지 않는다.
- 적용 대상은 `image` 크레이트가 디코드할 수 있는 래스터(PNG/JPEG/BMP/ICO/정지 WebP)이며, EXIF Orientation(5~8)은 픽셀에 반영한 뒤 축소한다. 알파 채널이 있으면 투명도 보존을 위해 PNG로 저장한다.
- GIF, 움직이는 WebP, 움직이는 PNG(APNG, 확장자가 `.png`여도 `acTL` 청크로 판별), SVG, 디코드 불가 포맷(AVIF 등)은 원본 바이트를 그대로 렌더한다.
- sidecar는 활성 캐시 루트의 `scaled/`에 캐시되며 상한(500MB)을 넘기면 오래된 것부터 상한의 90%까지 제거한다. 캐시 경로는 원본 식별 해시 + 상한 + 픽셀 아트 필터 알고리즘 revision 기준이라 설정이나 필터 알고리즘이 바뀌면 다른 사본을 만든다.
- sidecar 축소는 표시 정책과 무관하게 항상 보간 필터(`Triangle`)를 사용한다. Nearest 축소는 스크린톤 같은 주기 패턴을 계단·무아레로 깨뜨린다. `smooth`/`auto`는 JPEG로, `pixelated`는 lossless PNG로 저장한다. `auto`는 휴리스틱 분석을 실행해 결과를 후속 `detect_pixel_art`가 재사용하지만, 분류가 `pixel_art`여도 축소 필터에는 영향을 주지 않는다(픽셀 보존 표시는 확대 배율에서만 적용되기 때문). 분석은 원본을 바꾸지 않는다.
- 새 상한으로 로드한 `ImageInfo.width/height`는 축소 사본 기준이고, `source_path`는 항상 원본 파일이다(11절). 히스토그램은 렌더 바이트(`file_path`), EXIF/파일 상세는 원본(`source_path`) 기준이다.
- 해상도 상한이나 이미지 표시 정책을 바꾸면 UI는 페이지 캐시와 픽셀 보존 뷰를 비우고 현재 이미지를 새 기준으로 다시 로드한다.

### 9.5 캐시 관리 화면

- 설정 > 성능 탭에서 현재 활성 저장 방식, 전체 사용량, 종류별 사용량, 파일 수, 보호된 파일 수를 확인한다.
- `get_cache_stats`는 썸네일, 변환 이미지, 축소본, 아카이브 추출물, 기타 임시 파일을 분류한다.
- `clear_cache(scope)`는 `all`, `thumbnails`, `converted`, `scaled`, `archives`, `other`를 지원한다.
- 현재 화면이 쓰고 있는 파일과 `mark_in_use` 보호 파일은 삭제하지 않는다. 보호된 용량은 결과 통계에 남는다.
- 진행 중인 `*.tmp-<pid>-...` 파일은 통계와 삭제 대상에서 제외한다.
- 삭제 성공 후 UI는 페이지 캐시와 썸네일 상태를 무효화한다. 다음 탐색에서 필요한 캐시를 다시 만든다.

## 10. EXIF/파일 정보

진실: `crates/araview-core/src/ops/metadata.rs`, `crates/araview-core/src/image_info.rs`, `src-gpui/src/app/info_panel.rs`.

- `get_exif_data`는 문자열 맵을 반환한다. EXIF가 없거나 읽을 수 없는 파일은 오류가 아니라 빈 맵이고, UI는 이를 "EXIF 없음" 상태로 보여준다. 파일이 없으면 `not_found` 오류다.
- 파일 없음이면 `not_found`, EXIF 없으면 `unsupported`.
- `I`로 패널 토글. 패널 내부 포커스에서는 `I` 닫기를 허용한다.
- 패널은 데이터를 기다리지 않고 바로 열린다. 읽는 동안(`infoLoading`) 로딩 안내를 보여주고, EXIF/히스토그램/파일 상세는 도착하는 대로 채운다. 읽는 중에는 아직 오지 않은 섹션을 "없음"으로 표시하지 않는다.
- 표시 범주는 Camera, Exposure, Image, Lens, DateTime, GPS, Software 계열이다.
- HEIC는 원본 경로 기준 EXIF를 읽으므로 비어 있는 경우가 많다.
- JPEG의 Orientation은 표시·썸네일에 반영한다. 패널에는 EXIF 원문 설명을 그대로 보여준다.
- Comic 섹션: 아카이브 모드이고 `showComicInfo`가 켜져 있고(기본 켜짐, 설정 보기 탭 읽기) `comicInfo`가 있으면 파일/히스토그램/EXIF보다 위에 표시한다. 설정을 끄면 읽은 메타데이터는 유지한 채 섹션만 숨긴다(표지 판정은 계속 동작한다, 6절). 첫 줄은 `Series #Number`(있는 것만), 둘째 줄은 `Title`, 이어서 Writer, Penciller, Inker, Colorist, Letterer, CoverArtist, Editor, Publisher, 발행일(`Year-Month-Day`, 연도가 있을 때만), 읽기 방향(`Manga`가 `YesAndRightToLeft`/`No`일 때만), Genre, Tags, Volume, Count, PageCount, LanguageISO, AgeRating, CommunityRating, 마지막에 Summary를 줄바꿈 그대로 표시한다. 값이 없는 필드는 행을 만들지 않는다. 파싱 실패면 재시도 버튼 없이 에러 문구만 남기고(복구 수단은 아카이브 다시 열기), 메타데이터가 없으면 섹션을 숨긴다.

### 10.1 히스토그램 `get_image_histogram`

- 입력 `file_path`, 반환 `Histogram`(16절).
- 디코드 불가 포맷(SVG, AVIF 등)은 에러를 내고, UI는 차트 대신 안내 문구를 표시한다.
- 최대 변 256px로 줄여 집계한다. JPEG는 썸네일과 같은 DCT 축소 디코드로 풀해상도 디코드를 건너뛴다.
- 렌더는 신규 의존성 없이 SVG 영역 차트이다.

### 10.2 파일 상세 `get_image_details`

- 입력 `file_path`, 반환 `ImageDetails`(16절).
- EXIF가 없어도 명령은 성공한다. 디코드 실패 파일도 상세를 반환한다.
- 색상 모드는 픽셀을 전부 디코드하지 않고 디코더의 `color_type()`으로 판정한다.
- ICC 검사는 jpg/png만 한다. JPEG은 선두 1MiB(+최대 세그먼트 길이) prefix만, PNG는 청크를 스트리밍으로 걸어 iCCP 데이터만 읽는다(전체 파일 로드 없음).
- 패널 표시: 경로(아카이브 모드면 아카이브 경로와 엔트리명 함께), 크기, 치수 + 픽셀 수, 생성/수정 시각(아카이브 모드 숨김), 색상 + 비트/채널, DPI, 색상 프로파일.
- 조회 실패가 있어도 패널은 열리고 섹션별 안내 문구를 표시한다.

## 11. 파일 작업

진실: `src-gpui/src/app/file_ops.rs`, `src-gpui/src/app/menu.rs`, `crates/araview-core/src/ops/file_ops.rs`, `crates/araview-core/src/clipboard_png.rs`.

공통: 아카이브 모드(전체/미리보기)면 원본 아카이브 경로를 대상으로 삼는다. 단, 휴지통/이름 변경은 아카이브에서 차단된다. 파일 작업은 렌더 경로(`file_path`)가 아니라 사용자가 연 원본(`source_path`)을 대상으로 한다(9.4절).

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

### 11.3 클립보드 복사 `Ctrl+C`

- `export_clipboard_png`: 렌더 바이트(`file_path`)를 코어가 디코드해 PNG로 재인코딩하고, 파생 이미지 캐시의 `clipboard/` 아래 발행해 경로를 반환한다. UI가 그 PNG를 클립보드에 쓴다.
- 표시와 같은 내용을 복사한다: EXIF Orientation은 표시와 같은 기준으로 JPEG에만 픽셀에 반영하고(§18), GIF/APNG/움직이는 WebP는 첫 프레임만 담는다. SVG는 내재 치수로 래스터화하되 긴 변 8192px 상한을 둔다.
- 알파는 렌더 바이트에 있는 만큼 유지한다(AVIF는 libheif RGBA 디코드, SVG는 resvg 래스터화). AVIF는 8비트로 디코드하므로 10비트/HDR 원본은 8비트로 내려간다.
- 일반 래스터는 1억 픽셀(할당 1GiB)을 넘으면 `TooLarge`로 거부한다. HEIC/HEIF/PSD 등 JPEG sidecar로 표시하는 포맷은 sidecar를 복사하므로 알파가 없다.
- 픽셀 값은 색 변환하지 않고, 렌더 바이트의 ICC 프로파일을 PNG에 옮겨 싣는다(JPEG/PNG/WebP/AVIF). 디코드 결과와 색 공간이 다른 프로파일(CMYK 등)은 싣지 않는다.
- 결과 PNG는 원본 식별 해시 기준으로 캐시되며, 상한(500MB)을 넘기면 오래된 것부터 상한의 90%까지 제거한다(9절). `clear_cache`에서는 기타(other) 범주다.
- 이미지가 없거나 변환·복사 실패 시 에러 안내를 표시한다.

### 11.4 경로/외부 열기

- `Ctrl+Shift+C`: 유효 경로를 텍스트로 복사한다.
- `Ctrl+Shift+E`: 탐색기에 표시한다.
- `Ctrl+Shift+O`: 기본 앱으로 연다.

## 12. 최근 파일/영속화

진실: `src-gpui/src/settings.rs`.

`settings.json` 키(`%APPDATA%\<식별자>\settings.json`):

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

진실: `src-gpui/src/settings.rs`(`Settings`).

| 설정                    | 값                                                                      | 기본값                             |
| ----------------------- | ----------------------------------------------------------------------- | ---------------------------------- |
| `language`              | `ko \| en`                                                              | `ko`(초기 로드는 시스템 감지 우선) |
| `loopNavigation`        | 끝에서 루프 여부                                                        | `false`                            |
| `cacheMode`             | `off \| nearby \| extended \| memory-1gb \| memory-2gb`                 | `nearby`                           |
| `cacheStorageMode`      | `temporary \| persistent`                                               | `persistent`                       |
| `maxResolution`         | `original \| 4k \| 1080p` (긴 변 상한, 9.4절)                           | `original`                         |
| `imageScalingMode`      | `auto \| smooth \| pixelated` 이미지 보간 방식                          | `auto`                             |
| `autoDetectPixelArt`    | 자동 모드에서 픽셀 아트 감지 사용                                       | `true`                             |
| `viewMode`              | `single \| left-to-right \| right-to-left \| webtoon`                   | `single`                           |
| `webtoonPageBoundaries` | 웹툰 페이지 경계선 표시                                                 | `false`                            |
| `webtoonFitWidth`       | 웹툰 이미지를 읽기 영역 너비까지 확대                                   | `false`                            |
| `webtoonShowProgress`   | 웹툰 현재 장 번호와 스크롤 진행률 표시                                  | `true`                             |
| `webtoonThumbnailJump`  | 진행 표시에서 썸네일 그리드로 바로가기                                  | `true`                             |
| `autoOpenLastFile`      | 시작 시 마지막 파일 자동 열기                                           | `false`                            |
| `recordRecentFiles`     | 최근 기록 유지                                                          | `true`                             |
| `viewerBackground`      | `theme \| black \| white \| checker`                                    | `theme`                            |
| `autoHideUI`            | 읽기 중 크롬 자동 숨김                                                  | `false`                            |
| `menuBarHidden`         | 상단바 수동 숨김 (이미지 보기 중만, 상단 호버 시 peek)                  | `false`                            |
| `alwaysOnTop`           | 항상 위                                                                 | `false`                            |
| `sortKey`               | `name \| date \| size`                                                  | `name`                             |
| `sortDescending`        | 내림차순                                                                | `false`                            |
| `includeSubfolders`     | 하위 폴더 포함(재귀)                                                    | `false`                            |
| `skipBrokenFiles`       | 손상 파일 자동 건너뛰기                                                 | `false`                            |
| `resumeReading`         | 아카이브 재진입 시 이어보기                                             | `true`                             |
| `showCoverAlone`        | 양쪽 보기에서 첫 페이지(표지)를 단독 표시 (지정 없는 아카이브의 기본값) | `true`                             |
| `showWidePageAlone`     | 양쪽 보기에서 가로가 더 긴 페이지(가로 스캔)를 단독 표시                | `true`                             |
| `showComicInfo`         | 정보 패널에 만화 정보(ComicInfo.xml) 섹션 표시                          | `true`                             |
| `comicAutoDualView`     | 아카이브(만화)를 열면 자동으로 양쪽 보기, ComicInfo 방향 적용           | `true`                             |
| `fitMode`               | 맞춤 기억 `width \| height \| screen \| auto`                           | `auto`                             |
| `dockPosition`          | 이미지 목록 위치 `top \| bottom \| left \| right`                       | `bottom`                           |
| `dockVisible`           | 이미지 목록 표시                                                        | `true`                             |
| `dockThumbSize`         | 썸네일 크기 `s \| m \| l`                                               | `s`                                |
| `dockShowName`          | 썸네일 파일명 표시                                                      | `false`                            |
| `dockShowIndex`         | 썸네일 번호 표시                                                        | `false`                            |
| `shortcuts`             | 단축키 맵                                                               | 아래 기본표                        |
| `wheel`                 | 휠 맵                                                                   | 아래 기본표                        |
| `mouse`                 | 마우스 맵                                                               | 아래 기본표                        |

설정 항목에 연결된 단축키가 있으면 항목 옆에 현재 할당된 단축키를 배지로 표시한다. 재할당하거나 해제하면 배지도 즉시 따라간다.

## 14. 단축키/휠/마우스/명령 팔레트

진실: `src-gpui/src/settings.rs`(기본 단축키·휠·마우스), `src-gpui/src/keys.rs`, `src-gpui/src/app/menu.rs`(명령 팔레트).

### 14.1 기본 단축키

| 동작                   | 기본값            |
| ---------------------- | ----------------- |
| 이전                   | `ArrowLeft`       |
| 다음                   | `ArrowRight`      |
| 왼쪽 팬                | `Ctrl+ArrowLeft`  |
| 오른쪽 팬              | `Ctrl+ArrowRight` |
| 위 팬                  | `ArrowUp`         |
| 아래 팬                | `ArrowDown`       |
| 확대                   | `=`               |
| 축소                   | `-`               |
| 보기 초기화            | `0`               |
| 가로 맞춤              | `1`               |
| 세로 맞춤              | `2`               |
| 화면 맞춤              | `3`               |
| 파일 열기              | `Ctrl+O`          |
| 이미지 닫기            | `Escape`          |
| EXIF                   | `I`               |
| 시계 회전              | `R`               |
| 반시계 회전            | `Shift+R`         |
| 좌우 반전              | `H`               |
| 상하 반전              | `V`               |
| 전체화면               | `F11`             |
| 항상 위                | `T`               |
| 이미지 복사            | `Ctrl+C`          |
| 휴지통                 | `Delete`          |
| 탐색기에 표시          | `Ctrl+Shift+E`    |
| 기본 앱으로 열기       | `Ctrl+Shift+O`    |
| 배경 순환              | `B`               |
| 이름 변경              | `F2`              |
| 경로 복사              | `Ctrl+Shift+C`    |
| 명령 팔레트            | `Ctrl+K`          |
| 10장 이전              | `PageUp`          |
| 10장 다음              | `PageDown`        |
| 처음                   | `Home`            |
| 마지막                 | `End`             |
| 썸네일 그리드          | `G`               |
| 애니메이션 재생/정지   | `P`               |
| 이전 애니메이션 프레임 | `,`               |
| 다음 애니메이션 프레임 | `.`               |

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
- 이미지가 있을 때만, 이동 가능할 때만, 제어 가능한 GIF일 때만, 아카이브 안일 때만(표지 지정) 활성화되는 명령이 있다. `system` 그룹은 항상 활성이다.
- 공백 분리 토큰 AND 매칭이며 라벨 앞부분 일치를 우선한다.
- 한국어 UI에서도 영문 별칭으로 검색된다.

## 15. 코어 연산 계약

진실: `crates/araview-core/src/ops/`, `crates/araview-core/src/cache.rs`, `crates/araview-core/src/thumb_shell.rs`, `src-gpui/src/app/menu.rs`(`run_action`).

| 연산                                     | 입력 (camelCase)                                                                            | 반환                                   |
| ---------------------------------------- | ---------------------------------------------------------------------------------------------- | -------------------------------------- |
| `load_image`                             | `filePath`, `maxSide?`, `imageScalingMode?`, `autoDetectPixelArt?`                             | `ImageInfo`                            |
| `detect_pixel_art`                       | `filePath`                                                                                     | `PixelArtDetection`                    |
| `get_directory_images`                   | `filePath`, `options?`                                                                         | `DirectoryImages`                      |
| `resolve_dropped_path`                   | `path`                                                                                         | 해석된 파일 경로 `string`              |
| `get_exif_data`                          | `filePath`                                                                                     | `Record<string, string>`               |
| `get_image_histogram`                    | `filePath`                                                                                     | `Histogram`                            |
| `get_image_details`                      | `filePath`                                                                                     | `ImageDetails`                         |
| `get_archive_images`                     | `filePath`                                                                                     | `DirectoryImages`(엔트리 목록)         |
| `get_comic_info`                         | `filePath`                                                                                     | `ComicInfo \| null`                    |
| `set_comic_cover_pages`                  | `filePath`, `coverPages`                                                                       | `ComicInfo`                            |
| `load_archive_image`                     | `archivePath`, `entryName`, `maxSide?`, `protect?`, `imageScalingMode?`, `autoDetectPixelArt?` | `ImageInfo`                            |
| `archive_prefetch`                       | `archivePath`, `entryNames`                                                                    | 추출 개수 `number`                     |
| `generate_thumbnail`                     | `filePath`, `maxSide?`                                                                         | `ThumbnailInfo`                        |
| `generate_thumbnails_batch`              | `filePaths`, `maxSide?`                                                                        | `BatchThumb[]`                         |
| `get_cached_thumbnail`                   | `filePath`                                                                                     | `ThumbnailInfo \| null`(캐시 히트만)   |
| `get_cache_stats`                        | 없음                                                                                           | `CacheStats`                           |
| `clear_cache`                            | `scope`                                                                                        | `CacheClearResult`                     |
| `generate_archive_thumbnail`             | `archivePath`, `entryName`, `maxSide?`                                                         | `ThumbnailInfo`                        |
| `generate_archive_thumbnails_batch`      | `archivePath`, `entryNames`, `maxSide?`                                                        | `BatchThumb[]`(source=엔트리 이름)     |
| `generate_archive_file_thumbnail`        | `archivePath`, `maxSide?`                                                                      | `ThumbnailInfo`(첫 이미지 엔트리 기준) |
| `generate_archive_file_thumbnails_batch` | `archivePaths`, `maxSide?`                                                                     | `BatchThumb[]`(source=아카이브 경로)   |
| `get_file_associations`                  | 없음                                                                                           | `FileAssociation[]`                    |
| `set_file_association`                   | `extension`, `associate`                                                                       | `FileAssociation`                      |
| `open_default_apps_settings`             | 없음                                                                                           | 없음                                   |
| `get_license_bundle`                     | 없음                                                                                           | `LicenseBundle`(GPUI 앱은 미사용)                        |
| `get_psd_thumbnail_status`               | 없음                                                                                           | `PsdThumbStatus`                       |
| `register_psd_thumbnail`                 | 없음                                                                                           | `PsdThumbStatus`                       |
| `unregister_psd_thumbnail`               | 없음                                                                                           | `PsdThumbStatus`                       |
| `trash_file`                             | `filePath`                                                                                     | 없음                                   |
| `rename_file`                            | `oldPath`, `newName`, `maxSide?`, `imageScalingMode?`, `autoDetectPixelArt?`                   | `ImageInfo`                            |
| `export_clipboard_png`                   | `filePath`                                                                                     | `ClipboardPng`                         |

`load_image`, `load_archive_image`, `rename_file`은 표시 정책에 따라 선택적으로 `imageScalingMode`(`auto | smooth | pixelated`)와 `autoDetectPixelArt`를 받는다.

코어 함수 이름은 표의 연산 이름에 `_impl` 또는 `_blocking`을 붙인다(예: `get_directory_images_impl`). 응답은 snake_case 필드를 가진 Rust 타입이다. 설정 화면의 라이선스는 `get_license_bundle` 대신 컴파일 시점에 포함된 `src-gpui/THIRD_PARTY_LICENSES.json`을 읽는다.

파일 연결 주의: 설정에서 연결 변경은 해당 확장자의 Windows 기본 앱 선택 창을 연다. 조용한 UserChoice 레지스트리 쓰기는 할 수 없다.

개발 빌드의 별도 등록(`AraView (Dev)`)은 `docs/development.md`를 따른다.

## 16. 데이터 모델

진실: `crates/araview-core/src/image.rs`, `crates/araview-core/src/thumbnail.rs`, `crates/araview-core/src/file_assoc.rs`, `crates/araview-core/src/display.rs`.

### 16.1 `ImageInfo`

Rust와 TypeScript는 같은 모양을 유지한다.

- `file_path: string`: 화면이 디코드할 경로. HEIC/HEIF/PSD/TGA/DDS/EXR/QOI는 JPEG sidecar 경로, 표시 해상도 제한이 걸린 큰 래스터는 `scaled/` 사본 경로일 수 있다(9.4절).
- `source_path: string`: 사용자가 연 원본 파일 경로. sidecar가 아니며, 파일 작업·EXIF·파일 상세가 이 경로를 쓴다. 아카이브 엔트리는 활성 파생 이미지 캐시에서 추출된 경로다.
- `mime_type: string`
- `file_name: string`
- `file_size: number`
- `width: number | null`, `height: number | null`: 렌더 바이트 기준 치수(축소 사본이면 사본 치수). SVG는 헤더 파싱으로 복원하며 해석 불가분만 null.

- `PixelArtDetection`: `{ classification: "pixel_art" | "continuous" | "uncertain", confidence: number, pixel_scale: number | null, method: "runs" | "edges" | "hybrid" | "unsupported" }`. `pixel_scale`은 픽셀 아트로 분류된 경우의 추정 격자 주기이며, 분석은 표시 힌트일 뿐 원본 파일을 변경하지 않는다.

### 16.2 기타

- `DirectoryImages`: `{ images: string[], current_index: number, availability: ("local"|"cloud_only"|"unknown")[] }`.
- `ThumbnailInfo`: `{ file_path: string, width: number, height: number }`.
- `ClipboardPng`: `{ file_path: string }`. 클립보드에 쓸 PNG의 파생 이미지 캐시 경로다(11.3절).
- `ExifData`: `Record<string, string>`.
- `Histogram`: `{ r: number[256], g: number[256], b: number[256], sampled_pixels: number }`.
- `ImageDetails`: 16.2 모양 그대로. 색상 모드, 비트/채널, 생성/수정 시각, DPI, ICC 상태를 포함한다.
- `ArchiveState`: `{ archivePath: string | null }`.
- `ComicInfo`: 아카이브의 `ComicInfo.xml`(8절). `{ title, series, number, summary, writer, penciller, inker, colorist, letterer, cover_artist, editor, year, month, day, publisher, genre, tags, language_iso, age_rating, community_rating, manga: string | null, count, volume, page_count: number | null, pages: ComicPage[] | null }`. `number`와 `community_rating`은 소수 값을 보존하려고 문자열이다.
- `ComicPage`: `{ image: number, page_type: string | null }`. `image`는 ComicRack 스키마대로 0 기반 페이지 인덱스이고 `page_type`은 `FrontCover` 같은 값이다. 양쪽 표지 판정(6절)의 근거다.
- `FileAssociation`: `{ extension, associated, current_prog_id, needs_os_confirmation }`.
- `PsdThumbStatus`: 탐색기 썸네일 등록 상태(20.2절).
- `DirListOptions`: camelCase `{ sortKey, descending, recursive }`.
- `CacheStats`: `{ storage_mode: "temporary"|"persistent", persistent_available, total_bytes, file_count, protected_bytes, protected_file_count, total_limit_bytes, categories }`.
- `CacheCategoryStats`: `{ key: "thumbnails"|"converted"|"scaled"|"archives"|"other", bytes, file_count, protected_bytes, protected_file_count, limit_bytes }`.
- `CacheClearResult`: `{ removed_bytes, removed_file_count, failed_file_count, stats }`.

## 17. 에러 모델

진실: `crates/araview-core/src/app_error.rs`.

백엔드 `ErrorCode` 8종(snake_case 직렬화):

`not_found`, `permission`, `unsupported`, `too_large`, `corrupt`, `invalid_input`, `already_exists`, `unknown`.

UI 분류 5종:

- `not-found`: 파일 이동/삭제, 빈 폴더/아카이브.
- `permission`: 권한/잠금.
- `unsupported`: 미지원 포맷, EXIF 없음 포함.
- `corrupt`: 디코드/읽기 실패, 손상.
- `unknown`: 그 외.

규칙:

- 모든 command는 `Result<T, AppError>`이다.
- `io` 에러는 NotFound/Permission을 정확히 매핑하고 나머지는 호출자가 고른 fallback을 쓴다.
- 새 코어 에러 문구를 추가하면 UI의 에러 분류(`src-gpui/src/app.rs`의 에러 카드)도 함께 확인한다.
- 표시는 번역 키로 보여준다.

## 18. 렌더링 경로

- 코어는 디코드용 파일 경로를 돌려주고, UI는 그 파일을 `display.rs`로 RGBA 프레임으로 디코드해 GPU 텍스처(타일)로 올려 그린다. GPUI 텍스처는 한 변 16384px 제한과 밉맵 없는 선형 보간뿐이라 큰 이미지는 8192px 타일로 나누고 축소 단계를 CPU에서 만든다(`src-gpui/src/picture.rs`).
- 디코더가 따로 필요한 HEIC/HEIF/PSD/TGA/DDS/EXR/QOI는 JPEG sidecar를(`transcode.rs`가 디코더를 고르고 `sidecar.rs`의 `SidecarSpec` 파이프라인을 공유한다), 표시 해상도 제한이 걸린 큰 래스터는 `scaled/` 사본(9.4절)을 만든다. sidecar와 아카이브 추출물, 썸네일 캐시는 `cacheStorageMode`가 `persistent`면 `%LOCALAPPDATA%\<식별자>\`의 버전된 루트(`cache-v2/`)에, `temporary`면 프로세스 수명 TempDir에 둔다.
- JPEG의 EXIF Orientation(1~8)은 디코드할 때 적용하고, 치수(`ImageInfo.width/height`, `get_image_details`)와 썸네일도 같은 기준으로 회전해 보고한다(SVG/WebP/PNG/HEIC는 대상 아님).
- `detect_pixel_art`는 표시 바이트 경로를 제한된 분석 이미지로 읽고, 작은 색상 팔레트·평탄도·동일 색상 run·주기적 경계 신호를 결합한다. ML 모델이나 네트워크를 사용하지 않으며, 분석 제한 초과·디코드 실패·불확실 결과는 안전하게 부드러운 표시로 대체한다.
- 표시 보간은 `smooth`(GPU 선형 보간과 CPU 축소 단계)와 `pixelated`(최근접 확대 래스터) 두 가지다. 픽셀 보존 판정(pixelated 모드와 확신 있는 자동 감지 포함)은 표시 배율이 1x 이상(확대)일 때만 적용되며, 축소 배율에서는 설정·감지와 무관하게 항상 `smooth`로 강제된다. 자동 모드는 추가로 표시 배율이 2x 이상이면 감지 결과와 무관하게 `pixelated`로 전환한다(원본 픽셀 표시). nearest 축소는 스크린톤 같은 주기 패턴을 계단·무아레로 깨뜨린다. 단일 보기 배율은 이미지 크기에 대한 `zoom`이고, 웹툰/양쪽 보기는 렌더된 크기에서 실측한다(측정 전에는 기존 판정 유지).
- 사용자 원본 파일은 필터링하지 않는다. 표시 해상도 상한 sidecar와 썸네일은 기존 파생 이미지 파이프라인을 유지하며, 픽셀 보존 판정은 메인 이미지 표시 힌트로만 사용한다.

## 19. 다국어

진실: `src-gpui/src/i18n.rs`, `src-gpui/locales/ko.json`, `src-gpui/locales/en.json`.

- 지원 언어: `ko`, `en`.
- 시스템 감지: Windows 사용자 로캘 이름이 `ko`로 시작하면 `ko`, 아니면 `en`.
- 폴백: `ko`.
- 설정 변경 즉시 적용된다.

## 20. 윈도우/배포

진실: `src-gpui/installer/araview.nsi`, `scripts/Build-Installer.ps1`, `src-gpui/src/app/update.rs`, `docs/releasing.md`, `.github/workflows/release.yml`, `.github/workflows/ci.yml`.

- 창: 제목 기본값 `AraView`(이미지가 열리면 파일명, 아니면 앱 이름), 1024x768, 최소 500x400, 프레임리스. 최소 너비 500은 Windows 11 Snap Layouts의 모든 배치에 창이 들어가기 위한 상한이다(Microsoft 권장 ≤500epx).
- 창 위치와 크기를 `settings.json`의 `window`에 저장하고 다음 실행에 복원한다.
- 읽기 영역 드래그는 팬·스크롤 여유가 없을 때 창 이동이 된다(§7.2).
- Windows 11 Snap Layouts: gpui-kit 타이틀바의 최대화 버튼은 창 컨트롤 영역(`WindowControlArea::Max`)으로 등록돼 있어 Windows가 `HTMAXBUTTON`으로 처리한다. 앱이 따로 오버레이를 두지 않는다. 헤더의 일반 버튼은 `.occlude()`로 묶어 캡션 드래그 영역으로 취급되지 않게 한다.
- 설치 프로그램: NSIS 하나만 만든다(`pwsh scripts/Build-Installer.ps1`, 결과물 `target/AraView-<버전>-setup.exe`). 1.x(Tauri) 설치본과 같은 정체성(설치 폴더 `%LOCALAPPDATA%\AraView`, 실행 파일 `araview.exe`, 제거 항목 `AraView`, 시작 메뉴 바로가기)을 써서 같은 자리에 덮어쓴다. 설치 중 앱이 실행 중이면 종료를 요청한다.
- 릴리스 파이프라인: `npm run release -- <버전>`(`scripts/release.mjs`)이 `src-gpui/Cargo.toml`과 `Cargo.lock`의 버전을 올려 커밋하고 `vX.Y.Z` 태그와 함께 푸시한다. 태그 푸시를 `.github/workflows/release.yml`이 받아 태그와 버전 파일의 일치, 서명 키 Secrets를 확인한 뒤 검증(cargo fmt/test/clippy, 보안 감사, 라이선스 검사), 설치 프로그램 빌드, 서명, `latest-gpui.json` 생성, 원본 저장소 `ara-hwang/araview` 릴리스 생성까지 자동으로 수행한다. `ci.yml`이 검증을 담당하고 릴리스가 이를 재사용한다. 권한과 절차 상세는 `docs/releasing.md`를 따른다.
- 파일 연결 3그룹:
  - Image 17종: png, apng, jpg, jpeg, gif, bmp, webp, svg, ico, avif, heic, heif, psd, tga, dds, exr, qoi.
  - Comic 1종: cbz.
  - Archive 1종: zip.
- HEIC/HEIF는 vcpkg `libheif[core,aom]` 동적 링크 + `libde265`(HEVC), `aom`(AV1, AVIF 썸네일/히스토그램)만 사용한다. 설치와 DLL 복사는 `docs/development.md`를 따른다.

### 20.1 자동 업데이트

수동 확인만 제공한다. 시작 시 자동 확인이나 백그라운드 폴링은 없다(오프라인 우선).

- 진입점: 설정 일반 탭의 `새 버전 확인` 버튼, 명령 팔레트의 `업데이트 확인`.
- 흐름: 업데이트 확인 → 없으면 최신 안내, 있으면 대화상자에서 다운로드를 시작한다. 내려받은 설치 프로그램은 서명을 앱에 내장한 공개키로 확인한 뒤에만 실행하고, 실행하면 앱이 종료된다. 설치와 새 버전 재실행은 NSIS가 담당하므로 수동 재시작 UI는 없다. 다운로드 진행률은 표시하지 않고 시작과 실패를 알린다. 중복 확인은 무시한다.
- HTTP 타임아웃: 확인 요청은 30초, 다운로드 요청은 10분(본문 수신을 포함한 요청 전체 기한)이다. 초과하면 각각 확인 실패와 다운로드 실패 알림으로 처리한다.
- 피드: 원본 저장소(`ara-hwang/araview`)의 최신 릴리스에 올라간 `latest-gpui.json`이고(버전, 노트, 플랫폼별 URL과 서명), 서명 공개키는 `src-gpui/update-pubkey.txt`다. 1.x(Tauri) 앱이 보던 `latest.json`은 2.0부터 올리지 않으므로 1.x는 자동으로 2.0을 받지 못하고 직접 설치해야 한다.
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

작업 절차는 `AGENTS.md`와 `docs/playbooks.md`를, 코드 작성 규칙은 `CODING_STANDARDS.md`를 따른다. 변경 후 아래 SPEC 절을 갱신한다.

- 포맷 추가: 2절 + 필요 시 15/16절, `samples/` 검증.
- 코어 연산 추가: 15절 연산 표 + 16절 데이터 모델.
- 설정/단축키 추가: 12~14절.
- 파생 이미지 캐시 변경: 9절 + 12~16절 + `docs/development.md`.
- 문서 정합성 검사: `npm run docs:check`(버전, 확장자, 설정 키, plan). CI에서 PR마다 돈다.
