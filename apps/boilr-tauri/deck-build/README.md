# Steam Deck test build

Builds an AppImage on Ubuntu 22.04 (glibc 2.35), so it runs on SteamOS, and repacks it without
the bundled `libwayland-*`, which otherwise crash on current Mesa with
`Could not create default EGL display: EGL_BAD_PARAMETER`.

From the repository root:

```bash
docker build -t boilr-deckbuild apps/boilr-tauri/deck-build
mkdir -p .dockerhome
docker run --rm --user "$(id -u):$(id -g)" -e HOME=/work/.dockerhome \
  -e CARGO_TARGET_DIR=/work/target-deck -e APPIMAGE_EXTRACT_AND_RUN=1 -e NO_STRIP=true \
  -v "$PWD":/work boilr-deckbuild /work/apps/boilr-tauri/deck-build/build.sh
```

Output: `target-deck/release/bundle/appimage/BoilR-deck.AppImage`. For testing only; releases
should ship as a Flatpak.
