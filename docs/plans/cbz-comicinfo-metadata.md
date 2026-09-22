# CBZ ComicInfo.xml 메타데이터 인식 기능 계획

> 상태: 계획 (미구현). 기준 소스: `main` 최신.
> 관련 문서: `SPEC.md` §8(아카이브), §15(IPC), §16(데이터 모델), `PRODUCT.md`, `ROADMAP.md`.

## 배경

현재 아카이브 파이프라인은 이미지 엔트리 목록/추출만 수행한다
(`src-tauri/src/archive.rs`, `src-tauri/src/commands.rs`의
`get_archive_images`/`load_archive_image`). CBZ 표준 메타데이터인
`ComicInfo.xml`(ComicRack/Komga/Kavita 호환)은 읽지 않으며, 정보 패널도
EXIF/히스토그램/파일 상세만 표시한다
(`src/components/ExifPanel.tsx`, `src/hooks/useExifLoader.ts`).

## 범위 (MVP)

1. **대상 포맷: CBZ/ZIP만.** `zip::ZipArchive`를 재사용한다.
   CB7/CBR/CBT는 리더가 각각 달라 Phase 2로 분리한다.
2. **읽기 전용, 표시용.** 편집/쓰기-back, 페이지 순서 변경은 제외한다.
   `Page Type=FrontCover` 힌트가 있어도 MVP에서는 읽기 순서
   (`get_archive_images`의 소문자 정렬)를 바꾸지 않는다.
3. **표시 위치: 기존 정보 패널(`ExifPanel`) 최상단 `Comic` 섹션.**
   별도 패널을 만들지 않아 키보드(`I`), 포커스, empty/loading/error
   규칙을 그대로 재사용한다. 아카이브 모드 + `comicInfo != null`일 때만 표시한다.

## 백엔드

- 신규 모듈 `src-tauri/src/comic_info.rs`:
  - ZIP에서 대소문자 무시 `ComicInfo.xml` 탐색 (루트 우선, 없으면 첫 번째 중첩 경로).
  - 상한 적용: 초과 시 `too_large`, XML 없음은 `None`(에러 아님),
    깨진 XML은 `corrupt`.
  - UTF-8 + BOM 처리, ComicRack식 UTF-16이 있으면 디코딩 폴백 검토.
  - 파서: `quick-xml` + `serde` 신규 의존성 (`src-tauri/Cargo.toml`에 추가).
- 파싱 필드 MVP:
  - `title, series, number, count, volume, summary, writer, penciller,`
    `publisher, genre, tags, language_iso, page_count, age_rating, community_rating`
  - `pages` 배열은 MVP에서 제외하거나 `page_count` 검증용으로만 최소 포함.
  - 빈 문자열은 `None`으로 정규화, 앞뒤 공백 trim.
- 신규 명령 `get_comic_info(filePath) -> ComicInfo | null`:
  - `src-tauri/src/commands.rs`에 추가 후 `src-tauri/src/lib.rs`
    `invoke_handler`에 등록한다 (백엔드 명령 추가 플레이북 준수).
  - 응답은 기존 규칙대로 snake_case (`SPEC.md` §0/§15/§16).
- Rust 테스트 (`comic_info.rs` colocated + `commands.rs` 픽스처):
  - 정상 XML 파싱, 필드 누락, 대소문자 파일명, XML 없음 → `None`,
    깨진 XML → `corrupt`, 크기 상한.

## 프론트

- `src/types/index.ts`에 `ComicInfo` 타입 추가.
- 상태: `src/store/appStore.ts`에 `comicInfo: ComicInfo | null`
  (+ 필요시 `comicInfoError`) 추가. `closeImage`와 일반 이미지 로드 시 초기화한다.
- 로더: `useImageLoader.loadArchive`(`src/hooks/useImageLoader.ts`)에서
  `get_archive_images` 성공 뒤 `get_comic_info`를 호출한다.
  `loadArchivePreview`(첫 페이지만 보는 모드)에서는 호출하지 않는다.
- UI: `ExifPanel` 상단에 Comic 섹션:
  - 1행: `Series #Number - Title` (있는 것만 조합).
  - 본문: Summary, Writer/Publisher/Genre/Tags, PageCount, AgeRating.
  - 없음: 섹션 숨김 (`exif.empty`와 충돌 없는 문구).
  - UI 금칙 준수: em dash 금지, 가짜 통계 금지.
- i18n: `src/i18n/locales/ko.json`, `src/i18n/locales/en.json`에
  `comic.*` 키 추가.

## 제외 항목

- `ComicInfo.xml` 편집 후 CBZ에 쓰기-back.
- 표지 타입 기반 읽기 순서 변경, 라이브러리 관리 기능.
- 최근 파일 카드/홈 화면의 시리즈명 표시 (후속 검토).

## 테스트/문서/검증

- 프론트: 로더 훅 테스트 + `ExifPanel` Comic 섹션 렌더 테스트.
- Rust 변경이 있으므로 `cargo test`, `cargo fmt`, `cargo clippy`.
- 공통: `npm test`, `npx tsc --noEmit`, `npm run lint:fix`, `npm run format`.
- 문서: `SPEC.md` §8/§15/§16, 필요시 `PRODUCT.md` Capabilities 한 줄 갱신.
- 런타임: `npm run dev:up` + Tauri MCP로 ComicInfo 포함/미포함 CBZ 각각
  열고 `I` 패널 확인 (`AGENTS.md` 검증 절차 준수).

## 결정 사항 (확정 필요)

1. CBZ/ZIP-only MVP로 시작하는가, CBR/CB7/CBT까지 1차에 포함하는가?
2. 표시 위치를 `ExifPanel` 상단으로 하는가, 별도 진입점이 필요한가?
3. 한 CBZ 안의 복수 `ComicInfo.xml`(챕터별)은 첫 번째만 읽는가?
4. MVP 필드 목록에서 제외/추가할 필드가 있는가?
