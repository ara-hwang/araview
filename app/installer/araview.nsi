; AraView 설치 프로그램. scripts/Build-Installer.ps1이 /DVERSION,
; /DSTAGE(설치할 파일을 모아 둔 폴더), /DOUTFILE, 선택적으로 /DNAME_SUFFIX를 넘겨 실행한다.
; 사용자 단위 설치라 관리자 권한이 필요 없다. 파일 연결은 앱의 설정 화면이
; HKCU에 등록하므로 여기서는 설치/제거와 바로가기만 다룬다.
;
; 1.x(Tauri) 설치본과 같은 정체성(설치 폴더, 실행 파일 이름, 제거 항목, 바로가기)을 쓴다.
; 그래서 1.x 위에 설치하면 같은 자리를 덮어쓰고, 기존 파일 연결과 PSD 썸네일 등록,
; 바로가기도 그대로 유효하다. NAME_SUFFIX는 실제 설치본과 겹치지 않게 설치 흐름을
; 시험할 때만 쓴다.

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
  !define OUTFILE "AraView-${VERSION}-setup.exe"
!endif
!ifndef NAME_SUFFIX
  !define NAME_SUFFIX ""
!endif

!define APP_NAME "AraView${NAME_SUFFIX}"
!define APP_EXE "araview.exe"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"

Name "${APP_NAME}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\${APP_NAME}"
InstallDirRegKey HKCU "${UNINSTALL_KEY}" "InstallLocation"

!define MUI_ICON "..\resources\icon.ico"
!define MUI_UNICON "..\resources\icon.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Korean"
!insertmacro MUI_LANGUAGE "English"

LangString RunningMessage ${LANG_KOREAN} "AraView가 실행 중입니다. 앱을 종료한 뒤 다시 시도하세요."
LangString RunningMessage ${LANG_ENGLISH} "AraView is running. Close it and try again."

; 실행 중인 앱의 실행 파일은 덮어쓸 수 없다. 지워 보고 안 되면 종료를 요청한다.
Function EnsureAppClosed
  retry:
  IfFileExists "$INSTDIR\${APP_EXE}" 0 done
  ClearErrors
  Delete "$INSTDIR\${APP_EXE}"
  IfErrors 0 done
  MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "$(RunningMessage)" IDRETRY retry
  Abort
  done:
FunctionEnd

Section "Install"
  Call EnsureAppClosed

  SetOutPath "$INSTDIR"
  File /r "${STAGE}\*.*"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  ; 1.x 설치본이 남긴 번들 보조 폴더.
  RMDir /r "$INSTDIR\_up_"

  CreateShortcut "$SMPROGRAMS\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"

  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "araview"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$SMPROGRAMS\${APP_NAME}.lnk"
  Delete "$DESKTOP\${APP_NAME}.lnk"
  RMDir /r "$INSTDIR"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  ; 설정과 캐시는 사용자 데이터라 남긴다.
SectionEnd
