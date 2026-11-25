#!/usr/bin/env bash
# Builds the Linux (.deb, .rpm) or macOS (.dmg) packages into dist-installer/, each with a .sha256 file for the updater.
# Linux needs: libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libdbus-1-dev librsvg2-dev rpm, pnpm, Rust.
# macOS needs: Xcode command line tools, pnpm, Rust with targets aarch64-apple-darwin and x86_64-apple-darwin.
set -euo pipefail
cd "$(dirname "$0")/../.."
VERSION=$(node -p "require('./package.json').version")
mkdir -p src-tauri/binaries dist-installer
(cd browser-extension && pnpm build && pnpm build:firefox)

if [[ "$(uname)" == "Darwin" ]]; then
  for t in aarch64-apple-darwin x86_64-apple-darwin; do cargo build --release --target $t -p gale-nmh; done
  lipo -create -output src-tauri/binaries/gale-nmh-universal-apple-darwin \
    target/aarch64-apple-darwin/release/gale-nmh target/x86_64-apple-darwin/release/gale-nmh
  pnpm tauri build --target universal-apple-darwin
  cp "target/universal-apple-darwin/release/bundle/dmg/Gale_${VERSION}_universal.dmg" dist-installer/
  OUT=("Gale_${VERSION}_universal.dmg")
else
  TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
  cargo build --release -p gale-nmh
  cp target/release/gale-nmh "src-tauri/binaries/gale-nmh-$TRIPLE"
  pnpm tauri build
  cp "target/release/bundle/deb/Gale_${VERSION}_amd64.deb" dist-installer/
  cp "target/release/bundle/rpm/Gale-${VERSION}-1.x86_64.rpm" "dist-installer/Gale_${VERSION}_x86_64.rpm"
  # The Chrome/Edge extension, loaded unpacked (Linux and macOS have no Start menu page that points at it).
  rm -f "dist-installer/Gale-Chrome-Extension-${VERSION}.zip"
  (cd browser-extension/dist && zip -qr "$OLDPWD/dist-installer/Gale-Chrome-Extension-${VERSION}.zip" .)
  rm -f "dist-installer/Gale-Firefox-Extension-${VERSION}.zip"
  (cd browser-extension/dist-firefox && zip -qr "$OLDPWD/dist-installer/Gale-Firefox-Extension-${VERSION}.zip" .)
  OUT=("Gale_${VERSION}_amd64.deb" "Gale_${VERSION}_x86_64.rpm" "Gale-Chrome-Extension-${VERSION}.zip" "Gale-Firefox-Extension-${VERSION}.zip")
fi

cd dist-installer
for f in "${OUT[@]}"; do shasum -a 256 "$f" > "$f.sha256" 2>/dev/null || sha256sum "$f" > "$f.sha256"; echo "Built dist-installer/$f"; done
