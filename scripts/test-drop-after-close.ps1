#requires -Version 7
<#
.SYNOPSIS
  그리드/설정을 닫은 직후 개발 브리지의 드롭이 이전 히트테스트에 막히지 않는지 확인한다.
.DESCRIPTION
  Windows에서 실행 중인 개발 빌드에 연결한다. 사용자 입력 장치는 건드리지 않는다.
  임시 PNG 두 개로 검사하고 원래 파일로 돌아온다. 설정은 바꾸지 않는다.
  보이는 창에서 반복한 뒤 창을 잠시 최소화하여 이전 프레임을 유지한 채 닫기와 드롭을
  보낸다. 복원 후 드롭이 완료되어야 한다. 캡처는 창을 복원하므로 검사 중 호출하지 않는다.
.EXAMPLE
  pwsh scripts/test-drop-after-close.ps1
#>
[CmdletBinding()]
param(
  [ValidateRange(1, 100)][int]$Iterations = 3
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $IsWindows) { throw '이 회귀 검사는 Windows 개발 빌드가 필요합니다.' }
Import-Module (Join-Path $PSScriptRoot 'AraViewControl.psm1') -Force

if (-not ('AraViewDropRegression.Win32' -as [type])) {
  Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace AraViewDropRegression {
    public static class Win32 {
        [DllImport("user32.dll")]
        public static extern bool ShowWindowAsync(IntPtr window, int command);
        [DllImport("user32.dll")]
        public static extern bool IsIconic(IntPtr window);
    }
}
'@
}

function Invoke-CheckedRequest([hashtable]$Request) {
  $reply = Invoke-AraViewRequest -Request $Request -Identity dev
  if (-not $reply.ok) { throw "$($Request.cmd) failed: $($reply.error)" }
  return $reply.state
}

function Assert-Path($State, [string]$Expected, [string]$Step) {
  if ($State.path -ne $Expected) {
    throw "$Step expected path=$Expected, actual=$($State.path) error=$($State.error)"
  }
}

function Set-Minimized([bool]$Minimized) {
  # SW_SHOWMINNOACTIVE / SW_SHOWNOACTIVATE: 다른 창의 포커스나 실제 커서를 바꾸지 않는다.
  $command = if ($Minimized) { 7 } else { 4 }
  $null = [AraViewDropRegression.Win32]::ShowWindowAsync($handle, $command)
  $deadline = [DateTime]::UtcNow.AddSeconds(3)
  while ([AraViewDropRegression.Win32]::IsIconic($handle) -ne $Minimized) {
    if ([DateTime]::UtcNow -ge $deadline) { throw "Window minimized=$Minimized timed out" }
    Start-Sleep -Milliseconds 20
  }
}

$initial = Get-AraViewState
$target = Get-AraViewTarget -Identity dev
$handle = [IntPtr]$target.Handle
if ($initial.dialogOpen -or $initial.gridOpen -or $initial.fullscreen -or
    [AraViewDropRegression.Win32]::IsIconic($handle)) {
  throw '다이얼로그와 그리드를 닫고 최소화/전체화면이 아닌 개발 창에서 실행하세요.'
}

$fixture = Join-Path ([IO.Path]::GetTempPath()) "araview-drop-$([Guid]::NewGuid().ToString('N'))"
$null = New-Item -ItemType Directory -Path $fixture
$before = Join-Path $fixture 'before.png'
$after = Join-Path $fixture 'after.png'
# 독립적인 1x1 PNG. 저장소에서 무시되는 samples/ 자산에 의존하지 않는다.
$png = [Convert]::FromBase64String('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==')
[IO.File]::WriteAllBytes($before, $png)
[IO.File]::WriteAllBytes($after, $png)

try {
  foreach ($minimized in @($false, $true)) {
    $mode = if ($minimized) { 'minimized/restored' } else { 'visible' }
    foreach ($overlay in @('grid', 'settings')) {
      for ($iteration = 1; $iteration -le $Iterations; $iteration++) {
        $null = Open-AraViewPath -Path $before -WaitMs 400
        $ready = Wait-AraView -Until @{ name = 'before.png'; opening = $false } -TimeoutMs 5000
        if ($ready.Timeout) { throw 'The baseline image did not finish opening' }
        Assert-Path $ready.State $before 'baseline image'
        $action = if ($overlay -eq 'grid') { 'toggleGrid' } else { 'openSettings' }
        $opened = Send-AraViewAction -Id $action -WaitMs 300
        $isOpen = if ($overlay -eq 'grid') { $opened.gridOpen } else { $opened.dialogOpen }
        if (-not $isOpen) { throw "$mode/$overlay did not open" }

        if ($overlay -eq 'settings') {
          # 실제로 보이는 모달은 드롭을 계속 막아야 한다. 현재 파일과 모달을 모두 확인한다.
          $blocked = Invoke-CheckedRequest @{
            cmd = 'drop'; paths = @($after); x = 200; y = 200; space = 'logical'; waitMs = 300
          }
          Assert-Path $blocked $before "$mode/visible modal blocks drop"
          if (-not $blocked.dialogOpen) { throw 'A drop unexpectedly closed the settings modal' }
          Write-Output "PASS $mode/visible modal blocks drop/$iteration"
        }

        if ($minimized) { Set-Minimized $true }
        # 이 두 요청 사이에는 상태 조회, 캡처, 좌표 변환, 대기를 넣지 않는다.
        # 최소화 상태에서는 이전 overlay 프레임이 남아 있어 stale hit-test 경로를 재현한다.
        $closed = Send-AraViewKey -Spec escape -WaitMs 0
        $dropped = Invoke-CheckedRequest @{
          cmd = 'drop'; paths = @($after); x = 200; y = 200; space = 'logical'; waitMs = 0
        }
        if ($closed.gridOpen -or $closed.dialogOpen) { throw "$mode/$overlay did not close" }
        if ($minimized) {
          $deferred = $dropped.path -eq $before
          Write-Output "INFO $mode/$overlay deferred before restore=$deferred path=$($dropped.path)"
          Set-Minimized $false
        }
        # 제출은 다음 프레임에서 처리된다. 닫기와 드롭을 이미 보낸 뒤에만 기다린다.
        $done = Wait-AraView -Until @{ name = 'after.png'; opening = $false } -TimeoutMs 5000
        if ($done.Timeout) { throw "$mode/$overlay immediate drop/$iteration timed out" }
        Assert-Path $done.State $after "$mode/$overlay immediate drop/$iteration"
        Write-Output "PASS $mode/$overlay immediate drop/$iteration"
      }
    }
  }
} finally {
  if ([AraViewDropRegression.Win32]::IsIconic($handle)) { Set-Minimized $false }
  $state = Get-AraViewState
  if ($state.dialogOpen -or $state.gridOpen) { $null = Send-AraViewKey -Spec escape -WaitMs 200 }
  if ($initial.path) {
    $null = Open-AraViewPath -Path $initial.path -WaitMs 400
  } else {
    $null = Send-AraViewAction -Id closeImage -WaitMs 200
  }
  Remove-Item -LiteralPath $fixture -Recurse -Force
}
