# AGENTS.md — Tauri Image Viewer

이 문서는 이 저장소에서 작업하는 AI 에이전트를 위한 프로젝트 개요와 가이드라인입니다.

## 프로젝트 개요

**tauri-image-viewer**는 Tauri 2 기반 데스크톱 이미지 뷰어입니다.  
지원 형식: PNG, JPG, GIF, BMP, WebP, SVG, ICO, TIFF, AVIF.

- **프론트엔드**: React 18, TypeScript, Vite 6, Tailwind CSS 4, shadcn/ui
- **백엔드**: Rust (Tauri 2), 파일/이미지 처리
- **실행**: `npm run tauri dev` (개발), `npm run tauri build` (빌드)

## 디렉터리 구조

```
tauri-image-viewer/
├── src/                      # 프론트엔드 (React)
│   ├── App.tsx               # 루트 컴포넌트, useImageViewer 훅 사용
│   ├── main.tsx
│   ├── types.ts              # ImageInfo, DirectoryImages, Settings 등 공용 타입
│   ├── hooks/
│   │   └── useImageViewer.ts # 이미지 로드/캐시, 줌, 드래그, 설정, Tauri invoke
│   ├── components/
│   │   ├── Toolbar.tsx       # 열기, 이전/다음, 줌, 설정
│   │   ├── ImageContainer.tsx# 이미지 표시, 휠 줌, 드래그
│   │   ├── StatusBar.tsx     # 파일명/크기 등 상태
│   │   ├── SettingsDialog.tsx
│   │   └── ui/               # shadcn 컴포넌트 (button, radio-group, spinner 등)
│   ├── lib/
│   │   └── utils.ts          # cn() 등 유틸
│   └── utils/
│       └── format.ts         # 포맷 유틸
├── src-tauri/                # Rust 백엔드 (Tauri)
│   ├── src/
│   │   ├── lib.rs            # Tauri 앱 설정, 플러그인, open-file 이벤트
│   │   ├── main.rs           # 진입점
│   │   ├── commands.rs       # load_image, get_directory_images (invoke 대상)
│   │   └── image.rs          # MIME 타입, ImageInfo/DirectoryImages, is_image_file
│   ├── Cargo.toml
│   └── tauri.conf.json       # Tauri 설정
├── components.json           # shadcn 설정 (aliases: @/components, @/lib, @/hooks)
├── package.json
└── AGENTS.md                 # 이 파일
```

## 프론트엔드 (React/TypeScript)

- **상태/로직**: `useImageViewer` 훅에 집중. 이미지 캐시, 줌, 드래그, 설정, Tauri `invoke`/`listen` 처리.
- **UI**: `App.tsx`는 훅에서 받은 핸들러와 상태를 `Toolbar`, `ImageContainer`, `StatusBar`, `SettingsDialog`에 전달.
- **타입**: `src/types.ts`의 `ImageInfo`, `DirectoryImages`, `Settings`, `BackgroundType`, `CacheMode`는 Rust와 공유. 변경 시 백엔드와 맞출 것.
- **스타일**: Tailwind 4 + shadcn. 새 UI 컴포넌트는 `src/components/ui/`에 shadcn 추가 후 사용 (`npx shadcn@latest add ...`).
- **경로 별칭**: `@/components`, `@/lib`, `@/hooks` (components.json 기준).

## 백엔드 (Rust/Tauri)

- **명령**: `commands.rs`의 `load_image`, `get_directory_images`만 프론트에서 `invoke`로 호출. 시그니처/이름 변경 시 프론트의 `useImageViewer` 호출부도 함께 수정.
- **이미지 판별**: `image.rs`의 `get_mime_type`/`is_image_file` (확장자 기반). 새 포맷 추가 시 여기와 README 목록을 갱신.
- **이벤트**: `lib.rs`에서 `open-file` 이벤트 emit (Windows/Linux: CLI 인자, macOS: `RunEvent::Opened`). 프론트는 `listen("open-file", ...)` 로 수신.

## 에이전트 작업 가이드

1. **이미지 로드/디렉터리 목록**: Rust `commands.rs` + `image.rs`, 프론트 `useImageViewer.ts` (invoke, 캐시, prefetch).
2. **줌/드래그/핫키**: `useImageViewer.ts` + `ImageContainer.tsx`.
3. **설정(배경, 루프, 캐시 모드)**: `types.ts`의 `Settings`, `useImageViewer`, `SettingsDialog`.
4. **UI 컴포넌트 추가**: shadcn 사용 시 `components.json`과 `@/` alias 유지. 기존 `Toolbar`/`StatusBar` 패턴 따르기.
5. **타입 변경**: `src/types.ts`와 `src-tauri/src/image.rs`(및 commands에서 쓰는 구조체)를 동시에 맞출 것.
6. **Merge**: 사용자 규칙에 따라 no fast-forward merge만 사용.

## 참고

- 개발: `npm run tauri dev`
- 프로덕션 빌드: `npm run tauri build` (결과물: `src-tauri/target/release/bundle/`)
- 상세 설정/빌드 옵션: 저장소 루트의 `README.md` 참고.
