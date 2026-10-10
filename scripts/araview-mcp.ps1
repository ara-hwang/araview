#requires -Version 7
<#
.SYNOPSIS
  AraView 제어 도구를 MCP 서버(stdio)로 노출한다.
.DESCRIPTION
  MCP 클라이언트가 실행 중인 AraView 개발 빌드를 직접 클릭, 키, 드래그, 휠로 조작하고
  화면과 상태를 확인할 수 있게 한다. 메시지는 표준 MCP 규약대로 줄 단위 JSON-RPC 2.0이다.

  등록 예(.mcp.json):
    { "mcpServers": { "araview": { "command": "pwsh", "args": [
        "-NoProfile", "-File", "scripts/araview-mcp.ps1" ] } } }

  개발 빌드만 제어 통로를 연다. 앱이 없으면 `araview_launch`로 띄운다.
.PARAMETER Identity
  제어할 빌드. dev(기본)만 조작할 수 있고 release는 캡처만 된다.
#>
[CmdletBinding()]
param([ValidateSet('dev', 'release')][string] $Identity = 'dev')

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
# 표준 출력은 JSON-RPC 전용이다. 다른 스트림이 섞이지 않게 막는다.
$WarningPreference = 'SilentlyContinue'
$InformationPreference = 'SilentlyContinue'
$VerbosePreference = 'SilentlyContinue'
$ProgressPreference = 'SilentlyContinue'

Import-Module (Join-Path $PSScriptRoot 'AraViewControl.psm1') -Force

# MCP의 stdio는 UTF-8 고정이다. 콘솔 기본 인코딩(한국어 Windows는 CP949)을 쓰면
# 한글 설명이 섞여 클라이언트가 메시지를 파싱하지 못한다.
$script:In = [System.IO.StreamReader]::new([Console]::OpenStandardInput(), [System.Text.UTF8Encoding]::new($false))
$script:Out = [System.IO.StreamWriter]::new([Console]::OpenStandardOutput(), [System.Text.UTF8Encoding]::new($false))
$script:Out.AutoFlush = $true
$script:Err = [System.IO.StreamWriter]::new([Console]::OpenStandardError(), [System.Text.UTF8Encoding]::new($false))
$script:Err.AutoFlush = $true

$script:Identity = $Identity
$script:ServerName = 'araview'
$script:ServerVersion = '1.0.0'
$script:SupportedProtocols = @('2025-06-18', '2025-03-26', '2024-11-05')
$script:LatestProtocol = '2025-06-18'

# ----- JSON-RPC -----

function Write-Message {
  param([hashtable]$Message)
  $script:Out.WriteLine(($Message | ConvertTo-Json -Compress -Depth 20))
}

function Send-Result {
  param($Id, $Result)
  Write-Message @{ jsonrpc = '2.0'; id = $Id; result = $Result }
}

function Send-Error {
  param($Id, [int]$Code, [string]$Message)
  Write-Message @{ jsonrpc = '2.0'; id = $Id; error = @{ code = $Code; message = $Message } }
}

function New-TextContent {
  param($Value)
  # 파이프로 넘기면 빈 배열이 $null이 돼 text가 null로 나간다. -InputObject는 빈 배열도 '[]'로 만든다.
  $text = if ($Value -is [string]) { $Value } else { ConvertTo-Json -InputObject $Value -Compress -Depth 10 }
  @{ type = 'text'; text = $text }
}

# ----- 도구 -----

function New-Tool {
  param([string]$Name, [string]$Description, [hashtable]$Properties, [string[]]$Required = @())
  [pscustomobject]@{
    name        = $Name
    description = $Description
    inputSchema = @{
      type       = 'object'
      properties = $Properties
      required   = $Required
    }
  }
}

# StrictMode에서도 없는 속성을 안전하게 읽는다.
function Get-Property {
  param($Object, [string]$Name, $Default = $null)
  if ($null -eq $Object) { return $Default }
  $property = $Object.PSObject.Properties[$Name]
  if ($null -eq $property -or $null -eq $property.Value) { return $Default }
  return $property.Value
}

function Get-Field {
  param($Arguments, [string]$Name, $Default)
  Get-Property -Object $Arguments -Name $Name -Default $Default
}

function Test-Logical {
  param($Arguments)
  return ((Get-Field $Arguments 'space' 'image') -eq 'logical')
}

$script:Tools = @(
  (New-Tool -Name 'araview_windows' -Description '실행 중인 AraView 창 목록(hwnd, 프로세스, 제목, 크기, 배율)을 돌려준다. 조작 대상이 있는지 먼저 확인할 때 쓴다.' -Properties @{}),
  (New-Tool -Name 'araview_launch' -Description '개발 빌드를 실행하고 제어 통로가 열릴 때까지 기다린다. 이미 실행 중이면 그 창에 path를 넘긴다(단일 인스턴스).' -Properties @{
      path      = @{ type = 'string'; description = '처음 열 파일 경로(선택)' }
      exe       = @{ type = 'string'; description = '실행 파일 경로(기본: target/debug/araview.exe)' }
      timeoutMs = @{ type = 'integer'; description = '기다릴 최대 시간(ms, 기본 30000)' }
    }),
  (New-Tool -Name 'araview_state' -Description '창의 상태를 돌려준다: kind(home/image/archive), name, path, index, count, viewMode, zoom, position, viewport, scaleFactor, infoOpen, gridOpen, dialogOpen, fullscreen, playing, opening, error.' -Properties @{}),
  (New-Tool -Name 'araview_screenshot' -Description '창을 캡처해 이미지로 돌려준다. 저장 경로를 주면 PNG로도 남긴다. 좌표가 필요하면 clientX/clientY(캡처 이미지 안 클라이언트 영역 왼쪽 위)를 참고한다.' -Properties @{
      savePath = @{ type = 'string'; description = 'PNG로 저장할 경로(선택)' }
      method   = @{ type = 'string'; enum = @('auto', 'render', 'print', 'screen'); description = '캡처 방법(기본 auto)' }
    }),
  (New-Tool -Name 'araview_open' -Description '파일이나 폴더를 실행 중인 창에서 연다.' -Properties @{
      path   = @{ type = 'string'; description = '열 경로' }
      waitMs = @{ type = 'integer'; description = '결과를 읽기 전 대기(ms, 기본 400)' }
    } -Required @('path')),
  (New-Tool -Name 'araview_drop' -Description '파일을 창에 끌어다 놓은 것처럼 연다(드롭으로 열기 경로 확인).' -Properties @{
      paths  = @{ type = 'array'; items = @{ type = 'string' }; description = '놓을 파일 경로' }
      x      = @{ type = 'number'; description = '놓을 x 좌표(기본 200)' }
      y      = @{ type = 'number'; description = '놓을 y 좌표(기본 200)' }
      space  = @{ type = 'string'; enum = @('image', 'logical'); description = '좌표계(기본 image)' }
      waitMs = @{ type = 'integer'; description = '결과를 읽기 전 대기(ms, 기본 400)' }
    } -Required @('paths')),
  (New-Tool -Name 'araview_action' -Description '뷰어 동작을 실행한다. id는 단축키 동작 ID다(zoomIn, zoomOut, resetView, navigateNext, jumpFirst, toggleExif, toggleGrid, togglePalette, openSettings, toggleFullscreen, viewSingle, viewWebtoon 등).' -Properties @{
      id     = @{ type = 'string'; description = '동작 ID' }
      waitMs = @{ type = 'integer'; description = '결과를 읽기 전 대기(ms)' }
    } -Required @('id')),
  (New-Tool -Name 'araview_key' -Description '단축키 하나를 창에 보낸다. 수식키 조합도 그대로 실린다. 예: right, enter, escape, pageDown, =, ctrl-shift-c, ctrl-left.' -Properties @{
      spec   = @{ type = 'string'; description = '키 표기(예: ctrl-shift-c)' }
      waitMs = @{ type = 'integer'; description = '결과를 읽기 전 대기(ms)' }
    } -Required @('spec')),
  (New-Tool -Name 'araview_type' -Description '글자를 입력한다. 포커스가 있는 입력 상자(명령 팔레트, 파일 이름 입력 등)로 들어간다.' -Properties @{
      value  = @{ type = 'string'; description = '입력할 글자' }
      waitMs = @{ type = 'integer'; description = '결과를 읽기 전 대기(ms)' }
    } -Required @('value')),
  (New-Tool -Name 'araview_click' -Description '마우스를 클릭한다. count를 2로 주면 더블클릭이다(뷰어 캡션은 전체화면 전환).' -Properties @{
      x         = @{ type = 'number'; description = 'x 좌표' }
      y         = @{ type = 'number'; description = 'y 좌표' }
      button    = @{ type = 'string'; enum = @('left', 'right', 'middle'); description = '버튼(기본 left)' }
      count     = @{ type = 'integer'; description = '클릭 횟수(기본 1)' }
      modifiers = @{ type = 'array'; items = @{ type = 'string' }; description = 'ctrl, shift, alt, win' }
      space     = @{ type = 'string'; enum = @('image', 'logical'); description = '좌표계(기본 image = 캡처 픽셀)' }
      waitMs    = @{ type = 'integer'; description = '결과를 읽기 전 대기(ms)' }
    } -Required @('x', 'y')),
  (New-Tool -Name 'araview_drag' -Description '누른 채 끌어 놓는다. 확대한 그림을 팬할 때 쓴다.' -Properties @{
      x         = @{ type = 'number'; description = '시작 x' }
      y         = @{ type = 'number'; description = '시작 y' }
      toX       = @{ type = 'number'; description = '끝 x' }
      toY       = @{ type = 'number'; description = '끝 y' }
      steps     = @{ type = 'integer'; description = '중간 이동 횟수(기본 8)' }
      button    = @{ type = 'string'; enum = @('left', 'right', 'middle'); description = '버튼(기본 left)' }
      modifiers = @{ type = 'array'; items = @{ type = 'string' }; description = 'ctrl, shift, alt, win' }
      space     = @{ type = 'string'; enum = @('image', 'logical'); description = '좌표계(기본 image)' }
      waitMs    = @{ type = 'integer'; description = '결과를 읽기 전 대기(ms)' }
    } -Required @('x', 'y', 'toX', 'toY')),
  (New-Tool -Name 'araview_move' -Description '마우스를 좌표로 옮긴다(호버 상태 확인).' -Properties @{
      x      = @{ type = 'number'; description = 'x 좌표' }
      y      = @{ type = 'number'; description = 'y 좌표' }
      space  = @{ type = 'string'; enum = @('image', 'logical'); description = '좌표계(기본 image)' }
      waitMs = @{ type = 'integer'; description = '결과를 읽기 전 대기(ms)' }
    } -Required @('x', 'y')),
  (New-Tool -Name 'araview_wheel' -Description '휠을 돌린다. lines가 음수면 다음 화면(아래), 양수면 이전 화면이다. modifiers에 ctrl을 주면 커서 기준 확대/축소가 된다.' -Properties @{
      x         = @{ type = 'number'; description = 'x 좌표' }
      y         = @{ type = 'number'; description = 'y 좌표' }
      lines     = @{ type = 'number'; description = '휠 줄 수(기본 1)' }
      delta     = @{ type = 'number'; description = '픽셀 단위 스크롤(선택)' }
      modifiers = @{ type = 'array'; items = @{ type = 'string' }; description = 'ctrl, shift, alt, win' }
      space     = @{ type = 'string'; enum = @('image', 'logical'); description = '좌표계(기본 image)' }
      waitMs    = @{ type = 'integer'; description = '결과를 읽기 전 대기(ms)' }
    } -Required @('x', 'y')),
  (New-Tool -Name 'araview_wait' -Description '창이 멈추거나 상태가 조건에 맞을 때까지 기다린다. until을 주면 상태 필드가 모두 맞을 때까지, 없으면 캡처가 그대로일 때까지 기다린다.' -Properties @{
      until     = @{ type = 'object'; description = '기대하는 상태 필드. 예: {"name":"sample.jpg","index":3}' }
      stableMs  = @{ type = 'integer'; description = '같은 화면이 유지되어야 하는 시간(ms, 기본 300)' }
      timeoutMs = @{ type = 'integer'; description = '기다릴 최대 시간(ms, 기본 5000)' }
    }),
  (New-Tool -Name 'araview_selftest' -Description '실행 중인 개발 빌드로 창 찾기, 상태, 캡처, 키, 수식키 휠, 드래그, 더블클릭, 글자 입력, 동작, 파일 열기를 차례로 확인한다.' -Properties @{})
)

function Invoke-AraViewTool {
  param([string]$Name, $Arguments)

  switch ($Name) {
    'araview_windows' {
      $windows = @(Get-AraViewWindow -Identity 'any' | Select-Object -Property * -ExcludeProperty Png)
      return @(New-TextContent $windows)
    }
    'araview_launch' {
      $result = Start-AraViewApp -Path (Get-Field $Arguments 'path' $null) -Exe (Get-Field $Arguments 'exe' $null) `
        -TimeoutMs ([int](Get-Field $Arguments 'timeoutMs' 30000))
      return @(New-TextContent $result)
    }
    'araview_state' {
      return @(New-TextContent (Get-AraViewState -Identity $script:Identity))
    }
    'araview_screenshot' {
      $shot = Get-AraViewShot -Identity $script:Identity -Method (Get-Field $Arguments 'method' 'auto') `
        -Out (Get-Field $Arguments 'savePath' $null)
      $summary = $shot | Select-Object -Property Width, Height, ClientX, ClientY, Mean, Method, Restored, Blank, Bytes, Path
      return @(
        @{ type = 'image'; data = [Convert]::ToBase64String($shot.Png); mimeType = 'image/png' },
        (New-TextContent $summary)
      )
    }
    'araview_open' {
      $state = Open-AraViewPath -Path (Get-Field $Arguments 'path' $null) -Identity $script:Identity `
        -WaitMs ([int](Get-Field $Arguments 'waitMs' 400))
      return @(New-TextContent $state)
    }
    'araview_drop' {
      $state = Send-AraViewDrop -Paths ([string[]](Get-Field $Arguments 'paths' @())) `
        -X ([double](Get-Field $Arguments 'x' 200)) -Y ([double](Get-Field $Arguments 'y' 200)) `
        -Logical:(Test-Logical $Arguments) -Identity $script:Identity `
        -WaitMs ([int](Get-Field $Arguments 'waitMs' 400))
      return @(New-TextContent $state)
    }
    'araview_action' {
      $state = Send-AraViewAction -Id (Get-Field $Arguments 'id' $null) -Identity $script:Identity `
        -WaitMs ([int](Get-Field $Arguments 'waitMs' 0))
      return @(New-TextContent $state)
    }
    'araview_key' {
      $state = Send-AraViewKey -Spec (Get-Field $Arguments 'spec' $null) -Identity $script:Identity `
        -WaitMs ([int](Get-Field $Arguments 'waitMs' 0))
      return @(New-TextContent $state)
    }
    'araview_type' {
      $state = Send-AraViewText -Value (Get-Field $Arguments 'value' $null) -Identity $script:Identity `
        -WaitMs ([int](Get-Field $Arguments 'waitMs' 0))
      return @(New-TextContent $state)
    }
    'araview_click' {
      $state = Send-AraViewMouse -Kind click -X ([double](Get-Field $Arguments 'x' 0)) -Y ([double](Get-Field $Arguments 'y' 0)) `
        -Button (Get-Field $Arguments 'button' 'left') -Count ([int](Get-Field $Arguments 'count' 1)) `
        -Modifiers ([string[]](Get-Field $Arguments 'modifiers' @())) -Logical:(Test-Logical $Arguments) `
        -Identity $script:Identity -WaitMs ([int](Get-Field $Arguments 'waitMs' 0))
      return @(New-TextContent $state)
    }
    'araview_drag' {
      $state = Send-AraViewMouse -Kind drag -X ([double](Get-Field $Arguments 'x' 0)) -Y ([double](Get-Field $Arguments 'y' 0)) `
        -ToX ([double](Get-Field $Arguments 'toX' 0)) -ToY ([double](Get-Field $Arguments 'toY' 0)) `
        -Steps ([int](Get-Field $Arguments 'steps' 8)) -Button (Get-Field $Arguments 'button' 'left') `
        -Modifiers ([string[]](Get-Field $Arguments 'modifiers' @())) -Logical:(Test-Logical $Arguments) `
        -Identity $script:Identity -WaitMs ([int](Get-Field $Arguments 'waitMs' 0))
      return @(New-TextContent $state)
    }
    'araview_move' {
      $state = Send-AraViewMouse -Kind move -X ([double](Get-Field $Arguments 'x' 0)) -Y ([double](Get-Field $Arguments 'y' 0)) `
        -Logical:(Test-Logical $Arguments) -Identity $script:Identity -WaitMs ([int](Get-Field $Arguments 'waitMs' 0))
      return @(New-TextContent $state)
    }
    'araview_wheel' {
      $delta = Get-Field $Arguments 'delta' $null
      $lines = Get-Field $Arguments 'lines' 1
      $mouse = @{
        Kind      = 'wheel'
        X         = [double](Get-Field $Arguments 'x' 0)
        Y         = [double](Get-Field $Arguments 'y' 0)
        Modifiers = [string[]](Get-Field $Arguments 'modifiers' @())
        Logical   = (Test-Logical $Arguments)
        Identity  = $script:Identity
        WaitMs    = [int](Get-Field $Arguments 'waitMs' 0)
      }
      if ($null -ne $delta) { $mouse.Delta = [double]$delta } else { $mouse.Lines = [double]$lines }
      $state = Send-AraViewMouse @mouse
      return @(New-TextContent $state)
    }
    'araview_wait' {
      $untilValue = Get-Field $Arguments 'until' $null
      $until = $null
      if ($null -ne $untilValue) {
        $until = @{}
        foreach ($property in $untilValue.PSObject.Properties) { $until[$property.Name] = $property.Value }
      }
      if ($null -eq $until -or $until.Count -eq 0) { $until = $null }
      $result = Wait-AraView -Identity $script:Identity -Until $until `
        -StableMs ([int](Get-Field $Arguments 'stableMs' 300)) -TimeoutMs ([int](Get-Field $Arguments 'timeoutMs' 5000))
      $summary = $result | Select-Object -Property Stable, Timeout, State, @{ Name = 'Shot'; Expression = {
          if ($_.Shot) { $_.Shot | Select-Object -Property Method, Width, Height, Mean, Bytes } else { $null } } }
      return @(New-TextContent $summary)
    }
    'araview_selftest' {
      $results = Test-AraViewControl
      $lines = $results | ForEach-Object { "[{0}] {1}: {2}" -f $(if ($_.Passed) { 'PASS' } else { 'FAIL' }), $_.Name, $_.Detail }
      return @(New-TextContent ($lines -join "`n"))
    }
    default { throw "알 수 없는 도구입니다: $Name" }
  }
}

# ----- 루프 -----

while ($true) {
  $line = $script:In.ReadLine()
  if ($null -eq $line) { break }
  if ([string]::IsNullOrWhiteSpace($line)) { continue }

  $request = $null
  try { $request = $line | ConvertFrom-Json } catch {
    Send-Error $null -32700 "JSON을 읽지 못했습니다: $($_.Exception.Message)"
    continue
  }

  $isNotification = -not ($request.PSObject.Properties.Name -contains 'id')
  $id = Get-Property -Object $request -Name 'id'
  $method = Get-Property -Object $request -Name 'method' ''
  $params = Get-Property -Object $request -Name 'params'

  try {
    switch ($method) {
      'initialize' {
        $requested = Get-Property -Object $params -Name 'protocolVersion' ''
        $protocol = if ($script:SupportedProtocols -contains $requested) { $requested } else { $script:LatestProtocol }
        Send-Result $id @{
          protocolVersion = $protocol
          capabilities    = @{ tools = @{ listChanged = $false } }
          serverInfo      = @{ name = $script:ServerName; version = $script:ServerVersion }
          instructions    = '실행 중인 AraView 개발 빌드를 조작한다. 먼저 araview_windows로 대상을 확인하고, 없으면 araview_launch로 띄운다. 좌표는 기본이 캡처 이미지 픽셀이고 araview_screenshot 결과와 같은 좌표계다. 응답의 state는 명령 직후 값이라 디코드 같은 배경 작업이 끝나지 않았을 수 있다: waitMs를 주거나 araview_wait로 기다린 뒤 다시 확인한다.'
        }
      }
      'ping' { Send-Result $id @{} }
      'tools/list' { Send-Result $id @{ tools = $script:Tools } }
      'tools/call' {
        $name = Get-Property -Object $params -Name 'name'
        $arguments = Get-Property -Object $params -Name 'arguments'
        try {
          $content = Invoke-AraViewTool -Name $name -Arguments $arguments
          Send-Result $id @{ content = @($content); isError = $false }
        } catch {
          # 클라이언트에는 짧은 이유만, 자세한 위치는 로그(stderr)에 남긴다.
          $script:Err.WriteLine("[araview-mcp] $name 실패: $($_.Exception.Message)`n$($_.ScriptStackTrace)")
          Send-Result $id @{ content = @(New-TextContent $_.Exception.Message); isError = $true }
        }
      }
      { $_ -in 'notifications/initialized', 'notifications/cancelled', 'notifications/progress' } {
        # 알림에는 응답하지 않는다.
      }
      default {
        if (-not $isNotification) { Send-Error $id -32601 "지원하지 않는 메서드입니다: $method" }
      }
    }
  } catch {
    if (-not $isNotification) { Send-Error $id -32603 $_.Exception.Message }
    $script:Err.WriteLine("[araview-mcp] $($_.Exception.Message)")
  }
}
