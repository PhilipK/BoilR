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

### Compile-time warnings cleanup (Priority: High · Estimate: 2)
- **Goal:** Resolve the persistent warnings that surface on every build/test (unused imports, dead code, lifetime hints) to keep CI noise low.
- **Where to look:** 
  - `crates/boilr-core/src/sync/synchronization.rs` (unused parentheses)  
  - `src/platforms/*` dead-code warnings (e.g., `SteamFolderNotFound`, `SteamUsersDataEmpty`, `HeroicGameType::title`)  
  - `src/ui/images/possible_image.rs` (`thumbnail_path`) and `src/platforms/uplay/platform.rs` lifetime hints.
- **Plan:** Either address the warnings (e.g., remove unused types) or document why they must remain (conditionally compiled paths) and silence them with `#[allow]` alongside a comment.

### Noise-free platform discovery (Priority: High · Estimate: 3)
- **Goal:** Remove the repeated log spam about “Unknown platform named amazon/playnite/gamepass” during startup.
- **Likely cause:** Tauri bootstrap loads all platform sections but we now build only UNIX or Windows subsets—`load_platform` returns an error when a platform is behind a `cfg` and not compiled in.
- **Approach:** 
  - Adjust `load_platform` so it silently skips sections for platforms that aren’t compiled for the current OS.  
  - Alternatively prune those sections when we serialise settings (e.g., remove them from `collect_platform_sections` if the platform isn’t available).  
  - Update logging to use `debug!` instead of `eprintln!` if we intentionally skip optional sections.

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
