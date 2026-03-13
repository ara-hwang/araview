# Tauri + React + Typescript

A desktop image viewer built with Tauri, React, and TypeScript. Supports a wide range of image formats including PNG, JPG, GIF, BMP, WebP, SVG, ICO, TIFF, and AVIF.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Prerequisites

Before setting up the development environment, install the following tools:

- **[Node.js](https://nodejs.org/)** (LTS version recommended)
- **[Rust](https://www.rust-lang.org/tools/install)** (stable toolchain)

### Platform-specific system libraries

#### Linux (Ubuntu/Debian)

```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```

#### macOS

No additional system libraries are required.

#### Windows

No additional system libraries are required.

## Development Environment Setup

1. Clone the repository:

   ```bash
   git clone https://github.com/ara-hwang/tauri-image-viewer.git
   cd tauri-image-viewer
   ```

2. Install frontend dependencies:

   ```bash
   npm install
   ```

3. Start the development server:

   ```bash
   npm run tauri dev
   ```

   This command starts the Vite dev server (port 1420) and the Tauri application window simultaneously. Hot-reload is enabled for frontend changes.

## Build

To create a production build:

```bash
npm run tauri build
```

The compiled application and installable bundles (`.deb`, `.dmg`, `.msi`, etc.) will be output to `src-tauri/target/release/bundle/`.

### macOS — Universal Binary

To build for both Apple Silicon and Intel:

```bash
npm run tauri build -- --target aarch64-apple-darwin
npm run tauri build -- --target x86_64-apple-darwin
```
