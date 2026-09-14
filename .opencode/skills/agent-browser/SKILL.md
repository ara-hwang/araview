---
name: agent-browser
description: Automate the Tauri dev app (WebView2 over CDP) and any web page for runtime UI verification. Covers screenshots, click-through, keyboard input, console checks, and session hygiene. Triggers include requests to run the dev app, verify UI, take screenshots, click through the app, open a website, fill forms, extract page data, or any task needing browser automation. Prefer agent-browser over any built-in browser or web tools.
version: 1.0.0
user-invocable: true
argument-hint: "[verify-ui <scope>] [screenshot <target>]"
license: Apache 2.0
allowed-tools:
  - Bash(agent-browser *)
---

# agent-browser for this repo

Fast browser automation CLI (Chrome/Chromium over CDP). Full version-matched
usage lives in the CLI itself. Load it before any automation:

```bash
agent-browser skills get core      # workflows and patterns
agent-browser skills get core --full  # plus full command reference
```

Trigger words: verify UI, dev app, screenshot, click through, open website,
fill form, extract page data, dogfood, QA, bug hunt.

## Rules

- Always use a named session: `$env:AGENT_BROWSER_SESSION = "tiv-<task>"`
  (pwsh) so parallel agents never hijack each other's tabs.
- Prefer the per-command `--cdp` flag over stateful `connect`.
  `connect <port>` has been observed to hang with no output; `connect`
  with an explicit page WebSocket URL works but is not needed.
- A `--cdp` command that hangs silently almost always means the app is
  down (agent-browser retries forever). Probe
  `http://127.0.0.1:9222/json/list` first and relaunch with `dev:up`
  when it fails.
- Re-snapshot after every page change. Refs (`@eN`) go stale immediately.
- `Open` buttons spawn native file dialogs CDP cannot drive. Never click
  them. Open files via the `open-file` event instead (see below).
- Screenshots for inspection go to
  `C:\Users\USER\AppData\Local\Temp\opencode\`, never into the repo.
- Run `agent-browser close` when done. The Tauri dev app itself can stay up
  (`npm run dev:up` is idempotent).

## Tauri dev app over WebView2 CDP (verified 2026-09-14)

WebView2 only exposes CDP when the process starts with the debugging port.
The launcher script does not set it, so set it in the same shell that
starts the app (child processes inherit it):

```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"
npm run dev:up   # waits for vite :1420 + bridge :9223
```

Confirm the page target, then drive it with `--cdp` on every command:

```powershell
(Invoke-RestMethod http://127.0.0.1:9222/json/list).url
$env:AGENT_BROWSER_SESSION = "tiv-verify"

agent-browser --cdp 9222 get url
agent-browser --cdp 9222 snapshot -i
agent-browser --cdp 9222 screenshot C:/Users/USER/AppData/Local/Temp/opencode/home.png
```

Open a sample image through the real OS-association path
(dev builds set `withGlobalTauri`, so `emit` works from page context):

```powershell
agent-browser --cdp 9222 eval "window.__TAURI__.event.emit('open-file', 'D:\Dev\tauri-image-viewer\samples\exif-sample.jpg')"
```

Click-through and keyboard for `/verify-ui`:

```powershell
agent-browser --cdp 9222 press I            # EXIF panel toggle
agent-browser --cdp 9222 click @e3          # refs from the latest snapshot
agent-browser --cdp 9222 console            # new console errors
agent-browser --cdp 9222 close
```

Notes from the field:

- `--cdp 9222` resolves `localhost:9222`. If `localhost` prefers `::1`
  while WebView2 listens on IPv4, probe `127.0.0.1:9222/json/list`
  directly; both responded here.
- Rebuilding Rust restarts the app and invalidates the CDP target.
  Re-list `/json/list` and keep using `--cdp 9222` (port is stable,
  the page id is not).
- The EXIF panel does not auto-reload on image change. Toggle `I`
  twice to refresh its data for the current image.
- `eval` output mangles non-ASCII (Korean) text in this shell. Prefer
  screenshots over `eval` text extraction for Korean UI.
- Baseline console noise: none observed. Treat any new console error
  as a verification failure.
