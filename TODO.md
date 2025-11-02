# Todo

## High priority

### Per-game selection & renaming (Priority: High · Estimate: 8)
- **Goal:** Match egui’s ability to include/exclude individual shortcuts and edit names before import.
- **Where to start:** Backend already exposes rename map helpers (`src/renames.rs`) and blacklisting happens in `apps/boilr-tauri/src/main.rs::prepare_additions`. Extend the plan/sync commands if extra data is needed (e.g., platform labels per shortcut).
- **UI approach:** In `apps/boilr-tauri/src/App.tsx`, add per-game checkboxes (backed by a local `Set` of excluded app IDs) and a rename modal or inline edit. Persist changes by updating `blacklisted_games` (for exclusions) via `update_settings` and writing to `renames.json` through a new Tauri command that mirrors egui’s `rename_map` logic.
- **Edge cases:** Provide a reset option to revert to stored names; ensure renames trigger `calculate_app_id_for_shortcut` exactly once (follow `prepare_additions`).

### Platform-specific settings (Priority: High · Estimate: 5)
- **Goal:** Surface each platform’s configuration UI in the Tauri app so toggles/paths can be adjusted without falling back to egui.
- **Where to start:** Every platform implements `GamesPlatform::render_ui` and `get_settings_serializable`. Introduce a Tauri command that returns serialisable settings metadata per platform (code name, current values, schema hints). We can adapt the egui widgets by porting them to React forms.
- **UI approach:** Create a “Platform Settings” section that iterates over `get_platforms()` results (perhaps reusing the overview panel). Provide editors for common types (checkbox, text input) and call a new `update_platform_settings` command to persist to the config via `save_settings_with_sections`.
- **Notes:** Some platform views are OS-specific; guard them with `cfg` info so we only show applicable settings.

### Artwork management tools (Priority: High · Estimate: 8)
- **Goal:** Reintroduce the “Images” experience—browse SteamGridDB alternatives, ban assets, and run the “fix icons” action.
- **Backend:** Commands already exist (`download_images_for_users`, `fix_all_shortcut_icons`, ban logic in `SteamGridDbSettings`). Add Tauri commands for listing cached search results (`steamgriddb/cached_search.rs`), toggling bans, and invoking `fix_all_shortcut_icons`.
- **UI approach:** Build a dedicated “Artwork” view with:
  - A gallery listing for each imported shortcut (hero, icon, big picture).  
  - Buttons to pick alternate art, ban/unban, and trigger re-download.  
  - A banner to run the “Fix icons” maintenance task.
- **Implementation hints:** Inspect egui code under `src/ui/images/` for the flow; reuse `convertFileSrc` to preview local assets.

### Backup & disconnect flows (Priority: High · Estimate: 5)
- **Goal:** Match egui’s Backup and Disconnect panels.
- **Backend:** We already have reusable functions (`src/backups.rs`, `boilr_core::sync::disconnect_shortcut`, etc.). Expose new commands:
  - `list_backups` + `create_backup` + `restore_backup`.
  - `disconnect_boilr_shortcuts` (wraps existing sync helpers).
- **UI:** Add tabs mirroring egui: a list of backups with restore buttons, and a disconnect confirmation card explaining the impact.
- **Considerations:** Backups are per-user; surface user IDs in the list; ensure long-running tasks show progress.

### Platform enable/disable + quick actions (Priority: High · Estimate: 3)
- **Goal:** Let users toggle whole platforms directly in the Tauri UI.
- **Backend:** The enable flag is stored per platform (`GamesPlatform::enabled`). We already compute it inside `prepare_additions`. Add an `update_platform_enabled` field when saving platform sections.
- **UI:** On the overview page, add a toggle near the platform name. When switched off, call the settings update command with the appropriate serialised platform section so the change persists.
- **Quick actions:** Include a “Refresh now” and “Open platform settings” action button in each card for smoother workflows.

### Consistent platform error messaging (Priority: High · Estimate: 5)
- **Goal:** Present clear, user-friendly errors when a platform isn’t installed or misconfigured instead of raw OS messages.
- **Backend:** Normalise `GamesPlatform::get_shortcut_info` errors—wrap common `io::ErrorKind::NotFound`/`PermissionDenied` cases so consumers can distinguish “not installed” from unexpected failures. Consider adding a helper (e.g., `PlatformError::MissingDependency { hint }`) and update each platform module (Epic, Itch, Legendary, Lutris, etc.) accordingly.
- **UI:** Replace bland messages like “No such file or directory (os error 2)” with actionable hints (“Epic is not installed—install via Heroic or point BoilR at the manifest folder in Settings”). Suppress redundant “No games detected…” text when an error is shown.
- **References:** Example noisy cases observed on a machine without those launchers: Epic (“Manifests not found”), Flatpak/GOG/Legendary/Lutris (“No such file or directory (os error 2)”), Itch (“Path not found: ~/.config/itch/db/butler.db-wal”), Origin (“Default path not found”).

## Nice to have

### Blacklisted games manager (Priority: Medium · Estimate: 3)
- **Goal:** Provide a central list of blacklisted shortcuts with add/remove controls.
- **Approach:** Leverage `settings.blacklisted_games` (already exposed). Render a table with app ID + inferred platform + quick buttons to remove. Allow manual entry by pasting app IDs.
- **Follow-up:** Optionally expose search functionality to quickly blacklist from the discovered games list.

### Accessibility & keyboard support (Priority: Medium · Estimate: 5)
- **Tasks:** Audit interactive elements for ARIA labels, keyboard navigation, and focus outlines.  
- **Implementation:** Use `@headlessui/react` or custom keyboard handlers where necessary.  
- **Testing:** Use screen reader/lighthouse to validate.

### Telemetry / logging console (Priority: Medium · Estimate: 5)
- **Goal:** Show sync logs (platform warnings, filesystem errors) in the UI for debugging.
- **Backend:** Emit log entries via a Tauri event each time we `println!` or `eprintln!` in sync flows (or integrate `tracing` subscriber that forwards to the frontend).  
- **Frontend:** Add a collapsible “Logs” panel with streaming output.

### Sync log surface (Priority: Medium · Estimate: 2)
- **Goal:** Display the textual status updates we currently print to stdout.  
- **Approach:** Build on the telemetry console above; at minimum, show the last N log lines in the overview page so users see why a platform failed quickly.

### Artwork preview & editing (Priority: Medium · Estimate: 8)
- **Goal:** Preview the SteamGridDB artwork BoilR will apply for each shortcut (both planned imports and existing Steam entries) and allow users to change images before syncing.
- **Backend:** Extend the image helpers so Tauri commands can return candidate art (icon/hero/grid) for each `ShortcutToImport`, respecting user preferences (`prefer_animated`, `allow_nsfw`). For existing Steam shortcuts, surface the resolved grid paths or URLs so the frontend can display current art.
- **Frontend:** In `App.tsx`, replace the placeholder tiles with real art thumbnails via `convertFileSrc`. Add an “Edit artwork” action that opens a picker similar to the egui image workflow, allowing users to fetch alternates, ban images, or upload custom art. Persist choices using the existing ban/unban logic and rename map as needed.
- **Notes:** React can rely on the browser’s image decoding without the egui memory loader. Reuse the banning/fix helpers under `boilr_core::steamgriddb` to keep behaviour consistent.

## Later

### Document bundle size & optimisation ideas (Priority: Low · Estimate: 2)
- **Current state:** `target/release/boilr` ≈ 30 MB; `target/release/boilr-tauri` ≈ 20 MB but packaged bundle will land around 60–120 MB.  
- **Tasks:** Record these figures in README (build section) once packaging is automated.  
- **Optimisations:** Explore tree-shaking the frontend, trimming unused assets, or offering a “minimal” egui distribution.

### Cross-platform packaging validation (Priority: Low · Estimate: 3)
- **Goal:** Run `cargo tauri build` across Linux/macOS/Windows, document dependencies (GTK/WebKit, MSVC), and sanity-check the outputs.  
- **Suggested steps:** Add packaging notes to README/TODO once each platform build has been verified locally or in CI.

### CI / tooling enhancements (Priority: Low · Estimate: 5)
- **Objective:** Ensure both frontends build and test on every PR.  
- **Plan:** 
  - Add workflows that run `cargo test`, `cargo fmt`, `npm run build`, and `cargo tauri build --ci`.  
  - Cache `node_modules`/`cargo` to keep runtimes reasonable.

### Codebase TODO audit (Priority: Low · Estimate: 2)
- **Scope:** Search for remaining `TODO` markers (e.g., `src/platforms/minigalaxy/platform.rs`, `src/ui/images/ui_image_download.rs`).  
- **Action:** Decide whether to implement, convert to GitHub issues, or delete as obsolete.
