# Coding standards - AraView

Rules for writing and reviewing code in this repo. Each section is self-contained: read the ones your change touches. Behavior is specified in `SPEC.md`, the visual system in `DESIGN.md`.

## TypeScript / React

- Prefer `type` over `interface`
- Domain logic lives in hooks in `src/hooks/`; components stay presentational
- Use Zustand selectors + `useShallow` for grouped subscriptions
- Import through the `@/...` alias

## Component stack, icons, typeface

- UI components are shadcn/ui on `@base-ui/react`; do not introduce `@radix-ui/*`.
- `src/components/ui/` primitives are generated: change behavior at the call site, not in those files
- Icons are Phosphor (`@phosphor-icons/react`), typeface is Pretendard Variable (`DESIGN.md`); newly added shadcn components arrive with `lucide-react` imports, swap them to Phosphor equivalents on arrival

## Rust / Tauri

- Run blocking work in commands through `run_blocking`
- Every command returns `Result<T, AppError>` (`SPEC.md` §17)

## IPC

Arguments are camelCase on the JS side; responses are snake_case and match the TypeScript types in `src/types/index.ts` 1:1, so a Rust `serde` shape change and its TS type change ship together.

`src/constants/commands.ts` holds command palette (`Ctrl+K`) commands, not Tauri commands.

File association open (`SPEC.md` §4.3): the backend buffers the CLI path in `PendingOpenFile` until the webview calls `frontend_ready`, then emits `open-file`; `useOpenFileBridge` (`src/hooks/useOpenFileListener.ts`, mounted in the root layout) bridges the event to the active route's loader.

## Rendering path

Image bytes reach the WebView only as a path:

- The backend returns a decodable filesystem path in `ImageInfo.file_path`; the frontend converts it with `convertFileSrc(...)`.
- `ImageInfo` stays metadata-only (fields: `SPEC.md` §16.1). Do not reintroduce base64 payload fields unless explicitly required.
- Formats WebView2 cannot paint are transcoded at load to JPEG sidecars under the active derived-image cache root (format list: `SPEC.md` §18).

Where the code lives:

- `src-tauri/src/transcode.rs`: dispatches sidecar formats to their decoder; shared pipeline is `SidecarSpec` in `sidecar.rs`; HEIC/HEIF decode in `heif.rs`
- `src-tauri/src/pixel_art.rs`: conservative display-only detection; the frontend resolves `auto | smooth | pixelated` in `src/utils/imageRendering.ts`

## Derived-image cache

- The derived-image cache is best-effort local data, never a source of truth.
- A `cacheStorageMode` change (`persistent | temporary`, `SPEC.md` §9.2) applies on the next launch.
- `src-tauri/src/process_temp.rs`: derived-image cache root, protection, eviction, startup cleanup

## Window and titlebar

- The window is frameless (`decorations: false`) and starts hidden (`visible: false`); the custom titlebar is `src/components/Header.tsx`.
- Windows 11 Snap Layouts (`tauri-plugin-snap-layout`, mechanism in `SPEC.md` §20): a transparent native overlay owns the mouse over the maximize caption button (`id=caption-maximize`), so that button's hover wash/tooltip are mirrored from plugin events in `src/hooks/useSnapLayout.ts`; keep the Rust `button_id` and the DOM id in sync.

## Dialogs and Sheets

Dialogs/Sheets pass `modal="trap-focus"` at each call site so the caption buttons stay clickable.

- Why: `modal={true}` makes Base UI render a transparent `position: fixed; inset: 0` `InternalBackdrop` that swallows every click outside the popup (including the titlebar) and dismisses on press, so minimize/maximize/close would do nothing while a dialog is open.
- What `trap-focus` keeps: the focus trap, `Esc`, outside `aria-hidden`, and backdrop-click dismissal.
- Overlays start at `--header-height`, so the header strip is the only clickable area outside the popup.
- A header `pointer-events: none` rule in `src/App.css` is not the fix.

## Browser support

WebView2 (Chromium, Windows 11 x64) is the only frontend runtime.

- Baseline Widely available features: use without fallbacks.
- Baseline Newly available features: feature-detect and degrade gracefully.
- No polyfills or compatibility libraries. A custom fallback is allowed when it adds roughly 20 lines or fewer and no dependency.
- Core Web Vitals (LCP, INP, CLS) are secondary to desktop viewer responsiveness; image loading and scroll performance still matter.

## Motion

- Motion tokens live in `src/App.css` (`@theme`): `--ease-motion-out` (entrances, exits), `--ease-motion-in-out` (on-screen movement), `--ease-motion-drawer` (sheets, drawers), available as `ease-motion-*` utilities; the built-in `ease-out`/`ease-in-out` stay untouched.
- Durations come from the duration table in `.agents/skills/review-animations/STANDARDS.md`.
- Ship `prefers-reduced-motion` and `@media (hover: hover) and (pointer: fine)` gating in the same change as the motion.

## Design non-negotiables

- Every data view ships empty, loading, and error states.
- Every control is keyboard reachable and operable with a visible focus indicator.
- No em dash (U+2014) in UI copy; use comma, period, colon, or parentheses.
- No fabricated claims, statistics, testimonials, or ghost navigation targets.
- Final logos, avatars, and statistics need explicit instructions; until then use honest placeholders.
- Verify by running the app and clicking through every interactive element (`.opencode/commands/verify-ui.md`); report the click-through element by element.
