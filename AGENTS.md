# AGENTS.md - AraView

`araview` is a desktop image viewer written in Rust with GPUI Kit (gpui-kit). Windows 11 x64 is the only supported OS.

## Guardrails

- Release: run `npm run release` only when the user explicitly asks, because the tag push publishes a public release. Flow: `docs/releasing.md`.
- Never move the user's mouse or take over their input for a runtime check unless they agreed; prefer headless tests and the dev control channel (`pwsh scripts/araview-drive.ps1`, `docs/playbooks.md` "Runtime check").
- Agent memory lives in its own repo outside this one; never commit memory files here.

## Read first

- Changing behavior: read the matching `SPEC.md` section before editing (formats §2, file open §4, cache §9, settings §13, shortcuts and command palette §14, operations §15, data model §16, errors §17, rendering path §18, window and release §20), then update the sections §22 lists.
- Writing or reviewing code: `CODING_STANDARDS.md`.
- Changing visuals or motion: `DESIGN.md`. Judging product scope: `PRODUCT.md`.
- Locating a module or setting up the dev environment: `docs/development.md`.
- Adding a supported format or a core operation: the matching playbook in `docs/playbooks.md`.

## Verify

Run both passes yourself, unasked, and report every command with its result.

### After every code change

Each command must exit 0.

1. `cargo fmt --all`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `npm run docs:check` (changed supported extensions, settings keys, versions, or the docs listing them: `SPEC.md` §2/§13, `docs/development.md`)

### On completion, once

1. `cargo test --workspace`
2. Runtime: run the dev app (`cargo run -p araview-gpui`) and check every screen the change touches. Confirm the target first: the dev build uses identifier `com.araview.viewer.dev`; the installed build is `com.araview.viewer` and must not be the target. Report each checked screen or action as PASS or FAIL. Drive the window with `pwsh scripts/araview-drive.ps1` (CLI) or the `araview` MCP server, and use `selftest` as the floor. Procedure: `docs/playbooks.md`, "Runtime check".

Done when every step above is reported with its exit code, and the runtime checks element by element as PASS or FAIL.
