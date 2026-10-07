# AGENTS.md - AraView

`araview` is a desktop image viewer: Tauri 2 (Rust) backend, React + TypeScript frontend in WebView2. Windows 11 x64 is the only supported OS.

## Guardrails

- Release: run `npm run release` only when the user explicitly asks, because the tag push publishes a public release. Flow: `docs/releasing.md`.
- UI primitives are shadcn/ui on `@base-ui/react`; never add `@radix-ui/*`.
- Agent memory lives in its own repo outside this one; never commit memory files here.

## Read first

- Changing behavior: read the matching `SPEC.md` section before editing (formats §2, file open §4, cache §9, settings §13, shortcuts and command palette §14, IPC §15, data model §16, errors §17, rendering path §18, window and release §20), then update the sections §22 lists.
- Writing or reviewing code: `CODING_STANDARDS.md`.
- Changing visuals or motion: `DESIGN.md`. Judging product scope: `PRODUCT.md`.
- Locating a backend module or setting up the dev environment: `docs/development.md`.
- Adding a supported format or a backend command: the matching playbook in `docs/playbooks.md`.

## Verify

Run both passes yourself, unasked, and report every command with its result.

### After every code change

Each command must exit 0.

1. `npx tsc --noEmit`
2. `npm run lint:fix`
3. `npm run format`
4. `cd src-tauri && cargo fmt` (Rust sources changed)
5. `npm run docs:check` (changed supported extensions, IPC commands, settings keys, Tauri plugins, versions, or the docs listing them: `SPEC.md` §13/§15, `docs/development.md`)

### On completion, once

1. `npm test`
2. `cd src-tauri && cargo test` (Rust sources changed)
3. Runtime: run every item of `.opencode/commands/verify-ui.md` against the dev app with the `tauri-mcp` CLI from the shell. It is not a session MCP tool, so its absence from the tool list means nothing.
   - Confirm the target first: `ipc-get-backend-state` reports `environment.debug: true` and identifier `com.araview.viewer.dev`. `debug: false` with `com.araview.viewer` is the installed build: stop and report.
   - A UI change also needs a click-through of every interactive element on each view it touches.
   - Tauri MCP is the only runtime path, with no browser-automation fallback. When the dev app or bridge will not start, report the exact failure.

Done when every step above is reported with its exit code, and the click-through element by element as PASS or FAIL.

Runtime trouble, both in `docs/playbooks.md`:

- Tauri MCP looks missing or will not connect: run its ordered checks before concluding anything is broken.
- A frontend dependency was bumped: run its stale-bundle check before trusting runtime evidence.

## Skills

Precedence when frontend skills under `.agents/skills/` overlap: shadcn, modern-web-guidance, animate, review-animations.

- modern-web-guidance applies to web platform work only (image loading, scroll, forms, accessibility, CSS layout). Adapt its guides to the shadcn + `@base-ui/react` stack, which stays in place of native `<dialog>`/Popover API.
- animate and review-animations take motion-only work; broad design polish goes straight to the design non-negotiables in `CODING_STANDARDS.md`.
- Updating animate or review-animations from upstream overwrites local edits: read "Update vendored skills" in `docs/playbooks.md` first.
