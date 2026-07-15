@echo off
setlocal
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-installer.ps1"
exit /b %ERRORLEVEL%
