# BoilR Development Notes

## Project Overview

BoilR is a Rust utility that discovers games from various gaming platforms (Epic, GOG, itch.io, etc.) and adds them as non-Steam shortcuts to Steam, with optional artwork from SteamGridDB.

**Version**: 1.9.6
**Language**: Rust 2021 Edition
**UI Framework**: egui/eframe
**Async Runtime**: tokio

## Build & Run

```bash
# Build
cargo build --release

# Run GUI mode
cargo run --release

# Run headless/CLI mode
cargo run --release -- --no-ui

# With fullscreen
cargo run --release -- --fullscreen
```

## Configuration Paths

| Platform | Config Folder | Log File |
|----------|---------------|----------|
| Windows | `%APPDATA%\boilr\` | `%APPDATA%\boilr\boilr.log` |
| Linux | `~/.config/boilr/` | `~/.config/boilr/boilr.log` |
| macOS | `~/.config/boilr/` | `~/.config/boilr/boilr.log` |

**Key Files:**
- `config.toml` - Main configuration
- `renames.json` - Game rename mappings
- `cache.json` - SteamGridDB search cache
- `boilr.log` - Application log (added in this update)

## Architecture

```
src/
├── main.rs                 # Entry point
├── logging.rs              # Logging infrastructure (tracing)
├── single_instance.rs      # Single-instance enforcement (lock file + PID)
├── config.rs               # Path helpers
├── settings.rs             # TOML config management
├── platforms/              # Game platform implementations
│   ├── platform.rs         # GamesPlatform trait
│   ├── egs/                # Epic Games Store
│   ├── gog/                # GOG Galaxy
│   ├── itch/               # itch.io
│   ├── amazon/             # Amazon Games (Windows)
│   ├── origin/             # EA Origin
│   ├── uplay/              # Ubisoft Connect
│   ├── heroic/             # Heroic Launcher (Linux)
│   ├── legendary/          # Legendary (Linux)
│   ├── lutris/             # Lutris (Linux)
│   ├── bottles/            # Bottles (Linux)
│   ├── flatpak/            # Flatpak apps (Linux)
│   ├── minigalaxy/         # MiniGalaxy (Linux)
│   ├── playnite/           # Playnite (Windows)
│   └── gamepass/           # Xbox Game Pass (Windows)
├── steam/                  # Steam integration
│   ├── utils.rs            # Path finding, shortcuts
│   ├── collections.rs      # Steam collections (LevelDB)
│   └── restarter.rs        # Steam process management
├── steamgriddb/            # SteamGridDB artwork
│   ├── downloader.rs       # Image downloading
│   └── cached_search.rs    # Search caching
├── sync/                   # Synchronization logic
│   └── synchronization.rs  # Main sync orchestration
└── ui/                     # egui UI
    ├── uiapp.rs            # Main app struct
    ├── ui_import_games.rs  # Import tab
    ├── ui_settings.rs      # Settings tab
    └── images/             # Images tab
```

## Known Issues & Bugs

### Critical

1. ~~**Permissions / Error Feedback (Windows)**~~ (FIXED)
   - Analysis confirmed admin is NOT required for normal operation
   - All registry access is READ-only (Uplay, Epic, Origin)
   - Steam userdata folder is typically writable by current user
   - **Fix**: Added `SyncProgress::Error` to surface permission errors in UI

2. ~~**Images Tab "Freak Out"**~~ (FIXED)
   - **Fixes applied**:
     - Converted blocking `block_on()` calls to async with background spawning
     - Fixed Windows file path backslashes in `file://` URLs
     - Removed blocking `thread::sleep(100ms)` call
     - Added proper loading states for async operations

### Moderate

3. ~~**Silent Error Handling**~~ (PARTIALLY FIXED)
   - **Fixed**: Sync/shortcut saving now reports errors to user via UI
   - **Remaining**: Some `.unwrap_or_default()` and `.ok()` patterns still exist in other areas

4. **Steam Process Management**
   - Uses SIGKILL without proper wait verification
   - Could cause data loss if Steam is writing
   - **Location**: `steam/restarter.rs`

5. **Image Cache Growth**
   - No cleanup strategy for cached images/thumbnails
   - Cache can grow indefinitely
   - **Location**: `config::get_thumbnails_folder()`

6. **SteamGridDB Rate Limiting**
   - Fixed 10-concurrent-request limit
   - No backpressure or retry logic
   - Network errors silently ignored

### Minor

7. **TODO Comments** (incomplete features)
   - `platforms/minigalaxy/platform.rs` - Detection incomplete
   - `steam/proton_vdf_util.rs` - Error handling question

8. ~~**Typo in Function Name**~~ (FIXED)
   - ~~`config.rs:44` - `get_backups_flder()` should be `get_backups_folder()`~~

## Roadmap

### Phase 1: Stability & Diagnostics (Current)

- [x] Implement proper logging with `tracing`
  - File output to `boilr.log`
  - Console output for development
  - Structured logging with context
- [x] Add logging to sync/import process
- [x] Single instance enforcement (prevents multiple BoilR instances)
- [x] Fix Windows file:// URL path handling (backslashes)
- [x] Investigate admin privilege requirements (confirmed NOT needed)
- [x] Fix `block_on()` usage in async context (converted UI-blocking calls to async)
- [x] Add proper permission error handling (surface errors to user via SyncProgress::Error)
- [x] Fix typo: `get_backups_flder` -> `get_backups_folder`
- [x] Remove duplicate `get_log_file` function
- [x] Fix unnecessary parentheses in sync/synchronization.rs

### Phase 2: Error Handling

- [ ] Replace `.unwrap_or_default()` with proper error propagation
- [ ] Add user-visible error messages in UI
- [ ] Implement retry logic for network operations
- [ ] Add timeout handling for external commands

### Phase 3: UX Improvements

- [ ] Add progress indicators for long operations
- [ ] Implement proper loading states in Images tab
- [ ] Add SteamGridDB API key validation
- [ ] Show log file location in UI

### Phase 4: Modernization (Optional)

- [ ] Consider migrating from `block_on` to proper async channels
- [ ] Implement image cache cleanup
- [ ] Add configuration validation
- [ ] Consider breaking up monolithic config.toml

## Key Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `egui` | 0.29.1 | UI framework |
| `eframe` | 0.29.1 | Window management |
| `tokio` | 1.41.0 | Async runtime |
| `steam_shortcuts_util` | 1.1.8 | VDF shortcut format |
| `steamgriddb_api` | 0.3.1 | SteamGridDB client |
| `tracing` | 0.1 | Logging (newly added) |
| `tracing-subscriber` | 0.3 | Log output |
| `tracing-appender` | 0.2 | File logging |
| `sysinfo` | 0.32 | Process detection for single-instance (newly added) |

## Platform Support Matrix

| Platform | Windows | Linux | Detection Method |
|----------|---------|-------|------------------|
| Epic Games Store | Yes | Yes* | Manifest JSON files |
| GOG Galaxy | Yes | No | config.json parsing |
| itch.io | Yes | Yes | butler.db binary parsing |
| Origin/EA | Yes | Yes* | Registry / Proton |
| Ubisoft Connect | Yes | Yes* | Registry / Proton |
| Amazon Games | Yes | No | SQLite database |
| Playnite | Yes | No | SQL database |
| Game Pass | Yes | No | PowerShell script |
| Legendary | No | Yes | CLI execution |
| Heroic | No | Yes | JSON config |
| Lutris | No | Yes | CLI execution |
| Bottles | No | Yes | Flatpak CLI |
| Flatpak | No | Yes | flatpak list |
| MiniGalaxy | No | Yes | Folder scanning |

*Requires Proton compatibility layer

## Debugging

### Enable Debug Logging

Set environment variable before running:
```bash
RUST_LOG=debug cargo run
# or for even more detail:
RUST_LOG=boilr=trace cargo run
```

### Check Log File

Windows:
```
%APPDATA%\boilr\boilr.log
```

Linux/macOS:
```
~/.config/boilr/boilr.log
```

### Common Issues

1. **"Could not find Steam user location"**
   - Check if Steam is installed
   - Verify Steam location in settings
   - Check log for specific error

2. **No games discovered**
   - Verify platform is enabled in settings
   - Check if games are actually installed
   - Look for platform-specific errors in log

3. **Images not loading**
   - Verify SteamGridDB API key is set
   - Check network connectivity
   - Look for API errors in log

## Notes

- The codebase uses strict clippy lints (no unwrap, no indexing, no panic)
- Cross-platform support via conditional compilation (`#[cfg(windows)]`, etc.)
- egui uses immediate-mode rendering (redraws every frame when active)
- SteamGridDB requires free API key from https://www.steamgriddb.com/profile/preferences/api
