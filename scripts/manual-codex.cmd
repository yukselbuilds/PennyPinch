@echo off
setlocal
if "%~1"=="" (
    powershell.exe -NoLogo -NoProfile -File "%~dp0manual-codex.ps1" -IncludeLocalStats
) else (
    powershell.exe -NoLogo -NoProfile -File "%~dp0manual-codex.ps1" %*
)
set "result=%ERRORLEVEL%"
echo.
pause
exit /b %result%
