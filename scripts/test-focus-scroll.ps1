#requires -Version 7
<#
.SYNOPSIS
  개발 빌드의 설정 6개 탭에서 Tab·Shift+Tab 자동 스크롤을 검증한다.
.DESCRIPTION
  agent-desktop의 접근성 관찰과 개발 제어 통로만 사용한다.
  물리 입력·포커스 활성화·설정 변경은 하지 않는다. 실행 전 다이얼로그를 닫아 둔다.
#>
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot 'AraViewControl.psm1') -Force

function Read-Desktop {
  param([string[]]$Arguments)
  $raw = & agent-desktop @Arguments
  if ($LASTEXITCODE -ne 0) { throw "agent-desktop 실패: $raw" }
  $result = $raw | ConvertFrom-Json
  if (-not $result.ok) { throw "agent-desktop 실패: $raw" }
  return $result.data
}

function Get-Nodes($Node) {
  $Node
  if ($Node.PSObject.Properties['children']) {
    foreach ($child in $Node.children) { Get-Nodes $child }
  }
}

function Read-Settings {
  $snapshot = Read-Desktop -Arguments @(
    'snapshot', '--app', 'araview', '--window-id', $script:windowId,
    '-i', '--compact', '--include-bounds'
  )
  $dialogs = @(Get-Nodes $snapshot.tree | Where-Object {
    $_.PSObject.Properties['subrole'] -and $_.subrole -eq 'dialog'
  })
  if ($dialogs.Count -ne 1) { throw '설정 다이얼로그 하나가 필요합니다.' }
  $nodes = @(Get-Nodes $dialogs[0])
  $controls = @($nodes | Where-Object {
    $_.role -in @('button', 'switch', 'combobox') -and
    $_.PSObject.Properties['available_actions'] -and
    'SetFocus' -in $_.available_actions -and
    (-not $_.PSObject.Properties['states'] -or 'disabled' -notin $_.states)
  })
  # 설정 본문 뒤에는 초기화, 확인, 창 닫기 순으로 고정 컨트롤이 있다.
  if ($controls.Count -lt 4) { throw '설정 컨트롤을 읽지 못했습니다.' }
  return @{
    tabs = @($nodes | Where-Object role -eq 'tab')
    content = @($controls | Select-Object -SkipLast 3)
    footer = $controls[-3].bounds
    focusObserved = @(Get-Nodes $snapshot.tree | Where-Object {
      $_.PSObject.Properties['states'] -and 'focused' -in $_.states
    }).Count -gt 0
  }
}

function Assert-InBody($Control, $Settings, [string]$Step) {
  # 비활성 창에서는 OS가 focused 상태를 생략할 수 있다.
  if ($Settings.focusObserved -and
      (-not $Control.PSObject.Properties['states'] -or 'focused' -notin $Control.states)) {
    throw "FAIL ${Step}: 예상한 본문 컨트롤에 포커스가 없습니다."
  }
  $bounds = $Control.bounds
  $tabsBottom = ($Settings.tabs | ForEach-Object { $_.bounds.y + $_.bounds.height } |
    Measure-Object -Maximum).Maximum
  if ($bounds.y -lt $tabsBottom -or
      $bounds.y + $bounds.height -gt $Settings.footer.y) {
    throw ("FAIL {0}: 대상 {1}..{2}, 본문 {3}..{4}" -f
      $Step, $bounds.y, ($bounds.y + $bounds.height), $tabsBottom, $Settings.footer.y)
  }
}

$initial = Get-AraViewState
if ($initial.dialogOpen) { throw '먼저 개발 빌드의 다이얼로그를 닫아 주세요.' }
$target = @(Get-AraViewWindow -Identity dev)[0]
$script:windowId = "w-$($target.Handle)"
try {
  foreach ($tab in 0..5) {
    $null = Send-AraViewAction -Id openSettings -WaitMs 200
    $settings = Read-Settings
    if ($tab -ne 0) {
      $null = Read-Desktop -Arguments @('click', $settings.tabs[$tab].ref_id)
      $null = Invoke-AraViewRequest -Request @{ cmd = 'wait'; waitMs = 300 }
      $settings = Read-Settings
    }
    $name = $settings.tabs[$tab].name
    $count = $settings.content.Count
    $firstBefore = $settings.content[0].bounds
    $null = Send-AraViewKey -Spec tab -WaitMs 30
    $firstFocused = Read-Settings
    Assert-InBody $firstFocused.content[0] $firstFocused "$name/첫 Tab"
    if ([math]::Abs($firstBefore.y - $firstFocused.content[0].bounds.y) -gt 2) {
      throw "FAIL $name/Tab: 이미 보이는 첫 항목에서 스크롤했습니다."
    }
    for ($i = 1; $i -lt $count; $i++) {
      $null = Send-AraViewKey -Spec tab -WaitMs 30
    }
    $null = Invoke-AraViewRequest -Request @{ cmd = 'wait'; waitMs = 100 }
    $settings = Read-Settings
    Assert-InBody $settings.content[-1] $settings "$name/Tab"
    Write-Output "PASS $name/Tab ($count controls)"

    # 포커스가 그대로면 휠로 옮긴 위치를 강제로 되돌리지 않는다.
    $window = @(Get-AraViewWindow -Identity dev)[0]
    $bounds = $settings.content[-1].bounds
    $x = ($bounds.x + $bounds.width / 2 - $window.Left - $window.ClientX) / $window.Scale
    $y = ($bounds.y + $bounds.height / 2 - $window.Top - $window.ClientY) / $window.Scale
    $null = Send-AraViewMouse -Kind wheel -X $x -Y $y -Logical -Lines 3 -WaitMs 500
    $afterWheel = Read-Settings
    $null = Invoke-AraViewRequest -Request @{ cmd = 'wait'; waitMs = 300 }
    $settled = Read-Settings
    if ([math]::Abs($afterWheel.content[0].bounds.y - $settled.content[0].bounds.y) -gt 2) {
      throw "FAIL $name/wheel: 포커스 때문에 스크롤이 되돌아왔습니다."
    }
    Write-Output "PASS $name/wheel"

    # 새 휠 감속과 역방향 포커스 이동이 겹쳐도 예약된 프레임이 되돌리지 않아야 한다.
    $null = Send-AraViewMouse -Kind wheel -X $x -Y $y -Logical -Lines 3
    for ($i = 1; $i -lt $count; $i++) {
      $null = Send-AraViewKey -Spec shift-tab -WaitMs 30
    }
    $null = Invoke-AraViewRequest -Request @{ cmd = 'wait'; waitMs = 100 }
    $settings = Read-Settings
    Assert-InBody $settings.content[0] $settings "$name/Shift+Tab"
    Write-Output "PASS $name/Shift+Tab"
    $null = Send-AraViewKey -Spec escape -WaitMs 200
  }
} finally {
  if ((Get-AraViewState).dialogOpen) {
    $null = Send-AraViewKey -Spec escape -WaitMs 200
  }
}
