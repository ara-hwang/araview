# Playbooks - AraView

Task-specific procedures for agents, reached from `../AGENTS.md`. Read the section that matches the task. Releases have their own doc: `releasing.md`.

## Add a supported format

1. Add the extension and MIME mapping, with tests, in `crates/araview-core/src/image.rs` (`SUPPORTED_EXTENSIONS`, `get_mime_type`)
2. Make `crates/araview-core/src/display.rs` decode it into RGBA frames (most formats go through the `image` crate; HEIC/HEIF through libheif)
3. Add a sample file to `samples/`, generated with tools installed on the PC and outside the repo's dependencies: raster/vector via `sharp` in a temp dir, HEIC/HEIF via Python `pillow-heif`, CBZ via `Compress-Archive`. If the PC has no encoder, install one on the PC.
4. Update `SPEC.md` §2 (and §16/§18/§20 when affected), `README.md`, and `docs/`
5. Done when `npm run docs:check` exits 0 and the sample opens in the dev app (see "Runtime check")

## Add an `araview-core` operation

1. Implement it as a sync function in the matching `crates/araview-core/src/ops/<domain>.rs` and export it from `ops/mod.rs`
2. Call it from the GPUI app inside `cx.background_spawn(...)`; never block the UI thread
3. Add a unit test next to it and, when the UI reaches it, a case in `src-gpui/src/app/tests.rs`
4. Update `SPEC.md` §15/§16 when the contract or a type changes

## Runtime check

`cargo run -p araview-gpui` opens the dev build (identifier `com.araview.viewer.dev`; the installed build uses `com.araview.viewer`). The dev build accepts debug-only controls through its single-instance pipe:

- `araview.exe action:<id>` runs a viewer action in the running window (`run_action` in `src-gpui/src/app/menu.rs`: shortcut action ids, `openSettings`, `viewLtr`, and so on). Release builds have no such channel.
- Screenshots: capture the window with `PrintWindow`; post key and wheel messages with `PostMessage` (client area only). Caption-area hit testing needs a real cursor, so ask before moving the user's mouse.
- Headless checks: `cargo test -p araview-gpui` drives keys, wheel, drag, drop, context menus, and dialogs through `VisualTestContext` (`src-gpui/src/app/tests.rs`).

## Update the third-party license data

Run `node scripts/generate-license-data.mjs` after changing dependencies (needs `cargo install cargo-about --locked --features cli` and network). It rewrites `src-gpui/THIRD_PARTY_LICENSES.json` (embedded in the app) and the section 2 table of `THIRD_PARTY_LICENSES.md`.

## Test the installer without touching the real install

`pwsh scripts/Build-Installer.ps1 -Suffix " Test" -OutFile target/AraView-Test-setup.exe` builds an installer whose install folder, uninstall entry, and shortcut carry the suffix, so installing it leaves a real AraView install alone.
