; The staging script checks this budget against every pinned resource path.
!define HEADROOM_MAX_INSTALL_DIR_CHARS 100

AllowSkipFiles off
LangString HeadroomPathTooLong 1033 "The installation folder is too long. Choose a shorter path (up to ${HEADROOM_MAX_INSTALL_DIR_CHARS} characters)."
LangString HeadroomPathTooLong 1055 "Kurulum klasörü çok uzun. Daha kısa bir yol seçin (en fazla ${HEADROOM_MAX_INSTALL_DIR_CHARS} karakter)."

!macro NSIS_HOOK_PREINSTALL
  StrLen $0 "$INSTDIR"
  ${If} $0 > ${HEADROOM_MAX_INSTALL_DIR_CHARS}
    MessageBox MB_OK|MB_ICONSTOP "$(HeadroomPathTooLong)" /SD IDOK
    SetErrorLevel 2
    Quit
  ${EndIf}
!macroend

LangString HeadroomDeleteData 1033 "Delete PennyPinch history and preferences"
LangString HeadroomDeleteData 1055 "PennyPinch geçmişini ve tercihlerini sil"
LangString HeadroomRemovalBusy 1033 "Quit PennyPinch from the system tray and close connected OpenCode sessions, then try uninstalling again."
LangString HeadroomRemovalBusy 1055 "PennyPinch'i sistem tepsisinden kapatın ve bağlı OpenCode oturumlarını kapatıp kaldırmayı yeniden deneyin."
LangString HeadroomRemovalConnections 1033 "A PennyPinch connection could not be removed. Close programs using its settings file, then use Disconnect in PennyPinch and retry. The installation was kept."
LangString HeadroomRemovalConnections 1055 "Bir PennyPinch bağlantısı kaldırılamadı. Ayar dosyasını kullanan programları kapatın, PennyPinch'te bağlantıyı kaldırıp yeniden deneyin. Kurulum korundu."
LangString HeadroomRemovalState 1033 "PennyPinch could not finish removing its local data or startup entry. Close programs using those files and try again. The installation was kept."
LangString HeadroomRemovalState 1055 "PennyPinch yerel verilerini veya başlangıç kaydını kaldıramadı. Bu dosyaları kullanan programları kapatıp yeniden deneyin. Kurulum korundu."
LangString HeadroomRemovalUnavailable 1033 "PennyPinch could not check its installation data. Repair this installation and try again."
LangString HeadroomRemovalUnavailable 1055 "PennyPinch kurulum verilerini kontrol edemedi. Bu kurulumu onarıp yeniden deneyin."

!macro HeadroomAbortRemoval message
  DetailPrint "${message}"
  ${If} $PassiveMode <> 1
    MessageBox MB_OK|MB_ICONSTOP "${message}" /SD IDOK
  ${EndIf}
  SetErrorLevel 2
  Quit
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    SetShellVarContext current
    ; GENERIC_READ, no sharing, OPEN_ALWAYS, FILE_FLAG_OPEN_REPARSE_POINT.
    System::Call 'kernel32::CreateFileW(w "$LOCALAPPDATA\${BUNDLEID}.desktop-use", i 0x80000000, i 0, p 0, i 4, i 0x00200000, p 0) p .r0'
    ${If} $0 == -1
      !insertmacro HeadroomAbortRemoval "$(HeadroomRemovalBusy)"
    ${EndIf}
    ; Keep the handle open until the uninstaller process exits.
    StrCpy $1 "keep"
    ${If} $DeleteAppDataCheckboxState = 1
      StrCpy $1 "delete"
    ${EndIf}
    ClearErrors
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --headroom-uninstall $1' $0
    ${If} ${Errors}
      !insertmacro HeadroomAbortRemoval "$(HeadroomRemovalUnavailable)"
    ${ElseIf} $0 = 10
      !insertmacro HeadroomAbortRemoval "$(HeadroomRemovalConnections)"
    ${ElseIf} $0 = 11
      !insertmacro HeadroomAbortRemoval "$(HeadroomRemovalState)"
    ${ElseIf} $0 <> 0
      !insertmacro HeadroomAbortRemoval "$(HeadroomRemovalUnavailable)"
    ${EndIf}
  ${EndIf}
!macroend
