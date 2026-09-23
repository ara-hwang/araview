# AraView

Tauri 2 + React 19 + TypeScript 기반의 Windows 데스크톱 이미지 뷰어입니다.

- 일반 이미지 포맷과 만화 아카이브(`.cbz`, `.cb7`, `.cbr`, `.cbt`) 및 일반 아카이브(`.zip`, `.7z`, `.rar`)를 지원합니다.
- 폴더 내 이미지 탐색, EXIF 표시, 멀티 페이지 보기 모드를 제공합니다.
- CBZ/ZIP의 `ComicInfo.xml` 메타데이터(시리즈, 권 번호, 작가, 줄거리)를 정보 패널에서 읽기 전용으로 보여줍니다.
- 로컬 파일 경로 기반 렌더링(Asset Protocol)으로 동작합니다.

## 요구 사항

- Windows 10/11 (x64), Node.js `>= 22`, Rust stable

## 빠른 시작

```bash
git clone https://github.com/ara-hwang/araview.git
cd araview
npm install
npm run tauri dev
```

## 주요 기능

- **지원 포맷**: `png`, `jpg`, `jpeg`, `gif`, `bmp`, `webp`, `svg`, `ico`, `tiff`, `tif`, `avif`, `heic`, `heif`, `psd`(읽기 전용 미리보기), `cbz`, `cb7`, `cbr`, `rar`, `zip`, `7z`, `cbt`
- **파일 열기**: 파일 선택, 드래그 앤 드롭(파일/폴더), OS 파일 연동 실행
- **탐색/보기**: 이전/다음과 썸네일 점프, 확대/축소와 화면 맞춤, 회전/반전, `single`/`left-to-right`/`right-to-left`/`webtoon` 모드
- **부가 기능**: EXIF 패널, 최근 파일, 전체화면, 이미지 클립보드 복사, 명령 팔레트(`Ctrl+K`), 탐색기 PSD 썸네일(선택, `SPEC.md` §20.2)

## 문서

- 사용법(단축키, 설정, 업데이트 확인): `docs/usage.md`
- 개발 안내(환경, 실행, 스크립트, 구조): `docs/development.md`
- 릴리스와 업데이트(maintainer): `docs/releasing.md`
- 제품 정의: `PRODUCT.md`
- 기능/기술 명세: `SPEC.md`
- 비주얼 시스템: `DESIGN.md`
- 계획: `ROADMAP.md`
