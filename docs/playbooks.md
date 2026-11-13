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
3. When the UI reaches it, check it in the dev app (see "Runtime check"); there is no headless UI suite
4. Update `SPEC.md` §15/§16 when the contract or a type changes

## Runtime check

`cargo run -p araview` opens the dev build (identifier `com.araview.viewer.dev`; the installed build uses `com.araview.viewer`). The dev build accepts debug-only controls; release builds have neither channel.

**No headless UI suite.** `cargo test -p araview` runs unit tests only; keys, wheel, drag, drop, context menus, dialogs, and the bridge's own commands go through the control channel below (`selftest`).

**Control channel (preferred).** `app/src/app/bridge.rs` opens `\\.\pipe\com.araview.viewer.dev-control` and handles one line of JSON per request. Input goes into GPUI's own event path (`Window::dispatch_event`, `Window::dispatch_keystroke`), so modifier combinations survive, the user's mouse and keyboard stay untouched, and the window does not have to be active or on top. Window messages cannot do this: GPUI reads modifiers from `GetKeyState`, which posting cannot set.

Requests: `state`, `key` (`right`, `enter`, `ctrl-shift-c`), `text`, `mouse` (`click`, `down`, `up`, `move`, `drag`, `wheel`, with buttons, `modifiers`, `count`, `steps`, and `space: "image"|"logical"`), `drop`, `action`, `open`, `activate`, `resize` (`width`/`height` in logical px, at least the window minimum 500x400 and at most 8000, one side may be omitted; refused while maximized or fullscreen; the OS shrinks sizes beyond the monitor, and a non-integer scale such as 125% rounds to physical px, so read the real size from `state.window`), `wait`, `resetFrames` (clears the frame-pacing stats so a measurement starts fresh). Any request may carry `waitMs`; the reply arrives with `ok` and the current `state` (kind, name, path, index, count, viewMode, zoom, position, viewport, scaleFactor, infoOpen, gridOpen, dialogOpen, fullscreen, playing, frame, opening, error, errorKind). `errorKind` is `open` when the error card is for a file that failed to open (its retry opens the same path again) and `page` when the current page failed to decode (its retry decodes that position again). The top-level `playing`/`frame` fields describe image-animation state. The state in a reply is read right after the command, so background work (decode, prefetch) may still be running: wait with `waitMs` or repeat `state` instead of assuming it failed.

**Frame pacing (debug builds only).** `state.frameStats` carries the GPUI frame-interval statistics since the last `resetFrames`: `intervals` (count), `lastMs`, `avgMs`, and `maxMs` — the max is the hitch bound. A bench repeats an `action` (page turns, mode changes) with small `waitMs` gaps until done, then reads `frameStats`; clear it first so the numbers cover only the repeated motion. Real page loads wait for decode, so prefer motion that is already cached, and expect the first action after a jump to be slower.

Two wrappers ship with it. Most commands need a running dev window; `launch` (CLI) or `araview_launch` (MCP) starts the dev build if needed. Both reuse the existing single instance:

- `pwsh scripts/araview-drive.ps1 <command>` is the shell CLI: `launch [-Path <file>]`, `windows`, `state`, `activate`, `resize <width> <height>`, `shot [-Out <png>]`, `key <spec>`, `type <text>`, `click|down|up|move <x> <y>`, `drag <x> <y> <toX> <toY>`, `wheel <x> <y> <lines>`, `action <id>`, `open <path>`, `drop <path>...`, `wait [-Until 'name=x,index=2']`, `hittest`, `selftest`, `mcp`. Results print as one JSON line. Screenshots use `PrintWindow` with `PW_RENDERFULLCONTENT` and fall back to plain `PrintWindow`, then to a screen copy (that last one captures overlapping windows too). Coordinates default to capture pixels and are mapped to window coordinates for you; `-Logical` uses window logical px. A locked or signed-out session, or a window that is not on screen, makes every capture method return black; the shot then reports `Blank: true` instead of pretending it saw the UI.
- `pwsh scripts/araview-mcp.ps1` serves the same tools over MCP stdio: `araview_windows`, `araview_launch`, `araview_state`, `araview_resize`, `araview_screenshot` (returns image content plus metrics), `araview_open`, `araview_drop`, `araview_action`, `araview_key`, `araview_type`, `araview_click`, `araview_drag`, `araview_move`, `araview_wheel`, `araview_wait`, `araview_hittest`, `araview_selftest`. The repo's `.mcp.json` already registers it.

`pwsh scripts/araview-drive.ps1 selftest` drives a running dev window through window discovery, state, capture, key, ctrl-wheel, plain wheel, drag pan, click in both coordinate spaces, double click, the caption hit-test baseline, backdrop drag and click with a dialog open, text input, action, file open, and file drop, and prints PASS/FAIL per step. A window smaller than 700x768 logical px is enlarged for the run and restored afterwards, because the centered palette would otherwise cover the points the backdrop steps probe. Use it as the runtime check evidence; read the rest of this section when a step fails.

`pwsh scripts/test-autohide.ps1` checks that revealing and hiding edge chrome leaves viewport bounds, zoom, pan, and webtoon reading position unchanged in all four view modes. Open an image, enable UI auto-hide, and close dialogs, the grid, and the information panel first. Run with each dock position and with the dock collapsed; the script uses the current dock settings and restores the original view mode.

`pwsh scripts/test-focus-scroll.ps1` checks forward Tab and reverse Shift+Tab scrolling in all six settings tabs, and checks that a wheel scroll is not undone while focus stays on the same control. It uses `agent-desktop` accessibility observations and the dev control channel, without physical input or changing settings. Close dialogs first.

**Single-instance pipe.** `araview.exe action:<id>` runs a viewer action in the running window (`run_action` in `app/src/app/menu.rs`: shortcut action ids, `openSettings`, `viewLtr`, and so on).

**Caption-area hit testing.** `pwsh scripts/araview-drive.ps1 hittest` (`araview_hittest` over MCP) moves the *virtual* mouse to a point through the control channel to prime GPUI's hit-test cache, then sends `WM_NCHITTEST` with `SendMessage` and prints the code Windows returns (`HTCLIENT`, `HTCAPTION`, `HTCLOSE`, `HTMINBUTTON`, `HTMAXBUTTON`). The physical cursor never moves, so this needs no permission from the user. Baseline (no dialog open): the three caption buttons report `HTMINBUTTON`/`HTMAXBUTTON`/`HTCLOSE` and everything else `HTCLIENT`. With a dialog open the backdrop covers the window, so every point outside the dialog reports `HTCAPTION` (caption buttons stop acting, and a press there drags the window) while the dialog surface stays `HTCLIENT`. Not covered: the press-to-drag move loop itself, which only a real press starts. The selftest runs the same probe as a step, so a regression in window control areas (or a backdrop left over the window) fails the standard runtime check.

## Check under Wine

Judging Windows behaviour on Linux: build the Windows target, run it under Wine, and watch startup, DLL loading, and the window. The values below are what WineHQ stable 11.0 with `wine-stable-i386` does on this machine.

```bash
export WINEPREFIX=/data/workspace/wine-prefix        # ~1.4 GB per prefix
export WINEDLLOVERRIDES="mscoree,mshtml="            # required, see below
DISPLAY=:1 wineboot -i                               # ~8 s, exit 0
wine target/x86_64-pc-windows-gnu/release/araview.exe FILE
```

- **Keep the prefix on real disk.** A prefix takes about 1.4 GB. On a tmpfs it fills up, and init then fails with `err:setupapi:... copy error 112` (disk full) and skips the nls files.
- **The override is part of the setup.** This Wine package ships no wine-mono/wine-gecko MSI and `~/.cache/wine` is empty, so the first `wineboot` or app start opens a "Wine Mono Installer" dialog and stays there; a headless run times out instead.
- **A healthy init** writes `drive_c`, `system32` (804 files), `syswow64`, and `*.reg`, exits 0, and repeats cleanly, including after `wineserver -k` cuts a boot short. `err:ole:start_rpcss` and `err:setupapi ... Unsupported style(s)` on the first run are expected.
- **A missing DLL points at the app.** `err:module:import_dll Library X ... not found` with `status c0000135` (process exit code 53) means code imports a DLL the package does not ship: read the import table with `x86_64-w64-mingw32-objdump -p target/x86_64-pc-windows-gnu/release/araview.exe | grep 'DLL Name'` and remove the import. Wine has no `icuuc.dll`; #149 was this exact case.
- **Screens.** `xvfb-run -s "-screen 0 1600x1000x24" wine ...`, then `import -window root shot.png` (ImageMagick). The Windows build keeps the named-pipe bridge and Wine has no PowerShell, so `scripts/araview-drive.ps1` and the `araview` MCP server stay out of reach: check the window from screenshots.

Done when every screen the change touches is reported PASS or FAIL, or listed as unverified with the reason.

## Update the third-party license data

Run `node scripts/generate-license-data.mjs` after changing dependencies (needs `cargo install cargo-about --locked --features cli` and network). It rewrites `app/THIRD_PARTY_LICENSES.json` (embedded in the app) and the section 2 table of `THIRD_PARTY_LICENSES.md`.

## Test the installer without touching the real install

`pwsh scripts/Build-Installer.ps1 -Suffix " Test" -OutFile target/AraView-Test-setup.exe` builds an installer whose install folder, uninstall entry, and shortcut carry the suffix, so installing it leaves a real AraView install alone.
