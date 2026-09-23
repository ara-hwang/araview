# CBZ ComicInfo.xml 메타데이터 + 양면 보기 표지 단독 표시 계획

> 상태: 계획 (미구현). 기준 소스: `main` 최신.
> 관련 문서: `SPEC.md` §5(탐색), §6(뷰 모드), §8(아카이브), §15(IPC), §16(데이터 모델), `PRODUCT.md`, `ROADMAP.md`.
> 구성: Part A(ComicInfo 메타데이터)와 Part B(양면 보기 표지 단독 표시)는 독립 배포 가능하다. B의 Phase 2(ComicInfo 표지 판정)만 A의 `pages` 파싱에 의존한다.

## Part A. ComicInfo.xml 메타데이터

### A-1. 배경

현재 아카이브 파이프라인은 이미지 엔트리 목록/추출만 수행한다
(`src-tauri/src/archive.rs`의 `list_archive_images`/`extract_archive_image`,
명령은 `get_archive_images`/`load_archive_image`). CBZ 표준 메타데이터인
`ComicInfo.xml`(ComicRack/Komga/Kavita 호환)은 읽지 않으며, 정보 패널도
EXIF/히스토그램/파일 상세만 표시한다
(`src/components/ExifPanel.tsx`, `src/hooks/useExifLoader.ts`).

### A-2. 범위 (MVP)

1. **대상 포맷: CBZ/ZIP만.** `zip::ZipArchive`(기존 의존성 `zip = "2"`)를
   재사용한다. CB7/CBR/CBT는 리더가 각각 다르므로(`sevenz-rust2`, `rars`,
   `tar`) Phase 2로 분리한다. CBZ/ZIP 외 확장자가 들어오면 에러 대신
   `null`을 반환해 프론트 분기를 단순하게 유지한다.
2. **읽기 전용, 표시용.** 편집/쓰기-back, 페이지 순서 변경은 제외한다.
   `Page Type=FrontCover` 힌트는 읽기 순서(`get_archive_images`의 소문자
   정렬)를 바꾸지 않고, Part B의 표지 판정에만 쓴다.
3. **표시 위치: 기존 정보 패널(`ExifPanel`) 최상단 Comic 섹션.** 별도 패널을
   만들지 않아 키보드(`I`), 포커스, empty/loading/error 규칙을 그대로
   재사용한다. 아카이브 모드 + `comicInfo != null`일 때만 표시한다.

### A-3. 백엔드 상세

신규 모듈 `src-tauri/src/comic_info.rs`:

```rust
pub const MAX_COMICINFO_BYTES: u64 = 1024 * 1024; // 1 MiB

#[derive(Serialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct ComicInfo {
    pub title: Option<String>,
    pub series: Option<String>,
    pub number: Option<String>,   // "1.5" 같은 값 보존을 위해 문자열
    pub count: Option<i32>,
    pub volume: Option<i32>,
    pub summary: Option<String>,
    pub writer: Option<String>,
    pub penciller: Option<String>,
    pub publisher: Option<String>,
    pub genre: Option<String>,
    pub tags: Option<String>,
    pub language_iso: Option<String>,
    pub page_count: Option<i32>,
    pub age_rating: Option<String>,
    pub community_rating: Option<String>, // 소수 등급 보존을 위해 문자열
    pub pages: Option<Vec<ComicPage>>,     // 확정: MVP 포함 (결정 사항 5)
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ComicPage {
    pub image: u32,                // ComicRack 스키마의 0 기반 페이지 인덱스
    pub page_type: Option<String>, // FrontCover 등
}

pub fn read_comic_info(archive_path: &Path) -> Result<Option<ComicInfo>, AppError>
```

- 탐색: 엔트리를 순회하며 basename이 `comicinfo.xml`과 대소문자 무시로
  일치하는 항목을 찾는다. 루트(`ComicInfo.xml`)를 우선하고, 없으면 첫 번째
  중첩 경로를 사용한다(결정 사항 3).
- 읽기: `archive.rs`의 bounded read(`read_bounded`)와 같은 패턴으로
  `MAX_COMICINFO_BYTES`(1 MiB)를 강제한다. 초과 시 `AppError::too_large`.
  구현 시 `read_bounded`를 `pub(crate)`로 승격해 재사용한다.
- 인코딩: UTF-8(BOM strip 포함)이 기본이다. UTF-16 LE/BE BOM을 감지하면
  `String::from_utf16` 계열로 디코딩한다(신규 의존성 없이 std만 사용).
  그 외 인코딩/깨진 바이트는 `AppError::corrupt`.
- 파싱: `quick-xml`의 serde 역직렬화. `src-tauri/Cargo.toml`에
  `quick-xml = { version = "0.38", features = ["serialize"] }` 추가(구현
  시점에 최신 마이너로 고정). 루트 요소 `ComicInfo`, 필드는 PascalCase
  (`Title`, `Series`, `Number`, `Count`, `Volume`, `Summary`, `Writer`,
  `Penciller`, `Publisher`, `Genre`, `Tags`, `LanguageISO`, `PageCount`,
  `AgeRating`, `CommunityRating`, `Pages`/`Page`의 `Image`, `Type` 속성).
- 정규화: 앞뒤 공백 trim, 빈 문자열은 `None`. `pages`는 `image` 오름차순
  정렬, 상한 1000개.
- 에러 매핑: XML 부재 → `Ok(None)`(에러 아님). 깨진 XML/디코딩 실패 →
  `AppError::corrupt`. 크기 초과 → `AppError::too_large`.
- 명령 추가 (백엔드 명령 플레이북 준수):

```rust
// src-tauri/src/commands.rs
#[tauri::command]
pub fn get_comic_info(file_path: String) -> Result<Option<ComicInfo>, AppError>
```

`src-tauri/src/lib.rs`의 use 목록과 `invoke_handler!`에 `get_comic_info`를
등록한다. 응답 키는 기존 규칙대로 snake_case (`SPEC.md` §15/§16).

- Rust 테스트 (`comic_info.rs` colocated, 픽스처는 `zip::write`로 인메모리
  ZIP 생성): 전체 필드 정상 파싱, 필드 누락, 파일명 대소문자
  (`COMICINFO.XML`), 루트/중첩 우선순위, XML 없음 → `None`, 깨진 XML →
  `corrupt`, 1 MiB 초과 → `too_large`, UTF-16 BOM, 빈 요소 → `None` 정규화,
  CBZ/ZIP 외 확장자 → `None`.

### A-4. 프론트 상세

- 타입 (`src/types/index.ts`):

```ts
export type ComicPage = { image: number; page_type: string | null };

export type ComicInfo = {
  title: string | null;
  series: string | null;
  number: string | null;
  count: number | null;
  volume: number | null;
  summary: string | null;
  writer: string | null;
  penciller: string | null;
  publisher: string | null;
  genre: string | null;
  tags: string | null;
  language_iso: string | null;
  page_count: number | null;
  age_rating: string | null;
  community_rating: string | null;
  pages: ComicPage[] | null;
};
```

- 상태 (`src/store/appStore.ts`): `comicInfo: ComicInfo | null`,
  `comicInfoError: string | null` 추가. 초기화 지점은 `loadImage` /
  `loadArchive` / `loadArchivePreview` 시작 시(`loading: true`와 같은
  setState)와 `closeImage`(`src/hooks/useCloseImage.ts`).
- 로더 (`src/hooks/useImageLoader.ts`): `loadArchive` 시작 시
  `get_archive_images`와 병렬로 `get_comic_info`를 호출한다(아카이브 경로만
  필요하므로 목록 의존 없음). 응답 반영 전 `isCurrentImageLoad(loadToken)`으로
  최신 로드만 커밋한다. `corrupt`/`too_large`는 조용히 무시하지 않고
  `comicInfoError`로 저장해 패널의 에러 상태로 노출한다.
  `loadArchivePreview`(첫 페이지 미리보기)에서는 호출하지 않는다.
- UI (`src/components/ExifPanel.tsx`): ScrollArea 최상단에 Comic 섹션을 두고
  순서는 Comic → 파일(`details.title`) → 히스토그램 → EXIF로 한다.
  - 헤더 1행: `Series #Number` 조합(있는 것만). 2행: `Title`(있으면).
  - 본문(있는 필드만 한 행씩): Writer, Penciller, Publisher, Genre, Tags,
    Volume, Count, PageCount, LanguageISO, AgeRating, CommunityRating.
    Summary는 마지막에 `whitespace-pre-wrap` 멀티라인으로 표시한다.
  - `comicInfoError`가 있으면 Comic 섹션 자리에 에러 문구만 표시한다(재시도
    버튼 없음. 복구 수단은 아카이브 다시 열기).
  - `comicInfo == null && comicInfoError == null`이면 섹션 자체를 숨긴다.
    (`exif.empty` 문구와 충돌하지 않는다.)
  - UI 금칙 준수: em dash 금지, 가짜 통계 금지.
- i18n (`src/i18n/locales/ko.json`, `src/i18n/locales/en.json`):
  `comic.section`, `comic.title`, `comic.series`, `comic.number`,
  `comic.volume`, `comic.count`, `comic.writer`, `comic.penciller`,
  `comic.publisher`, `comic.genre`, `comic.tags`, `comic.language`,
  `comic.pageCount`, `comic.ageRating`, `comic.rating`, `comic.summary`,
  `comic.loadFail`.

### A-5. 테스트/문서 (Part A)

- 프론트: 로더 훅 테스트(`invoke` 목으로 comicInfo 커밋/에러/이전 로드 무시),
  `ExifPanel` Comic 섹션 렌더 테스트(정상/에러/숨김).
- Rust 변경이 있으므로 `cargo test`, `cargo fmt`, `cargo clippy`.
- 공통: `npm test`, `npx tsc --noEmit`, `npm run lint:fix`, `npm run format`.
- 문서: `SPEC.md` §8(아카이브), §15(IPC 명령표에 `get_comic_info` 추가),
  §16(데이터 모델에 `ComicInfo` 추가), 필요시 `PRODUCT.md` Capabilities 한 줄.
- 런타임: `npm run dev:up` + Tauri MCP로 ComicInfo 포함/미포함 CBZ를 각각
  열고 `I` 패널을 확인한다(`AGENTS.md` 검증 절차 준수).

### A-6. 제외 항목 (Part A)

- `ComicInfo.xml` 편집 후 CBZ에 쓰기-back.
- `FrontCover` 힌트 기반 읽기 순서 변경, 라이브러리 관리 기능.
- 최근 파일 카드/홈 화면의 시리즈명 표시 (후속 검토).
- CB7/CBR/CBT 메타데이터 (리더별 Phase 2).

## Part B. 양면 보기 표지 단독 표시

### B-1. 배경

양면 모드(`left-to-right`/`right-to-left`)는 항상 `[current, next]` 두 장을
보여주고(`useMultiPageImages.ts`의 offsets `[0, 1]`) 2장씩 넘긴다
(`useDirectoryNavigation.ts`의 `step = 2`). 목록 첫 장이 표지인 만화/화보에서는
표지와 2페이지가 한 화면에 붙어 버린다. Komga/Kavita/Honeyview 등 만화 리더의
표준 동작은 표지(첫 페이지)를 단독으로 보여주고 그 뒤부터 `(1,2), (3,4)` 쌍을
맞추는 것이다.

### B-2. 동작 정의 (MVP)

- 대상: `left-to-right`/`right-to-left` 모드만. `single`/`webtoon`은 변경
  없음. 폴더 목록과 아카이브 목록 모두에 적용한다.
- 표지 인덱스: MVP는 0번(첫 이미지) 고정. Phase 2에서 ComicInfo `FrontCover`로
  대체한다(B-6).
- 신규 설정 `showCoverAlone: boolean` (기본값 `true`, 결정 사항 6 확정):
  - 켜짐: 쌍은 `[0], [1,2], [3,4], ...`가 된다. 표지 단독 렌더는 기존 「마지막
    홀수 장 단일 중앙 표시」 분기(`ImageContainer.tsx`의
    `pages.length === 1`)를 그대로 재사용한다.
  - 꺼짐: 현재 동작 그대로 `[0,1], [2,3], ...`.
- 쌍 시작 인덱스(켜짐): 0은 단독. i >= 1의 쌍 시작은
  `1 + 2 * floor((i - 1) / 2)`.

### B-3. 넘김 규칙 (켜짐 기준)

- 다음: 0 → 1. i >= 1 → `i + 2`.
- 이전: 1 → 0(표지 복귀). i >= 3 → `i - 2`.
- 끝 clamp/루프 wrap은 기존 `resolveStepIndex` 규칙을 유지한다(비루프 시
  마지막 장은 clamp로 표시).
- 썸네일/도크/점프(`navigateToIndex`)로 임의 인덱스에 착지하면 쌍 시작으로
  스냅한다. `Home/End`, `PageUp/PageDown`(10장 점프)도 `navigateToIndex`를
  거치므로 자동 적용된다.
- 기존 동작 변경 (결정 사항 8 확정): 현재는 스냅이 없어 쌍이 겹칠 수 있다
  (예: 3으로 점프 → `[3,4]`가 앞 쌍 `[2,3]`과 3을 공유). 쌍 스냅은
  `showCoverAlone`과 무관하게 양면 모드 전체에 적용해 쌍 겹침을 제거한다.

### B-4. 변경 지점

1. `src/store/settingsStore.ts`: `SettingsState.showCoverAlone` 추가.
   `sanitizeSettings`는
   `record.showCoverAlone === undefined ? initial : sanitizeBoolean(...)`
   패턴(`recordRecentFiles`와 동일)으로 기본값을 보존한다.
2. `src/components/settings/ViewTabPanel.tsx`: 「읽기」 필드셋
   (`settings.reading`)에 Switch 추가. i18n `settings.reading.coverAlone`.
3. `src/utils/dirNavigation.ts`: 순수 함수 추가.

   ```ts
   export function resolvePairStart(
     index: number,
     total: number,
     coverAlone: boolean,
   ): number;

   export function resolveDualStepIndex(
     current: number,
     total: number,
     loop: boolean,
     direction: "prev" | "next",
     coverAlone: boolean,
   ): number | null;
   ```

   `coverAlone = false`이면 기존 `resolveStepIndex(..., step = 2)`와 동일
   결과를 보장한다(회귀 테스트로 고정).

4. `src/hooks/useDirectoryNavigation.ts`: 양면 모드에서 고정 `step` 대신
   `resolveDualStepIndex`를 사용하고, `navigateToIndex`에 쌍 스냅을 적용한다.
5. `src/hooks/useMultiPageImages.ts`: 오프셋 계산을
   `dualPageOffsets(index, total, coverAlone): number[]`(결과 `[0]` 또는
   `[0, 1]`)로 분리한다. 표지에서는 `[0]`만 로드해 두 번째 페이지 디코딩을
   하지 않는다.
6. `src/components/ImageContainer.tsx`: 변경 없음(단일 중앙 렌더 재사용).
   회귀 확인만 한다.
7. `src/components/StatusBar.tsx`: MVP는 `current+1/total` 표기를 유지한다.
   `2-3/20` 같은 범위 표기는 후속(결정 사항 7).
8. 아카이브 이어보기(`src/utils/archiveResume.ts` 경유 진입): 저장된 인덱스가
   쌍 중간이면 쌍 시작으로 스냅해 연다.

### B-5. 프리페치 영향

`useImageLoader.prefetchArchiveNeighbors`는 `viewMode`별 bonus를 이미 둔다.
표지 단독 페이지(0번)에서는 다음 1장만 있으면 충분하므로 현행 유지한다. 미세
조정이 필요하면 후속으로 분리한다.

### B-6. Phase 2: ComicInfo 표지 판정 (Part A 의존)

Part A의 `pages`(MVP 포함 확정)에서 `page_type === "FrontCover"`인 첫
`ComicPage.image`를 표지 인덱스로 사용한다. 없으면 0번 폴백. 스키마는 0
기반이지만 제작자가 1 기반으로 쓴 파일이 있으므로, `image`가 목록 범위를
벗어나면 0번 폴백한다. 우선순위: ComicInfo FrontCover > 0번.

### B-7. 제외 항목 (Part B)

- `single`/`webtoon` 모드의 렌더/넘김 변경.
- 와이드 페이지(가로로 긴 스프레드) 자동 단독 표시, 뒤표지 단독 표시 옵션.
- 상태바 범위 카운터(후속), 웹툰 모드 표지 처리(해당 없음).

## 공통 검증

- 프론트: `dirNavigation` 순수 함수 테스트(켜짐/꺼짐 × prev/next × 경계/루프),
  `useDirectoryNavigation.test.ts`의 기존 「양면 모드는 2장씩 넘긴다」 회귀
  (꺼짐 상태) + 켜짐 상태 신규 케이스, `useMultiPageImages` 오프셋 테스트 신설,
  설정 sanitize 테스트, 로더/패널 테스트(Part A).
- Rust: `cargo test`, `cargo fmt`, `cargo clippy` (Part A).
- 공통: `npm test`, `npx tsc --noEmit`, `npm run lint:fix`, `npm run format`.
- 문서: `SPEC.md` §5.3(이전/다음), §6(뷰 모드 표와 쌍 규칙), 설정 표,
  `docs/usage.md`, `README.md` 한 줄.
- 런타임: `npm run dev:up` + Tauri MCP. 시나리오: ComicInfo 포함 CBZ와 미포함
  CBZ 각각 → 양면 전환 → 표지 단독 확인 → 다음 `[1,2]` → 이전 표지 복귀 →
  설정 off 시 `[0,1]` 회귀 → `I` 패널 Comic 섹션 확인. 클릭스루 결과를 요소별로
  기록한다(`AGENTS.md` 검증 절차).

## 결정 사항

### 확정

- (5) `pages` 배열은 MVP에 포함한다 (`image`/`page_type` 최소 형태, 상한
  1000). B-6의 표지 판정 근거다.
- (6) `showCoverAlone` 기본값은 `true`다.
- (8) 쌍 스냅(`navigateToIndex`)은 `showCoverAlone`과 무관하게 양면 모드
  전체에 적용한다(쌍 겹침 제거).

### 미확정 (계획안을 따름)

- (1) CBZ/ZIP-only MVP로 시작하는가, CBR/CB7/CBT까지 1차에 포함하는가?
  (계획안: CBZ/ZIP-only.)
- (2) 표시 위치를 `ExifPanel` 상단으로 하는가, 별도 진입점이 필요한가?
  (계획안: `ExifPanel` 상단.)
- (3) 한 CBZ 안의 복수 `ComicInfo.xml`(챕터별)은 루트 우선 + 첫 중첩만 읽는가?
  (계획안: 루트 우선 + 첫 중첩.)
- (4) MVP 필드 목록에서 제외/추가할 필드가 있는가? (계획안: A-3 목록대로.)
- (7) 상태바 카운터를 쌍 범위(`2-3/20`)로 바꾸는가? (계획안: 현행 유지.)
