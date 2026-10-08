# BoilR (Tauri)

The next BoilR interface: a React frontend in `src/` on top of the same Rust library as the egui app
(`boilr` for launchers, `boilr-core` for Steam and settings). Work in progress; see `TODO.md` for
what the egui app still does that this one doesn't.

This is its own Cargo workspace, so Tauri's dependencies stay out of the root `Cargo.lock` and the
egui Flatpak.

## Work on the interface in a browser

```bash
npm install
npm run dev
```

Open http://127.0.0.1:1420. Outside Tauri, `src/devMock.ts` answers the backend commands with
sample data (launchers with errors, Flatpak apps, an old shortcut to remove, a simulated sync),
so the whole interface can be clicked through without Rust or Steam.

The Artwork tab shows drawn placeholders by default. To judge it with real SteamGridDB art, point
the dev server at a file holding your API key; it then proxies SteamGridDB and adds the key
itself, so the key never reaches the page:

```bash
BOILR_SGDB_KEY_FILE=~/path/to/apikey.txt npm run dev
```

## Run the real app

```bash
npm run build
cargo run --features tauri/custom-protocol
```

Without `tauri/custom-protocol` the app loads the dev server at port 1420 instead of the built
frontend. Linux needs `libwebkit2gtk-4.1-dev` to build.

## Flatpak (preview)

`flatpak/io.github.philipk.boilr.Devel.yml` builds the app on the GNOME runtime with the same
sandbox as the egui app on Flathub, under a separate app id so it installs next to the release.
The build command is at the top of the manifest; `flatpak build-bundle` turns the result into a
single file to install elsewhere, such as a Steam Deck. Verified on a desktop: it finds Flatpak
apps through `flatpak-spawn`, like the egui Flatpak.

## Steam Deck test build (AppImage)

See `deck-build/README.md`. The AppImage is for trying the interface on a Deck; it does not find
games reliably (bundled libraries break the launcher commands BoilR runs). Releases will be a
Flatpak.
