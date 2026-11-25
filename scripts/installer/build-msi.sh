#!/usr/bin/env bash
# Builds dist-installer/Gale_<version>_x64.msi and Gale_Setup_<version>.exe from Linux (cargo-xwin + wixl), or reuses existing Windows binaries.
# Requirements: rustup target x86_64-pc-windows-msvc, cargo-xwin, msitools (wixl, msibuild, msiinfo), nsis (makensis), osslsigncode, pnpm.
# On Windows you can instead run `pnpm tauri build` for Tauri's own WiX/NSIS installers.
#
# Code signing (optional): set SIGN_PFX to a .pfx/.p12 code-signing certificate and SIGN_PASS to its password.
# Every .exe inside the MSI, the MSI and the setup .exe are then signed (SHA-256, RFC 3161 timestamp from SIGN_TS).
# shortcut: only file-based certificates; a USB token or cloud HSM needs osslsigncode's -pkcs11module options here.
set -euo pipefail
cd "$(dirname "$0")/../.."
VERSION=$(node -p "require('./package.json').version")
TARGET=x86_64-pc-windows-msvc
BIN=target/$TARGET/release
STAGE=target/msi-stage
OUT=dist-installer/Gale_${VERSION}_x64.msi
SIGN_TS=${SIGN_TS:-http://timestamp.digicert.com}

sign() {
  [[ -n "${SIGN_PFX:-}" ]] || return 0
  local dse=(); [[ "$1" == *.msi ]] && dse=(-add-msi-dse) # also covers the MSI's metadata streams
  osslsigncode sign "${dse[@]}" -pkcs12 "$SIGN_PFX" -pass "${SIGN_PASS:-}" -h sha256 -n "Gale Download Manager" \
    -i https://github.com/faizzyhon/Gale -ts "$SIGN_TS" -in "$1" -out "$1.signed"
  mv "$1.signed" "$1"
  # Linux has no code-signing roots to check trust against; check the signature matches the file.
  local v cur calc
  v=$(osslsigncode verify -in "$1" 2>&1 || true)
  cur=$(grep -m1 -E 'Current (message digest|DigitalSignature)' <<<"$v" | awk '{print $NF}')
  calc=$(grep -m1 -E 'Calculated (message digest|DigitalSignature)' <<<"$v" | awk '{print $NF}')
  [[ -n "$cur" && "$cur" == "$calc" ]] || { echo "signature check failed for $1" >&2; exit 1; }
  echo "Signed $1"
}

if [[ "${SKIP_BUILD:-}" != "1" ]]; then
  pnpm build
  (cd browser-extension && pnpm build && pnpm build:firefox)
  cargo xwin build --release --target $TARGET -p gale --features tauri/custom-protocol
  cargo xwin build --release --target $TARGET -p gale-nmh
fi

# Third-party video tools, verified against the publishers' checksum files and cached between builds.
TOOLS_CACHE=target/wintools
mkdir -p "$TOOLS_CACHE"
if [[ ! -f "$TOOLS_CACHE/yt-dlp.exe" ]]; then
  YT=https://github.com/yt-dlp/yt-dlp/releases/latest/download
  curl -fsSL -o "$TOOLS_CACHE/yt-dlp.exe" "$YT/yt-dlp.exe"
  curl -fsSL "$YT/SHA2-256SUMS" | grep ' yt-dlp.exe$' | (cd "$TOOLS_CACHE" && sha256sum -c -)
  curl -fsSL -o "$TOOLS_CACHE/yt-dlp-LICENSE.txt" https://raw.githubusercontent.com/yt-dlp/yt-dlp/master/LICENSE
fi
if [[ ! -f "$TOOLS_CACHE/ffmpeg.exe" ]]; then
  FF=https://github.com/BtbN/FFmpeg-Builds/releases/download/latest
  curl -fsSL -o "$TOOLS_CACHE/ffmpeg.zip" "$FF/ffmpeg-master-latest-win64-lgpl.zip"
  echo "$(curl -fsSL "$FF/checksums.sha256" | grep 'ffmpeg-master-latest-win64-lgpl.zip$' | cut -d' ' -f1)  $TOOLS_CACHE/ffmpeg.zip" | sha256sum -c -
  unzip -o -q -j "$TOOLS_CACHE/ffmpeg.zip" '*/bin/ffmpeg.exe' '*/bin/ffprobe.exe' '*/LICENSE.txt' -d "$TOOLS_CACHE"
  mv "$TOOLS_CACHE/LICENSE.txt" "$TOOLS_CACHE/FFmpeg-LICENSE.txt" && rm "$TOOLS_CACHE/ffmpeg.zip"
fi

if [[ ! -f "$TOOLS_CACHE/MicrosoftEdgeWebview2Setup.exe" ]]; then
  # Microsoft's redistributable WebView2 bootstrapper, run by the setup .exe on PCs that lack WebView2.
  curl -fsSL -o "$TOOLS_CACHE/wv2.tmp" "https://go.microsoft.com/fwlink/p/?LinkId=2124703"
  # Linux has no Microsoft code-signing roots: check the file matches its signature and the signer is Microsoft.
  sig=$(osslsigncode verify -in "$TOOLS_CACHE/wv2.tmp" 2>&1 || true)
  cur=$(grep -m1 'Current message digest' <<<"$sig" | awk '{print $NF}')
  calc=$(grep -m1 'Calculated message digest' <<<"$sig" | awk '{print $NF}')
  [[ -n "$cur" && "$cur" == "$calc" ]] && grep -q 'O=Microsoft Corporation/CN=Microsoft Corporation' <<<"$sig" \
    || { echo "WebView2 bootstrapper failed signature check" >&2; exit 1; }
  mv "$TOOLS_CACHE/wv2.tmp" "$TOOLS_CACHE/MicrosoftEdgeWebview2Setup.exe"
fi

rm -rf "$STAGE" && mkdir -p "$STAGE/src" "$STAGE/ext" "$STAGE/tools" dist-installer
cp "$TOOLS_CACHE"/{yt-dlp.exe,ffmpeg.exe,ffprobe.exe,yt-dlp-LICENSE.txt,FFmpeg-LICENSE.txt} scripts/installer/THIRD-PARTY-NOTICES.txt "$STAGE/tools/"
cp "$BIN/gale.exe" "$BIN/gale-nmh.exe" src-tauri/icons/icon.ico "$STAGE/src/"
cp scripts/installer/com.gale.nmh.json scripts/installer/com.gale.nmh.firefox.json "scripts/installer/Install browser extension.html" "$STAGE/src/"
cp -r browser-extension/dist/. "$STAGE/ext/"
# Staged copies are signed, so the cached tools stay as their publishers made them.
for f in "$STAGE"/src/*.exe "$STAGE"/tools/*.exe; do sign "$f"; done

wixl -a x64 -D Version="$VERSION" -D Src="$STAGE/src" -D Ext="$STAGE/ext" -D Tools="$STAGE/tools" -o "$OUT" scripts/installer/gale.wxs

# Windows only shows toasts for unpackaged apps whose Start-menu shortcut carries the app's AppUserModelID.
# wixl has no <ShortcutProperty>, so add the MsiShortcutProperty rows directly.
printf 'MsiShortcutProperty\tShortcut_\tPropertyKey\tPropVariantValue\ns72\ts72\ts0\ts0\nMsiShortcutProperty\tMsiShortcutProperty\nAumidStart\tStartGale\tSystem.AppUserModel.ID\tcom.faizzyhon.gale\nAumidDesktop\tDesktopGale\tSystem.AppUserModel.ID\tcom.faizzyhon.gale\n' > "$STAGE/MsiShortcutProperty.idt"
(cd "$STAGE" && msibuild "$OLDPWD/$OUT" -i MsiShortcutProperty.idt)

msiinfo export "$OUT" MsiShortcutProperty >/dev/null
sign "$OUT" # last: any change to the MSI after this breaks the signature
sha256sum "$OUT" | tee "$OUT.sha256"
echo "Built $OUT"

# Gale_Setup_<version>.exe: asks for admin rights and runs the MSI above (see setup.nsi).
SETUP=dist-installer/Gale_Setup_${VERSION}.exe
makensis -V2 -DVERSION="$VERSION" -DMSI="$PWD/$OUT" -DWV2="$PWD/$TOOLS_CACHE/MicrosoftEdgeWebview2Setup.exe" -DICON="$PWD/src-tauri/icons/icon.ico" -DOUT="$PWD/$SETUP" scripts/installer/setup.nsi
sign "$SETUP"
sha256sum "$SETUP" | tee "$SETUP.sha256"
echo "Built $SETUP"

# Microsoft Store kit: the same files in MSIX layout, packed on Windows by build-msix.cmd (MakeAppx has no Linux build).
KIT=target/msix-kit
rm -rf "$KIT" && mkdir -p "$KIT/layout/assets" "$KIT/layout/tools" "$KIT/layout/browser-extension"
cp "$STAGE"/src/{gale.exe,gale-nmh.exe,com.gale.nmh.json,com.gale.nmh.firefox.json,"Install browser extension.html"} "$KIT/layout/"
cp "$STAGE"/tools/* "$KIT/layout/tools/"
cp -r "$STAGE"/ext/. "$KIT/layout/browser-extension/"
cp src-tauri/icons/{Square44x44Logo.png,Square150x150Logo.png,StoreLogo.png} "$KIT/layout/assets/"
cp scripts/installer/msix/{AppxManifest.xml,build-msix.cmd,test-msix.cmd} "$KIT/"
rm -f "dist-installer/Gale-MSIX-kit-${VERSION}.zip"
(cd "$KIT" && zip -qr "$OLDPWD/dist-installer/Gale-MSIX-kit-${VERSION}.zip" .)
echo "Built dist-installer/Gale-MSIX-kit-${VERSION}.zip"

# Portable ZIP: no installer. portable.txt keeps Gale from adding itself to Windows startup or removing other installs.
PORT=target/portable/Gale-${VERSION}
rm -rf target/portable && mkdir -p "$PORT/tools"
cp "$STAGE"/src/{gale.exe,gale-nmh.exe,com.gale.nmh.json,com.gale.nmh.firefox.json} "$PORT/"
cp "$STAGE"/tools/* "$PORT/tools/"
cp -r browser-extension/dist "$PORT/browser-extension-chrome"
cp -r browser-extension/dist-firefox "$PORT/browser-extension-firefox"
cat > "$PORT/portable.txt" <<TXT
Gale ${VERSION}, portable. Run gale.exe.

Your download list and settings are kept in %APPDATA%\com.faizzyhon.gale, like the installed version.
On first start Gale registers its browser helper for your user (so the extension can reach it); it does not add
itself to Windows startup unless you turn that on in Settings. Delete this folder to remove it.
WebView2 must be installed (it is on Windows 11 and current Windows 10).
TXT
sed -i 's/$/\r/' "$PORT/portable.txt"
rm -f "dist-installer/Gale-${VERSION}-windows-x64-portable.zip"
(cd target/portable && zip -qr "$OLDPWD/dist-installer/Gale-${VERSION}-windows-x64-portable.zip" "Gale-${VERSION}")
sha256sum "dist-installer/Gale-${VERSION}-windows-x64-portable.zip" | sed 's#dist-installer/##' > "dist-installer/Gale-${VERSION}-windows-x64-portable.zip.sha256"
echo "Built dist-installer/Gale-${VERSION}-windows-x64-portable.zip"
