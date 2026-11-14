# araview-thumb

Windows Explorer thumbnail provider for PSD, CBZ/ZIP comic archives,
HEIC/HEIF/AVIF, EXR, and the viewer's supported RAW extensions. It implements
`IThumbnailProvider`, `IInitializeWithStream`, and `IInitializeWithFile`.
The preview pane (`Alt+P`) is outside its scope.

Decoding uses `araview-core::ops::decode_shell_thumbnail`. Comic archives
use a valid `ComicInfo.xml` `FrontCover` page, or the first image in natural
order. HEIF/AVIF, EXR, RAW and PSD use the same decoders as the viewer. Images
keep their aspect ratio, never upscale, and flatten transparency onto white.
PSB and corrupt files return errors so Explorer can show its default icon.

Registration is per user, under `HKCU\Software\Classes`, without changing
default apps. Settings and `DllRegisterServer` share the registration code
in `araview-core::thumb_shell`. Previous user handlers are restored when
disabling; slots now owned by another provider are left intact.

Published CLSIDs must remain stable:

- Release: `{FD6BD976-2DF4-4656-94F2-1D166163EC59}`
- Dev: `{BD277595-1702-4AC5-AA7C-A67965C3D570}`
- Shell slot: extension and channel ProgID
  `ShellEx\{E357FCCD-A995-4576-B01F-234630154E96}`

Build from the repository root:

```powershell
cargo build -p araview -p araview-thumb
```

The debug DLL is next to the dev executable in `target/debug/`. Native
codecs (`heif.dll`, `libde265.dll`, `aom.dll`) must be next to the thumbnail
DLL as well. The installer stages them in `resources/` with the DLL.

Enable **Settings → Extensions → Explorer thumbnails**. Existing PSD-only
registrations show **Apply to all supported formats**. Explorer may need
to restart or its thumbnail cache may need clearing before changes appear.
Manual registration uses `regsvr32 <absolute DLL>` and
`regsvr32 /u <absolute DLL>`.

Validation without registration or physical input:

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p araview-thumb --example probe -- D:\araview\target\debug\araview_thumb.dll D:\araview\samples
```

Use absolute paths for your checkout. The probe loads the actual DLL and
checks file and unnamed stream initialization, repeated thumbnail requests,
size bounds, corrupt input, PSB rejection, and initialization guards.

A missing native codec can prevent the COM DLL from loading. Check its
import table and packaged files. A running Explorer or surrogate can hold
the DLL open during rebuilds. High-integrity Explorer can ignore HKCU COM
registrations.
