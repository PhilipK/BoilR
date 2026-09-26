#!/bin/bash
# Builds the Tauri AppImage, then repacks it without the bundled libwayland-*,
# which clash with the host's Mesa ("Could not create default EGL display").
set -euo pipefail
cd /work/apps/boilr-tauri
npx --yes @tauri-apps/cli@2 build --bundles appimage
cd /work/target-deck/release/bundle/appimage
rm -f BoilR.AppDir/usr/lib/libwayland-*.so*
if [ ! -x /work/.dockerhome/appimagetool ]; then
  curl -fsSL -o /work/.dockerhome/appimagetool \
    https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x /work/.dockerhome/appimagetool
fi
ARCH=x86_64 /work/.dockerhome/appimagetool --no-appstream BoilR.AppDir BoilR-deck.AppImage
ls -la BoilR-deck.AppImage
