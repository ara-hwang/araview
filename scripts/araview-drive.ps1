#requires -Version 7
<#
.SYNOPSIS
  실행 중인 AraView 개발 빌드를 셸에서 직접 조작한다.
.DESCRIPTION
  `scripts/AraViewControl.psm1`을 얇게 감싼 CLI다. MCP 클라이언트가 없는 환경에서도
  클릭, 키, 드래그, 휠, 캡처, 상태 읽기를 그대로 쓸 수 있다. 결과는 JSON 한 줄로 나온다.

  개발 빌드(디버그)만 제어 통로를 연다. 앱이 실행 중이 아니면 `launch`로 띄운다.

.EXAMPLE
  pwsh scripts/araview-drive.ps1 launch -Path samples/sample.jpg
.EXAMPLE
  pwsh scripts/araview-drive.ps1 state
.EXAMPLE
  pwsh scripts/araview-drive.ps1 key right
.EXAMPLE
  pwsh scripts/araview-drive.ps1 wheel 320 200 -3 -Modifiers ctrl -Logical
.EXAMPLE
  pwsh scripts/araview-drive.ps1 resize 780 700
.EXAMPLE
  pwsh scripts/araview-drive.ps1 shot -Out target/araview-shot.png
.EXAMPLE
  pwsh scripts/araview-drive.ps1 selftest
#>
[CmdletBinding()]
param(
  [Parameter(Position = 0)][string] $Command = 'help',
  [Parameter(Position = 1, ValueFromRemainingArguments = $true)][string[]] $Arguments = @(),
  [ValidateSet('left', 'right', 'middle')][string] $Button = 'left',
  [string[]] $Modifiers = @(),
  [switch] $Logical,
  [int] $Count = 1,
  [int] $Steps = 8,
  [int] $WaitMs = 0,
  [int] $StableMs = 300,
  [int] $TimeoutMs = 5000,
  [ValidateSet('dev', 'release', 'any')][string] $Identity = 'dev',
  [ValidateSet('auto', 'render', 'print', 'screen')][string] $Method = 'auto',
  [string] $Out,
  [string] $Path,
  [string] $Exe,
  [string] $Until
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
# 결과를 파일이나 다른 도구로 넘길 때도 한글이 깨지지 않게 UTF-8로 내보낸다.
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
Import-Module (Join-Path $PSScriptRoot 'AraViewControl.psm1') -Force

function Write-Json {
  param($Value)
  $Value | ConvertTo-Json -Compress -Depth 10
}

function Get-Number {
  param([string[]]$Values, [int]$Index, [string]$Name)
  if ($Values.Count -le $Index) { throw "$Name 값이 필요합니다." }
  $parsed = 0.0
  if (-not [double]::TryParse($Values[$Index], [ref]$parsed)) { throw "$Name 값이 숫자가 아닙니다: $($Values[$Index])" }
  return $parsed
}

function Show-Help {
  @'
AraView 제어 CLI

  launch [-Path <파일>] [-Exe <실행 파일>]   개발 빌드를 띄우고 제어 통로를 기다린다
  windows                                     실행 중인 AraView 창 목록
  state                                       현재 상태(열린 파일, 인덱스, 배율, 패널)
  activate                                    창을 앞으로 가져온다
  resize <너비> <높이>                        창 내용 크기를 논리 px로 바꾼다(한쪽만 주려면 -Width/-Height)
  shot [-Out <png>] [-Method auto|render|print|screen]
                                              창을 캡처한다(기본 저장: target/araview-shot.png)
  key <spec>                                  단축키 (예: right, enter, ctrl-shift-c)
  type <text>                                 글자 입력(포커스된 입력 상자)
  click <x> <y> [-Count n] [-Button left|right|middle]
  down <x> <y> / up <x> <y> / move <x> <y>
  drag <x> <y> <toX> <toY> [-Steps n]
  wheel <x> <y> <lines>                       음수는 아래로, 양수는 위로
  action <id>                                 뷰어 동작 (예: zoomIn, toggleExif, togglePalette)
  open <경로>                                 파일이나 폴더를 연다
  drop <경로> [<경로>...]                     끌어다 놓은 것처럼 연다
  wait [-Until 'name=sample.jpg,index=3'] [-StableMs 300] [-TimeoutMs 5000]
  selftest                                    실행 중인 앱으로 전체 기능을 확인한다
  mcp                                         MCP 서버(stdio)로 실행한다

좌표는 기본이 캡처 이미지 픽셀(물리 px)이고, -Logical을 주면 논리 px로 본다.
공통: -Modifiers ctrl,shift,alt / -Identity dev|release|any / -WaitMs <ms>
'@
}

switch ($Command.ToLowerInvariant()) {
  'help' { Show-Help; break }

  'windows' {
    Write-Json @(Get-AraViewWindow -Identity $Identity | Select-Object -Property * -ExcludeProperty Png)
    break
  }

  'state' {
    Write-Json (Get-AraViewState -Identity $Identity)
    break
  }

  'activate' {
    Write-Json (Enable-AraViewWindow -Identity $Identity)
    break
  }

  'resize' {
    $width = if ($Arguments.Count -ge 1) { Get-Number $Arguments 0 '너비' } else { $null }
    $height = if ($Arguments.Count -ge 2) { Get-Number $Arguments 1 '높이' } else { $null }
    Write-Json (Set-AraViewWindowSize -Width $width -Height $height -Identity $Identity -WaitMs ([math]::Max($WaitMs, 300)))
    break
  }

  'launch' {
    Write-Json (Start-AraViewApp -Path $Path -Exe $Exe -TimeoutMs ([math]::Max($TimeoutMs, 30000)))
    break
  }

  'shot' {
    if (-not $Out) { $Out = Join-Path (Split-Path $PSScriptRoot -Parent) 'target\araview-shot.png' }
    $shot = Get-AraViewShot -Identity $Identity -Method $Method -Out $Out
    Write-Json ($shot | Select-Object -Property Handle, Width, Height, ClientX, ClientY, Mean, Method, Restored, Blank, Bytes, Path)
    break
  }

  'key' {
    if ($Arguments.Count -lt 1) { throw "key <spec> 형식으로 쓰세요." }
    Write-Json (Send-AraViewKey -Spec $Arguments[0] -Identity $Identity -WaitMs $WaitMs)
    break
  }

  { $_ -in 'type', 'text' } {
    if ($Arguments.Count -lt 1) { throw "type <text> 형식으로 쓰세요." }
    Write-Json (Send-AraViewText -Value $Arguments[0] -Identity $Identity -WaitMs $WaitMs)
    break
  }

  { $_ -in 'click', 'down', 'up', 'move' } {
    $x = Get-Number $Arguments 0 'x'
    $y = Get-Number $Arguments 1 'y'
    $state = Send-AraViewMouse -Kind $Command.ToLowerInvariant() -X $x -Y $y -Button $Button -Count $Count `
      -Modifiers $Modifiers -Logical:$Logical -Identity $Identity -WaitMs $WaitMs
    Write-Json $state
    break
  }

  'drag' {
    $x = Get-Number $Arguments 0 'x'
    $y = Get-Number $Arguments 1 'y'
    $toX = Get-Number $Arguments 2 'toX'
    $toY = Get-Number $Arguments 3 'toY'
    $state = Send-AraViewMouse -Kind drag -X $x -Y $y -ToX $toX -ToY $toY -Steps $Steps -Button $Button `
      -Modifiers $Modifiers -Logical:$Logical -Identity $Identity -WaitMs $WaitMs
    Write-Json $state
    break
  }

  'wheel' {
    $x = Get-Number $Arguments 0 'x'
    $y = Get-Number $Arguments 1 'y'
    $lines = Get-Number $Arguments 2 'lines'
    $state = Send-AraViewMouse -Kind wheel -X $x -Y $y -Lines $lines -Modifiers $Modifiers `
      -Logical:$Logical -Identity $Identity -WaitMs $WaitMs
    Write-Json $state
    break
  }

  'action' {
    if ($Arguments.Count -lt 1) { throw "action <id> 형식으로 쓰세요." }
    Write-Json (Send-AraViewAction -Id $Arguments[0] -Identity $Identity -WaitMs $WaitMs)
    break
  }

  'open' {
    if ($Arguments.Count -lt 1 -and -not $Path) { throw "open <경로> 형식으로 쓰세요." }
    $target = if ($Path) { $Path } else { $Arguments[0] }
    Write-Json (Open-AraViewPath -Path $target -Identity $Identity -WaitMs ([math]::Max($WaitMs, 400)))
    break
  }

  'drop' {
    if ($Arguments.Count -lt 1) { throw "drop <경로> [<경로>...] 형식으로 쓰세요." }
    Write-Json (Send-AraViewDrop -Paths $Arguments -Logical:$Logical -Identity $Identity -WaitMs ([math]::Max($WaitMs, 400)))
    break
  }

  'wait' {
    $untilTable = $null
    if ($Until) {
      $untilTable = @{}
      foreach ($pair in $Until -split ',') {
        $parts = $pair -split '=', 2
        if ($parts.Count -ne 2) { throw "-Until 형식은 'key=value,key=value'입니다: $pair" }
        $untilTable[$parts[0].Trim()] = $parts[1].Trim()
      }
    }
    $result = Wait-AraView -Identity $Identity -Until $untilTable -StableMs $StableMs -TimeoutMs $TimeoutMs
    Write-Json ($result | Select-Object -Property Stable, Timeout, State, @{ Name = 'Shot'; Expression = {
          if ($_.Shot) { $_.Shot | Select-Object -Property Method, Width, Height, Mean, Bytes } else { $null } } })
    if ($result.Timeout) { exit 2 }
    break
  }

  'selftest' {
    $results = Test-AraViewControl -Sample $Path -TimeoutMs ([math]::Max($TimeoutMs, 30000))
    $failed = 0
    foreach ($result in $results) {
      $mark = if ($result.Passed) { 'PASS' } else { 'FAIL'; $failed++ }
      "[{0}] {1}: {2}" -f $mark, $result.Name, $result.Detail
    }
    if ($failed -gt 0) {
      "실패 $failed 건"
      exit 1
    }
    '전부 통과'
    break
  }

  'mcp' {
    & (Join-Path $PSScriptRoot 'araview-mcp.ps1') -Identity $Identity
    break
  }

  default { throw "알 수 없는 명령입니다: $Command (help를 보세요)" }
}
