#requires -Version 7
<#
.SYNOPSIS
  AraView 릴리스 빌드와 NSIS 설치 프로그램을 만든다.
.DESCRIPTION
  앱과 PSD 썸네일 DLL을 릴리스로 빌드하고, 실행 파일, libheif 런타임 DLL, 썸네일 DLL,
  라이선스 자료를 스테이징 폴더에 모은 뒤 makensis로 설치 프로그램을 만든다.
  서명과 업로드는 하지 않는다.
.PARAMETER SkipBuild
  이미 빌드한 릴리스 산출물을 그대로 쓴다.
.PARAMETER OutFile
  설치 프로그램 경로. 기본은 target/AraView-<버전>-setup.exe.
.PARAMETER Suffix
  설치 정체성(설치 폴더, 제거 항목, 바로가기 이름)에 붙일 접미사. 실제 설치본과
  겹치지 않게 설치 흐름을 시험할 때만 쓴다(예: -Suffix " Test").
#>
param(
  [switch]$SkipBuild,
  [string]$OutFile,
  [string]$Suffix = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$app = Join-Path $root "src-gpui"
$release = Join-Path $root "target\release"
$stage = Join-Path $root "target\installer-stage"

$makensis = (Get-Command makensis -ErrorAction SilentlyContinue)?.Source
if (-not $makensis) {
  # PATH에 없으면 기본 설치 위치(GitHub 러너 포함)와, 예전 Tauri CLI가 내려받아 둔 NSIS를 찾는다.
  $candidates = @(
    (Join-Path ${env:ProgramFiles(x86)} "NSIS\makensis.exe"),
    (Join-Path $env:ProgramFiles "NSIS\makensis.exe"),
    (Join-Path $env:LOCALAPPDATA "tauri\NSIS\makensis.exe")
  )
  $makensis = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $makensis) { throw "makensis를 찾지 못했습니다. NSIS를 설치하세요." }

if (-not $SkipBuild) {
  Push-Location $root
  try {
    # 썸네일 DLL은 앱과 별개의 cdylib이라 따로 빌드한다.
    cargo build --release -p araview-gpui -p araview-thumb --no-default-features
    if ($LASTEXITCODE -ne 0) { throw "cargo build --release 실패 (exit $LASTEXITCODE)" }
  } finally { Pop-Location }
}

$exe = Join-Path $release "araview.exe"
if (-not (Test-Path $exe)) { throw "릴리스 실행 파일이 없습니다: $exe" }

$version = (Select-String -Path (Join-Path $app "Cargo.toml") -Pattern '^version = "(.+)"' |
  Select-Object -First 1).Matches.Groups[1].Value

if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage | Out-Null
Copy-Item $exe $stage

# libheif 런타임 DLL은 빌드 스크립트가 실행 파일 옆에 복사해 둔다. 없으면 앱이 시작되지 않는다.
foreach ($dll in "heif.dll", "libde265.dll", "aom.dll") {
  $source = Join-Path $release $dll
  if (-not (Test-Path $source)) { throw "런타임 DLL이 없습니다: $source (VCPKG_ROOT 확인)" }
  Copy-Item $source $stage
}

# PSD 탐색기 썸네일 DLL. 1.x 설치본의 등록 경로(resources\)를 그대로 쓰도록 같은 자리에 둔다.
$thumb = Join-Path $release "araview_thumb.dll"
if (-not (Test-Path $thumb)) { throw "썸네일 DLL이 없습니다: $thumb" }
$resources = New-Item -ItemType Directory -Path (Join-Path $stage "resources")
Copy-Item $thumb $resources

# 패키지별 라이선스 데이터는 실행 파일에 들어 있다. 여기서는 vcpkg 네이티브
# 라이브러리(libheif, libde265, aom)의 저작권 문서만 함께 넣는다(빌드 스크립트가 모은다).
$licenses = Join-Path $release "licenses"
$documents = @(Get-ChildItem $licenses -Filter "*-copyright.txt" -ErrorAction SilentlyContinue)
if ($documents.Count -gt 0) {
  $target = New-Item -ItemType Directory -Path (Join-Path $stage "licenses")
  $documents | Copy-Item -Destination $target
} else {
  Write-Warning "네이티브 라이브러리 저작권 문서 없음: $licenses (VCPKG_ROOT를 지정해 다시 빌드하세요)."
}

if (-not $OutFile) { $OutFile = Join-Path $root "target\AraView-$version-setup.exe" }
# 스크립트는 BOM 없는 UTF-8이라 입력 인코딩을 명시한다.
& $makensis /INPUTCHARSET UTF8 "/DVERSION=$version" "/DSTAGE=$stage" "/DOUTFILE=$OutFile" "/DNAME_SUFFIX=$Suffix" (Join-Path $app "installer\araview.nsi")
if ($LASTEXITCODE -ne 0) { throw "makensis 실패 (exit $LASTEXITCODE)" }
"설치 프로그램: $OutFile"
