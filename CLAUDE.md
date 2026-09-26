# BoilR

Rust desktop app (egui/eframe) that finds games from other launchers (Epic, GOG, Heroic, Lutris, itch, Bottles, Xbox/Game Pass, ...) and writes them into Steam as non-Steam shortcuts, with artwork from SteamGridDB. Main users: Linux and Steam Deck (often via Flatpak), then Windows. Owner: Philip (@PhilipK).

## Build and verify

Linux build needs only `pkg-config` and `libssl-dev`. OpenSSL comes in through `steamgriddb_api` (Philip's crate), which uses reqwest 0.11 with native-tls; BoilR's own reqwest uses rustls. X11, Wayland, xkbcommon and GL are loaded at runtime (dlopen), so no dev headers are needed for them. The longer apt list in the CI workflows is a leftover from older dependencies and includes GTK, which nothing uses.

```bash
sudo apt-get install -y pkg-config libssl-dev
```

A change is verified when all of these pass:

```bash
cargo build
cargo test
cargo test --features flatpak
cargo clippy --all-targets
```

Windows-only code (`#[cfg(windows)]`: Amazon, Playnite, Game Pass, registry lookups) does not compile on Linux. Only CI's `test_Windows` job verifies it, so a green Linux build says nothing about those files.

The GUI cannot be exercised in a headless environment. `boilr --no-ui` runs a sync without the UI.

## Architecture

- `src/platforms/<name>/`: one module per launcher, each implementing `GamesPlatform` (`src/platforms/platform.rs`) and registered in `src/platforms/platforms_load.rs`.
- `src/sync/synchronization.rs`: the import pipeline (collect shortcuts, write `shortcuts.vdf`, collections, images).
- `src/steam/`: Steam file formats: shortcuts, collections (`collections.rs`), Proton mapping, user folders.
- `src/steamgriddb/`: artwork lookup and download.
- `src/ui/`: egui screens. `uiapp.rs` is the root.
- Settings: `src/defaultconfig.toml` merged with the user's `settings.toml` in the config folder (`~/.config/boilr` or `%APPDATA%\boilr`). `src/migration.rs` handles old config shapes.

## Direction

Core first, then a new UI. Decided by Philip, 2026-09-26.

1. **Extract a UI-free core.** Branch `feature/tauri-migration` (Nov 2025) already splits the backend into `crates/boilr-core`. Port that split onto `main` in small PRs while the egui UI keeps working. Target: `GamesPlatform` has no egui dependency (today `render_ui` takes `&mut egui::Ui`); platforms expose settings as data.
2. **Keep egui alive meanwhile**: minimal upgrades for user-facing bugs (paste, launch failures, DPI). Several UI paths call `block_on` on the UI thread (import, image download, image picking), causing freezes; fix those only where users hit them.
3. **Tauri UI** (`apps/boilr-tauri` on that branch, React) rebased onto the core and shipped as an opt-in beta beside egui. Its `TODO.md` lists the feature-parity gaps. It replaces egui only after parity and testing on Steam Deck and Wayland, where WebKitGTK rendering is the known risk.

## Rules that are not obvious from the code

- `code_name()` of a platform is the key in users' `settings.toml`. Never change it, even when renaming a platform's display name (Uplay stays `uplay`, Origin stays `origin`, Game Pass stays `gamepass`).
- `main.rs` denies `unwrap`, `expect`, `panic`, `todo` and slice indexing. Propagate errors with `eyre`.
- Whenever `Cargo.lock` changes, regenerate `flatpak/cargo-lock.json` with `flatpak/update-cargo-lock-json.sh`, or the Flatpak build breaks. CI job `flatpak_lock_sync` goes red on drift.
- Steam changes its file formats without notice. Collections moved from LevelDB to `userdata/<id>/config/cloudstorage/cloud-storage-namespace-1.json` in late 2025. When a Steam-facing bug appears, check what Steam writes today before trusting the existing code.
- Release tags have the form `v.1.9.6` (note the dot after `v`). Pushing one triggers `.github/workflows/release_on_v_tag.yml`, which makes a draft release. Releases are Philip's call: see the `release` skill.

## Skills

- `maintain`: one maintenance pass over issues and PRs. The scheduled maintainer routine runs this.
- `review-pr`: review a pull request against BoilR's rules before merging.
- `triage`: label, answer, deduplicate or close issues.
- `release`: cut a release (Philip only).
