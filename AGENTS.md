# AGENTS

This document surveys the major collaborators in BoilR and what each is responsible for. Treat every section as an “agent”: a self-contained unit with a clear mandate, inputs, and outputs. Use it as a map when you’re touching new code or adding features.

Each description lists:
- **Role** – why the agent exists.
- **Collaborators** – who it talks to.
- **Key I/O** – important files, network calls, or user-facing side effects.
- **Notes** – quirks, invariants, or tips for extending the agent.

## Runtime Orchestration

### Application Bootstrap (`src/main.rs`)
- **Role:** Initializes diagnostics, ensures configuration directories exist, runs migrations, then dispatches either headless sync (`--no-ui`) or the egui-based desktop app.
- **Collaborators:** `config`, `migration`, `ui`.
- **Key I/O:** Creates folders under the user config directory; runs migration helpers before any UI mounts.
- **Notes:** Keep bootstrap logic minimal—push new responsibilities into downstream agents.

### UI Orchestrator (`src/ui/uiapp.rs`)
- **Role:** Central egui app (`MyEguiApp`) hosting menus for import, settings, artwork, backups, and disconnect flows.
- **Collaborators:** `platforms` (fetching game lists), `settings` (load/save), `sync` (runs Steam updates), `tokio` runtime (async tasks), `ui/*` submodules for panels.
- **Key I/O:** Persists settings before launches, manages rename map (`config/renames.json`), renders status for sync progress, spawns async jobs via `tokio::Runtime`.
- **Notes:** State is kept on the struct; UI callbacks mutate it directly. When adding new views follow the existing menu/state pattern. This remains the “legacy” UI path even as the Tauri frontend matures.

### Tauri Frontend (`apps/boilr-tauri`)
- **Role:** Modern desktop shell built with Tauri + React. It exposes dashboard widgets, settings controls, and orchestrates sync actions via Tauri commands.
- **Collaborators:** `boilr-core` (settings, sync, Steam helpers), `boilr` library (platform discovery, rename helpers), frontend build chain (Vite/Tailwind).
- **Key I/O:** Commands in `apps/boilr-tauri/src/main.rs` (`load_settings`, `discover_games`, `plan_sync`, `update_settings`, `run_full_sync`). The React layer in `src/App.tsx` consumes those commands, renders data, and calls back for updates.
- **Notes:**
  - `plan_sync` precomputes additions/removals so the UI can present a safe preview without touching disk.
  - `update_settings` merges partial settings patches and re-serialises platform sections using `save_settings_with_sections`.
  - Frontend assets live under `apps/boilr-tauri/src/` and are bundled with Vite; see `README.md` → **Development** for scripts.
  - Outstanding parity items (per-game selection, artwork tools, etc.) are tracked in `TODO.md`. When adding new commands, document them here.

### Headless Sync (`ui::run_sync` from `src/ui/mod.rs`)
- **Role:** Lightweight CLI flow triggered by `--no-ui`, executing the same synchronization pipeline minus GUI.
- **Collaborators:** Shares logic with the UI orchestrator; reuses `sync` and `platforms`.
- **Key I/O:** Prints progress to stdout; expects config files to be present.
- **Notes:** Keep parity between CLI and UI flows—new sync parameters should work in both.

## Configuration & Persistence

### Configuration Paths (`src/config.rs`)
- **Role:** Computes per-OS config locations and helper paths (thumbnails, cache, backups, symbolic-link staging).
- **Collaborators:** Used by nearly every module that touches the file system; migration tasks rely on it.
- **Key I/O:** Uses `XDG_CONFIG_HOME`/`HOME` on Unix and `APPDATA` on Windows.
- **Notes:** Functions create folders as needed; avoid duplicating path logic elsewhere.

### Settings Loader & Saver (`src/settings.rs`)
- **Role:** Hydrates `Settings` from layered TOML sources (defaults, user overrides, environment), scrubs placeholder API keys, and persists settings plus per-platform configuration.
- **Collaborators:** `config`, `steam::SteamSettings`, `steamgriddb::SteamGridDbSettings`, `platforms::Platforms`.
- **Key I/O:** Reads/writes `config.toml`, removes sensitive sections when enumerating settings for the UI.
- **Notes:** Saving reserializes platform-specific blocks through each platform agent—ensure new platform implementations populate `get_settings_serializable`.

### Config Migration (`src/migration.rs`)
- **Role:** Moves legacy config/cache files into the new directory structure and stamps the stored config version.
- **Collaborators:** `settings`, `platforms::get_platforms`.
- **Key I/O:** Renames `config.toml`, `.thumbnails`, and `cache.json` from repo root into the managed config folder.
- **Notes:** Runs on every startup prior to settings load; safe to extend with additional “version bump” logic.

### Local Renames (`src/ui/uiapp.rs::get_rename_map`)
- **Role:** Optional per-shortcut rename map stored as JSON.
- **Collaborators:** `sync::sync_shortcuts` (applies rename before computing app IDs).
- **Key I/O:** Reads `renames.json` from the config directory.
- **Notes:** Maintain compatibility with existing JSON structure (`{ "<appid>": "<new name>", … }`).

## Platform Import Agents

### Platform Registry (`src/platforms/platforms_load.rs`)
- **Role:** Builds the list of enabled `GamesPlatform` implementations for the current OS and hydrates their saved settings.
- **Collaborators:** Individual platform modules (`amazon`, `egs`, `heroic`, etc.), `settings`.
- **Key I/O:** Each platform’s section in `config.toml`.
- **Notes:** Adding a new platform requires registering it here and implementing the trait plus serialization helpers.

### Platform Trait (`src/platforms/platform.rs`)
- **Role:** Defines the `GamesPlatform` contract: metadata (`name`, `code_name`), enablement flag, async discovery (`get_shortcut_info`), UI rendering hook, and settings serialization.
- **Collaborators:** Consumers like `ui::create_games_to_sync` and `sync::sync_shortcuts`.
- **Key I/O:** Returns `ShortcutToImport`, including Proton/symlink requirements.
- **Notes:** Helper `to_shortcuts`/`to_shortcuts_simple` adapt raw platform records into Steam shortcut objects; reuse them in new platform modules to stay consistent.

### Platform Implementations (`src/platforms/*`)
- **Role:** Discover games from external launchers (Epic, GOG, Lutris, etc.) and translate them into BoilR shortcuts.
- **Collaborators:** `steam_shortcuts_util` for building Steam-compatible shortcuts, OS-specific path helpers, platform APIs if/when enabled.
- **Key I/O:** Reads launcher manifests/configs; no direct writes—the sync layer handles Steam files.
- **Notes:** Many modules are behind `cfg(target_family)` gates; ensure platform-specific code stays guarded.

## Steam Integration Agents

### Steam Settings (`src/steam/settings.rs`)
- **Role:** Stores user preferences like Steam install path, Proton/collections options.
- **Collaborators:** `settings`.
- **Key I/O:** Serialized in `config.toml`.
- **Notes:** Keep fields in sync with UI toggles; new toggles should default in `defaultconfig.toml`.

### Steam Shortcut Loader (`src/steam/installed_games.rs`)
- **Role:** Enumerates Steam users, locates `shortcuts.vdf`, and converts entries into `ShortcutInfo`.
- **Collaborators:** `sync::sync_shortcuts`, `sync::disconnect_shortcut`, `sync::fix_all_shortcut_icons`.
- **Key I/O:** Reads/writes `shortcuts.vdf`; detects per-user directories.
- **Notes:** Must gracefully handle missing files; returns `SteamUsersInfo` used downstream.

### Steam Collections Writer (`src/steam/collections.rs`)
- **Role:** Maintains Steam custom collections to group BoilR-imported games.
- **Collaborators:** `sync::sync_shortcuts` (conditionally writes collections when enabled).
- **Key I/O:** Updates LevelDB files under Steam userdata; optionally patches `localconfig.vdf`.
- **Notes:** Removes existing BoilR-tagged collections before rewriting to avoid duplication.

### Steam Utilities (`src/steam/utils.rs`, `restarter.rs`, etc.)
- **Role:** Misc helpers—path discovery, restarting Steam, Proton config discovery (Unix only).
- **Collaborators:** Settings/UI to offer actions like restarting Steam.
- **Key I/O:** File-system probing; launching external Steam processes.
- **Notes:** Some structs land unused on non-target platforms (expect dead-code warnings in cross-compiles).

## Synchronization & Artwork Agents

### Shortcut Synchronizer (`src/sync/synchronization.rs`)
- **Role:** Core pipeline that merges platform shortcuts into Steam, tags them, applies renames, removes previous BoilR entries, and optionally writes collections.
- **Collaborators:** `steam` loaders/writers, `platforms`, `settings`, `tokio` progress channels.
- **Key I/O:** Overwrites `shortcuts.vdf` per user; logs progress.
- **Notes:** Emits warnings when config files are missing on first run. Keep `SyncProgress` states in sync with UI expectations.

### Image Downloader (`src/steamgriddb/downloader.rs`)
- **Role:** Queries SteamGridDB (via `steamgriddb_api`), downloads images, and writes them into Steam’s artwork directories.
- **Collaborators:** `sync::download_images`, UI image-selection workflows.
- **Key I/O:** Network calls to SteamGridDB; writes artwork into `Steam/.../config/grid`.
- **Notes:** Respects `Settings::steamgrid_db` flags for NSFW allowances, animated preference, and fallbacks.

### Cached Search & Image Types (`src/steamgriddb/cached_search.rs`, `image_type.rs`)
- **Role:** Cache SteamGridDB results locally to avoid repeated queries and define expected asset variants (icon, hero, big picture, etc.).
- **Collaborators:** Downloader, UI panels for selecting alternate artwork.
- **Key I/O:** Cache files under the BoilR config directory; manipulates image filenames with Steam’s conventions.
- **Notes:** Ensure cache invalidation keeps schema compatibility—document changes here first.

### Backup & Disconnect (`src/ui/ui_backup.rs`, `src/ui/ui_disconnect.rs`)
- **Role:** UI flows to back up Steam shortcut files and purge BoilR-added entries.
- **Collaborators:** `sync::disconnect_shortcut`, `sync::fix_all_shortcut_icons`, file-system helpers.
- **Key I/O:** Copies shortcuts into the BoilR backup folder; rewrites `shortcuts.vdf` when disconnecting.
- **Notes:** These flows rely on the same Steam inspection APIs—prefer reusing those helpers rather than introducing new file logic.

## Data Flow Summary

1. **Startup:** `main` ensures directories, migrates configs, loads settings.
2. **Platform discovery:** UI (or CLI) builds the platform list and starts async fetch jobs per platform.
3. **User action:** Import button triggers `sync::sync_shortcuts`, which aggregates shortcuts, applies renames, updates Steam files, and optionally writes collections.
4. **Artwork:** After sync, `sync::download_images` invokes the SteamGridDB downloader for configured image types.
5. **Maintenance:** Backup, disconnect, and icon-fix flows reuse Steam helpers to keep user libraries tidy.

## Extension Tips

- **Adding a platform:** Implement `GamesPlatform` (respecting Proton/symlink flags), register it in `platforms_load`, add UI toggles, and provide serialization of platform-specific settings.
- **New settings:** Default them in `defaultconfig.toml`, expose them in the settings UI, and thread them through `Settings`.
- **New sync steps:** Extend `SyncProgress` with new states and update UI handlers (`render_import_button` and status displays).
- **Artwork enhancements:** Update `ImageType` definitions and ensure downloader/UI expect the same naming scheme before touching the filesystem.

## First-Run Behavior

On a fresh machine, expect log warnings about missing config files and unknown optional platforms (e.g., Amazon, Playnite, Game Pass) until the user enables or configures them. The sync pipeline still succeeds; those warnings are informational.
