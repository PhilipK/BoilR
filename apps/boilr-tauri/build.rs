// Every command in `generate_handler!` must be listed here. Declaring the commands turns on
// Tauri's permission check for them, and `capabilities/main.json` grants them only to the
// app's own `main` window. Without it, any page loaded in any window (such as a store's
// sign-in page) could call every command.
const COMMANDS: &[&str] = &[
    "load_settings",
    "discover_games",
    "plan_sync",
    "update_settings",
    "list_platform_settings",
    "update_platform_settings",
    "update_platform_enabled",
    "run_full_sync",
    "list_backups",
    "create_backup",
    "restore_shortcuts",
    "list_boilr_shortcuts",
    "release_shortcut",
    "list_steam_accounts",
    "list_artwork",
    "artwork_options",
    "set_artwork",
    "clear_artwork",
    "artwork_game_match",
    "set_artwork_game",
    "find_missing_artwork",
    "list_store_accounts",
    "update_store_account",
    "refresh_store_library",
    "sign_out_store",
    "submit_store_login",
    "start_store_login",
    "open_store_sign_in_page",
    "rename_game",
    "ping",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
