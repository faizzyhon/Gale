#!/usr/bin/env bash
# Builds dist/Gale_<version>_x64.msi from Linux (cargo-xwin + wixl), or reuses existing Windows binaries.
# Requirements: rustup target x86_64-pc-windows-msvc, cargo-xwin, msitools (wixl, msibuild, msiinfo), pnpm.
# On Windows you can instead run `pnpm tauri build` for Tauri's own WiX/NSIS installers.
set -euo pipefail
cd "$(dirname "$0")/../.."
VERSION=$(node -p "require('./package.json').version")
TARGET=x86_64-pc-windows-msvc
BIN=target/$TARGET/release
STAGE=target/msi-stage
OUT=dist-installer/Gale_${VERSION}_x64.msi

if [[ "${SKIP_BUILD:-}" != "1" ]]; then
  pnpm build
  (cd browser-extension && pnpm build)
  cargo xwin build --release --target $TARGET -p gale --features tauri/custom-protocol
  cargo xwin build --release --target $TARGET -p gale-nmh
fi

rm -rf "$STAGE" && mkdir -p "$STAGE/src" "$STAGE/ext" dist-installer
cp "$BIN/gale.exe" "$BIN/gale-nmh.exe" src-tauri/icons/icon.ico "$STAGE/src/"
cp scripts/installer/com.gale.nmh.json "scripts/installer/Install browser extension.html" "$STAGE/src/"
cp -r browser-extension/dist/. "$STAGE/ext/"

wixl -a x64 -D Version="$VERSION" -D Src="$STAGE/src" -D Ext="$STAGE/ext" -o "$OUT" scripts/installer/gale.wxs

# Windows only shows toasts for unpackaged apps whose Start-menu shortcut carries the app's AppUserModelID.
# wixl has no <ShortcutProperty>, so add the MsiShortcutProperty rows directly.
printf 'MsiShortcutProperty\tShortcut_\tPropertyKey\tPropVariantValue\ns72\ts72\ts0\ts0\nMsiShortcutProperty\tMsiShortcutProperty\nAumidStart\tStartGale\tSystem.AppUserModel.ID\tcom.faizzyhon.gale\nAumidDesktop\tDesktopGale\tSystem.AppUserModel.ID\tcom.faizzyhon.gale\n' > "$STAGE/MsiShortcutProperty.idt"
(cd "$STAGE" && msibuild "$OLDPWD/$OUT" -i MsiShortcutProperty.idt)

msiinfo export "$OUT" MsiShortcutProperty >/dev/null
sha256sum "$OUT" | tee "$OUT.sha256"
echo "Built $OUT"
