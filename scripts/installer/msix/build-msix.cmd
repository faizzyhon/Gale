@echo off
rem Builds the Microsoft Store package (MSIX) from the "layout" folder next to this script.
rem
rem   build-msix.cmd "<Name>" "<Publisher>" "<Publisher display name>" [version]
rem
rem The three values are on Partner Center -> your MSIX product -> Product management -> Product identity:
rem   Package/Identity/Name, Package/Identity/Publisher (starts with CN=) and Package/Properties/PublisherDisplayName.
rem Needs MakeAppx.exe from the Windows SDK. The Store signs the package, so no certificate is needed.
setlocal
if "%~3"=="" (
  echo Usage: %~nx0 "Package/Identity/Name" "CN=..." "Publisher display name" [version]
  exit /b 1
)
set "N=%~1"
set "P=%~2"
set "D=%~3"
set "V=%~4"
if "%V%"=="" set "V=0.7.1.0"
cd /d "%~dp0"

set "MAKEAPPX="
for /f "delims=" %%M in ('where makeappx 2^>nul') do if not defined MAKEAPPX set "MAKEAPPX=%%M"
if not defined MAKEAPPX for /f "delims=" %%M in ('dir /b /s /o-n "%ProgramFiles(x86)%\Windows Kits\10\bin\*makeappx.exe" 2^>nul ^| findstr /i "\\x64\\"') do if not defined MAKEAPPX set "MAKEAPPX=%%M"
if not defined MAKEAPPX (
  echo MakeAppx.exe not found. Install the Windows SDK: https://developer.microsoft.com/windows/downloads/windows-sdk/
  exit /b 1
)

powershell -NoProfile -Command "$m = Get-Content -Raw 'AppxManifest.xml'; $m = $m.Replace('@NAME@', $env:N).Replace('@PUBLISHER@', $env:P).Replace('@PUBLISHER_DISPLAY@', $env:D).Replace('@VERSION@', $env:V); [IO.File]::WriteAllText((Join-Path (Get-Location) 'layout\AppxManifest.xml'), $m)" || exit /b 1

"%MAKEAPPX%" pack /d layout /p "Gale_%V%_x64.msix" /o /h SHA256 || exit /b 1
echo.
echo Built Gale_%V%_x64.msix. Upload it on Partner Center -^> Packages.
