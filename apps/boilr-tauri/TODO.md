# Todo

## Parked: controllers in Steam Deck Game Mode (2026-09-27)

Arrow keys, Enter, Escape, Page Up/Down and gamepads work on a desktop (a virtual Xbox pad is
seen inside the Flatpak with `--device=input`). In Deck Game Mode the web view reports no
controllers although the window has focus. Next thing to try: the Deck's Flatpak may predate
`--device=input` (added in Flatpak 1.15.6); test with
`flatpak override --user --device=all io.github.philipk.boilr.Devel`. Touch works meanwhile.

## High priority

### Per-game selection & renaming (Priority: High · Estimate: 8)
- **Goal:** Match egui’s ability to include/exclude individual shortcuts and edit names before import.
- **Where to start:** Backend already exposes rename map helpers (`src/renames.rs`) and blacklisting happens in `apps/boilr-tauri/src/main.rs::prepare_additions`. Extend the plan/sync commands if extra data is needed (e.g., platform labels per shortcut).
- **UI approach:** In `apps/boilr-tauri/src/App.tsx`, add per-game checkboxes (backed by a local `Set` of excluded app IDs) and a rename modal or inline edit. Persist changes by updating `blacklisted_games` (for exclusions) via `update_settings` and writing to `renames.json` through a new Tauri command that mirrors egui’s `rename_map` logic.
- **Edge cases:** Provide a reset option to revert to stored names; ensure renames trigger `calculate_app_id_for_shortcut` exactly once (follow `prepare_additions`).

### Backup & disconnect flows (Priority: High · Estimate: 5)
- **Goal:** Match egui’s Backup and Disconnect panels.
- **Backend:** We already have reusable functions (`src/backups.rs`, `boilr_core::sync::disconnect_shortcut`, etc.). Expose new commands:
  - `list_backups` + `create_backup` + `restore_backup`.
  - `disconnect_boilr_shortcuts` (wraps existing sync helpers).
- **UI:** Add tabs mirroring egui: a list of backups with restore buttons, and a disconnect confirmation card explaining the impact.
- **Considerations:** Backups are per-user; surface user IDs in the list; ensure long-running tasks show progress.

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

### Artwork before import (Priority: Medium · Estimate: 5)
- **Done:** the Artwork tab (`src/artwork.rs`, `ArtworkView.tsx`) manages art for shortcuts already in Steam: browse SteamGridDB options per kind, replace, remove, never download, pick the right SteamGridDB game, download missing art.
- **Left:** show art on the Games tab for games not imported yet, and let users use an image from their own disk.

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
