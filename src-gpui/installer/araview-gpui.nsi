; AraView (GPUI) 설치 프로그램. scripts/Build-GpuiInstaller.ps1이 /DVERSION,
; /DSTAGE(설치할 파일을 모아 둔 폴더), /DOUTFILE을 넘겨 실행한다.
; 사용자 단위 설치라 관리자 권한이 필요 없다. 파일 연결은 앱의 설정 화면이
; HKCU에 등록하므로 여기서는 설치/제거와 바로가기만 다룬다.

Unicode true
RequestExecutionLevel user
SetCompressor /SOLID lzma

!include "MUI2.nsh"

!ifndef VERSION
  !error "VERSION is required"
!endif
!ifndef STAGE
  !error "STAGE is required"
!endif
!ifndef OUTFILE
  !define OUTFILE "AraView-GPUI-${VERSION}-setup.exe"
!endif

!define APP_NAME "AraView"
; Tauri 앱과 함께 설치돼도 시작 메뉴 바로가기가 겹치지 않게 이름을 나눈다.
!define SHORTCUT_NAME "AraView GPUI"
!define APP_EXE "araview-gpui.exe"
!define APP_ID "com.araview.viewer.gpui"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}"

Name "${APP_NAME}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\${APP_NAME} GPUI"
InstallDirRegKey HKCU "${UNINSTALL_KEY}" "InstallLocation"

!define MUI_ICON "..\..\src-tauri\icons\icon.ico"
!define MUI_UNICON "..\..\src-tauri\icons\icon.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Korean"
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetOutPath "$INSTDIR"
  File /r "${STAGE}\*.*"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateShortcut "$SMPROGRAMS\${SHORTCUT_NAME}.lnk" "$INSTDIR\${APP_EXE}"

  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${SHORTCUT_NAME}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "Ara"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$SMPROGRAMS\${SHORTCUT_NAME}.lnk"
  RMDir /r "$INSTDIR"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  ; 설정과 캐시는 사용자 데이터라 남긴다.
SectionEnd
