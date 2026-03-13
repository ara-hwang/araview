; NSIS installer hooks for Image Viewer
; These macros are called by Tauri's generated NSIS installer at the
; appropriate points during installation and uninstallation.

; --- Called after the main installation section ---
!macro NSIS_HOOK_POSTINSTALL
  ; The context-menu registry entries are written by the app itself on first
  ; launch (see context_menu.rs), so nothing extra is needed here.
!macroend

; --- Called before the uninstaller removes application files ---
!macro NSIS_HOOK_PREUNINSTALL
  ; Ask the app binary to remove its own context-menu registry entries before
  ; the executable is deleted.  Run hidden (/SW_HIDE) to avoid a flash.
  Var /GLOBAL ExitCode
  ExecWait '"$INSTDIR\Image Viewer.exe" --unregister-context-menu' $ExitCode

  ; Fallback: delete the registry key directly if the binary returned a
  ; non-zero exit code or was already missing.
  ${If} $ExitCode <> 0
    DeleteRegKey HKCU "Software\Classes\SystemFileAssociations\image\shell\ImageViewer"
  ${EndIf}
!macroend
