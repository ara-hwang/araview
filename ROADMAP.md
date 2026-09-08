# Roadmap — 기능 추가 및 최적화 계획

> **Tauri Image Viewer** 의 향후 기능 추가, 최적화 및 개선 아이디어를 정리한 로드맵입니다.
> 우선순위(P0 ~ P3)와 단계(Phase)를 기준으로 정리되어 있습니다.

---

## Phase 1 — 미완성 기능 완성 및 핵심 개선

> 이미 코드베이스에 구조가 잡혀 있으나 아직 완성되지 않은 기능, 또는 사용자 경험에 직결되는 핵심 개선.

### 1.1 뷰 모드 구현 (P0)

**현재 상태**: `settingsStore.ts`에 `ViewMode` 타입(`single | left-to-right | right-to-left | webtoon`)이 정의되어 있고 `SettingsDialog.tsx`에서 설정할 수 있지만, 실제 `ImageContainer.tsx`에서 뷰 모드에 따른 렌더링 로직이 구현되어 있지 않음.

**구현 계획**:
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

### 1.2 이미지 회전 및 뒤집기 (P0)

**현재 상태**: 회전/뒤집기 기능 없음.

**구현 계획**:
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

### 1.3 클립보드 복사 (P1)

**현재 상태**: 이미지 클립보드 복사 기능 없음.

**구현 계획**:
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

### 1.4 슬라이드쇼 모드 (P1)

**현재 상태**: 자동 재생 기능 없음.

**구현 계획**:
- 단축키 `Space` 또는 `F5`로 슬라이드쇼 시작/정지
- 설정 가능한 간격 (1초 ~ 30초, 기본 3초)
- 전체화면 지원 (Tauri window fullscreen API)
- 진행 바 표시 (현재 이미지 위치 및 타이머)
- 슬라이드쇼 중 마우스 이동 시 컨트롤 오버레이 표시
- 루프 설정 연동 (`loopNavigation`)

**관련 파일**:
- `src/hooks/useSlideshow.ts` — 새 훅 생성
- `src/store/settingsStore.ts` — slideshowInterval 설정 추가
- `src/components/Header.tsx` — 슬라이드쇼 버튼 추가
- `src/components/ImageContainer.tsx` — 전체화면 모드 오버레이

---

## Phase 2 — 사용자 경험 개선

> 기존 기능의 완성도를 높이고 편의 기능을 추가하는 단계.

### 2.1 썸네일 그리드 뷰 (P1)

**현재 상태**: 디렉토리 내 이미지를 순차적으로만 탐색 가능.

**구현 계획**:
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

### 2.2 이미지 정보 패널 강화 (P1)

**현재 상태**: `ExifPanel.tsx`와 `useExifLoader.ts`를 통해 EXIF 메타데이터를 추출·표시하고 있으나 (Camera, Exposure, Image, Lens, DateTime, GPS, Software 카테고리), 히스토그램이나 색상 정보는 없음.

**구현 계획**:
- **히스토그램**: RGB 채널별 히스토그램 표시 (Rust에서 계산, `recharts`로 시각화)
- **색상 프로파일**: ICC 프로파일 정보 표시
- **이미지 해상도**: DPI, 비트 뎁스 등 상세 정보
- **파일 상세 정보**: 생성일, 수정일, 파일 경로

**관련 파일**:
- `src-tauri/src/commands.rs` — get_image_histogram, get_image_details 커맨드 추가
- `src/components/ExifPanel.tsx` — 히스토그램 차트 및 추가 정보 표시
- `src/hooks/useExifLoader.ts` — 추가 데이터 로딩

---

### 2.3 최근 파일 목록 (P2)

**현재 상태**: 앱을 다시 열면 이전에 본 이미지를 기억하지 않음.

**구현 계획**:
- Tauri Store에 최근 열어본 파일 목록 저장 (최대 50개)
- 홈 화면(`index.tsx`)에 최근 파일 목록 표시
- 마지막으로 본 이미지를 앱 시작 시 자동으로 열기 옵션
- 최근 파일 목록 삭제 기능

**관련 파일**:
- `src/store/settingsStore.ts` — recentFiles 상태 추가
- `src/routes/index.tsx` — 최근 파일 목록 UI
- `src/hooks/useImageLoader.ts` — 파일 열 때 최근 목록에 추가

---

### 2.4 다국어 지원 (i18n) (P2)

**현재 상태**: UI 텍스트가 영어/한국어 혼재. 체계적 다국어 지원 없음.

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

### 2.5 드래그 앤 드롭 개선 (P2)

**현재 상태**: 단일 파일 드래그 앤 드롭만 지원.

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

### 3.1 스트리밍 이미지 디코딩 (P1)

**현재 상태**: 이미지 전체를 base64로 인코딩하여 프론트엔드로 전달. 대용량 이미지(20MB+ RAW/TIFF)에서 메모리 사용량이 높고 전송이 느림.

**최적화 계획**:
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

**현재 상태**: 썸네일 기능 없음. 모든 이미지를 원본 크기로 로드.

**최적화 계획**:
- Rust 백엔드에서 `image` 크레이트로 리사이즈된 썸네일 생성
- 디렉토리 내 `.thumbcache/` 폴더에 썸네일 캐싱
- 백그라운드 스레드에서 비동기 생성 (`tokio::spawn`)
- 네비게이션 바에서 원본 대신 썸네일 로드

**관련 파일**:
- `src-tauri/Cargo.toml` — `image` 크레이트 추가
- `src-tauri/src/commands.rs` — generate_thumbnail 커맨드
- `src/hooks/useImageCache.ts` — 썸네일 캐시 레이어 분리

---

### 3.3 디렉토리 스캐닝 최적화 (P2)

**현재 상태**: `get_directory_images`가 매번 디렉토리 전체를 스캔. 수천 개 파일이 있는 폴더에서 느릴 수 있음.

**최적화 계획**:
- 디렉토리 스캔 결과를 Rust 측에서 캐싱 (파일 수정 시간 기반 무효화)
- 파일 시스템 워처(`notify` 크레이트)로 변경 감지 및 증분 업데이트
- 초기 로드 시 현재 파일 주변만 먼저 반환하고, 전체 목록은 비동기로 완성

**관련 파일**:
- `src-tauri/Cargo.toml` — `notify` 크레이트 추가
- `src-tauri/src/commands.rs` — 캐시된 디렉토리 스캔
- `src-tauri/src/lib.rs` — 파일 시스템 워처 설정

---

### 3.4 메모리 사용량 최적화 (P2)

**현재 상태**: base64 인코딩으로 원본 대비 약 33% 메모리 오버헤드 (base64는 3바이트를 4문자로 표현). `useImageCache.ts`에서 `string.length × 2`로 추정.

**최적화 계획**:
- Phase 3.1의 asset protocol 도입으로 base64 오버헤드 제거
- 캐시 키를 파일 경로 기반으로 변경 (데이터를 메모리에 보관하지 않음)
- 브라우저 이미지 캐시를 활용한 메모리 관리 위임
- 대용량 이미지 로드 시 해상도 제한 옵션 (4K 이상은 리사이즈 후 표시)

**관련 파일**:
- `src/hooks/useImageCache.ts` — 캐시 전략 재설계
- `src/utils/cacheConfig.ts` — 캐시 설정 업데이트

---

### 3.5 렌더링 성능 최적화 (P3)

**현재 상태**: CSS transform으로 zoom/pan 처리. 초대형 이미지에서 리페인트 비용 높을 수 있음.

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

### 4.1 이미지 비교 모드 (P2)

**구현 계획**:
- 두 이미지를 나란히(side-by-side) 또는 오버레이(overlay)로 비교
- 슬라이더로 비교 영역 조절 (split view)
- 동기화된 zoom/pan (한 쪽 조작 시 다른 쪽도 동일하게)
- 단축키 `C`로 비교 모드 진입

**관련 파일**:
- `src/components/CompareView.tsx` — 새 컴포넌트
- `src/hooks/useCompareMode.ts` — 비교 모드 로직
- `src/routes/compare.tsx` — 비교 전용 라우트 (선택)

---

### 4.2 이미지 기본 편집 (P3)

**구현 계획**:
- **크롭**: 마우스로 영역 선택 후 잘라내기
- **리사이즈**: 지정 크기로 변경
- **밝기/대비 조절**: CSS filter 기반 미리보기 → Rust에서 실제 적용
- 편집된 이미지를 "다른 이름으로 저장"
- 비파괴 편집 (원본 보존)

**관련 파일**:
- `src-tauri/src/commands.rs` — crop_image, resize_image, adjust_image 커맨드
- `src/components/EditToolbar.tsx` — 편집 도구 UI
- `src/hooks/useImageEditor.ts` — 편집 상태 관리

---

### 4.3 배치 작업 (P3)

**구현 계획**:
- 여러 이미지 선택 (썸네일 그리드에서 Ctrl+클릭)
- **배치 포맷 변환**: PNG→JPEG, WebP→PNG 등
- **배치 리사이즈**: 일괄 크기 변경
- **배치 이름 변경**: 패턴 기반 (예: `photo_{n:04}.jpg`)
- 진행률 표시 및 취소 기능

**관련 파일**:
- `src-tauri/src/commands.rs` — batch_convert, batch_resize, batch_rename 커맨드
- `src/components/BatchDialog.tsx` — 배치 작업 UI
- `src/hooks/useBatchOperations.ts` — 배치 로직

---

### 4.4 추가 이미지 포맷 지원 (P2)

**현재 상태**: 14개 포맷 지원 (PNG, JPG, JPEG, GIF, BMP, WebP, SVG, ICO, TIFF, TIF, AVIF, HEIC, HEIF, CBZ).

**HEIC/HEIF**: 구현됨. `libheif-rs`가 vcpkg `libheif`를 동적 링크하고, 로드 시 JPEG sidecar를 만듭니다. WebView2는 HEIC를 그리지 못합니다.

**추가 후보**:
- **RAW**: 카메라 RAW 포맷 (CR2, NEF, ARW 등) — `rawloader` 크레이트
- **PSD**: Photoshop 파일 미리보기 — `psd` 크레이트
- **JXL (JPEG XL)**: 차세대 이미지 포맷 — `jxl-oxide` 크레이트
- **QOI**: 빠른 무손실 포맷

**관련 파일**:
- `src-tauri/Cargo.toml` — 포맷별 크레이트 추가
- `src-tauri/src/image.rs` — MIME 타입 및 확장자 매핑 추가
- `src/constants/imageExtensions.ts` — 프론트엔드 확장자 목록 업데이트
- `src-tauri/tauri.conf.json` — 파일 연결 설정 추가

---

### 4.5 인쇄 기능 (P3)

**구현 계획**:
- `Ctrl+P`로 현재 이미지 인쇄
- 인쇄 미리보기 다이얼로그
- 용지 크기, 방향, 여백 설정
- "페이지에 맞춤" 또는 "원본 크기" 옵션

**관련 파일**:
- `src/hooks/usePrint.ts` — 인쇄 훅
- `src/components/PrintDialog.tsx` — 인쇄 미리보기 UI

---

## Phase 5 — 접근성 및 품질

> 앱의 전반적 품질과 접근성을 높이는 개선.

### 5.1 접근성 (A11y) 개선 (P1)

**현재 상태**: 기본적인 키보드 단축키만 있음. 스크린 리더 지원 부족.

**개선 계획**:
- 모든 이미지에 `alt` 텍스트 제공 (파일명 기반)
- ARIA 레이블 추가 (툴바 버튼, 슬라이더, 패널)
- 포커스 관리 개선 (탭 순서, 포커스 표시)
- 고대비 모드 지원
- 키보드만으로 모든 기능 접근 가능

**관련 파일**:
- `src/components/Header.tsx` — ARIA 레이블 추가
- `src/components/ImageContainer.tsx` — alt 텍스트, role 속성
- `src/components/ImageNavBar.tsx` — ARIA 슬라이더 속성
- `src/components/ExifPanel.tsx` — 구조적 마크업 개선

---

### 5.2 에러 처리 개선 (P1)

**현재 상태**: 에러 메시지가 기본적. 사용자 친화적 안내 부족.

**개선 계획**:
- 에러 유형별 구체적 메시지 (파일 없음, 권한 없음, 손상된 파일, 미지원 포맷)
- 에러 발생 시 대안 제시 ("다른 뷰어로 열기", "포맷 변환 시도")
- `sonner` 토스트를 활용한 비침투적 경고 (경미한 에러)
- 치명적 에러 시 복구 옵션 (홈으로 돌아가기, 마지막 성공 이미지로 돌아가기)

**관련 파일**:
- `src/store/appStore.ts` — 에러 타입 세분화
- `src/components/ImageContainer.tsx` — 에러 UI 개선
- `src-tauri/src/commands.rs` — 상세 에러 코드 반환

---

### 5.3 테스트 커버리지 확대 (P2)

**현재 상태**: 유틸리티 함수 위주의 단위 테스트만 존재 (5개 테스트 파일). 컴포넌트 및 훅 테스트 부재.

**개선 계획**:
- **훅 테스트**: `useDirectoryNavigation`, `useZoomPan` 등 핵심 훅 테스트
- **컴포넌트 테스트**: `Header`, `ImageNavBar`, `SettingsDialog` 등 렌더링/상호작용 테스트
- **통합 테스트**: 이미지 로드 → 표시 → 네비게이션 플로우 테스트
- **Rust 테스트**: `commands.rs` 단위 테스트 추가
- 테스트 커버리지 리포트 생성 설정

**관련 파일**:
- `src/hooks/*.test.ts` — 훅 테스트 추가
- `src/components/*.test.tsx` — 컴포넌트 테스트 추가
- `src-tauri/src/commands.rs` — Rust 테스트 추가
- `vitest.config.ts` — 커버리지 설정

---

### 5.4 CI/CD 파이프라인 강화 (P2)

**현재 상태**: 릴리즈 빌드만 있는 단일 워크플로우 (`release.yml`).

**개선 계획**:
- PR별 자동 테스트 실행 (프론트엔드 + Rust)
- 코드 린팅 체크 (Prettier + Clippy)
- TypeScript 타입 체크 (`tsc --noEmit`)
- 의존성 보안 감사 (`npm audit`, `cargo audit`)
- 자동 릴리즈 노트 생성

**관련 파일**:
- `.github/workflows/ci.yml` — 새 CI 워크플로우 생성
- `.github/workflows/release.yml` — 릴리즈 노트 자동화 추가

---

## 우선순위 요약

| 우선순위 | 항목 | Phase |
|---------|------|-------|
| **P0** | 뷰 모드 구현 | 1 |
| **P0** | 이미지 회전/뒤집기 | 1 |
| **P1** | 클립보드 복사 | 1 |
| **P1** | 슬라이드쇼 모드 | 1 |
| **P1** | 썸네일 그리드 뷰 | 2 |
| **P1** | 이미지 정보 패널 강화 | 2 |
| **P1** | 스트리밍 이미지 디코딩 | 3 |
| **P1** | 접근성 개선 | 5 |
| **P1** | 에러 처리 개선 | 5 |
| **P2** | 최근 파일 목록 | 2 |
| **P2** | 다국어 지원 | 2 |
| **P2** | 드래그 앤 드롭 개선 | 2 |
| **P2** | 썸네일 생성 최적화 | 3 |
| **P2** | 디렉토리 스캐닝 최적화 | 3 |
| **P2** | 메모리 사용량 최적화 | 3 |
| **P2** | 이미지 비교 모드 | 4 |
| **P2** | 추가 이미지 포맷 지원 | 4 |
| **P2** | 테스트 커버리지 확대 | 5 |
| **P2** | CI/CD 파이프라인 강화 | 5 |
| **P3** | 렌더링 성능 최적화 | 3 |
| **P3** | 이미지 기본 편집 | 4 |
| **P3** | 배치 작업 | 4 |
| **P3** | 인쇄 기능 | 4 |

---

## 기술 참고 사항

### Tauri Asset Protocol (Phase 3.1)

현재 `load_image` 커맨드는 파일을 읽어 base64 인코딩 후 IPC로 전달합니다.
Tauri 2의 `asset:` 프로토콜을 사용하면 프론트엔드에서 파일을 직접 참조할 수 있어 메모리와 전송 비용을 크게 줄일 수 있습니다.

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

HEIC/HEIF는 `libheif`를 동적 링크합니다. `embedded-libheif`는 켜지 않습니다. `libheif`는 LGPL이므로 배포 시 동적 링크와 라이선스 고지를 유지해야 합니다. EXIF는 아직 소스 경로의 `kamadak-exif`만 사용하므로 HEIC에서는 비어 있는 경우가 많습니다.
