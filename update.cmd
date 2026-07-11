@echo off
setlocal EnableExtensions

cd /d "%~dp0"

where git >nul 2>nul
if errorlevel 1 (
    echo FEHLER: git wurde nicht gefunden.
    exit /b 1
)

echo BitShit Update-Bootstrap startet.
echo Hole zuerst die aktuelle Version der Update-Skripte.
git fetch origin --prune
if errorlevel 1 exit /b %errorlevel%

git switch main
if errorlevel 1 exit /b %errorlevel%

git reset --hard origin/main
if errorlevel 1 exit /b %errorlevel%

set "BACKEND=%~1"
if not defined BACKEND set "BACKEND=auto"

if /I not "%BACKEND%"=="auto" if /I not "%BACKEND%"=="cpu" if /I not "%BACKEND%"=="cuda" (
    echo FEHLER: Ungueltiges Backend "%BACKEND%". Erlaubt sind auto, cpu oder cuda.
    exit /b 2
)

echo Starte aktualisiertes PowerShell-Skript mit Backend %BACKEND%.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0update.ps1" %BACKEND%
exit /b %errorlevel%
