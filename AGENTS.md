# AGENTS.md - AraView

This document provides repository-specific guidance for AI coding assistants.

Product truth lives in `PRODUCT.md`, visual system in `DESIGN.md`, full functional/technical spec in `SPEC.md`, plans in `ROADMAP.md`.

## Project Overview

`araview` is a Windows desktop image viewer built with Tauri 2 + React 18 + TypeScript. Windows 10/11 x64 is the only supported OS.

- Frontend: React 18, TypeScript, Vite 6, Tailwind CSS 4, TanStack Router v1, Zustand 5
- Backend: Rust + Tauri 2 commands (`src-tauri/src/commands.rs`)
- UI primitives: shadcn/ui built on `@base-ui/react` (do not introduce `@radix-ui/*`)
- Package manager/runtime assumptions: Node.js `>=22`, npm scripts in `package.json`

## Current Feature Scope

- Supported file extensions: `png`, `jpg`, `jpeg`, `gif`, `bmp`, `webp`, `svg`, `ico`, `tiff`, `tif`, `avif`, `heic`, `heif`, `cbz`, `cb7`, `cbr`, `rar`, `zip`, `7z`, `cbt`
- Input flows: file picker, drag-and-drop (file/folder), OS file association open
- Viewer controls: zoom, fit-to-width/height/screen, pan, rotate, flip
- Navigation: previous/next, slider jump, thumbnail strip, optional loop navigation
- Extra features: EXIF panel, slideshow, fullscreen, copy image to clipboard (PNG), recent files, manual update check (tauri-plugin-updater, no background polling)
- Command palette (`Ctrl+K`): searchable global commands, see `src/constants/commands.ts`
- File associations: settings open the Windows per-extension default-app picker; silent UserChoice writes are not possible. Dev builds register as `AraView (Dev)` with separate registry keys/ProgIDs so they cannot collide with the installed build (`src-tauri/src/file_assoc.rs`)
- Multi-page view modes: `single`, `left-to-right`, `right-to-left`, `webtoon`

## Commands

```bash
# Development
npm install
npm run tauri dev

# Build
npm run build
npm run tauri build

# Local release (CI alternative, needs the signing key)
npm run release:local

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

- Functional/technical spec: `SPEC.md`
- Product definition: `PRODUCT.md`
- Visual system: `DESIGN.md`
- Plans: `ROADMAP.md`
- Frontend routes: `src/routes/`
- Business logic hooks: `src/hooks/`
- Stores: `src/store/`
- Shared TS types: `src/types/index.ts`
- Extension source of truth: `src/constants/imageExtensions.ts`
- Rust commands: `src-tauri/src/commands.rs`
- MIME/extension logic and tests: `src-tauri/src/image.rs`
- Windows file association registry: `src-tauri/src/file_assoc.rs`
- HEIC/HEIF JPEG sidecar decode: `src-tauri/src/heif.rs`
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
- `get_file_associations()`
- `set_file_association(extension, associate)`
- `set_all_file_associations(associate)`
- `open_default_apps_settings()`

Full IPC contract (18 commands including thumbnails, archive prefetch, trash/rename/save): see `SPEC.md` §16.

When app is opened from file association, Windows passes the file path as a CLI argument. Backend buffers it in `PendingOpenFile` until the webview signals readiness (`frontend_ready`), then emits `open-file`; the root layout (`useOpenFileBridge`) bridges the event to the active route's loader.

Frontend listener: `src/hooks/useOpenFileListener.ts`.

### Shared types and data model

Keep Rust `serde` output aligned with TypeScript types.

`ImageInfo` currently contains file metadata only:

- `file_path`
- `mime_type`
- `file_name`
- `file_size`
- `width`, `height` (`number | null`)

Do not reintroduce base64 payload fields unless explicitly required.

Full data model: see `SPEC.md` §17.

### Rendering path

Image rendering is path-based:

- Backend returns a filesystem path the WebView can decode (`ImageInfo.file_path`)
- HEIC/HEIF is transcoded to a JPEG sidecar under the process temp dir at load
- Frontend converts that path via `convertFileSrc(...)`

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
- Icon family is Phosphor (`@phosphor-icons/react`), typeface is Pretendard
  Variable (see `DESIGN.md`); newly added shadcn components arrive with
  `lucide-react` imports, swap them to Phosphor equivalents on arrival

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
5. Add a sample file to `samples/` and verify it opens in the dev app
   (`load_image` must succeed; EXIF-bearing formats also need a `get_exif_data`
   check). Generation notes per format: raster/vector via `sharp` in a temp
   dir (never add generator deps to the repo), HEIC/HEIF via PC-installed
   Python `pillow-heif` (repo must not depend on it), CBZ via
   `Compress-Archive`, CB7 via 7-Zip. If no encoder exists on the PC, install
   the tool on the PC instead of vendoring it into the repo.
6. Update docs (`README.md`, `docs/` usage/development/releasing as relevant, `SPEC.md`, and this file when relevant)

### Add a new backend command

1. Implement command in `src-tauri/src/commands.rs` with `#[tauri::command]`
2. Register in `src-tauri/src/lib.rs`
3. Call from frontend using `invoke(...)` (usually from a hook)
4. Update TypeScript types if payload/response shape changes

## Testing Guidance

- Frontend test files: `src/**/*.test.{ts,tsx}`
- Rust tests are colocated with source modules
- After every code change, run verification automatically without being asked:
  1. `npm test`
  2. `cd src-tauri && cargo test` (only when Rust sources changed)
  3. `npx tsc --noEmit`
  4. `npx prettier --check` on changed files (fix with `--write`)
  5. Runtime verification with Tauri MCP (primary) or agent-browser
     over WebView2 CDP (fallback, skill:
     `.opencode/skills/agent-browser/SKILL.md`):
     1. `npm run dev:up` (idempotent launcher, waits for `:1420` + `:9223`)
     2. Tauri MCP: `tauri-mcp driver-session start --port 9223`, then
        `webview-screenshot`, `webview-execute-js` (`open-file` event),
        `webview-keyboard`, `read-logs --source console`. See `/verify-ui`.
        (MCP client tools: `driver_session`, `webview_*`, `read_logs`.)
     3. Fallback when the MCP server is unreachable: per-command
        `agent-browser --cdp 9222` with
        `$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"`
        set in the launching shell. Never use stateful `connect`, it hangs.
- Do not finish a change with "run the tests yourself" or similar. If the
  dev app or bridge cannot start, report the exact failure instead of
  skipping verification silently.

## Window / UX Notes

- App window is frameless (`decorations: false`), custom titlebar is in `src/components/Header.tsx`
- Window state persistence uses `tauri-plugin-window-state`
- App starts hidden (`visible: false`) and appears after webview startup

## Browser Support

WebView2 (Chromium-based, Windows 10/11 x64) is the only frontend runtime.

- Baseline Widely available features: use without fallbacks.
- Baseline Newly Available features: feature-detect and degrade gracefully.
- Do not add polyfills or external compatibility libraries.
- Custom fallbacks are allowed only when they add roughly 20 lines or fewer and need no new dependencies.
- Core Web Vitals (LCP, INP, CLS) are secondary to desktop viewer responsiveness; image loading and scroll performance still matter.

## Agent Skills

Skill precedence for overlapping frontend work:

1. **shadcn** (`.agents/skills/shadcn/`): component selection, styling rules, forms, and `components.json` workflows.
2. **impeccable** (`.agents/skills/impeccable/`): design polish, UX review, accessibility hardening, and visual craft.
3. **modern-web-guidance** (`.agents/skills/modern-web-guidance/`): web platform APIs, performance patterns, and browser compatibility when introducing or changing HTML/CSS/clientside JS behavior.

Use modern-web-guidance when adding or changing image loading, scroll/motion, forms, accessibility patterns, or CSS layout features. Skip it for Rust/Tauri IPC, Zustand state, and routine shadcn component edits. Adapt framework-agnostic guides to React + `@base-ui/react` + shadcn; do not replace the existing component stack with native `<dialog>`/Popover API unless explicitly requested.

```bash
# Search for a relevant guide
npx -y modern-web-guidance@latest search "<query>" --skill-version 2026_09_04-7de96777

# Retrieve full guide(s) by ID
npx -y modern-web-guidance@latest retrieve "<id>"
```

Update with `npx -y modern-web-guidance@latest update`.

<!-- impeccable:start -->
## Design workflow (impeccable)

Product truth lives in `PRODUCT.md`; incumbent visual system in `DESIGN.md`.
For UI work, run `.opencode/skills/impeccable/scripts/impeccable.cmd context`
once per session, then route: `audit` / `critique` for review, `polish` before
shipping, `harden` for errors/i18n/edge cases, `typeset` / `layout` for
typography and spacing. This repo has no automatic design hook in opencode,
so after finishing changed UI run the detector manually:

```bash
npx impeccable detect --json <changed targets>
```

Non-negotiables carried over from the previous design filter:

- Every data view ships empty, loading, and error states.
- Every control is keyboard reachable and operable with a visible focus indicator.
- No em dash (`—`) in UI copy; use comma, period, colon, or parentheses.
- No fabricated claims, statistics, testimonials, or ghost navigation targets.
- Never generate final logos, avatars, or statistics without explicit
  instructions; use honest placeholders instead.
- Verify by running the app and clicking through every interactive element
  (`/verify-ui`); report the click-through element by element.
<!-- impeccable:end -->
