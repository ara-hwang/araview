# Playbooks - AraView

Task-specific procedures for agents, reached from `../AGENTS.md`. Read the section that matches the task. Releases have their own doc: `releasing.md`.

## Add a supported format

1. Add the extension and MIME mapping in `crates/araview-core/src/image.rs` (`SUPPORTED_EXTENSIONS`, `get_mime_type`)
2. Make `crates/araview-core/src/display.rs` decode it into RGBA frames (most formats go through the `image` crate; HEIC/HEIF through libheif)
3. Add a sample file to `samples/`, generated with tools installed on the PC and outside the repo's dependencies: raster/vector via `sharp` in a temp dir, HEIC/HEIF via Python `pillow-heif`, CBZ via `Compress-Archive`. If the PC has no encoder, install one on the PC.
4. Update `SPEC.md` §2 (and §16/§18/§20 when affected), `README.md`, and `docs/`
5. Done when `npm run docs:check` exits 0 and the sample opens in the dev app (see "Runtime check")

## Add an `araview-core` operation

1. Implement it as a sync function in the matching `crates/araview-core/src/ops/<domain>.rs` and export it from `ops/mod.rs`
2. Call it from the GPUI app inside `cx.background_spawn(...)`; never block the UI thread
3. When the UI reaches it, add a case in `src-gpui/src/app/tests.rs` (the kept E2E suite; unit tests are not kept)
4. Update `SPEC.md` §15/§16 when the contract or a type changes

## Runtime check

`cargo run -p araview-gpui` opens the dev build (identifier `com.araview.viewer.dev`; the installed build uses `com.araview.viewer`). The dev build accepts debug-only controls; release builds have neither channel.

**Control channel (preferred).** `src-gpui/src/app/bridge.rs` opens `\\.\pipe\com.araview.viewer.dev-control` and handles one line of JSON per request. Input goes into GPUI's own event path (`Window::dispatch_event`, `Window::dispatch_keystroke`), so modifier combinations survive, the user's mouse and keyboard stay untouched, and the window does not have to be active or on top. Window messages cannot do this: GPUI reads modifiers from `GetKeyState`, which posting cannot set.

Requests: `state`, `key` (`right`, `enter`, `ctrl-shift-c`), `text`, `mouse` (`click`, `down`, `up`, `move`, `drag`, `wheel`, with buttons, `modifiers`, `count`, `steps`, and `space: "image"|"logical"`), `drop`, `action`, `open`, `activate`, `resize` (`width`/`height` in logical px, at least the window minimum 500x400 and at most 8000, one side may be omitted; refused while maximized or fullscreen; the OS shrinks sizes beyond the monitor, and a non-integer scale such as 125% rounds to physical px, so read the real size from `state.window`), `wait`. Any request may carry `waitMs`; the reply arrives with `ok` and the current `state` (kind, name, path, index, count, viewMode, zoom, position, viewport, scaleFactor, infoOpen, gridOpen, dialogOpen, fullscreen, playing, frame, opening, error). The state in a reply is read right after the command, so background work (decode, prefetch) may still be running: wait with `waitMs` or repeat `state` instead of assuming it failed.

Two wrappers ship with it. Most commands need a running dev window; `launch` (CLI) or `araview_launch` (MCP) starts the dev build if needed. Both reuse the existing single instance:

- `pwsh scripts/araview-drive.ps1 <command>` is the shell CLI: `launch [-Path <file>]`, `windows`, `state`, `activate`, `resize <width> <height>`, `shot [-Out <png>]`, `key <spec>`, `type <text>`, `click|down|up|move <x> <y>`, `drag <x> <y> <toX> <toY>`, `wheel <x> <y> <lines>`, `action <id>`, `open <path>`, `drop <path>...`, `wait [-Until 'name=x,index=2']`, `selftest`, `mcp`. Results print as one JSON line. Screenshots use `PrintWindow` with `PW_RENDERFULLCONTENT` and fall back to plain `PrintWindow`, then to a screen copy (that last one captures overlapping windows too). Coordinates default to capture pixels and are mapped to window coordinates for you; `-Logical` uses window logical px. A locked or signed-out session, or a window that is not on screen, makes every capture method return black; the shot then reports `Blank: true` instead of pretending it saw the UI.
- `pwsh scripts/araview-mcp.ps1` serves the same tools over MCP stdio: `araview_windows`, `araview_launch`, `araview_state`, `araview_resize`, `araview_screenshot` (returns image content plus metrics), `araview_open`, `araview_drop`, `araview_action`, `araview_key`, `araview_type`, `araview_click`, `araview_drag`, `araview_move`, `araview_wheel`, `araview_wait`, `araview_selftest`. The repo's `.mcp.json` already registers it.

`pwsh scripts/araview-drive.ps1 selftest` drives a running dev window through window discovery, state, capture, key, ctrl-wheel, plain wheel, drag pan, click in both coordinate spaces, double click, text input, action, file open, and file drop, and prints PASS/FAIL per step. Use it as the runtime check evidence; read the rest of this section when a step fails.

**Single-instance pipe.** `araview.exe action:<id>` runs a viewer action in the running window (`run_action` in `src-gpui/src/app/menu.rs`: shortcut action ids, `openSettings`, `viewLtr`, and so on).

- Headless checks: `cargo test -p araview-gpui` drives keys, wheel, drag, drop, context menus, and dialogs through `VisualTestContext` (`src-gpui/src/app/tests.rs`). The bridge's commands are covered there too (`control_bridge_reports_state_and_drives_input`).
- Caption-area hit testing still needs a real cursor, so ask before moving the user's mouse.

## Update the third-party license data

Run `node scripts/generate-license-data.mjs` after changing dependencies (needs `cargo install cargo-about --locked --features cli` and network). It rewrites `src-gpui/THIRD_PARTY_LICENSES.json` (embedded in the app) and the section 2 table of `THIRD_PARTY_LICENSES.md`.

## Test the installer without touching the real install

`pwsh scripts/Build-Installer.ps1 -Suffix " Test" -OutFile target/AraView-Test-setup.exe` builds an installer whose install folder, uninstall entry, and shortcut carry the suffix, so installing it leaves a real AraView install alone.
