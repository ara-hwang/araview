#Requires -Version 7
<#
.SYNOPSIS
  Build, sign, and publish a release from this machine instead of CI.
.DESCRIPTION
  Use when the release workflow is unavailable or a full CI build takes too long.
  Steps:
    1. Read the version from src-tauri/tauri.conf.json and derive the tag (vX.Y.Z).
    2. Refuse to run on a dirty tree or when the tag points at another commit.
    3. Load <repo>/.env.local when present, the way Vite does, so a gitignored local
       file can hold the key and its password instead of exported variables. Values
       already present in the real environment win.
    4. Resolve the signing key: -KeyPath, then TAURI_SIGNING_PRIVATE_KEY, then
       <repo>/araview.key, then ~/.tauri/araview.key. The password comes from
       TAURI_SIGNING_PRIVATE_KEY_PASSWORD, otherwise the build prompts for it.
    5. Run a signed release build (skip with -SkipBuild to reuse artifacts, cap
       cargo parallelism with -Jobs to keep the machine responsive).
    6. Check that the signature belongs to plugins.updater.pubkey.
    7. Collect the installer, its signature, and latest.json in <repo>/release/<tag>.
    8. Push the tag and create or update the release in -UpdatesRepo (defaults to
       the public updates feed). That release holds the installer, its signature,
       and latest.json, which is what the updater endpoint points at while the
       source stays private.

  The signing key stays on this machine; only the installer, its signature, and
  latest.json are uploaded.
.EXAMPLE
  # Copy .env.example to .env.local (gitignored) and fill in the key and password.
  # No environment variables needed.
  npm run release:local
.EXAMPLE
  # Or keep the key at <repo>/araview.key and only export the password.
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "<password>"
  npm run release:local
.EXAMPLE
  # Explicit key path, or inspect the flow without touching GitHub.
  pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/Publish-LocalRelease.ps1 -KeyPath D:\keys\araview.key -DryRun -SkipBuild
.EXAMPLE
  # Updates feed: release assets go to the public repo by default.
  # Pass -UpdatesRepo to put them in the source repo instead.
  npm run release:local -- -UpdatesRepo ara-hwang/araview
.EXAMPLE
  # Reuse the last build (publishes immediately by default).
  npm run release:local -- -SkipBuild
.EXAMPLE
  # Keep the machine responsive: cap cargo at 4 parallel jobs.
  npm run release:local -- -Jobs 4
#>
param(
  # GitHub repository that owns the source and, by default, the release.
  [string]$Repo = "ara-hwang/araview",
  # Repository that hosts the release assets the updater downloads.
  # Defaults to the public updates feed while the source stays private.
  [string]$UpdatesRepo = "ara-hwang/araview-updates",
  # Skip `npm run tauri build` and reuse the existing bundle artifacts.
  [switch]$SkipBuild,
  # Publish the release immediately. Pass -Publish:$false to leave it as a draft.
  [switch]$Publish = $true,
  # Release notes. Defaults to the same sentence the release workflow uses.
  [string]$Notes = "See the assets to download this version and install.",
  # Signing key file. Defaults to the search order documented in -Description.
  [string]$KeyPath,
  # Directory holding the NSIS installer and its .sig produced by the build.
  [string]$BundleDir,
  # Where the release assets (installer, .sig, latest.json) are collected.
  # Defaults to <repo>/release/<tag>.
  [string]$OutputDir,
  # Resolve everything and collect the assets, but do not create a tag or a release.
  [switch]$DryRun,
  # Cargo build parallelism. 0 keeps cargo's default (one job per logical CPU).
  # Lower it to keep the machine responsive during the release build.
  [int]$Jobs = 0
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$configPath = Join-Path $root "src-tauri/tauri.conf.json"
if (-not $BundleDir) {
  $BundleDir = Join-Path $root "src-tauri/target/release/bundle/nsis"
}

function Fail {
  param([string]$Message)
  [Console]::Error.WriteLine("ERROR: $Message")
  exit 1
}

# Key id of a minisign box: 2 bytes of algorithm followed by 8 bytes of key id.
function Get-KeyId {
  param([string]$BoxText)
  $lines = @($BoxText -split "`r?`n" | Where-Object { $_ -ne "" })
  if ($lines.Count -lt 2) { Fail "Unexpected key format: no key line" }
  $bytes = [Convert]::FromBase64String($lines[1])
  if ($bytes.Length -lt 10) { Fail "Unexpected key format: key line too short" }
  (($bytes[2..9])[7..0] | ForEach-Object { $_.ToString('X2') }) -join ''
}

# Load <repo>/.env.local when present, the same convention Vite uses, so a local
# copy of the key and password works without exporting anything. Variables already
# set in the real environment always win. Keep this file out of git.
$envLocalPath = Join-Path $root ".env.local"
if (Test-Path -LiteralPath $envLocalPath) {
  $loaded = @()
  foreach ($line in Get-Content -LiteralPath $envLocalPath) {
    $trimmed = $line.Trim()
    if ($trimmed -eq "" -or $trimmed.StartsWith("#")) { continue }
    $parts = $trimmed -split "=", 2
    if ($parts.Count -ne 2) { continue }
    $name = $parts[0].Trim()
    $value = $parts[1].Trim()
    if ($name -notin @("TAURI_SIGNING_PRIVATE_KEY", "TAURI_SIGNING_PRIVATE_KEY_PASSWORD")) { continue }
    if ($value.Length -ge 2 -and $value.StartsWith('"') -and $value.EndsWith('"')) {
      $value = $value.Substring(1, $value.Length - 2)
    }
    if (Test-Path "env:$name") { continue }
    Set-Item -Path "env:$name" -Value $value
    $loaded += $name
  }
  if ($loaded.Count -gt 0) {
    Write-Output "Loaded from .env.local: $($loaded -join ', ')"
  }
}

# 1. version and tag
$config = Get-Content -Raw -LiteralPath $configPath | ConvertFrom-Json
$version = $config.version
$productName = $config.productName
$tag = "v$version"
if (-not $OutputDir) {
  $OutputDir = Join-Path $root "release/$tag"
}
$releaseRepo = if ($UpdatesRepo) { $UpdatesRepo } else { $Repo }
Write-Output "Release: $productName $tag"

# 2. the artifact has to match the tagged commit
$dirty = @(git -C $root status --porcelain)
if ($dirty.Count -gt 0) {
  Fail "Working tree is dirty, commit or stash first:`n$($dirty -join "`n")"
}
$head = (git -C $root rev-parse HEAD).Trim()
$tagCommit = (git -C $root rev-list -n 1 $tag 2>&1 | Out-String).Trim()
if ($LASTEXITCODE -ne 0) { $tagCommit = "" }
if ($tagCommit) {
  if ($tagCommit -ne $head) {
    Fail "Tag $tag points at $tagCommit but HEAD is $head. Bump the version or move the tag on purpose."
  }
}
else {
  Write-Output "Tag $tag does not exist yet, it will be created at $head"
}

# 3. resolve the signing key: -KeyPath, environment, repo, then the home folder
if ($KeyPath) {
  if (-not (Test-Path -LiteralPath $KeyPath)) { Fail "Key file not found: $KeyPath" }
  $env:TAURI_SIGNING_PRIVATE_KEY = (Resolve-Path -LiteralPath $KeyPath).Path
  Write-Output "Signing key: $env:TAURI_SIGNING_PRIVATE_KEY (-KeyPath)"
}
elseif (Test-Path env:TAURI_SIGNING_PRIVATE_KEY) {
  $keyValue = $env:TAURI_SIGNING_PRIVATE_KEY
  if (Test-Path -LiteralPath $keyValue) {
    Write-Output "Signing key: $keyValue (from TAURI_SIGNING_PRIVATE_KEY)"
  }
  else {
    Write-Output "Signing key: from TAURI_SIGNING_PRIVATE_KEY (key content)"
  }
}
else {
  $candidates = @(Join-Path $root "araview.key")
  if ($env:USERPROFILE) { $candidates += Join-Path $env:USERPROFILE ".tauri/araview.key" }
  $found = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
  if (-not $found) {
    Fail "Signing key not found. Looked at:`n  $($candidates -join "`n  ")`nPass -KeyPath, set TAURI_SIGNING_PRIVATE_KEY (or put it in .env.local), or place the key at one of those paths."
  }
  $env:TAURI_SIGNING_PRIVATE_KEY = (Resolve-Path -LiteralPath $found).Path
  Write-Output "Signing key: $env:TAURI_SIGNING_PRIVATE_KEY"
}
if (-not (Test-Path env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD)) {
  Fail "TAURI_SIGNING_PRIVATE_KEY_PASSWORD is not set. Put it in <repo>/.env.local or export it (an empty value is allowed) so the build never prompts."
}

# 4. build
if (-not $SkipBuild) {
  # Limit cargo parallelism so a release build does not saturate every core.
  # Applies to the thumbnail DLL build and `tauri build` (both inherit this env var).
  if ($Jobs -gt 0) {
    $env:CARGO_BUILD_JOBS = "$Jobs"
    Write-Output "cargo jobs: $Jobs (CARGO_BUILD_JOBS)"
  }
  Write-Output "Running signed release build, this takes a few minutes..."
  Push-Location $root
  try {
    # PSD 썸네일 DLL을 먼저 빌드해 번들 resources에 스테이징한다.
    # tauri.windows.conf.json bundle.resources가 src-tauri/resources/를 참조한다.
    # (CI release.yml의 "Build PSD thumbnail DLL" 단계와 동일. 로컬 빌드에서
    # 빠지면 설치본에 DLL이 없어 탐색기 썸네일 등록이 실패한다.)
    pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/Build-ThumbDll.ps1
    if ($LASTEXITCODE -ne 0) { Fail "Build-ThumbDll.ps1 failed with exit code $LASTEXITCODE" }
    npm run tauri build
    if ($LASTEXITCODE -ne 0) { Fail "npm run tauri build failed with exit code $LASTEXITCODE" }
  }
  finally {
    Pop-Location
  }
}

# 5. artifacts
$installer = @(Get-ChildItem -LiteralPath $BundleDir -File -Filter "*$version*setup.exe" -ErrorAction SilentlyContinue)
if ($installer.Count -eq 0) {
  Fail "No installer for version $version in $BundleDir. Build first or pass -BundleDir."
}
$installer = $installer | Sort-Object LastWriteTime -Descending | Select-Object -First 1
$sigPath = "$($installer.FullName).sig"
if (-not (Test-Path -LiteralPath $sigPath)) {
  Fail "Missing $sigPath. Rebuild without --no-sign so the updater signature is produced."
}
Write-Output "Build output: $($installer.Name) ($($installer.Length) bytes)"

# 6. the signature must belong to the configured public key
$sigBox = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String((Get-Content -Raw -LiteralPath $sigPath).Trim()))
$sigKeyId = Get-KeyId $sigBox
$pubKeyRaw = $config.plugins.updater.pubkey
if (Test-Path -LiteralPath $pubKeyRaw) { $pubKeyRaw = (Get-Content -Raw -LiteralPath $pubKeyRaw).Trim() }
$pubKeyBox = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($pubKeyRaw))
$pubKeyId = Get-KeyId $pubKeyBox
if ($sigKeyId -ne $pubKeyId) {
  Fail "Signature key id $sigKeyId does not match plugins.updater.pubkey key id $pubKeyId. The updater would reject this release."
}
Write-Output "Signature key id matches the configured pubkey ($sigKeyId)"

# 7. collect the release assets inside the repo, then write latest.json, which is
#    served as https://github.com/<repo>/releases/latest/download/latest.json
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
$assetInstaller = Join-Path $OutputDir $installer.Name
$assetSig = "$assetInstaller.sig"
$latestPath = Join-Path $OutputDir "latest.json"
Copy-Item -LiteralPath $installer.FullName -Destination $assetInstaller -Force
Copy-Item -LiteralPath $sigPath -Destination $assetSig -Force
$latest = [ordered]@{
  version   = $version
  notes     = $Notes
  pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
  platforms = [ordered]@{
    "windows-x86_64" = [ordered]@{
      signature = (Get-Content -Raw -LiteralPath $assetSig).Trim()
      url       = "https://github.com/$releaseRepo/releases/download/$tag/$($installer.Name)"
    }
  }
}
$latest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $latestPath
Write-Output "Release assets in $OutputDir"
Get-ChildItem -LiteralPath $OutputDir -File | Sort-Object Name | ForEach-Object {
  Write-Output "  $($_.Name) ($($_.Length) bytes)"
}
Write-Output "Updater url: $($latest.platforms.'windows-x86_64'.url)"

if ($DryRun) {
  Write-Output "DryRun: skipped tag push and release creation"
  exit 0
}

# 8. tag and release
if (-not $tagCommit) {
  git -C $root tag -a $tag -m "$productName $tag"
  if ($LASTEXITCODE -ne 0) { Fail "git tag $tag failed" }
}
git -C $root push origin $tag
if ($LASTEXITCODE -ne 0) { Fail "git push origin $tag failed" }

# When the release repo differs from the source repo (interim public feed), the tag
# has to exist there as well. It points at that repo's default branch, since the feed
# repo carries no source history.
$null = gh api "repos/$releaseRepo/git/ref/tags/$tag" 2>&1
if ($LASTEXITCODE -ne 0) {
  $branch = (gh api "repos/$releaseRepo" --jq ".default_branch" | Out-String).Trim()
  $branchSha = (gh api "repos/$releaseRepo/git/ref/heads/$branch" --jq ".object.sha" 2>&1 | Out-String).Trim()
  if ($LASTEXITCODE -ne 0 -or -not $branchSha) { Fail "Release repo $releaseRepo has no branch $branch to tag." }
  gh api -X POST "repos/$releaseRepo/git/refs" -f "ref=refs/tags/$tag" -f "sha=$branchSha" | Out-Null
  if ($LASTEXITCODE -ne 0) { Fail "Failed to create tag $tag in $releaseRepo." }
  Write-Output "Created tag $tag in $releaseRepo at $branchSha"
}

$viewRaw = (gh release view $tag --repo $releaseRepo --json tagName,isDraft 2>&1 | Out-String).Trim()
$releaseExists = $LASTEXITCODE -eq 0
$isDraft = $false
if ($releaseExists) {
  $isDraft = ($viewRaw | ConvertFrom-Json).isDraft
}

if ($releaseExists) {
  Write-Output "Release $tag exists, uploading assets with --clobber"
  gh release upload $tag $assetInstaller $assetSig $latestPath --repo $releaseRepo --clobber
}
else {
  $releaseArgs = @(
    "release", "create", $tag,
    "--repo", $releaseRepo,
    "--title", "$productName $tag",
    "--notes", $Notes,
    "--verify-tag",
    $assetInstaller, $assetSig, $latestPath
  )
  if (-not $Publish) { $releaseArgs += "--draft" }
  gh @releaseArgs
}
if ($LASTEXITCODE -ne 0) { Fail "gh release failed with exit code $LASTEXITCODE" }

if ($Publish -and $isDraft) {
  gh release edit $tag --repo $releaseRepo --draft=false
  if ($LASTEXITCODE -ne 0) { Fail "gh release edit failed with exit code $LASTEXITCODE" }
}

if (-not $Publish) {
  Write-Output "Draft release ready. Publish with: gh release edit $tag --repo $releaseRepo --draft=false"
}
else {
  Write-Output "Release $tag published."
}
Write-Output "Done. Check with: gh release view $tag --repo $releaseRepo"
