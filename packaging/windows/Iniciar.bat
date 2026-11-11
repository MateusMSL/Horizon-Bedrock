@echo off
cd /d "%~dp0"
if not exist ".local\horizon-client\26.3" (
  powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0Preparar.ps1"
  if errorlevel 1 ( pause & exit /b 1 )
)
start "" "%~dp0horizon-client.exe"
