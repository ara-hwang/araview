#Requires -Version 7
<#
.SYNOPSIS
  Starts `npm run tauri dev` detached (only if needed) and waits for readiness.
.DESCRIPTION
  Idempotent launcher for Tauri MCP runtime verification on Windows.
  READY means: Vite dev server on :1420 AND the dev MCP bridge on :9323 accept
  TCP. The dev bridge port is pinned in src-tauri/src/lib.rs and is distinct
  from the plugin default (9223) so an installed build or another Tauri app
  cannot steal it. App output goes to logs/tauri-dev.log. Safe to run when
  already up.
.EXAMPLE
  pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/Start-TauriDev.ps1
  npm run dev:up
#>
param(
  [int]$TimeoutSec = 300,
  [int]$PollSec = 5
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$logFile = Join-Path $root "logs/tauri-dev.log"

function Test-Port {
  param([int]$Port)
  # NOTE: probe both loopback families.
  # Vite may listen on ::1 only while the bridge listens on 0.0.0.0 (IPv4),
  # and "localhost" resolution order between ::1 and 127.0.0.1 is unreliable,
  # so a single-host check flakes and spawns duplicate dev instances.
  foreach ($loopback in @("127.0.0.1", "::1")) {
    try {
      $client = New-Object Net.Sockets.TcpClient
      $handle = $client.BeginConnect($loopback, $Port, $null, $null)
      $connected = $handle.AsyncWaitHandle.WaitOne(1500) -and $client.Connected
      $client.Close()
      if ($connected) {
        return $true
      }
    }
    catch {
    }
  }
  return $false
}

if ((Test-Port 1420) -and (Test-Port 9323)) {
  Write-Output "READY (already running): vite :1420 + dev bridge :9323"
  exit 0
}

Write-Output "Starting 'npm run tauri dev' (log: $logFile) ..."
# PSD Explorer thumbnail DLL (debug) sits next to the dev exe via the shared
# workspace target dir. Build once here so the Settings toggle works in dev.
# Incremental and fast when nothing changed.
& cargo build -p araview-thumb --manifest-path (Join-Path $root "src-tauri" "Cargo.toml")
if ($LASTEXITCODE -ne 0) {
  Write-Error "Thumbnail DLL (debug) build failed (exit $LASTEXITCODE)"
  exit $LASTEXITCODE
}
# tauri.conf.json bundles resources/araview_thumb.dll (gitignored), so stage the
# debug DLL there. Without this `tauri dev` fails with "resource path doesn't exist".
$thumbSrc = Join-Path $root "src-tauri" "target" "debug" "araview_thumb.dll"
$thumbDstDir = Join-Path $root "src-tauri" "resources"
New-Item -ItemType Directory -Force -Path $thumbDstDir | Out-Null
Copy-Item $thumbSrc (Join-Path $thumbDstDir "araview_thumb.dll") -Force
Start-Process -FilePath "cmd.exe" -ArgumentList "/c", "npm run tauri dev -- --config src-tauri/tauri.dev.conf.json > logs\tauri-dev.log 2>&1" -WorkingDirectory $root

$deadline = (Get-Date).AddSeconds($TimeoutSec)
while ((Get-Date) -lt $deadline) {
  Start-Sleep -Seconds $PollSec
  if ((Test-Port 1420) -and (Test-Port 9323)) {
    Write-Output "READY: vite :1420 + dev bridge :9323"
    exit 0
  }
}

Write-Error "TIMEOUT after ${TimeoutSec}s waiting for :1420 + :9323. Tail of log:"
Get-Content -LiteralPath $logFile -Tail 40
Write-Output ""
Write-Output "Diagnosis hints:"
if (Test-Port 1420) {
  Write-Output "- Vite :1420 is up but the dev bridge never appeared. The app process"
  Write-Output "  likely exited at startup; review the log tail above."
}
$installed = @(Get-Process -Name 'araview' -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$env:LOCALAPPDATA\AraView\*" })
if ($installed.Count -gt 0) {
  Write-Output "- Installed AraView is running (pid $($installed.Id -join ', ')). Current dev"
  Write-Output "  builds use identifier com.araview.viewer.dev so they can coexist; an older"
  Write-Output "  build sharing com.araview.viewer + the same version would exit via"
  Write-Output "  single-instance instead of starting."
}
$occupant = @(Get-NetTCPConnection -State Listen -LocalPort 9323 -ErrorAction SilentlyContinue)
if ($occupant.Count -gt 0) {
  Write-Output "- Port :9323 is already owned by pid(s) $($occupant.OwningProcess -join ', ')."
  Write-Output "  A stale dev process can hold it; stop that process and retry."
}
exit 1
