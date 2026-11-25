@echo off
rem Signs Gale_Setup_<version>.exe and Gale_<version>_x64.msi on Windows with a code-signing certificate
rem installed in your certificate store: a USB token (Certum, Sectigo...) or a cloud certificate such as
rem Certum SimplySign. Run it from the folder that holds the two files, after the certificate's app has
rem connected the certificate. Needs signtool.exe from the Windows SDK.
rem
rem   sign-windows.cmd 0.7.0
rem
rem The installers built on Linux can instead be signed during the build: see SIGN_PFX in build-msi.sh.
setlocal enabledelayedexpansion
if "%~1"=="" (echo Usage: %~nx0 ^<version^>  e.g. %~nx0 0.7.0 & exit /b 1)
set TS=http://timestamp.digicert.com
where signtool >nul 2>nul || (echo signtool.exe not found. Install the Windows SDK "Signing Tools" or run from a Developer Command Prompt. & exit /b 1)

for %%F in ("Gale_Setup_%~1.exe" "Gale_%~1_x64.msi") do (
  if not exist %%F (echo %%~F not found in %CD% & exit /b 1)
  rem /a picks the best code-signing certificate in your store; the token or app asks for its PIN.
  signtool sign /a /fd sha256 /tr %TS% /td sha256 /d "Gale Download Manager" /du https://github.com/faizzyhon/Gale %%F || exit /b 1
  signtool verify /pa /q %%F || (echo Signature check failed for %%~F & exit /b 1)
  rem Older Windows prints the hash with spaces between bytes; the .sha256 file needs it in one piece.
  for /f "delims=" %%H in ('certutil -hashfile %%F SHA256 ^| findstr /v ":"') do (
    set "H=%%H"
    > "%%~F.sha256" echo !H: =!  %%~nxF
  )
  echo Signed %%~F
)
echo Done. Upload the signed files and their new .sha256 files to the GitHub release.
