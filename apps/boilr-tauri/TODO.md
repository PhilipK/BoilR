# Todo

## Parked: controllers in Steam Deck Game Mode (2026-09-27)

Arrow keys, Enter, Escape, Page Up/Down and gamepads work on a desktop (a virtual Xbox pad is
seen inside the Flatpak with `--device=input`). In Deck Game Mode the web view reports no
controllers although the window has focus. Next thing to try: the Deck's Flatpak may predate
`--device=input` (added in Flatpak 1.15.6); test with
`flatpak override --user --device=all io.github.philipk.boilr.Devel`. Touch works meanwhile.

## Parity with egui

Reached once #538 (artwork), #539 (renaming), #540 (Windows CI) and #541 (Bottles "not found")
are merged: game selection, renaming, backups and hand-over, artwork, settings, and plain-words
launcher errors. What's below goes beyond egui.

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
