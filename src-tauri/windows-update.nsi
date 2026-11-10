; Signed schema 2 packages only extract the app next to the package.
Unicode true
Name "Headroom Update Package"
OutFile "${OUTPUT}"
RequestExecutionLevel user
SilentInstall silent
SetCompressor /SOLID lzma
CRCCheck force

!include FileFunc.nsh

Function .onInit
  ${GetParameters} $0
  StrCmp $0 "/HEADROOM-EXTRACT" allowed
  SetErrorLevel 2
  Quit
  allowed:
FunctionEnd

Section
  InitPluginsDir
  ClearErrors
  File "/oname=$PLUGINSDIR\application.exe" "${APPLICATION}"
  IfErrors failed
  Rename "$PLUGINSDIR\application.exe" "$EXEDIR\application.exe"
  IfErrors failed
  SetErrorLevel 0
  Goto done
  failed:
    SetErrorLevel 2
  done:
SectionEnd
