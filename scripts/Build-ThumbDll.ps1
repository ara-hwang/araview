#Requires -Version 7
<#
.SYNOPSIS
  Builds the PSD thumbnail shell extension DLL and stages it for bundling.
.DESCRIPTION
  Compiles crates/araview-thumb and copies araview_thumb.dll to
  src-tauri/resources/ so `tauri build` includes it in the NSIS bundle
  (tauri.conf.json bundle.resources). Debug DLLs land next to the dev
  executable automatically via the shared workspace target dir, so this
  script is only needed for release/staged builds.
.EXAMPLE
  pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/Build-ThumbDll.ps1
  pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/Build-ThumbDll.ps1 -Configuration debug
#>
param(
  [ValidateSet("debug", "release")]
  [string]$Configuration = "release"
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$srcTauri = Join-Path $root "src-tauri"

$cargoArgs = @("build", "-p", "araview-thumb", "--manifest-path", (Join-Path $srcTauri "Cargo.toml"))
if ($Configuration -eq "release") {
  $cargoArgs += "--release"
}
Write-Output "cargo $($cargoArgs -join ' ')"
& cargo @cargoArgs
if ($LASTEXITCODE -ne 0) {
  Write-Error "Thumbnail DLL build failed (exit $LASTEXITCODE)"
  exit $LASTEXITCODE
}

$resources = Join-Path $srcTauri "resources"
New-Item -ItemType Directory -Force -Path $resources | Out-Null
Copy-Item (Join-Path $srcTauri "target" $Configuration "araview_thumb.dll") `
  (Join-Path $resources "araview_thumb.dll") -Force
Write-Output "Staged: src-tauri/resources/araview_thumb.dll ($Configuration)"
