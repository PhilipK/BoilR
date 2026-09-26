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
- `src/` (the `boilr` binary):
  - `platforms/<name>/`: one module per launcher, each implementing `GamesPlatform` (`platforms/platform.rs`) and registered in `platforms/platforms_load.rs`.
  - `ui/`: egui screens. `uiapp.rs` is the root.
  - `main.rs` re-imports the core modules (`use boilr_core::{config, settings, ...}`), so code here still writes `crate::settings::...`.

## Direction

Core first, then a new UI. Decided by Philip, 2026-09-26.

1. **Extract a UI-free core.** `crates/boilr-core` exists (settings, Steam, SteamGridDB, sync). Next: make the `boilr` crate a library with platforms usable without egui (`render_ui` behind an `egui-ui` feature), following branch `feature/tauri-migration` (Nov 2025). Target: `GamesPlatform` has no egui dependency (today `render_ui` takes `&mut egui::Ui`); platforms expose settings as data.
2. **Keep egui alive meanwhile**: minimal upgrades for user-facing bugs (paste, launch failures, DPI). Several UI paths call `block_on` on the UI thread (import, image download, image picking), causing freezes; fix those only where users hit them.
3. **Tauri UI** (`apps/boilr-tauri` on that branch, React) rebased onto the core and shipped as an opt-in beta beside egui. Its `TODO.md` lists the feature-parity gaps. It replaces egui only after parity and testing on Steam Deck and Wayland, where WebKitGTK rendering is the known risk.

## Rules that are not obvious from the code

- `code_name()` of a platform is the key in users' `settings.toml`. Never change it, even when renaming a platform's display name (Uplay stays `uplay`, Origin stays `origin`, Game Pass stays `gamepass`).
- `main.rs` denies `unwrap`, `expect`, `panic`, `todo` and slice indexing. Propagate errors with `eyre`.
- Whenever `Cargo.lock` changes, regenerate `flatpak/cargo-lock.json` with `flatpak/update-cargo-lock-json.sh`, or the Flatpak build breaks. CI job `flatpak_lock_sync` goes red on drift.
- Never identify processes by PID. In Flatpak, BoilR runs in its own PID namespace (usually PID 2) and cannot see host processes, so PID files and PID-based "is it running" checks misfire; 1.10.0 shipped a single-instance lock that locked users out this way (fixed in 1.10.1 with an OS file lock).
- Steam changes its file formats without notice. Collections moved from LevelDB to `userdata/<id>/config/cloudstorage/cloud-storage-namespace-1.json` in late 2025. When a Steam-facing bug appears, check what Steam writes today before trusting the existing code.
- Release tags have the form `v.1.9.6` (note the dot after `v`). Pushing one triggers `.github/workflows/release_on_v_tag.yml`, which makes a draft release. Releases are Philip's call: see the `release` skill.

## Skills

- `maintain`: one maintenance pass over issues and PRs. The scheduled maintainer routine runs this.
- `review-pr`: review a pull request against BoilR's rules before merging.
- `triage`: label, answer, deduplicate or close issues.
- `release`: cut a release (Philip only).
