# araview-thumb — PSD Explorer thumbnail provider

Windows File Explorer shows a generic icon for `.psd` files. This crate
builds `araview_thumb.dll`, an in-process COM server implementing
`IThumbnailProvider` (+ `IInitializeWithStream` / `IInitializeWithFile`)
so Explorer renders real PSD thumbnails instead.

- Decoder: `psd` crate (pure Rust), composite pixels flattened onto white,
  matching the app's JPEG sidecar rule (`src-tauri/src/psd_sidecar.rs`).
- PSB (`8BPS` version 2) is rejected; Explorer falls back to the default icon.
- Registration is per-user (`HKCU\Software\Classes`), no admin rights.

## Layout

| File          | Role                                                            |
| ------------- | --------------------------------------------------------------- |
| `src/lib.rs`  | `DllGetClassObject` / `DllCanUnloadNow` / `DllRegisterServer`   |
| `src/com.rs`  | `IClassFactory` + `PsdThumbProvider` (stream/file init, GetThumbnail) |
| `src/psd.rs`  | PSD byte decode, aspect-fit sizing, downscale                    |
| `src/bitmap.rs` | RGB8 to 32bpp DIB section (`HBITMAP`)                          |
| `src/registry.rs` | HKCU registration (CLSID, ShellEx, PerceivedType)            |
| `araview_thumb.def` + `build.rs` | Marks the four COM exports `PRIVATE` (silences LNK4104) |

## CLSIDs (never change)

A published CLSID is baked into users' registries. Dev and release use
separate IDs so the dev build never steals the installed build's
thumbnails. The strings are duplicated in the main app
(`src-tauri/src/thumb_shell.rs`, which avoids the `windows` dependency).
Keep both in sync.

- Release: `{FD6BD976-2DF4-4656-94F2-1D166163EC59}`
- Dev: `{BD277595-1702-4AC5-AA7C-A67965C3D570}`
- Shell slot: `.psd` / ProgID `ShellEx\{E357FCCD-A995-4576-B01F-234630154E96}`

## Build

```powershell
# From the repo root. Debug DLL lands in src-tauri/target/debug/
# (next to the dev exe via the shared workspace target dir).
cargo build -p araview-thumb --manifest-path src-tauri/Cargo.toml

# Release + stage for the NSIS bundle (tauri.conf.json bundle.resources).
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/Build-ThumbDll.ps1
# or: npm run build:thumb
```

`npm run dev:up` builds the debug DLL automatically before launching.

## Register / unregister

Preferred path is the app UI: Settings, Extensions tab,
Explorer thumbnails section (per-user, no admin).

Manual alternative (same HKCU keys):

```powershell
regsvr32 .\araview_thumb.dll
regsvr32 /u .\araview_thumb.dll
```

## Verify in Explorer

1. Build the DLL matching your channel (debug for dev, release for installed).
2. Register from the app Settings (Explorer thumbnails, Enable).
3. Open a folder with `samples/sample.psd` in Large icons view.
4. If an old generic icon persists, clear the thumbnail cache once:

```powershell
Stop-Process -Name explorer -Force
Remove-Item "$env:LOCALAPPDATA\Microsoft\Windows\Explorer\thumbcache_*.db" -Force
Start-Process explorer
```

## Troubleshooting

- Still the generic icon: another app may own the `.psd` ShellEx slot
  (last writer wins), or `dllhost.exe` holds a stale DLL. Kill `dllhost`,
  re-enable in Settings, reopen the folder.
- `araview_thumb.dll` locked during rebuild: Explorer/`dllhost` has it
  loaded. Close Explorer windows showing PSD files or kill `dllhost`,
  then rebuild.
- Windows Sandbox / high-integrity Explorer ignores HKCU COM entries.
  Run installed (release) builds there; dev-channel thumbnails are not
  supported in that configuration.
- Preview pane (`Alt+P`) is not covered; this provider supplies
  thumbnails only.

## Tests

```powershell
cd src-tauri
cargo test -p araview-thumb
cargo clippy -p araview-thumb --all-targets -- -D warnings
```
