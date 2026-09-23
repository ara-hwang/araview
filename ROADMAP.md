# Roadmap — 기능 추가 및 최적화 계획

> **AraView** 의 향후 기능 추가, 최적화 및 개선 아이디어를 정리한 로드맵입니다.
> 우선순위(P0 ~ P3)와 단계(Phase)를 기준으로 정리되어 있습니다.
>
> ## 구현 현황 (2026-09-14 기준 + 문서 동기화)
>
> - ✅ 완료: 뷰 모드 렌더(`useMultiPageImages` + `ImageContainer`), 회전/뒤집기(`appStore` + CSS transform + `R/Shift+R/H/V`), 클립보드 복사 PNG(`useCopyImage`), 최근 파일 목록(최대 20개, `settings.json`), Asset Protocol 경로 기반 렌더링(base64 제거).
> - ✅ Phase 1 폴리시 완료: 설정 UI 노출(viewMode/자동열기), 헤더 회전/뒤집기 + 컨텍스트 메뉴 풀셋, 양면 2장 넘김 + 회전 bounds, 다중 DnD + 오버레이, 에러 분류 + 홈으로 복구, EXIF 아카이브修正·썸네일 alt·테마 데드코드 제거.
> - ✅ 포맷/성능 라운드: 디렉토리 스캔 캐시+워처(`dir_cache.rs` + `notify`), 썸네일 파이프라인(`thumbnail.rs` + `generate_thumbnail` + `useThumbnailSrcs`, 500MB cap), 렌더링 성능(rAF 팬 스로틀·`translate3d`+`will-change`·Webtoon 지연 로드·모드별 프리페치), CB7 아카이브 지원(`sevenz-rust2`, 15개 확장자). CBR/RAR/ZIP/7Z/CBT는 Step 4에서 추가 지원(20개 확장자, `rars` 퓨어 Rust, MIT/Apache-2.0). QOI/JXL/RAW는 JPEG sidecar 전제가 필요해 제외 유지. PSD는 읽기 전용 미리보기로 추가 지원(21개 확장자, `psd` 크레이트 + JPEG sidecar, PSB 제외).
> - ✅ 썸네일 그리드 뷰(2.1): `G` 토글, 뷰포트 가상화(`ThumbnailGrid.tsx` + `gridWindow.ts`), 파일명 필터, 아카이브 전용 `generate_archive_thumbnail` 백엔드, 팔레트/컨텍스트 메뉴/하단 바 진입점.
> - ✅ 문서 동기화(2026-09-14, 코드 대조): 2.4 i18n·2.5 DnD·3.4 메모리 최적화(핵심)·5.2 에러 처리를 완료로 확정. 3.5 렌더링(타일 미도입)·5.1 접근성(고대비 미지원)·5.3 테스트(훅/컴포넌트/통합 일부)·5.4 CI(PR용 워크플로 미분리)는 부분 완료로 재정의. 4.1 비교 모드·4.2 잔여분(크롭/리사이즈/밝기대비)·4.3 배치 작업·4.5 인쇄는 범위 제외.
> - ✅ 2.2 이미지 정보 패널 강화(2026-09-14): `get_image_histogram` + `get_image_details` 백엔드(`image_info.rs`), SVG 히스토그램 차트 + 파일 상세 섹션(`ExifPanel`).
> - ✅ EXIF Orientation 반영(2026-09-22): JPEG/TIFF의 Orientation(1~8)을 치수 계산, 썸네일, 편집 저장에 적용해 WebView2 표시와 일치시킴(`orientation.rs`).
> - ✅ GIF 재생 제어(2026-09-22): 단일 보기에서 캔버스 재생/정지·프레임 이동·카운터·명령 팔레트/컨텍스트 메뉴(`useGifPlayer`, `gifStore`). `ImageDecoder` 미지원·정지 GIF·웹툰/양면은 네이티브 폴백.
> - ✅ 로드맵 잔여분(2026-09-22): 고대비(`prefers-contrast`/`forced-colors`), 캐시 썸네일 프리뷰(`get_cached_thumbnail`), PR CI(`ci.yml`)와 커버리지 리포트(`npm run test:coverage`), 컴포넌트 테스트(`Header`·`ImageNavBar`·`SettingsDialog`), 로드→표시→이동 통합 플로우(`useDirectoryNavigation`), `commands.rs` 단위 테스트 보강. 타일 렌더링은 보류.

---

## Phase 1 — 미완성 기능 완성 및 핵심 개선

> 이미 코드베이스에 구조가 잡혀 있으나 아직 완성되지 않은 기능, 또는 사용자 경험에 직결되는 핵심 개선.

### 1.1 뷰 모드 구현 (P0) — ✅ 렌더 완료 / 폴리시 남음

**현재 상태 (2026-09-09)**: `settingsStore.ViewMode` + `ImageContainer` + `useMultiPageImages`로 `single(1장) / left-to-right(현재+다음) / right-to-left(현재+다음) / webtoon(전 구간 연속 수직 스크롤)` 렌더 완료. 남은 폴리시는 Phase 1-3 (모드별 넘김 단위, 프리페치, zoom/pan 활성화, 회전 bounds).

**당시 구현 계획 (보존)**:

- `ImageContainer.tsx`에서 `viewMode` 상태를 구독하여 레이아웃 분기
- **single**: 현재 동작 유지 (단일 이미지 표시)
- **left-to-right**: 두 페이지를 좌→우로 나란히 표시 (만화/코믹스용)
- **right-to-left**: 두 페이지를 우→좌로 나란히 표시 (일본 만화용)
- **webtoon**: 세로 스크롤로 여러 이미지를 연속 표시
- 각 모드별 zoom/pan 동작 조정
- 프리페치 전략을 뷰 모드에 맞게 조정 (webtoon은 앞쪽 이미지를 더 많이 프리페치)

**관련 파일**:

- `src/store/settingsStore.ts` — ViewMode 타입 (이미 존재)
- `src/components/ImageContainer.tsx` — 렌더링 로직 추가
- `src/hooks/useDirectoryNavigation.ts` — 모드별 페이지 넘김 단위 변경
- `src/hooks/useImageCache.ts` — 프리페치 전략 조정

---

### 1.2 이미지 회전 및 뒤집기 (P0) — ✅ 완료

**현재 상태 (2026-09-09)**: `appStore.rotation/flipH/flipV` + `ImageContainer` CSS transform + `R/Shift+R/H/V` + `resetZoomPan` 초기화 완료. 남은 것은 헤더 버튼/컨텍스트 메뉴 노출(Phase 1-2)과 90°/270° bounds 반영(Phase 1-3).

**당시 구현 계획 (보존)**:

- `appStore.ts`에 `rotation` (0, 90, 180, 270)과 `flip` (none, horizontal, vertical) 상태 추가
- `ImageContainer.tsx`에서 CSS `transform: rotate() scaleX()` 적용
- 단축키 추가: `R` (시계 방향 90°), `Shift+R` (반시계 방향 90°), `H` (수평 뒤집기), `V` (수직 뒤집기)
- 컨텍스트 메뉴 및 헤더 툴바에 회전 버튼 추가
- zoom/pan 계산 시 회전 각도 반영 (90°/270° 회전 시 가로/세로 교체)

**관련 파일**:

- `src/store/appStore.ts` — rotation, flip 상태 추가
- `src/components/ImageContainer.tsx` — CSS transform 적용
- `src/components/Header.tsx` — 회전 버튼 추가
- `src/hooks/useImageViewerHotkeys.ts` — 단축키 등록
- `src/hooks/useContextMenu.ts` — 메뉴 항목 추가
- `src/utils/zoomPanUtils.ts` — 회전 시 bounds 계산 수정

---

### 1.3 클립보드 복사 (P1) — ✅ 완료

**현재 상태 (2026-09-09)**: `useCopyImage`로 Asset URL fetch → Canvas PNG 재인코딩 → `ClipboardItem` 복사 + `Ctrl+C` 완료. 남은 것은 헤더 버튼 노출(Phase 1-2)과 파일 경로 복사 옵션.

**당시 구현 계획 (보존)**:

- `Ctrl+C`로 현재 이미지를 클립보드에 복사
- Tauri의 클립보드 플러그인 (`tauri-plugin-clipboard-manager`) 활용
- 컨텍스트 메뉴에 "이미지 복사" 항목 추가
- 파일 경로 복사 옵션도 함께 제공

**관련 파일**:

- `src-tauri/Cargo.toml` — clipboard 플러그인 의존성 추가
- `src-tauri/src/lib.rs` — 플러그인 등록
- `src/hooks/useImageViewerHotkeys.ts` — Ctrl+C 단축키
- `src/hooks/useContextMenu.ts` — 메뉴 항목 추가

---

### 1.4 슬라이드쇼 모드 (P1) — ❌ 제거 (2026-09-20)

제품 범위에서 제외했다. `useSlideshow`, `slideshowIntervalMs` 설정, `Space` 단축키, 진행바 오버레이, 설정 UI를 모두 제거했다. 제거 사유는 로드맵 범위 밖이며, 재도입 계획은 없다.

---

## Phase 2 — 사용자 경험 개선

> 기존 기능의 완성도를 높이고 편의 기능을 추가하는 단계.

### 2.1 썸네일 그리드 뷰 (P1) — ✅ 완료

**현재 상태 (2026-09-11)**: 완료. 썸네일 파이프라인(`generate_thumbnail`/`generate_thumbnails_batch`, `thumbnail.rs` + `process_temp/thumbs/` 500MB cap) 위에 `ThumbnailGrid.tsx`(뷰포트 가상화, overscan 2행, `gridWindow.ts`)를 올렸다. `G` 토글, 클릭/`Enter` 점프 후 닫기, `Esc`/`G` 닫기, 파일명 필터, 실패 셀 재시도, 뷰어 단축키 비활성화(disabled), 명령 팔레트/컨텍스트 메뉴/하단 바 버튼 진입점을 포함한다. 아카이브는 `generate_archive_thumbnail`(추출물·썸네일 캐시 재사용, 동시 4개)으로 풀사이즈 로드를 피한다.

**당시 구현 계획 (보존)**:

- `ImageNavBar.tsx`를 확장하여 썸네일 그리드 모드 추가
- 디렉토리 내 이미지를 격자로 표시 (클릭하여 이동)
- Rust 백엔드에 `generate_thumbnail` 커맨드 추가 (리사이즈된 이미지 반환)
- 썸네일 전용 캐시 (메인 캐시와 분리)
- 가상 스크롤로 대용량 폴더 지원 (1000+ 이미지)
- 단축키 `G`로 그리드 뷰 토글

**관련 파일**:

- `src-tauri/src/commands.rs` — generate_thumbnail 커맨드 추가
- `src/components/ThumbnailGrid.tsx` — 새 컴포넌트 생성
- `src/components/ImageNavBar.tsx` — 그리드 토글 버튼 추가
- `src/hooks/useThumbnailLoader.ts` — 썸네일 로딩 훅 생성

---

### 2.2 이미지 정보 패널 강화 (P1) — ✅ 완료 (2026-09-14 구현)

**현재 상태**: 구현됨. `get_image_histogram`(RGB 256빈, 256px 다운샘플 집계) + `get_image_details`(크기/치수/색상 모드/비트뎁스/생성·수정 시각/EXIF DPI/JPEG APP2·PNG iCCP ICC 검사) 백엔드(`image_info.rs`)와 SVG 영역 차트(`HistogramChart.tsx`) + 파일 섹션(`ExifPanel.tsx`)을 추가했다. `useExifLoader`가 EXIF와 병렬 로드하며, 디코드 불가 포맷은 섹션별 안내 문구로 처리한다. SPEC §10.1·§10.2.

**구현 계획 (당시 안, 보존)**:

- **히스토그램**: RGB 채널별 히스토그램 표시 (Rust에서 계산, 신규 의존성 없이 SVG 영역 차트로 시각화)
- **색상 프로파일**: ICC 프로파일 정보 표시
- **이미지 해상도**: DPI, 비트 뎁스 등 상세 정보
- **파일 상세 정보**: 생성일, 수정일, 파일 경로

**관련 파일**:

- `src-tauri/src/commands.rs` — get_image_histogram, get_image_details 커맨드 추가
- `src/components/ExifPanel.tsx` — 히스토그램 차트 및 추가 정보 표시
- `src/hooks/useExifLoader.ts` — 추가 데이터 로딩

---

### 2.3 최근 파일 목록 (P2) — ✅ 완료 (최대 20개)

**현재 상태 (2026-09-09)**: `recentFilesStore` + `settings.json` 영속화 + 홈 그리드 + 개별 삭제/전체 삭제 + `navigate(/image)` 완료. 남은 옵션은 앱 시작 시 마지막 이미지 자동 열기(Phase 1-7).

**당시 구현 계획 (보존, 상한은 20개로 확정)**:

- Tauri Store에 최근 열어본 파일 목록 저장 (당시 안 최대 50개, 현행 최대 20개로 `SPEC.md` §12 확정)
- 홈 화면(`index.tsx`)에 최근 파일 목록 표시
- 마지막으로 본 이미지를 앱 시작 시 자동으로 열기 옵션
- 최근 파일 목록 삭제 기능

**관련 파일**:

- `src/store/settingsStore.ts` — recentFiles 상태 추가
- `src/routes/index.tsx` — 최근 파일 목록 UI
- `src/hooks/useImageLoader.ts` — 파일 열 때 최근 목록에 추가

---

### 2.4 다국어 지원 (i18n) (P2) — ✅ 완료 (2026-09-14 코드 대조 확정)

**현재 상태**: 구현됨. `src/i18n/index.ts` + `locales/ko.json`·`en.json`(`i18next` + `react-i18next`), 시스템 언어 자동 감지(`detectSystemLanguage`), 설정→즉시 적용(`settingsStore.language` + `GeneralTabPanel` 선택 UI), 폴백 `ko`. 컴포넌트는 `useTranslation` 사용.

**구현 계획**:

- 경량 i18n 솔루션 도입 (예: `i18next` 또는 자체 key-value 방식)
- 한국어(ko) / 영어(en) 기본 지원
- 시스템 언어 자동 감지
- 설정에서 언어 변경 가능

**관련 파일**:

- `src/i18n/` — 새 디렉토리 (언어 파일)
- `src/store/settingsStore.ts` — language 설정 추가
- 모든 컴포넌트 — 하드코딩 텍스트 → i18n 키 교체

---

### 2.5 드래그 앤 드롭 개선 (P2) — ✅ 완료 (2026-09-14 코드 대조 확정)

**현재 상태**: 구현됨. `useImageLoader.handleDrop`이 여러 파일/폴더 동시 드롭을 처리한다. 아카이브 경로는 그대로 유지하고, 그 외는 `resolve_dropped_path`(파일 그대로·폴더 내 첫 이미지)로 해석, 실패 항목 건너뜀 + 전부 실패 시 `toast.drop.fail`, 2개 이상이면 `toast.drop.firstOf`. 드래그 깊이 카운터 기반 오버레이(`index.tsx`·`image.tsx` + `home.drop` 문구). SPEC §4.2와 일치.

**구현 계획**:

- 여러 파일 동시 드롭 시 첫 파일을 열고 나머지는 디렉토리 목록에 포함
- 폴더 드롭 시 폴더 내 이미지 목록 로드
- 드롭 영역 시각적 피드백 개선 (오버레이 애니메이션)

**관련 파일**:

- `src/hooks/useImageLoader.ts` — 멀티 파일/폴더 드롭 처리
- `src/components/ImageContainer.tsx` — 드롭 오버레이 UI

---

## Phase 3 — 성능 최적화

> 대용량 이미지 및 대량 파일 처리 시 성능을 개선하는 최적화.

### 3.1 스트리밍 이미지 디코딩 (P1) — ✅ 완료 (asset protocol + 캐시 썸네일 프리뷰)

**현재 상태 (2026-09-22)**: `ImageInfo.file_path` + `convertFileSrc` 경로 기반 렌더링으로 base64 제거 완료. HEIC만 JPEG sidecar transcoding. 큰 이미지(2MP 또는 1.5MB 이상)는 `get_cached_thumbnail`으로 스트립/그리드가 만든 캐시 썸네일(256/128/96/72/48/32)을 풀사이즈 디코드와 병행 조회해 먼저 깔고, 원본 `onLoad`에서 페이드 인한다. 생성 없는 조회라 첫 방문에는 프리뷰가 없고, 두 번째 방문부터 즉시 표시된다.

**당시 최적화 계획 (보존)**:

- **Tauri asset protocol** 활용: base64 대신 `asset://` 프로토콜로 파일 직접 로드
  - 메모리 사용량 약 33% 감소 추정 (base64 인코딩은 원본 대비 약 33% 크기 증가를 유발)
  - IPC 직렬화/역직렬화 비용 제거
- 점진적 로딩: 저해상도 프리뷰 먼저 표시 후 고해상도 로드
- `commands.rs`의 `load_image`를 asset protocol 기반으로 리팩토링

**관련 파일**:

- `src-tauri/src/commands.rs` — asset protocol 경로 반환으로 변경
- `src-tauri/src/lib.rs` — asset scope 설정
- `src-tauri/tauri.conf.json` — asset protocol 허용 범위 설정
- `src/hooks/useImageLoader.ts` — base64 대신 asset URL 사용
- `src/hooks/useImageCache.ts` — 캐시 전략 재설계 (URL 기반)

---

### 3.2 썸네일 생성 최적화 (P2)

**현재 상태 (포맷/성능 라운드)**: 완료. `thumbnail.rs`가 `image` 크레이트로 리사이즈 썸네일을 만들고 `process_temp/thumbs/`에 캐시(500MB cap, 오래된 순 제거). 사용자 폴더에 `.thumbcache/`를 만들지 않음. 백그라운드 비동기 생성은 프론트 `useThumbnailSrcs`의 fire-and-forget 호출로 처리(`tokio::spawn` 미도입).

**최적화 계획 (당시 안, 보존. 아래는 채택되지 않음)**:

- Rust 백엔드에서 `image` 크레이트로 리사이즈된 썸네일 생성
- 디렉토리 내 `.thumbcache/` 폴더에 썸네일 캐싱 (미채택: `process_temp/thumbs/`를 사용하며 사용자 폴더를 건드리지 않음)
- 백그라운드 스레드에서 비동기 생성 (`tokio::spawn`) (미채택: 프론트 fire-and-forget 호출로 처리)
- 네비게이션 바에서 원본 대신 썸네일 로드

**관련 파일**:

- `src-tauri/Cargo.toml` — `image` 크레이트 추가
- `src-tauri/src/commands.rs` — generate_thumbnail 커맨드
- `src/hooks/useImageCache.ts` — 썸네일 캐시 레이어 분리

---

### 3.3 디렉토리 스캐닝 최적화 (P2)

**현재 상태 (포맷/성능 라운드)**: 완료. `src-tauri/src/dir_cache.rs`가 디렉토리+옵션별 정렬 목록을 캐시. 일반 모드는 폴더 mtime 지문으로 적중 판정, 재귀 모드는 워처 기반. `notify` 크레이트 워처가 변경 시 해당 캐시 무효화. 캐시 상한 128개.

**최적화 계획**:

- 디렉토리 스캔 결과를 Rust 측에서 캐싱 (파일 수정 시간 기반 무효화)
- 파일 시스템 워처(`notify` 크레이트)로 변경 감지 및 증분 업데이트
- 초기 로드 시 현재 파일 주변만 먼저 반환하고, 전체 목록은 비동기로 완성

**관련 파일**:

- `src-tauri/Cargo.toml` — `notify` 크레이트 추가
- `src-tauri/src/commands.rs` — 캐시된 디렉토리 스캔
- `src-tauri/src/lib.rs` — 파일 시스템 워처 설정

---

### 3.4 메모리 사용량 최적화 (P2) — ✅ 핵심 완료 (2026-09-14 코드 대조 확정)

**현재 상태**: base64 오버헤드는 해소됨. Asset Protocol 전환 이후 프론트는 메타데이터만 캐시하고 캐시 키는 파일 경로 기준이다(`useImageCache.estimateImageBytes`는 `file_size` 기준, 구 `string.length × 2` 추정 제거). 픽셀 프리웜 상한 12개 + 미사용 시 해제, `cacheMode: off`에서 예열 중단으로 브라우저 이미지 캐시에 위임한다. 남은 선택 사항은 대용량 이미지 해상도 제한 옵션(4K 이상 리사이즈 표시) 1건이다.

**최적화 계획**:

- Phase 3.1의 asset protocol 도입으로 base64 오버헤드 제거
- 캐시 키를 파일 경로 기반으로 변경 (데이터를 메모리에 보관하지 않음)
- 브라우저 이미지 캐시를 활용한 메모리 관리 위임
- 대용량 이미지 로드 시 해상도 제한 옵션 (4K 이상은 리사이즈 후 표시)

**관련 파일**:

- `src/hooks/useImageCache.ts` — 캐시 전략 재설계
- `src/utils/cacheConfig.ts` — 캐시 설정 업데이트

---

### 3.5 렌더링 성능 최적화 (P3) — 부분 완료 (2026-09-14 코드 대조 재확인)

**현재 상태 (포맷/성능 라운드)**: 부분 완료. Single 모드 `translate3d` + `will-change: transform`, `useZoomPan` rAF 기반 팬 스로틀, Webtoon `IntersectionObserver` 지연 로드(`rootMargin 100%`), viewMode별 프리페치(single=d·양면=d+1·webtoon=d×2). 미도입: 타일 기반 렌더링, 휠 이벤트 throttling(`useWheelNavigation`은 직접 실행). 타일 렌더링은 딥줌 전용 대형 작업이라 당분간 보류한다(2026-09-22).

**최적화 계획**:

- `will-change: transform` CSS 속성으로 GPU 레이어 분리
- 고해상도 이미지에 대해 타일 기반 렌더링 검토
- `requestAnimationFrame` 기반 부드러운 zoom/pan 애니메이션
- zoom/pan 이벤트 throttling 최적화

**관련 파일**:

- `src/components/ImageContainer.tsx` — GPU 가속 CSS 적용
- `src/hooks/useZoomPan.ts` — rAF 기반 애니메이션
- `src/hooks/useWheelNavigation.ts` — 이벤트 throttling

---

## Phase 4 — 고급 기능

> 이미지 뷰어의 경쟁력을 높이는 고급 기능.
>
> 범위 제외(2026-09-14): 4.1 비교 모드·4.2 잔여분(크롭/리사이즈/밝기대비)·4.3 배치 작업·4.5 인쇄는 로드맵에서 제외한다. 4.2의 회전/반전 저장 파이프라인은 구현되어 있으며 SPEC §11.3에 문서화되어 유지된다.

### 4.4 추가 이미지 포맷 지원 (P2) — 의도적 제외 유지 (2026-09-14 재확인)

**현재 상태**: 21개 포맷 지원 (PNG, JPG, JPEG, GIF, BMP, WebP, SVG, ICO, TIFF, TIF, AVIF, HEIC, HEIF, PSD, CBZ, CB7, CBR, RAR, ZIP, 7Z, CBT). CB7/7Z는 `sevenz-rust2`(순수 Rust), CBR/RAR는 `rars`(MIT OR Apache-2.0 순수 Rust, RAR 1.3부터 RAR 7까지), CBT는 `tar` 크레이트로 목록/추출 지원. ZIP은 `zip` 크레이트 별칭.

**HEIC/HEIF**: 구현됨. `libheif-rs`가 vcpkg `libheif[core]`를 동적 링크하고, 로드 시 JPEG sidecar를 만듭니다. HEVC 디코드는 `libde265`만 쓰고 `x265`는 넣지 않습니다. WebView2는 HEIC를 그리지 못합니다.

**추가 후보**:

- **PSD**: Photoshop 파일 미리보기 — 구현됨. `psd` 크레이트(순수 Rust)로 합성 디코드 후 JPEG sidecar 렌더, 읽기 전용(편집 저장 미지원). PSB는 디코더가 없어 제외 유지. 탐색기 썸네일은 별도 opt-in `IThumbnailProvider`이며 `SPEC.md` §20.2를 따른다.
- **RAW**: 카메라 RAW 포맷 (CR2, NEF, ARW 등) — `rawloader` 크레이트. JPEG sidecar 전제가 필요해 제외 유지.
- **JXL (JPEG XL)**: 차세대 이미지 포맷 — `jxl-oxide` 크레이트. JPEG sidecar 전제가 필요해 제외 유지.
- **QOI**: 빠른 무손실 포맷. 백엔드 디코드(`image` 크레이트)는 가능하나 WebView2 네이티브 렌더 불가로 sidecar 전제가 필요해 제외 유지.
- **CBR/RAR**: 지원됨. `rars`(MIT OR Apache-2.0) 리더로 목록/추출(RAR 1.3부터 RAR 7까지, RAR4/RAR5 픽스처 회귀 테스트 포함). GPL-3.0이던 `unrar-rs`에서 교체했고 네이티브 의존성은 없다.

**관련 파일**:

- `src-tauri/Cargo.toml` — 포맷별 크레이트 추가
- `src-tauri/src/image.rs` — MIME 타입 및 확장자 매핑 추가
- `src/constants/imageExtensions.ts` — 프론트엔드 확장자 목록 업데이트
- `src-tauri/tauri.conf.json` — 파일 연결 설정 추가

---

## Phase 5 — 접근성 및 품질

> 앱의 전반적 품질과 접근성을 높이는 개선.

### 5.1 접근성 (A11y) 개선 (P1) — ✅ 완료 (2026-09-22 고대비 추가)

**현재 상태**: 구현됨. 파일명 기반 `alt`(Single/양면/Webtoon/썸네일/홈 카드), `Header` 전 버튼 `aria-label` + 단축키 병기, `ImageNavBar` 슬라이더·그리드 버튼 레이블, 썸네일 그리드 `listbox`/`option`, 에러 카드 `role="alert"` + `StatusBar` `role="status"`, 전역 `focus-visible` 링, 키보드 조작(단축키·그리드 화살표/`Enter`/`Esc`·팔레트), GIF 컨트롤 `aria-label`/프레임 카운터. 고대비는 `prefers-contrast: more` 토큰 강화와 `forced-colors: active` 대응(체커보드 대체, 시스템 Highlight 외곽선)으로 지원한다.

**관련 파일**:

- `src/components/Header.tsx` — ARIA 레이블 추가
- `src/components/ImageContainer.tsx` — alt 텍스트, role 속성
- `src/components/ImageNavBar.tsx` — ARIA 슬라이더 속성
- `src/components/ExifPanel.tsx` — 구조적 마크업 개선

---

### 5.2 에러 처리 개선 (P1) — ✅ 완료 (2026-09-14 코드 대조 확정)

**현재 상태**: 구현됨. 백엔드 구조화 에러 8종(`app_error.rs`) + 프론트 5종 분류(`appError.ts`, `titleKey`/`hintKey` 번역 표시), 에러 카드에 재시도/홈으로 복구/닫기(`ImageContainer.tsx`, `role="alert"`), 경미한 실패는 base 토스트(`@/components/ui/toast`), 손상 파일 자동 건너뛰기(`skipBrokenFiles` + `failedPaths`), 대안 동작으로 기본 앱 열기(`Ctrl+Shift+O`)·포맷 변환 저장(`save_image_edits`)이 있다. SPEC §17과 일치.

**개선 계획**:

- 에러 유형별 구체적 메시지 (파일 없음, 권한 없음, 손상된 파일, 미지원 포맷)
- 에러 발생 시 대안 제시 ("다른 뷰어로 열기", "포맷 변환 시도")
- base 토스트를 활용한 비침투적 경고 (경미한 에러)
- 치명적 에러 시 복구 옵션 (홈으로 돌아가기, 마지막 성공 이미지로 돌아가기)

**관련 파일**:

- `src/store/appStore.ts` — 에러 타입 세분화
- `src/components/ImageContainer.tsx` — 에러 UI 개선
- `src-tauri/src/commands.rs` — 상세 에러 코드 반환

---

### 5.3 테스트 커버리지 확대 (P2) — ✅ 완료 (2026-09-22 보강)

**현재 상태**: 프런트 54개 테스트 파일(386 케이스). 컴포넌트(`Header` 7·`ImageNavBar` 7·`SettingsDialog` 6 + 기존 `ImageContainer`·`GifControls`·`ExifPanel` 등), 통합 플로우(`useDirectoryNavigation` 로드→표시→이동 7: 경계/루프/양면 2장/클램프/오프셋/아카이브 분기), Rust(`commands.rs` 23: `index_of_current`·`DirListOptions` 기본값·`resolve_dropped_path` 4·`trash_file`·`get_exif_data`·`get_archive_images` 3 추가, 전체 179 통과)가 있다. `npm run test:coverage`(v8, text+html)로 리포트를 만든다. 커버리지 포함 검사는 `ci.yml`을 수동 실행할 때 함께 돈다.

**개선 계획 (달성)**:

- ~~**컴포넌트 테스트**: `Header`, `ImageNavBar`, `SettingsDialog` 등 렌더링/상호작용 테스트~~ ✅
- ~~**통합 테스트**: 이미지 로드 → 표시 → 네비게이션 플로우 테스트~~ ✅ (스토어+네비게이션 훅 수준)
- ~~**Rust 테스트**: `commands.rs` 단위 테스트 추가~~ ✅

**관련 파일**:

- `src/hooks/*.test.ts` — 훅 테스트 추가
- `src/components/*.test.tsx` — 컴포넌트 테스트 추가
- `src-tauri/src/commands.rs` — Rust 테스트 추가
- `vitest.config.ts` — 커버리지 설정

---

### 5.4 CI/CD 파이프라인 강화 (P2) — ✅ 완료 (2026-09-22 보안 감사·릴리즈 노트 추가)

**현재 상태**: `.github/workflows/ci.yml`의 자동 실행은 꺼져 있다. PR과 main 푸시에서는 돌지 않고, Actions에서 수동 실행할 때만 프런트(린트, 커버리지 포함 테스트, 타입, 포맷, `npm audit --audit-level=moderate`)와 Rust(`cargo fmt --check`, `cargo test`, `clippy -D warnings`, `rustsec/audit-check` v2 핀)를 검사한다. `release.yml`은 태그 푸시에서 같은 검사를 게이트로 두고, 이전 태그 이후 커밋 목록과 비교 링크로 릴리스 본문을 자동 생성(`Generate release notes`, 전체 히스토리 checkout)한 뒤 릴리스를 빌드한다. 로컬 기준선: `npm audit` 0건, `cargo audit` 취약점 0건(비취약 경고 8건만, 게이트 통과).

**관련 파일**:

- `.github/workflows/ci.yml` — 새 CI 워크플로우 생성
- `.github/workflows/release.yml` — 릴리즈 노트 자동화 추가

---

## 우선순위 요약

| 우선순위 | 항목                   | Phase |
| -------- | ---------------------- | ----- |
| **P0**   | 뷰 모드 구현           | 1     |
| **P0**   | 이미지 회전/뒤집기     | 1     |
| **P1**   | 클립보드 복사          | 1     |
| **P1**   | 썸네일 그리드 뷰       | 2     |
| **P1**   | 이미지 정보 패널 강화  | 2     |
| **P1**   | 스트리밍 이미지 디코딩 | 3     |
| **P1**   | 접근성 개선            | 5     |
| **P1**   | 에러 처리 개선         | 5     |
| **P2**   | 최근 파일 목록         | 2     |
| **P2**   | 다국어 지원            | 2     |
| **P2**   | 드래그 앤 드롭 개선    | 2     |
| **P2**   | 썸네일 생성 최적화     | 3     |
| **P2**   | 디렉토리 스캐닝 최적화 | 3     |
| **P2**   | 메모리 사용량 최적화   | 3     |
| **P2**   | 추가 이미지 포맷 지원  | 4     |
| **P2**   | 테스트 커버리지 확대   | 5     |
| **P2**   | CI/CD 파이프라인 강화  | 5     |
| **P3**   | 렌더링 성능 최적화     | 3     |

---

## 기술 참고 사항

### Tauri Asset Protocol (Phase 3.1, 완료 기록)

과거 `load_image` 커맨드는 파일을 읽어 base64 인코딩 후 IPC로 전달했다.
현재는 Tauri 2의 `asset:` 프로토콜로 프론트엔드에서 파일을 직접 참조해 메모리와 전송 비용을 줄인다.

```rust
// 현재 방식 (base64)
let data = fs::read(&path)?;
let base64 = general_purpose::STANDARD.encode(&data);

// 개선 방식 (asset protocol)
// Rust 커맨드에서 파일 경로를 asset URL로 변환하여 반환
#[tauri::command]
fn get_asset_url(app: tauri::AppHandle, file_path: String) -> Result<String, String> {
    let path = std::path::PathBuf::from(&file_path);
    // asset scope에 경로 등록 후 URL 반환
    let url = format!("asset://localhost/{}", urlencoding::encode(&file_path));
    Ok(url)
}
// 프론트엔드에서 <img src={assetUrl} /> 로 직접 사용
```

### ViewMode 구현 방향 (Phase 1.1)

```typescript
// ImageContainer.tsx 렌더링 분기 예시
switch (viewMode) {
  case "single":
    return <SingleImageView {...props} />
  case "left-to-right":
    return <DualPageView direction="ltr" {...props} />
  case "right-to-left":
    return <DualPageView direction="rtl" {...props} />
  case "webtoon":
    return <WebtoonScrollView {...props} />
}
```

### HEIC 포맷 지원 시 고려사항 (Phase 4.4)

HEIC/HEIF는 `libheif[core]`를 동적 링크합니다. `embedded-libheif`는 켜지 않습니다. 디코드는 LGPL인 `libheif`와 `libde265`만 쓰고, GPL인 `x265`는 넣지 않습니다. 배포 시 동적 링크와 라이선스 고지를 유지해야 합니다. EXIF는 아직 소스 경로의 `kamadak-exif`만 사용하므로 HEIC에서는 비어 있는 경우가 많습니다.
