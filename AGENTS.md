# AGENTS.md

This file provides guidance for AI coding agents working in this repository.

## Project Overview

Tauri Image Viewer is a cross-platform desktop image viewer built with [Tauri 2](https://tauri.app/), [React 18](https://react.dev/), and [TypeScript](https://www.typescriptlang.org/). The frontend runs inside a Tauri webview and communicates with the Rust backend via Tauri's IPC command system.

**Supported image formats:** PNG, JPG/JPEG, GIF, BMP, WebP, SVG, ICO, TIFF/TIF, AVIF, HEIC, HEIF

## Repository Structure

```
tauri-image-viewer/
├── src/                        # Frontend — React/TypeScript
│   ├── components/
│   │   ├── Toolbar.tsx         # Navigation and zoom controls
│   │   ├── ImageContainer.tsx  # Image display, pan, and wheel-zoom
│   │   ├── StatusBar.tsx       # Filename and metadata display
│   │   └── ui/button.tsx       # Reusable shadcn/ui button
│   ├── hooks/
│   │   └── useImageViewer.ts   # Central state and logic hook
│   ├── lib/utils.ts            # Class-name utility (clsx + tailwind-merge)
│   ├── utils/format.ts         # Formatting helpers
│   ├── types.ts                # Shared TypeScript types
│   ├── App.tsx                 # Root component
│   ├── App.css                 # Tailwind CSS + dark theme variables
│   └── main.tsx                # React entry point
├── src-tauri/                  # Backend — Rust
│   ├── src/
│   │   ├── lib.rs              # Tauri builder, plugin registration, file-association handlers
│   │   ├── commands.rs         # IPC commands invoked from the frontend
│   │   ├── image.rs            # MIME-type detection, supported-format list
│   │   ├── context_menu.rs     # Windows-only: File Explorer "Open with" registry entry
│   │   └── main.rs             # Binary entry point
│   ├── Cargo.toml              # Rust dependencies
│   ├── tauri.conf.json         # Tauri app configuration
│   └── capabilities/          # Tauri permission capabilities
├── .github/workflows/
│   └── release.yml             # Cross-platform release workflow (triggered on v* tags)
├── package.json
├── vite.config.ts
├── tsconfig.json
└── README.md
```

## Technology Stack

| Layer | Technology |
|-------|-----------|
| Desktop framework | Tauri 2 |
| Frontend framework | React 18 |
| Language (frontend) | TypeScript ~5.6 |
| Build tool | Vite 6 |
| Styling | Tailwind CSS 4 |
| UI components | shadcn/ui + Lucide React icons |
| Hotkeys | @tanstack/react-hotkeys |
| Backend language | Rust (2021 edition) |
| IPC | Tauri API 2 |
| Tauri plugins | tauri-plugin-dialog, tauri-plugin-fs, tauri-plugin-opener |
| Windows registry | winreg (Windows target only) |

## Development Setup

### Prerequisites

- **Node.js** (LTS)
- **Rust** (stable toolchain via [rustup](https://rustup.rs/))
- **Linux only** — install system libraries:

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev build-essential curl wget file \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

### Install dependencies

```bash
npm install
```

### Run the development server

```bash
npm run tauri dev
```

Starts Vite (port 1420) and the Tauri application window with hot-reload enabled.

### Production build

```bash
npm run tauri build
```

Outputs platform-specific bundles to `src-tauri/target/release/bundle/`.

### Frontend-only build (no Tauri)

```bash
npm run build       # TypeScript + Vite
npm run preview     # Preview production build
```

## Key Files

### Frontend

- **`src/hooks/useImageViewer.ts`** — The central React hook that owns all application state: current image, zoom level, pan position, directory listing, drag handling, wheel-zoom, keyboard shortcuts, and IPC calls. Nearly all app logic lives here.
- **`src/types.ts`** — Defines `ImageInfo` (image data + metadata) and `DirectoryImages` (sorted list of images in a directory).
- **`src/components/ImageContainer.tsx`** — Renders the image with a checkered background, wires up mouse events for pan/drag, and handles the wheel-zoom event.
- **`src/components/Toolbar.tsx`** — Toolbar buttons for Open, Previous/Next, Zoom In/Out, and Fit modes.
- **`src/components/StatusBar.tsx`** — Displays the current filename and image metadata.

### Backend (Rust)

- **`src-tauri/src/commands.rs`** — Exposes two Tauri commands:
  - `load_image(file_path)` — reads a file, encodes it to base64, returns `ImageInfo`.
  - `get_directory_images(file_path)` — lists and alphabetically sorts all supported images in the same directory.
- **`src-tauri/src/image.rs`** — `get_mime_type()` and `is_image_file()` helpers; the single source of truth for supported extensions.
- **`src-tauri/src/lib.rs`** — Registers all Tauri plugins, IPC commands, and file-association open-file handlers for each platform.
- **`src-tauri/src/context_menu.rs`** — Windows-only helper that writes/removes the "Open with Image Viewer" entry in the Windows Registry.

## IPC Pattern

Frontend calls Rust commands with `invoke` from `@tauri-apps/api/core`:

```typescript
import { invoke } from "@tauri-apps/api/core";
const image = await invoke<ImageInfo>("load_image", { filePath: path });
```

Rust commands are defined with the `#[tauri::command]` attribute in `src-tauri/src/commands.rs` and registered in `src-tauri/src/lib.rs` via `.invoke_handler(tauri::generate_handler![...])`.

## Code Conventions

- **React**: Functional components with hooks; all state/logic in `useImageViewer.ts`.
- **TypeScript**: Strict mode; shared types in `src/types.ts`.
- **Styling**: Tailwind CSS utility classes; dark-theme CSS variables defined in `src/App.css`.
- **Class names**: Use the `cn()` helper from `src/lib/utils.ts` (wraps `clsx` + `tailwind-merge`) for conditional class merging.
- **Rust**: Follow standard `rustfmt` formatting; platform-specific code is gated with `#[cfg(target_os = "...")]`.
- **Adding a new image format**: Update `get_mime_type()` and `is_image_file()` in `src-tauri/src/image.rs`, and add the extension to the `fileAssociations.ext` array in `src-tauri/tauri.conf.json`.
- **Adding a new IPC command**: Define it in `src-tauri/src/commands.rs` and register it in the `generate_handler!` macro in `src-tauri/src/lib.rs`.

## Testing

There is currently no automated test suite in this repository. Validate changes by:

1. Running `npm run tauri dev` and exercising the affected functionality manually.
2. Running `npm run build` to confirm the TypeScript frontend compiles without errors.
3. Running `cargo check` inside `src-tauri/` to verify the Rust backend compiles.

## CI/CD

The `.github/workflows/release.yml` workflow is triggered when a tag matching `v*` is pushed. It builds and publishes a GitHub Release with platform-specific installers:

| Platform | Artifact |
|----------|---------|
| Ubuntu 22.04 | `.deb` package |
| macOS (Apple Silicon) | `.dmg` (aarch64) |
| macOS (Intel) | `.dmg` (x86_64) |
| Windows | `.msi` installer |

To trigger a release, create and push a version tag:

```bash
git tag v0.2.0
git push origin v0.2.0
```
