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
LangString HeadroomRuntimeReplacementFailed 1033 "Setup cannot replace the old OpenTelemetry package. Close programs that use PennyPinch. Run setup again. Use an empty folder for a new installation."
LangString HeadroomRuntimeReplacementFailed 1055 "Kurulum eski OpenTelemetry paketini değiştiremiyor. PennyPinch'i kullanan programları kapatın. Kurulumu yeniden başlatın. Yeni kurulum için boş bir klasör seçin."
Var HeadroomLegacyOpenTelemetry

!macro HeadroomAbortSetup message
  DetailPrint "${message}"
  ${If} $PassiveMode <> 1
    MessageBox MB_OK|MB_ICONSTOP "${message}" /SD IDOK
  ${EndIf}
  SetErrorLevel 2
  Quit
!macroend

!macro HeadroomRequireRuntimeDirectory path
  System::Call 'kernel32::GetFileAttributesW(w "${path}") i.r2'
  IntOp $2 $2 & 0x410
  ${If} $2 <> 16
    !insertmacro HeadroomAbortSetup "$(HeadroomRuntimeReplacementFailed)"
  ${EndIf}
!macroend

!macro HeadroomPrepareOpenTelemetryRetirement
  ; Python can select the old distribution when both metadata directories exist.
  StrCpy $0 "$INSTDIR\runtime\payload\Lib\site-packages"
  StrCpy $1 "$0\opentelemetry_api-1.45.1.dist-info"
  System::Call 'kernel32::GetFileAttributesW(w "$1") i.r2 ?e'
  Pop $3
  ${If} $2 <> -1
    ReadRegStr $3 SHCTX "${MANUPRODUCTKEY}" ""
    ${If} $3 != $INSTDIR
      !insertmacro HeadroomAbortSetup "$(HeadroomRuntimeReplacementFailed)"
    ${EndIf}
    !insertmacro HeadroomRequireRuntimeDirectory "$INSTDIR"
    !insertmacro HeadroomRequireRuntimeDirectory "$INSTDIR\runtime"
    !insertmacro HeadroomRequireRuntimeDirectory "$INSTDIR\runtime\payload"
    !insertmacro HeadroomRequireRuntimeDirectory "$INSTDIR\runtime\payload\Lib"
    !insertmacro HeadroomRequireRuntimeDirectory "$0"
    !insertmacro HeadroomRequireRuntimeDirectory "$1"
    System::Call 'kernel32::GetFileAttributesW(w "$INSTDIR\opentelemetry-1.45.1.retired") i.r2 ?e'
    Pop $3
    ${If} $2 <> -1
      !insertmacro HeadroomAbortSetup "$(HeadroomRuntimeReplacementFailed)"
    ${ElseIf} $3 <> 2
    ${AndIf} $3 <> 3
      !insertmacro HeadroomAbortSetup "$(HeadroomRuntimeReplacementFailed)"
    ${EndIf}
    StrCpy $HeadroomLegacyOpenTelemetry "$1"
  ${ElseIf} $3 <> 2
  ${AndIf} $3 <> 3
    !insertmacro HeadroomAbortSetup "$(HeadroomRuntimeReplacementFailed)"
  ${EndIf}
!macroend

!macro HeadroomRetireOpenTelemetry
  ${If} $HeadroomLegacyOpenTelemetry != ""
    StrCpy $3 "$INSTDIR\opentelemetry-1.45.1.retired"
    ClearErrors
    Rename "$HeadroomLegacyOpenTelemetry" "$3"
    ${If} ${Errors}
      !insertmacro HeadroomAbortSetup "$(HeadroomRuntimeReplacementFailed)"
    ${EndIf}
    ; Keep other files in the retired directory.
    !insertmacro HeadroomRequireRuntimeDirectory "$3"
    Delete "$3\INSTALLER"
    Delete "$3\METADATA"
    Delete "$3\RECORD"
    Delete "$3\REQUESTED"
    Delete "$3\WHEEL"
    Delete "$3\entry_points.txt"
    System::Call 'kernel32::GetFileAttributesW(w "$3\licenses") i.r2'
    IntOp $2 $2 & 0x410
    ${If} $2 = 16
      Delete "$3\licenses\LICENSE"
      RMDir "$3\licenses"
    ${EndIf}
    RMDir "$3"
    ClearErrors
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    SetShellVarContext current
    ; GENERIC_READ, no sharing, OPEN_ALWAYS, FILE_FLAG_OPEN_REPARSE_POINT.
    System::Call 'kernel32::CreateFileW(w "$LOCALAPPDATA\${BUNDLEID}.desktop-use", i 0x80000000, i 0, p 0, i 4, i 0x00200000, p 0) p .r0'
    ${If} $0 == -1
      !insertmacro HeadroomAbortSetup "$(HeadroomRemovalBusy)"
    ${EndIf}
    ; Keep the handle open until the uninstaller process exits.
    StrCpy $1 "keep"
    ${If} $DeleteAppDataCheckboxState = 1
      StrCpy $1 "delete"
    ${EndIf}
    ClearErrors
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --headroom-uninstall $1' $0
    ${If} ${Errors}
      !insertmacro HeadroomAbortSetup "$(HeadroomRemovalUnavailable)"
    ${ElseIf} $0 = 10
      !insertmacro HeadroomAbortSetup "$(HeadroomRemovalConnections)"
    ${ElseIf} $0 = 11
      !insertmacro HeadroomAbortSetup "$(HeadroomRemovalState)"
    ${ElseIf} $0 <> 0
      !insertmacro HeadroomAbortSetup "$(HeadroomRemovalUnavailable)"
    ${EndIf}
  ${EndIf}
!macroend
