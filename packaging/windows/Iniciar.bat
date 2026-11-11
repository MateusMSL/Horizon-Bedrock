@echo off
setlocal enabledelayedexpansion
set "ROOT=%~dp0"
set "VERS=%APPDATA%\horizon-launcher\versions"
set "V="
for /f "delims=" %%d in ('dir /b /ad /o-n "%VERS%" 2^>nul') do (
  if not defined V if exist "%VERS%\%%d\.local\horizon-client\26.3" set "V=%VERS%\%%d"
)
if not defined V (
  echo Horizon nao encontrado.
  echo Instalar o launcher em playhorizon.dev, entrar com a conta Minecraft, preparar o 26.3 e rodar de novo.
  pause
  exit /b 1
)
if not exist "%ROOT%addons" mkdir "%ROOT%addons"
if not exist "%V%\addons" mklink /J "%V%\addons" "%ROOT%addons" >nul
cd /d "%V%"
start "" "%ROOT%horizon-client.exe"
