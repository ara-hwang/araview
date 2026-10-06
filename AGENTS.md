# AGENTS.md - AraView

`araview` is a desktop image viewer: Tauri 2 (Rust) backend, React + TypeScript frontend in WebView2. Windows 11 x64 is the only supported OS.

## Sources of truth

- `SPEC.md`: functional/technical spec. Read the matching section before changing behavior: formats §2, settings §13, shortcuts and command palette §14, IPC contract §15, data model §16, error model §17, window/release §20. §22 lists which sections to update per kind of change.
- `PRODUCT.md`: product definition. `DESIGN.md`: visual system.
- `docs/development.md`: dev setup and the per-file module map of `src-tauri/src/`.
- `npm run docs:check` enforces doc/code agreement: extensions (`src/constants/imageExtensions.ts`, `src-tauri/src/image.rs`, `src-tauri/tauri.conf.json`), IPC commands (`SPEC.md` §15 table, `lib.rs` `invoke_handler`, command definitions), settings keys (`SPEC.md` §13, `settingsStore.ts`), plugin list (`docs/development.md`, `Cargo.toml`), versions. Run it after touching any of these.

## Coding standards

Before writing or reviewing code, read `CODING_STANDARDS.md`: TypeScript/React and Rust/Tauri conventions, component stack and icons, IPC shapes, rendering path, derived-image cache, window and titlebar, dialogs, browser support, motion tokens, design non-negotiables.

## Product constraints

- Update check is manual only (tauri-plugin-updater); no startup check, no background polling.
- File associations go through the Windows per-extension default-app picker; silent UserChoice writes are not possible.
- Dev builds register as `AraView (Dev)` with identifier `com.araview.viewer.dev` and separate registry keys/ProgIDs, so they cannot collide with the installed build (`src-tauri/src/file_assoc.rs`).
- PSB is excluded (no decoder). Explorer PSD thumbnails are opt-in via HKCU `IThumbnailProvider` (`SPEC.md` §20.2).

## Playbooks

- Adding a supported format or a backend command: follow the matching playbook in `docs/playbooks.md`.
- Preparing a release: run the release script (`npm run release`) only when the user explicitly asks: pushing the tag publishes a public release. Flow and flags: `docs/releasing.md`.

## Verification

Both steps below run automatically, without being asked.

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
3. Runtime verification with Tauri MCP: follow the checklist in `.opencode/commands/verify-ui.md` (`/verify-ui` in opencode).

Done when every step is reported with its result, and the runtime session target was confirmed first: `ipc-get-backend-state` reports `environment.debug: true` and identifier `com.araview.viewer.dev`. `debug: false` with `com.araview.viewer` is the installed build: stop and report instead of verifying the wrong app.

Report results yourself rather than handing the tests to the user. There is no browser-automation fallback: when the dev app or bridge cannot start, report the exact failure.

Runtime trouble, both in `docs/playbooks.md`:

- Tauri MCP looks missing or will not connect: run its ordered checks before concluding anything is broken.
- A frontend dependency was bumped: run its stale-bundle check before trusting runtime evidence.

## Agent skills

Precedence for overlapping frontend work (all under `.agents/skills/`):

1. **shadcn**: component selection, styling rules, forms, and `components.json` workflows.
2. **modern-web-guidance**: web platform APIs, performance patterns, and browser compatibility when adding or changing image loading, scroll behavior, forms, accessibility patterns, or CSS layout features. Skip it for Rust/Tauri IPC, Zustand state, and routine shadcn component edits. Adapt its framework-agnostic guides to React + `@base-ui/react` + shadcn; keep the existing component stack over native `<dialog>`/Popover API unless explicitly requested.
3. **animate** (vendored): building or changing motion, transitions, and micro-interactions.
4. **review-animations** (vendored): critiquing motion only.

Motion: motion-only work goes to animate/review-animations; broad design polish is handled directly against the design non-negotiables in `CODING_STANDARDS.md`.

Updating the vendored skills from upstream overwrites local edits: read "Update vendored skills" in `docs/playbooks.md` first.

Separate from the frontend precedence list: **agent-memory-repo** keeps cross-session agent memory in its own local git repo. Use it only when the user invokes it or asks to remember something. The memory repo must live outside this repository (default `~/agent-memory`); never commit memory files here.
