#requires -Version 7
<#
.SYNOPSIS
  UI 자동 숨김이 켜진 개발 뷰어에서 가장자리 UI가 이미지 영역을 바꾸지 않는지 확인한다.
.DESCRIPTION
  이미지를 열고 UI 자동 숨김을 켠 뒤 실행한다. 사용자 입력 장치를 건드리지 않는다.
  도크 위치와 펼침 여부는 현재 설정을 사용한다.
#>
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot 'AraViewControl.psm1') -Force

$initial = Get-AraViewState
if ($initial.kind -ne 'image' -or $initial.dialogOpen -or $initial.gridOpen) {
  throw '이미지를 열고 다이얼로그와 그리드를 닫은 개발 뷰어가 필요합니다.'
}
$actions = @{
  single = 'viewSingle'
  'left-to-right' = 'viewLtr'
  'right-to-left' = 'viewRtl'
  webtoon = 'viewWebtoon'
}
try {
  foreach ($mode in @('single', 'left-to-right', 'right-to-left', 'webtoon')) {
    $null = Send-AraViewAction -Id $actions[$mode] -WaitMs 500
    $hidden = Send-AraViewMouse -Kind move -X 250 -Y 250 -Logical -WaitMs 200
    if ($hidden.viewport.y -ne 0 -or $hidden.viewport.height -ne $hidden.window.height) {
      throw 'UI 자동 숨김을 켜고 정보 패널을 닫아 주세요.'
    }
    $expected = $hidden | Select-Object viewport, zoom, position, webtoon | ConvertTo-Json -Compress -Depth 5
    foreach ($edge in @('top', 'center', 'bottom', 'center', 'bottom', 'center')) {
      $y = switch ($edge) {
        top { 2 }
        bottom { $hidden.window.height - 2 }
        center { 250 }
      }
      $state = Send-AraViewMouse -Kind move -X 250 -Y $y -Logical -WaitMs 200
      $actual = $state | Select-Object viewport, zoom, position, webtoon | ConvertTo-Json -Compress -Depth 5
      if ($actual -ne $expected) {
        throw "$mode/$edge changed image geometry: $expected -> $actual"
      }
      Write-Output "PASS $mode/$edge"
    }
  }
} finally {
  $null = Send-AraViewAction -Id $actions[$initial.viewMode] -WaitMs 300
  $null = Send-AraViewMouse -Kind move -X 250 -Y 250 -Logical -WaitMs 200
}
