# AGENTS.md - Tauri Image Viewer

This document provides repository-specific guidance for AI coding assistants.

## Project Overview

`tauri-image-viewer` is a desktop image viewer built with Tauri 2 + React 18 + TypeScript.

- Frontend: React 18, TypeScript, Vite 6, Tailwind CSS 4, TanStack Router v1, Zustand 5
- Backend: Rust + Tauri 2 commands (`src-tauri/src/commands.rs`)
- UI primitives: shadcn/ui built on `@base-ui/react` (do not introduce `@radix-ui/*`)
- Package manager/runtime assumptions: Node.js `>=22`, npm scripts in `package.json`

## Current Feature Scope

- Supported file extensions: `png`, `jpg`, `jpeg`, `gif`, `bmp`, `webp`, `svg`, `ico`, `tiff`, `tif`, `avif`, `cbz`
- Input flows: file picker, drag-and-drop (file/folder), OS file association open
- Viewer controls: zoom, fit-to-width/height/screen, pan, rotate, flip
- Navigation: previous/next, slider jump, thumbnail strip, optional loop navigation
- Extra features: EXIF panel, slideshow, fullscreen, copy image to clipboard (PNG), recent files
- Multi-page view modes: `single`, `left-to-right`, `right-to-left`, `webtoon`

## Commands

```bash
# Development
npm install
npm run tauri dev

# Build
npm run build
npm run tauri build

# Frontend tests / checks
npm test
npm run test:watch
npx tsc --noEmit
npx prettier --check "src/**/*.{ts,tsx}"
npx prettier --write "src/**/*.{ts,tsx}"

# Rust tests / lint
cd src-tauri && cargo test
cd src-tauri && cargo clippy
```

## Important Paths

- Frontend routes: `src/routes/`
- Business logic hooks: `src/hooks/`
- Stores: `src/store/`
- Shared TS types: `src/types/index.ts`
- Extension source of truth: `src/constants/imageExtensions.ts`
- Rust commands: `src-tauri/src/commands.rs`
- MIME/extension logic and tests: `src-tauri/src/image.rs`
- Tauri app setup and command registration: `src-tauri/src/lib.rs`
- Tauri config and file associations: `src-tauri/tauri.conf.json`

## Architecture Notes

### Frontend <-> Backend contract

Frontend uses `invoke()` for these commands:

- `load_image(file_path)`
- `get_directory_images(file_path)`
- `resolve_dropped_path(path)`
- `get_exif_data(file_path)`
- `get_archive_images(file_path)`
- `load_archive_image(archive_path, entry_name)`

When app is opened from file association, backend emits `open-file` event.

- Windows/Linux: CLI argument path in `.setup()`
- macOS: `RunEvent::Opened` URL handling

Frontend listener: `src/hooks/useOpenFileListener.ts`.

### Shared types and data model

Keep Rust `serde` output aligned with TypeScript types.

`ImageInfo` currently contains file metadata only:

- `file_path`
- `mime_type`
- `file_name`
- `file_size`

Do not reintroduce base64 payload fields unless explicitly required.

### Rendering path

Image rendering is path-based:

- Backend returns filesystem path
- Frontend converts path via `convertFileSrc(...)`

### Persistence

Tauri Store (`settings.json`) is used for:

- Viewer settings (`settingsStore`)
- Recent files (`recentFilesStore`)

## Code Conventions

### TypeScript / React

- Prefer `type` over `interface` in this repository style
- Keep business/domain logic inside hooks in `src/hooks/`
- Keep components mostly presentational
- Use Zustand selectors + `useShallow` for grouped subscriptions
- Keep path alias usage consistent: `@/...`
- Do not manually edit generated primitives in `src/components/ui/`

### Rust / Tauri

- Add new Tauri commands in `src-tauri/src/commands.rs`
- Register every new command in `src-tauri/src/lib.rs` `invoke_handler`
- Keep extension and MIME logic in `src-tauri/src/image.rs`
- Add/update Rust unit tests when changing supported formats

## Common Change Playbooks

### Add a new supported format

1. Update MIME mapping in `src-tauri/src/image.rs`
2. Add/update tests in `src-tauri/src/image.rs`
3. Update `src/constants/imageExtensions.ts`
4. Update `src-tauri/tauri.conf.json` file associations if needed
5. Update docs (`README.md` and this file when relevant)

### Add a new backend command

1. Implement command in `src-tauri/src/commands.rs` with `#[tauri::command]`
2. Register in `src-tauri/src/lib.rs`
3. Call from frontend using `invoke(...)` (usually from a hook)
4. Update TypeScript types if payload/response shape changes

## Testing Guidance

- Frontend test files: `src/**/*.test.{ts,tsx}`
- Rust tests are colocated with source modules
- Minimum validation after non-trivial changes:
  1. `npm test`
  2. `cd src-tauri && cargo test`
  3. `npx tsc --noEmit`

## Window / UX Notes

- App window is frameless (`decorations: false`), custom titlebar is in `src/components/Header.tsx`
- Window state persistence uses `tauri-plugin-window-state`
- App starts hidden (`visible: false`) and appears after webview startup
