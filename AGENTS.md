# AGENTS.md - AraView

`araview` is a desktop image viewer written in Rust with GPUI Kit (gpui-kit). Supported OS: Windows 11 x64 (verified). macOS and Linux x64 are experimental: they build in CI but are not runtime-tested. OS-specific code stays behind `app/src/platform/` and `cfg` gates; a change must keep all three building (CI builds each).

## Guardrails

- Release: run `npm run release` only when the user explicitly asks, because the tag push publishes a public release. Flow: `docs/releasing.md`.
- Never move the user's mouse or take over their input for a runtime check unless they agreed; prefer headless tests and the dev control channel (`pwsh scripts/araview-drive.ps1`, `docs/playbooks.md` "Runtime check").
- Windows behaviour: before calling a Windows-only change broken or fixed, run the Windows build under Wine. The prefix location, the mandatory DLL overrides, and the missing-DLL triage are in `docs/playbooks.md` "Check under Wine". A `c0000135` load failure is an import in the app until its import table says otherwise, not a Wine setup problem.
- Agent memory lives in its own repo outside this one; never commit memory files here.

## Read first

- Changing behavior: read the matching `SPEC.md` section before editing (formats §2, file open §4, cache §9, settings §13, shortcuts and command palette §14, operations §15, data model §16, errors §17, rendering path §18, window and release §20), then update the sections §22 lists.
- Writing or reviewing code: `CODING_STANDARDS.md`.
- Changing visuals or motion: `DESIGN.md`.
- Judging product scope: `PRODUCT.md`.
- Locating a module: `docs/development.md` "앱 구조 요약".
- Setting up the dev environment: `docs/development.md` "요구 사항".
- Adding a supported format: `docs/playbooks.md` "Add a supported format".
- Adding a core operation: `docs/playbooks.md` "Add an `araview-core` operation".
- Checking Windows behaviour without a Windows machine: `docs/playbooks.md` "Check under Wine".

## Verify

Run both passes yourself, unasked, and report every command with its result.

### After every code change

Each command must exit 0.

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `npm run docs:check`

### On completion, once

1. `cargo test --workspace`
2. Runtime: run the dev app (`cargo run -p araview`) and check every screen the change touches. Confirm the target first: the dev build uses identifier `com.araview.viewer.dev`; the installed build is `com.araview.viewer` and must not be the target. Report each checked screen or action as PASS or FAIL. Drive the window with `pwsh scripts/araview-drive.ps1` (CLI) or the `araview` MCP server, and use `selftest` as the floor. Procedure: `docs/playbooks.md`, "Runtime check".

Done when every step above is reported with its exit code, and the runtime checks element by element as PASS or FAIL.
