# AGENTS.md - AraView

This document provides repository-specific guidance for AI coding assistants.

Product truth lives in `PRODUCT.md`, visual system in `DESIGN.md`, full functional/technical spec in `SPEC.md`.

## Project Overview

`araview` is a Windows desktop image viewer built with Tauri 2 + React 19 + TypeScript. Windows 10/11 x64 is the only supported OS.

- Frontend: React 19, TypeScript, Vite 6, Tailwind CSS 4, TanStack Router v1, Zustand 5
- Backend: Rust + Tauri 2 commands (`src-tauri/src/commands.rs`)
- UI primitives: shadcn/ui built on `@base-ui/react` (do not introduce `@radix-ui/*`)
- Package manager/runtime assumptions: Node.js `>=22`, npm scripts in `package.json`

## Current Feature Scope

- Supported file extensions: `png`, `jpg`, `jpeg`, `gif`, `bmp`, `webp`, `svg`, `ico`, `tiff`, `tif`, `avif`, `heic`, `heif`, `psd` (read-only preview via JPEG sidecar, no edit-save), `cbz`, `cb7`, `cbr`, `rar`, `zip`, `7z`, `cbt` (PSB excluded, no decoder)
- Input flows: file picker, drag-and-drop (file/folder), OS file association open
- Viewer controls: zoom, fit-to-width/height/screen, pan, rotate, flip
- Navigation: previous/next, index jump, thumbnail strip/grid, optional loop navigation
- Extra features: EXIF panel, fullscreen, copy image to clipboard (PNG), recent files, manual update check (tauri-plugin-updater, no background polling)
- Command palette (`Ctrl+K`): searchable global commands, see `src/constants/commands.ts`
- File associations: settings open the Windows per-extension default-app picker; silent UserChoice writes are not possible. Dev builds register as `AraView (Dev)` with separate registry keys/ProgIDs so they cannot collide with the installed build (`src-tauri/src/file_assoc.rs`). Explorer PSD thumbnails are opt-in via HKCU `IThumbnailProvider` (see `SPEC.md` §20.2).
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
npm run lint
npm run lint:fix
npx tsc --noEmit
npm run format:check
npm run format

# Rust tests / lint
cd src-tauri && cargo test
cd src-tauri && cargo clippy
cd src-tauri && cargo fmt
```

## Important Paths

- Functional/technical spec: `SPEC.md`
- Product definition: `PRODUCT.md`
- Visual system: `DESIGN.md`
- Frontend routes: `src/routes/`
- Business logic hooks: `src/hooks/`
- Stores: `src/store/`
- Shared TS types: `src/types/index.ts`
- Extension source of truth: `src/constants/imageExtensions.ts`
- Rust commands: `src-tauri/src/commands.rs`
- MIME/extension logic and tests: `src-tauri/src/image.rs`
- JPEG save metadata preservation (EXIF/ICC/XMP segments): `src-tauri/src/jpeg_meta.rs`
- Windows file association registry: `src-tauri/src/file_assoc.rs`
- HEIC/HEIF JPEG sidecar decode: `src-tauri/src/heif.rs`
- Derived-image cache root, protection, eviction, and startup cleanup: `src-tauri/src/process_temp.rs`
- Cache statistics and scoped deletion: `src-tauri/src/cache.rs`
- Tauri app setup and command registration: `src-tauri/src/lib.rs`
- Tauri config and file associations: `src-tauri/tauri.conf.json`

## Architecture Notes

### Frontend <-> Backend contract

Frontend uses `invoke()` for these commands:

- `load_image(file_path, max_side?, image_scaling_mode?, auto_detect_pixel_art?)`
- `load_archive_image(archive_path, entry_name, max_side?, protect?, image_scaling_mode?, auto_detect_pixel_art?)`
- `rename_file(old_path, new_name, max_side?, image_scaling_mode?, auto_detect_pixel_art?)`
- `detect_pixel_art(file_path)`
- `get_directory_images(file_path)`
- `resolve_dropped_path(path)`
- `get_exif_data(file_path)`
- `get_archive_images(file_path)`
- `load_archive_image(archive_path, entry_name, max_side?, protect?)`
- `get_cache_stats()`
- `clear_cache(scope)`
- `get_file_associations()`
- `set_file_association(extension, associate)`
- `open_default_apps_settings()`

Full IPC contract (including thumbnails, cache management, archive prefetch, trash/rename/save, PSD thumbnail): see `SPEC.md` §15.

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

Full data model: see `SPEC.md` §16.

### Rendering path

Image rendering is path-based:

- Backend returns a filesystem path the WebView can decode (`ImageInfo.file_path`)
- HEIC/HEIF/PSD are transcoded to JPEG sidecars under the active derived-image cache root at load
- Persistent cache is the default; temporary mode uses a session-only root. The active mode is managed from Settings and mode changes apply on the next launch
- Frontend converts that path via `convertFileSrc(...)`
- `src-tauri/src/pixel_art.rs` provides conservative display-only detection; the frontend resolves `auto | smooth | pixelated` through `src/utils/imageRendering.ts`

### Persistence

Tauri Store (`settings.json`) is used for:

- Viewer settings (`settingsStore`), including `cacheStorageMode` (`temporary | persistent`, default `persistent`)
- Recent files (`recentFilesStore`)
- Persistent derived-image cache is best-effort local data under the Tauri app cache directory; it is not a source of truth

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
- While developing, do not run the test suites (`npm test`, `cargo test`).
  They wait for the completion pass so edits stay fast.
- After every code change, auto-apply format/lint/typecheck without being asked
  (check-only is not enough):
  1. `npx tsc --noEmit`
  2. `npm run lint:fix` (auto-fix; `npm run lint` is check-only)
  3. `npm run format` (write mode; `npm run format:check` is check-only)
  4. `cd src-tauri && cargo fmt` (only when Rust sources changed; `cargo fmt --check` is check-only)
- Once the change is complete, run verification automatically without being
  asked:
  1. `npm test`
  2. `cd src-tauri && cargo test` (only when Rust sources changed)
  3. Runtime verification with Tauri MCP:
     1. `npm run dev:up` (idempotent launcher, waits for `:1420` + `:9323`;
        the dev bridge port is pinned to 9323 in `src-tauri/src/lib.rs` so an
        installed build or another Tauri app cannot take it)
     2. `tauri-mcp driver-session start --port 9323`, then
        `webview-screenshot`, `webview-execute-js` (`open-file` event),
        `webview-keyboard`, `read-logs --source console`. See `/verify-ui`.
        (MCP client tools: `driver_session`, `webview_*`, `read_logs`.)
     3. Confirm the session target before trusting any result:
        `ipc-get-backend-state` (client tool: `ipc_get_backend_state`) must
        report `environment.debug: true` and identifier
        `com.araview.viewer.dev`. An installed build answers with `debug: false`
        and `com.araview.viewer`, so stop and report instead of verifying the
        wrong app.
     4. If `driver-session start` fails, repair the bridge with
        `get_setup_instructions` or restart the session (`npx -y` download
        flake) before concluding anything is broken. There is no browser
        automation fallback: report the exact failure instead of skipping
        verification.
- Tauri MCP "missing" reports are usually misdiagnoses. Check in this order:
  1. Names: repo `mcp-server-tauri` is not an npm name. MCP server is
     `@hypothesi/tauri-mcp-server` (stdio protocol, no `--help`/`--version`
     output by design), terminal CLI is `@hypothesi/tauri-mcp-cli`
     (`tauri-mcp` binary, also no `--version`), Rust bridge is
     `tauri-plugin-mcp-bridge`. Never probe with `npm view mcp-server-tauri`
     (404 is expected) or `tauri-mcp --version` (unknown option is expected).
     Use `npm ls -g @hypothesi/tauri-mcp-cli` and `tauri-mcp --help`.
  2. Installed vs connected: the CLI responding means installed. Connecting
     additionally needs `npm run dev:up` (`:1420` + `:9323`) plus
     `driver-session start --port 9323`. The bridge only exists in dev builds
     (`dev-mcp` feature, `src-tauri/src/lib.rs`); release builds
     (`--no-default-features`, used by CI and `release:local`) have no bridge,
     so connection failure there is expected, not a missing installation.
  3. Fresh-session flake: opencode loads MCP at session start and the first
     `npx -y` download can time out. Restart the session before concluding
     anything is missing.
- Bump a frontend version? Kill the old dev processes first, then check the
  bundle is fresh: `dev:up` is idempotent by port, so a stale Vite server can
  keep serving the old optimized bundle and produce phantom errors from the
  previous library version. Stale evidence is indistinguishable from a real
  bug until the served chunk graph is checked
  (`node_modules/.vite/deps` timestamps and `page.url` chunk hashes).
- Do not finish a change with "run the tests yourself" or similar. If the
  dev app or bridge cannot start, report the exact failure instead of
  skipping verification silently.

## Window / UX Notes

- App window is frameless (`decorations: false`), custom titlebar is in `src/components/Header.tsx`
- Window state persistence uses `tauri-plugin-window-state`
- Windows 11 Snap Layouts comes from `tauri-plugin-snap-layout`: it floats a transparent native hit-test overlay (`WM_NCHITTEST` → `HTMAXBUTTON`) over the maximize caption button (`id=caption-maximize`). The overlay owns the mouse, so that button's hover wash/tooltip are mirrored from `tauri-snap://snap/mouseenter|mouseleave` events via `src/hooks/useSnapLayout.ts`; keep the Rust `button_id` and the DOM id in sync. Non-Windows and jsdom are no-ops.
- App starts hidden (`visible: false`) and appears after webview startup
- Dialogs/Sheets pass `modal="trap-focus"` so the caption buttons stay clickable. `modal={true}` makes Base UI render a transparent `position: fixed; inset: 0` `InternalBackdrop` that swallows every click outside the popup (including the titlebar) and dismisses on press, so minimize/maximize/close would do nothing while a dialog is open. `trap-focus` keeps the focus trap, `Esc`, outside `aria-hidden`, and backdrop-click dismissal; only the invisible shield and the body scroll lock go away (the shell is `h-screen overflow-hidden`, so the body never scrolls). Overlays start at `--header-height`, so the header strip is the only clickable area outside the popup. Do not "fix" this by re-adding a header `pointer-events: none` rule in `src/App.css`; pass the prop at each call site instead, since `src/components/ui/*` primitives are generated.

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
2. **modern-web-guidance** (`.agents/skills/modern-web-guidance/`): web platform APIs, performance patterns, and browser compatibility when introducing or changing HTML/CSS/clientside JS behavior.
3. **animate** (`.agents/skills/animate/`, vendored): building or changing motion, transitions, and micro-interactions. Load `RECIPES.md` when the request matches one of its components.
4. **review-animations** (`.agents/skills/review-animations/`, vendored): critiquing motion only. Load `STANDARDS.md` when a finding needs an exact curve, duration, or spring value.

Use modern-web-guidance when adding or changing image loading, scroll platform behavior, forms, accessibility patterns, or CSS layout features; motion taste and values belong to animate/review-animations. Skip it for Rust/Tauri IPC, Zustand state, and routine shadcn component edits. Adapt framework-agnostic guides to React + `@base-ui/react` + shadcn; do not replace the existing component stack with native `<dialog>`/Popover API unless explicitly requested.

Motion routing: send motion-only work to animate/review-animations; handle broad design polish directly against the design non-negotiables below. Motion tokens live in `src/App.css` (`@theme`): `--ease-motion-out` (entrances, exits), `--ease-motion-in-out` (on-screen movement), `--ease-motion-drawer` (sheets, drawers), available as `ease-motion-*` utilities; the built-in `ease-out`/`ease-in-out` stay untouched. Durations come from the vendored tables (press 100-160ms, tooltips 125-200ms, dropdowns 150-250ms, modals/drawers 200-500ms). Ship `prefers-reduced-motion` and `@media (hover: hover) and (pointer: fine)` gating in the same change as the motion, not after.

Vendored skills: `animate` and `review-animations` are copied from `emilkowalski/skills` (MIT, commit 85e8e23; provenance in `skills-lock.json`, license text in each skill folder). Their upstream "Initial Response" greeting block was removed so opencode can auto-invoke them, and `review-animations` keeps its upstream `disable-model-invocation: true` field, which opencode ignores. `npx skills@latest update` overwrites these local edits: review its diff and re-apply the greeting-block removal.

```bash
# Search for a relevant guide
npx -y modern-web-guidance@latest search "<query>" --skill-version 2026_09_04-7de96777

# Retrieve full guide(s) by ID
npx -y modern-web-guidance@latest retrieve "<id>"
```

Update with `npx -y modern-web-guidance@latest update`.

## Design non-negotiables

Product truth lives in `PRODUCT.md`; incumbent visual system in `DESIGN.md`.

- Every data view ships empty, loading, and error states.
- Every control is keyboard reachable and operable with a visible focus indicator.
- No em dash (`—`) in UI copy; use comma, period, colon, or parentheses.
- No fabricated claims, statistics, testimonials, or ghost navigation targets.
- Never generate final logos, avatars, or statistics without explicit
  instructions; use honest placeholders instead.
- Verify by running the app and clicking through every interactive element
  (`/verify-ui`); report the click-through element by element.
