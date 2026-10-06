# AGENTS.md - AraView

`araview` is a Windows 11 x64 desktop image viewer: Tauri 2 (Rust) backend, React + TypeScript frontend in WebView2. Windows 11 x64 is the only supported OS.

Stack: React, Vite, Tailwind CSS, TanStack Router, Zustand, shadcn/ui on `@base-ui/react` (do not introduce `@radix-ui/*`). Versions and scripts live in `package.json`; plugins in `src-tauri/Cargo.toml`.

## Sources of truth

- `SPEC.md`: functional/technical spec. Read the matching section before changing behavior: formats §2, settings §13, shortcuts and command palette §14, IPC contract §15, data model §16, error model §17, window/release §20. §22 lists which sections to update per kind of change.
- `PRODUCT.md`: product definition. `DESIGN.md`: visual system.
- `docs/development.md`: dev setup. `docs/releasing.md`: release flow.
- `npm run docs:check` enforces doc/code agreement: extensions (`src/constants/imageExtensions.ts`, `src-tauri/src/image.rs`, `src-tauri/tauri.conf.json`), IPC commands (`SPEC.md` §15 table, `lib.rs` `invoke_handler`, command definitions), settings keys (`SPEC.md` §13, `settingsStore.ts`), plugin list, versions. Run it after touching any of these.

## Product constraints

- Update check is manual only (tauri-plugin-updater); no startup check, no background polling.
- File associations go through the Windows per-extension default-app picker; silent UserChoice writes are not possible.
- Dev builds register as `AraView (Dev)` with identifier `com.araview.viewer.dev` and separate registry keys/ProgIDs, so they cannot collide with the installed build (`src-tauri/src/file_assoc.rs`).
- PSB is excluded (no decoder). Explorer PSD thumbnails are opt-in via HKCU `IThumbnailProvider` (`SPEC.md` §20.2).

## Where things live

Paths whose role the name does not confess:

- `src-tauri/src/commands/`: Tauri commands, one module per domain, re-exported from `mod.rs`
- `src-tauri/src/image.rs`: extension and MIME logic with its tests
- `src-tauri/src/transcode.rs`: dispatches sidecar formats to their decoder; shared pipeline is `SidecarSpec` in `sidecar.rs`; HEIC/HEIF decode in `heif.rs`
- `src-tauri/src/process_temp.rs`: derived-image cache root, protection, eviction, startup cleanup
- `src-tauri/src/cache.rs`: cache statistics and scoped deletion
- `src-tauri/src/pixel_art.rs`: conservative display-only detection; the frontend resolves `auto | smooth | pixelated` in `src/utils/imageRendering.ts`
- `src/constants/commands.ts`: command palette (`Ctrl+K`) commands
- `src/types/index.ts`: shared TS types mirroring Rust `serde` output

## Architecture

### IPC

The command table is `SPEC.md` §15. Arguments are camelCase on the JS side; responses are snake_case and match the TypeScript types 1:1, so a Rust `serde` shape change and its TS type change ship together.

File association open: Windows passes the path as a CLI argument. The backend buffers it in `PendingOpenFile` until the webview calls `frontend_ready`, then emits `open-file`; the root layout (`useOpenFileBridge`) bridges the event to the active route's loader. Listener: `src/hooks/useOpenFileListener.ts`.

### Rendering path

Image bytes reach the WebView only as a path:

- The backend returns a decodable filesystem path in `ImageInfo.file_path`; the frontend converts it with `convertFileSrc(...)`.
- `ImageInfo` stays metadata-only (fields: `SPEC.md` §16.1). Do not reintroduce base64 payload fields unless explicitly required.
- HEIC/HEIF/PSD/TGA/DDS/EXR/QOI are transcoded at load to JPEG sidecars under the active derived-image cache root.

### Persistence

- Tauri Store (`settings.json`) holds viewer settings (`settingsStore`) and recent files (`recentFilesStore`).
- `cacheStorageMode` is `persistent` (default, under the Tauri app cache directory) or `temporary` (session-only root); a mode change applies on the next launch.
- The derived-image cache is best-effort local data, never a source of truth.

## Code conventions

### TypeScript / React

- Prefer `type` over `interface`
- Domain logic lives in hooks in `src/hooks/`; components stay presentational
- Use Zustand selectors + `useShallow` for grouped subscriptions
- Import through the `@/...` alias
- `src/components/ui/` primitives are generated: change behavior at the call site, not in those files
- Icons are Phosphor (`@phosphor-icons/react`), typeface is Pretendard Variable (`DESIGN.md`); newly added shadcn components arrive with `lucide-react` imports, swap them to Phosphor equivalents on arrival

### Rust / Tauri

- Run blocking work in commands through `run_blocking`
- Every command returns `Result<T, AppError>` (`SPEC.md` §17)

## Playbooks

### Add a supported format

1. Add the extension and MIME mapping, with tests, in `src-tauri/src/image.rs`
2. Add the extension to `src/constants/imageExtensions.ts`
3. Add the file association to `src-tauri/tauri.conf.json`
4. Add a sample file to `samples/` and open it in the dev app: `load_image` must succeed, and EXIF-bearing formats also need a `get_exif_data` check. Generate samples with tools installed on the PC, never with repo dependencies: raster/vector via `sharp` in a temp dir, HEIC/HEIF via Python `pillow-heif`, CBZ via `Compress-Archive`. If the PC has no encoder, install one on the PC.
5. Update `SPEC.md` §2 (and §15/§16/§20 when affected), `README.md`, `docs/`, and this file's sidecar format list when relevant
6. Done when `npm run docs:check` passes and the sample opens

### Add a backend command

1. Implement it in the matching `src-tauri/src/commands/<domain>.rs` with `#[tauri::command]`
2. Register it in `src-tauri/src/lib.rs` `invoke_handler`
3. Call it from the frontend with `invoke(...)`, usually from a hook
4. Update `src/types/index.ts` when the payload or response shape changes
5. Add the row to the `SPEC.md` §15 table and any new type to §16
6. Done when `npm run docs:check` passes

### Prepare a release

Run the release script only when the user explicitly asks: pushing the tag publishes a public release.

1. From a clean, up-to-date `main`, run `npm run release -- <X.Y.Z|patch|minor|major>` (`--dry-run` previews the plan without touching anything). It bumps the five version files, checks them with `cargo metadata --locked` and `npm run docs:check`, commits, tags `vX.Y.Z`, and pushes `main` and the tag atomically after a confirmation prompt (`--yes` skips it).
2. The tag push starts `.github/workflows/release.yml`, which verifies the tag against the version files and the signing secret, then builds and publishes.
3. Full flow: `docs/releasing.md`.

## Verification

Frontend tests are `src/**/*.test.{ts,tsx}`; Rust tests are colocated with their modules. Both steps below run automatically, without being asked.

### After every code change

Apply fixes in write mode; each command must exit 0.

1. `npx tsc --noEmit`
2. `npm run lint:fix`
3. `npm run format`
4. `cd src-tauri && cargo fmt` (only when Rust sources changed)

The test suites (`npm test`, `cargo test`) wait for the completion pass so edits stay fast.

### Once the change is complete

1. `npm test`
2. `cd src-tauri && cargo test` (only when Rust sources changed)
3. Runtime verification with Tauri MCP: follow the checklist in `.opencode/commands/verify-ui.md` (`/verify-ui` in opencode). It starts the dev app with `npm run dev:up` (idempotent, waits for `:1420` + the dev bridge pinned to `:9323`) and connects with `tauri-mcp driver-session start --port 9323`.

Done when every step is reported with its result, and the runtime session target was confirmed first: `ipc-get-backend-state` reports `environment.debug: true` and identifier `com.araview.viewer.dev`. `debug: false` with `com.araview.viewer` is the installed build: stop and report instead of verifying the wrong app.

Report results yourself rather than handing the tests to the user. There is no browser-automation fallback: when the dev app or bridge cannot start, report the exact failure.

### When Tauri MCP looks missing

"Missing" is usually a misdiagnosis. Check in this order before concluding anything is broken:

1. Names: the repo name `mcp-server-tauri` is not an npm name. The MCP server is `@hypothesi/tauri-mcp-server` (stdio, prints nothing for `--help`/`--version` by design), the terminal CLI is `@hypothesi/tauri-mcp-cli` (`tauri-mcp` binary, no `--version`), the Rust bridge is `tauri-plugin-mcp-bridge`. Probe with `npm ls -g @hypothesi/tauri-mcp-cli` and `tauri-mcp --help`; `npm view mcp-server-tauri` (404) and `tauri-mcp --version` (unknown option) fail by design.
2. Installed vs connected: the CLI responding means installed. Connecting additionally needs `npm run dev:up` plus `driver-session start --port 9323`. The bridge exists only in dev builds (`dev-mcp` feature, `src-tauri/src/lib.rs`); release builds (`--no-default-features`, used by CI) have no bridge, so a connection failure there is expected.
3. Fresh-session flake: MCP loads at session start and the first `npx -y` download can time out. Repair with `get_setup_instructions` or restart the session.

### After bumping a frontend dependency

Kill the old dev processes, then check the bundle is fresh. `dev:up` is idempotent by port, so a stale Vite server keeps serving the old optimized bundle and produces phantom errors from the previous library version. Stale evidence is indistinguishable from a real bug until the served chunk graph is checked (`node_modules/.vite/deps` timestamps and `page.url` chunk hashes).

## Window / UX

- The window is frameless (`decorations: false`) and starts hidden (`visible: false`) until webview startup; the custom titlebar is `src/components/Header.tsx`.
- Windows 11 Snap Layouts comes from `tauri-plugin-snap-layout`: it floats a transparent native hit-test overlay (`WM_NCHITTEST` → `HTMAXBUTTON`) over the maximize caption button (`id=caption-maximize`). The overlay owns the mouse, so that button's hover wash/tooltip are mirrored from `tauri-snap://snap/mouseenter|mouseleave` events via `src/hooks/useSnapLayout.ts`; keep the Rust `button_id` and the DOM id in sync. Non-Windows and jsdom are no-ops.
- Dialogs/Sheets pass `modal="trap-focus"` at each call site so the caption buttons stay clickable. `modal={true}` makes Base UI render a transparent `position: fixed; inset: 0` `InternalBackdrop` that swallows every click outside the popup (including the titlebar) and dismisses on press, so minimize/maximize/close would do nothing while a dialog is open. `trap-focus` keeps the focus trap, `Esc`, outside `aria-hidden`, and backdrop-click dismissal; only the invisible shield and the body scroll lock go away (the shell is `h-screen overflow-hidden`, so the body never scrolls). Overlays start at `--header-height`, so the header strip is the only clickable area outside the popup. A header `pointer-events: none` rule in `src/App.css` is not the fix.

## Browser support

WebView2 (Chromium, Windows 11 x64) is the only frontend runtime.

- Baseline Widely available features: use without fallbacks.
- Baseline Newly available features: feature-detect and degrade gracefully.
- No polyfills or compatibility libraries. A custom fallback is allowed when it adds roughly 20 lines or fewer and no dependency.
- Core Web Vitals (LCP, INP, CLS) are secondary to desktop viewer responsiveness; image loading and scroll performance still matter.

## Agent skills

Precedence for overlapping frontend work (all under `.agents/skills/`):

1. **shadcn**: component selection, styling rules, forms, and `components.json` workflows.
2. **modern-web-guidance**: web platform APIs, performance patterns, and browser compatibility when adding or changing image loading, scroll behavior, forms, accessibility patterns, or CSS layout features. Skip it for Rust/Tauri IPC, Zustand state, and routine shadcn component edits. Adapt its framework-agnostic guides to React + `@base-ui/react` + shadcn; keep the existing component stack over native `<dialog>`/Popover API unless explicitly requested.
3. **animate** (vendored): building or changing motion, transitions, and micro-interactions. Load `RECIPES.md` when the request matches one of its components.
4. **review-animations** (vendored): critiquing motion only. Load `STANDARDS.md` when a finding needs an exact curve, duration, or spring value.

Separate from the frontend precedence list: **agent-memory-repo** (from `AgentMemoryRepo/agentmemoryrepo`) keeps cross-session agent memory in its own local git repo. Use it only when the user invokes it or asks to remember something. The memory repo must live outside this repository (default `~/agent-memory`); never commit memory files here. It adds no hooks or startup scripts and does not push anywhere unless the user names a private repo they own.

Motion: motion-only work goes to animate/review-animations; broad design polish is handled directly against the design non-negotiables below. Motion tokens live in `src/App.css` (`@theme`): `--ease-motion-out` (entrances, exits), `--ease-motion-in-out` (on-screen movement), `--ease-motion-drawer` (sheets, drawers), available as `ease-motion-*` utilities; the built-in `ease-out`/`ease-in-out` stay untouched. Durations come from the vendored tables (press 100-160ms, tooltips 125-200ms, dropdowns 150-250ms, modals/drawers 200-500ms). Ship `prefers-reduced-motion` and `@media (hover: hover) and (pointer: fine)` gating in the same change as the motion.

Vendored skills: `animate` and `review-animations` are copied from `emilkowalski/skills` (MIT, commit 85e8e23; provenance in `skills-lock.json`, license text in each skill folder). Their upstream "Initial Response" greeting block was removed so opencode can auto-invoke them, and `review-animations` keeps its upstream `disable-model-invocation: true` field, which opencode ignores. `npx skills@latest update` overwrites these local edits: review its diff and re-apply the greeting-block removal.

## Design non-negotiables

- Every data view ships empty, loading, and error states.
- Every control is keyboard reachable and operable with a visible focus indicator.
- No em dash (`—`) in UI copy; use comma, period, colon, or parentheses.
- No fabricated claims, statistics, testimonials, or ghost navigation targets.
- Final logos, avatars, and statistics need explicit instructions; until then use honest placeholders.
- Verify by running the app and clicking through every interactive element (`.opencode/commands/verify-ui.md`); report the click-through element by element.
