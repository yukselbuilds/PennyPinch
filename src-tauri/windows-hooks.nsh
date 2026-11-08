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
