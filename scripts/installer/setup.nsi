; Gale_Setup_<version>.exe — one-file setup for people who expect an .exe. It asks for administrator rights,
; installs Microsoft Edge WebView2 Runtime if the PC lacks it, and runs the MSI, which does the real work, so
; installing, updating and uninstalling behave the same either way.
; Built by build-msi.sh: makensis -DVERSION=x.y.z -DMSI=<msi> -DWV2=<bootstrapper> -DICON=<ico> -DOUT=<exe> setup.nsi
; Silent install for scripts: Gale_Setup_x.y.z.exe /S (exit code is msiexec's).

Target amd64-unicode ; Gale is 64-bit only; a 64-bit setup also avoids 32-bit folder redirection
SetCompress off ; the MSI inside is already compressed
RequestExecutionLevel admin
Name "Gale Download Manager ${VERSION}"
Caption "Gale Download Manager ${VERSION} Setup"
OutFile "${OUT}"
Icon "${ICON}"
BrandingText "Gale Download Manager"
ShowInstDetails nevershow

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Gale Download Manager"
VIAddVersionKey "CompanyName" "Muhammad Faizan"
VIAddVersionKey "LegalCopyright" "© 2026 Muhammad Faizan. MIT License."
VIAddVersionKey "FileDescription" "Gale Download Manager Setup"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"

!define WV2_KEY "Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

Page instfiles

Section
  SetDetailsPrint textonly
  InitPluginsDir

  ; Gale's window needs WebView2 (built into Windows 11 and current Windows 10). Microsoft's check: a "pv" version.
  ReadRegStr $1 HKLM "SOFTWARE\WOW6432Node\${WV2_KEY}" "pv"
  StrCmp $1 "" 0 +2
    ReadRegStr $1 HKCU "Software\${WV2_KEY}" "pv"
  StrCmp $1 "" wv2 0
  StrCmp $1 "0.0.0.0" wv2 msi
wv2:
  DetailPrint "Installing Microsoft Edge WebView2 Runtime…"
  SetDetailsPrint none
  File "/oname=$PLUGINSDIR\MicrosoftEdgeWebview2Setup.exe" "${WV2}"
  ExecWait '"$PLUGINSDIR\MicrosoftEdgeWebview2Setup.exe" /silent /install' $2
  SetDetailsPrint textonly
  StrCmp $2 0 msi
  IfSilent msi
    MessageBox MB_ICONEXCLAMATION "Microsoft Edge WebView2 Runtime could not be installed (error $2). Gale will be installed, but needs it to open its window. Get it from https://developer.microsoft.com/microsoft-edge/webview2/"

msi:
  DetailPrint "Installing Gale ${VERSION}…"
  SetDetailsPrint none
  File "/oname=$PLUGINSDIR\Gale.msi" "${MSI}"
  IfSilent 0 +3
    ExecWait '"$SYSDIR\msiexec.exe" /i "$PLUGINSDIR\Gale.msi" /qn' $0
    Goto done
  ExecWait '"$SYSDIR\msiexec.exe" /i "$PLUGINSDIR\Gale.msi" /passive' $0
done:
  SetDetailsPrint textonly
  SetErrorLevel $0
  StrCmp $0 0 ok
  StrCmp $0 3010 ok
  StrCmp $0 1602 cancelled
  IfSilent +2
    MessageBox MB_ICONSTOP "Gale could not be installed (Windows Installer error $0)."
  Abort
cancelled:
  Abort
ok:
  DetailPrint "Gale ${VERSION} is installed."
  ; Start Gale through Explorer so it runs as the signed-in user, not as administrator.
  SetDetailsPrint none
  IfSilent +2
    Exec '"$WINDIR\explorer.exe" "$PROGRAMFILES64\Gale\gale.exe"'
  SetAutoClose true
SectionEnd
