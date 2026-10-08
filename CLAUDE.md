# BoilR

Rust desktop app (egui/eframe) that finds games from other launchers (Epic, GOG, Heroic, Lutris, itch, Bottles, Xbox/Game Pass, ...) and writes them into Steam as non-Steam shortcuts, with artwork from SteamGridDB. Main users: Linux and Steam Deck (often via Flatpak), then Windows. Owner: Philip (@PhilipK).

## Build and verify

Linux build needs only `pkg-config` and `libssl-dev`. OpenSSL comes in through `steamgriddb_api` (Philip's crate), which uses reqwest 0.11 with native-tls; BoilR's own reqwest uses rustls. X11, Wayland, xkbcommon and GL are loaded at runtime (dlopen), so no dev headers are needed for them. The longer apt list in the CI workflows is a leftover from older dependencies and includes GTK, which nothing uses.

```bash
sudo apt-get install -y pkg-config libssl-dev
```

In Claude Code cloud sessions, the SessionStart hook in `.claude/settings.json` installs these automatically.

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

Cargo workspace with two crates; `cargo build`/`test`/`clippy` at the root cover both.

- `crates/boilr-core/`: everything that doesn't need a UI or knowledge of launchers. It must never depend on egui or on `src/platforms`; the pipeline takes plain `(platform name, shortcuts)` lists.
  - `src/sync/synchronization.rs`: the import pipeline (write `shortcuts.vdf`, collections, images).
  - `src/steam/`: Steam file formats: shortcuts, collections (`collections.rs`), Proton mapping, user folders.
  - `src/steamgriddb/`: artwork lookup and download.
  - Settings: `src/defaultconfig.toml` merged with the user's `config.toml` in the config folder (`~/.config/boilr` or `%APPDATA%\boilr`). `src/migration.rs` handles old config shapes.
- `src/` (the `boilr` crate: a library plus the egui binary):
  - `platforms/<name>/`: one module per launcher, each implementing `GamesPlatform` (`platforms/platform.rs`) and registered in `platforms/platforms_load.rs`. Must build without egui: anything egui-only (`render_ui`, settings panels) sits behind `#[cfg(feature = "egui-ui")]`. CI checks `cargo clippy -p boilr --lib --no-default-features`.
  - `backups.rs`, `renames.rs`: shared by all front ends.
  - `ui/`: egui screens behind the default `egui-ui` feature. `uiapp.rs` is the root. `main.rs` (the `boilr` binary) requires that feature.
- `apps/boilr-tauri/`: the next interface (Tauri + React), its own Cargo workspace with its own lockfile. See its README: `npm run dev` gives a clickable browser version with a mocked backend (`src/devMock.ts`), which is the way for agents to see and test UI changes. CI job `tauri_Ubuntu` builds and tests it.
  - `lib.rs` re-imports the core modules (`use boilr_core::{config, settings, ...}`), so code here still writes `crate::settings::...`.

## Direction

Core first, then a new UI. Decided by Philip, 2026-09-26.

1. **Extract a UI-free core.** `crates/boilr-core` exists (settings, Steam, SteamGridDB, sync). The `boilr` crate is a library whose platforms build without egui, and the Tauri app lives in `apps/boilr-tauri` on top of it. Target: `GamesPlatform` has no egui dependency (today `render_ui` takes `&mut egui::Ui`); platforms expose settings as data.
2. **Keep egui alive meanwhile**: minimal upgrades for user-facing bugs (paste, launch failures, DPI). Several UI paths call `block_on` on the UI thread (import, image download, image picking), causing freezes; fix those only where users hit them.
3. **Tauri UI** (`apps/boilr-tauri`): reached egui parity and shipped as 2.0.0-beta.1 on Flathub's `beta` branch (same app id) on 2026-09-27; stable keeps egui until the beta has proven itself. Controller navigation in Steam Deck Game Mode is parked (touch works). Beta upkeep is in the `maintain` skill.

## Rules that are not obvious from the code

- `code_name()` of a platform is the key in users' `settings.toml`. Never change it, even when renaming a platform's display name (Uplay stays `uplay`, Origin stays `origin`, Game Pass stays `gamepass`).
- `main.rs` denies `unwrap`, `expect`, `panic`, `todo` and slice indexing. Propagate errors with `eyre`.
- Whenever `Cargo.lock` changes, regenerate `flatpak/cargo-lock.json` with `flatpak/update-cargo-lock-json.sh`, or the Flatpak build breaks. CI job `flatpak_lock_sync` goes red on drift.
  The same goes for the Tauri app: when `apps/boilr-tauri/Cargo.lock` or `package-lock.json` changes (including dependabot PRs), run `apps/boilr-tauri/flatpak/update-sources.sh` and commit `cargo-sources.json` and `node-sources.json`.
- Never identify processes by PID. In Flatpak, BoilR runs in its own PID namespace (usually PID 2) and cannot see host processes, so PID files and PID-based "is it running" checks misfire; 1.10.0 shipped a single-instance lock that locked users out this way (fixed in 1.10.1 with an OS file lock).
- Steam changes its file formats without notice. Collections moved from LevelDB to `userdata/<id>/config/cloudstorage/cloud-storage-namespace-1.json` in late 2025. When a Steam-facing bug appears, check what Steam writes today before trusting the existing code.
- Release tags have the form `v.1.9.6` (note the dot after `v`). Pushing one triggers `.github/workflows/release_on_v_tag.yml`, which makes a draft release. Releases are Philip's call: see the `release` skill.

## Skills

- `maintain`: one maintenance pass over issues and PRs. The scheduled maintainer routine runs this.
- `review-pr`: review a pull request against BoilR's rules before merging.
- `triage`: label, answer, deduplicate or close issues.
- `release`: cut a release (Philip only).
