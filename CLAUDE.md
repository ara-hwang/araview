# CLAUDE.md — Tauri Image Viewer

This document provides guidance for AI assistants (Claude, Cursor, Copilot, etc.) working on this repository.

## Project Overview

**tauri-image-viewer** is a cross-platform desktop image viewer built with Tauri 2 + React 18. It supports 11 image formats: PNG, JPG, JPEG, GIF, BMP, WebP, SVG, ICO, TIFF, TIF, AVIF.

- **Frontend**: React 18, TypeScript, Vite 6, Tailwind CSS 4, shadcn/ui (**@base-ui** — never @radix-ui)
- **Backend**: Rust, Tauri 2 — file I/O, image loading, MIME detection
- **State**: Zustand 5 (in-memory) + Tauri Store plugin (persistent settings)
- **Routing**: TanStack Router v1 with auto-generated route tree
- **Node version**: 22 (`node --version` must be ≥22)
- **Rust MSRV**: 1.94 (use `rustup default stable`)

## Key Commands

```bash
# Development
npm install                             # Install frontend deps
npm run tauri dev                       # Start Vite (port 1420) + Tauri app

# Build
npm run tauri build                     # Full desktop build → src-tauri/target/release/bundle/
npm run build                           # Frontend only (tsc + vite build)

# Type checking & linting
npx tsc --noEmit                        # TypeScript check
npx prettier --check "src/**/*.{ts,tsx}" # Format check
npx prettier --write "src/**/*.{ts,tsx}" # Auto-format
cd src-tauri && cargo clippy            # Rust lint

# Tests
npm test                                # Run Vitest once
npm run test:watch                      # Vitest watch mode
cd src-tauri && cargo test              # Rust unit tests
```

> **Note**: `npm run tauri dev` requires an X11 display (`:1`). In VM environments, `libEGL DRI3` warnings are safe to ignore.

## Directory Structure

```
tauri-image-viewer/
├── src/                              # Frontend (React/TypeScript)
│   ├── components/
│   │   ├── Header.tsx               # Toolbar: Open, Zoom, Settings, window controls
│   │   ├── ImageContainer.tsx       # Main image display (zoom, pan, drag-drop)
│   │   ├── ImageNavBar.tsx          # Bottom thumbnail/progress navigation bar
│   │   ├── StatusBar.tsx            # File info (name, size, MIME type)
│   │   ├── SettingsDialog.tsx       # Settings modal (cache, loop, view mode)
│   │   ├── theme-provider.tsx       # Dark/Light/System theme + D hotkey
│   │   └── ui/                      # Auto-generated shadcn/ui components (do not edit manually)
│   ├── constants/
│   │   └── imageExtensions.ts       # Supported extension list (single source of truth)
│   ├── hooks/                        # All business logic lives here
│   │   ├── useImageLoader.ts        # File open dialog, drag-drop, load image + directory
│   │   ├── useImageCache.ts         # LRU in-memory cache with byte budget + prefetch
│   │   ├── useZoomPan.ts            # Mouse drag pan + position clamping
│   │   ├── useDirectoryNavigation.ts# Prev/next image navigation with loop support
│   │   ├── useImageViewerHotkeys.ts # Keyboard shortcuts
│   │   ├── useOpenFileListener.ts   # Tauri "open-file" event listener
│   │   ├── useWheelNavigation.ts    # Mouse wheel: zoom or navigate
│   │   ├── useViewerElements.ts     # Container/image DOM refs
│   │   └── useContextMenu.ts        # Right-click context menu
│   ├── lib/
│   │   └── utils.ts                 # cn() class merge utility
│   ├── routes/
│   │   ├── __root.tsx               # Root layout (Header + StatusBar)
│   │   ├── index.tsx                # Home page / file picker
│   │   ├── image.tsx                # Image viewer page
│   │   └── page/
│   │       ├── route.tsx            # Page layout wrapper
│   │       └── settings.tsx         # Settings page
│   ├── store/
│   │   ├── appStore.ts              # Zustand: zoom, position, imageInfo, dirImages, loading
│   │   └── settingsStore.ts         # Zustand + Tauri Store: cacheMode, loopNavigation, viewMode
│   ├── types/
│   │   └── index.ts                 # Shared TypeScript types (ImageInfo, DirectoryImages)
│   └── utils/
│       ├── cacheConfig.ts           # Cache limits/prefetch distance per cacheMode
│       ├── zoomPanUtils.ts          # Math: position bounds, clamp, fit-zoom
│       └── format.ts                # Formatters: file size, etc.
│
├── src-tauri/                        # Backend (Rust/Tauri)
│   ├── src/
│   │   ├── lib.rs                   # Tauri setup, plugins, CLI arg / macOS file open handlers
│   │   ├── main.rs                  # Binary entry point
│   │   ├── commands.rs              # #[tauri::command]: load_image, get_directory_images
│   │   └── image.rs                 # MIME detection, is_image_file — with unit tests
│   ├── Cargo.toml
│   └── tauri.conf.json              # Window config (frameless, 1024×768, file associations)
│
├── components.json                  # shadcn/ui config (style: base-nova, uses @base-ui)
├── package.json
├── vite.config.ts                   # Vite + TanStack Router + Tailwind plugins
├── vitest.config.ts                 # jsdom environment, src/**/*.test.{ts,tsx}
├── tsconfig.json                    # Strict mode, ES2020, excludes src/components/ui
└── AGENTS.md                        # Korean-language agent guidelines (legacy)
```

## Architecture

### Frontend → Backend Communication

The frontend invokes two Rust commands via Tauri's IPC:

| Command | File | Description |
|---|---|---|
| `load_image(file_path)` | `commands.rs` | Read file, detect MIME, encode to base64 → `ImageInfo` |
| `get_directory_images(file_path)` | `commands.rs` | List sibling images in directory → `DirectoryImages` |

The backend emits an `"open-file"` event (path string) to the frontend when:
- **Windows/Linux**: App launched with a file path CLI argument (`lib.rs` setup hook)
- **macOS**: `RunEvent::Opened` (file association)

The frontend listens via `useOpenFileListener.ts`.

### Shared Types

These types are serialized by Rust (`serde`) and deserialized by TypeScript. **Keep them in sync.**

```typescript
// src/types/index.ts
type ImageInfo = {
  base64: string       // Base64-encoded image data
  mime_type: string    // e.g., "image/png"
  file_name: string
  file_size: number    // bytes
}

type DirectoryImages = {
  images: string[]         // Absolute paths to all images in directory
  current_index: number    // Index of current image in list
}
```

### State Management

**`appStore.ts`** (Zustand, in-memory):
- `imageInfo`: current image data
- `dirImages`: directory listing + current index
- `zoom`, `position`: viewer state
- `loading`, `error`, `isDragging`
- `containerSize`, `imageSize`, `viewportSize`
- Actions: `setZoomToFit()`, `zoomIn()`, `zoomOut()`, `resetZoomPan()`, `startDrag()`, `moveDrag()`

**`settingsStore.ts`** (Zustand + Tauri Store plugin, persistent):
- `cacheMode`: `"off" | "nearby" | "extended" | "memory-1gb" | "memory-2gb"`
- `loopNavigation`: boolean
- `viewMode`: `"single" | "left-to-right" | "right-to-left" | "webtoon"`

### Image Cache

`useImageCache.ts` implements an LRU in-memory cache:

| Mode | Max Images | Max Bytes | Prefetch Distance |
|---|---|---|---|
| `off` | 1 | — | 0 |
| `nearby` | 24 | — | 1 |
| `extended` | 64 | — | 3 |
| `memory-1gb` | unlimited | 1 GB | 2 |
| `memory-2gb` | unlimited | 2 GB | 3 |

- Deduplicates in-flight requests (same file loading simultaneously waits for first)
- Evicts LRU entries when count or byte budget is exceeded
- Estimates base64 size as `string.length × 2` bytes

## Coding Conventions

### TypeScript

- **Use `type` not `interface`** for object/property types:
  ```typescript
  // Correct
  type ImageInfo = { base64: string; mime_type: string }
  // Wrong
  interface ImageInfo { base64: string; mime_type: string }
  ```
- Strict mode is enabled — no unused locals/parameters
- Path alias `@/` maps to `src/` (configured in tsconfig and vite.config)
- Do not manually edit files in `src/components/ui/` — they are shadcn auto-generated

### React & Components

- Business logic belongs in hooks (`src/hooks/`), not components
- Components receive handlers and state from hooks as props
- Use `useShallow` from Zustand when subscribing to multiple store fields
- Components use Tailwind CSS 4 classes for styling
- Use **`@base-ui/react`** for UI primitives — never `@radix-ui`
- Add new shadcn components with: `npx shadcn@latest add <component>`

### Rust

- All Tauri commands are in `commands.rs`; image utilities in `image.rs`
- Add unit tests for new image format support in `image.rs`
- Command signatures must match TypeScript `invoke()` calls — update both together
- Format with `rustfmt`, lint with `cargo clippy --deny warnings`

### Git

- Use **no-fast-forward merges** only: `git merge --no-ff`
- Commit messages may be in English or Korean

## Adding a New Image Format

1. `src-tauri/src/image.rs` — add extension to `get_mime_type()` and `is_image_file()`
2. `src-tauri/src/image.rs` — add unit tests for the new extension
3. `src/constants/imageExtensions.ts` — add the extension to the constants list
4. `tauri.conf.json` — add to `fileAssociations` if desktop file association is desired
5. Update the README format list

## Adding a Tauri Command

1. Define the Rust function in `src-tauri/src/commands.rs` with `#[tauri::command]`
2. Register it in the `.invoke_handler()` call in `src-tauri/src/lib.rs`
3. Call it from the frontend with `invoke("command_name", { arg })` — typically in a hook

## Testing

**Frontend** (Vitest + jsdom):
- Test files: `src/**/*.test.{ts,tsx}`
- Existing tests: `imageExtensions`, `utils`, `cacheConfig`, `format`, `zoomPanUtils`
- Run: `npm test`

**Backend** (Rust):
- Tests are co-located in source files (e.g., `image.rs` has 18 test cases)
- Run: `cd src-tauri && cargo test`

## Linux System Dependencies

Required to build Tauri on Linux (Debian/Ubuntu):

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

## Window & UI Notes

- The window is **frameless** (no OS decorations) — `Header.tsx` provides the custom titlebar with minimize/maximize/close controls
- Window state (size, position) is persisted via `tauri-plugin-window-state`
- Theme (dark/light/system) is managed by `theme-provider.tsx`; press `D` to cycle themes
- The app starts hidden and is shown after the webview is ready (`tauri.conf.json`: `visible: false`)
