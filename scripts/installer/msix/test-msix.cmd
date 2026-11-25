@echo off
rem Installs the "layout" folder as a development package so you can try the Store version before uploading.
rem Needs Developer Mode (Settings -> System -> For developers) and build-msix.cmd to have run once (it writes
rem layout\AppxManifest.xml). Uninstall the MSI version of Gale first; both use the same data folder.
cd /d "%~dp0"
powershell -NoProfile -Command "Add-AppxPackage -Register (Join-Path (Get-Location) 'layout\AppxManifest.xml')" && echo Installed. Start Gale from the Start menu.
