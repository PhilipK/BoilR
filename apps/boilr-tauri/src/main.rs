#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{HashMap, HashSet};

use boilr::{
    backups::backup_shortcuts,
    platforms::{get_platforms, GamesPlatform, ShortcutToImport},
    renames::load_rename_map,
};
use boilr_core::{
    settings::{save_settings_with_sections, Settings},
    steam::{
        ensure_steam_started, ensure_steam_stopped, get_shortcuts_for_user, get_shortcuts_paths,
    },
    sync::{self, download_images, IsBoilRShortcut},
};
use serde::{Deserialize, Serialize};
use steam_shortcuts_util::{
    calculate_app_id_for_shortcut,
    shortcut::{Shortcut, ShortcutOwned},
};
use tokio::runtime::Runtime;

#[cfg(target_family = "unix")]
use boilr_core::{
    steam::setup_proton_games,
    sync::symlinks::{create_sym_links, ensure_links_folder_created},
};

#[tauri::command]
fn load_settings() -> Result<Settings, String> {
    Settings::new().map_err(|err| err.to_string())
}

#[tauri::command]
fn discover_games() -> Result<Vec<PlatformSummary>, String> {
    let settings = Settings::new().map_err(|err| err.to_string())?;
    let snapshots = gather_platform_snapshots();
    let rename_map = load_rename_map();

    let mut result = Vec::new();
    for snapshot in snapshots {
        let mut games = Vec::new();
        if let Some(shortcuts) = snapshot.shortcuts {
            for shortcut in shortcuts {
                let ShortcutToImport {
                    shortcut,
                    needs_proton,
                    needs_symlinks,
                } = shortcut;
                let app_id = shortcut.app_id;
                let display_name = rename_map
                    .get(&app_id)
                    .cloned()
                    .unwrap_or_else(|| shortcut.app_name.clone());
                let blacklisted = settings.blacklisted_games.contains(&app_id);
                games.push(ShortcutSummary {
                    app_id,
                    app_name: shortcut.app_name,
                    display_name,
                    exe: shortcut.exe,
                    start_dir: shortcut.start_dir,
                    icon: icon_to_option(&shortcut.icon),
                    needs_proton,
                    needs_symlinks,
                    blacklisted,
                });
            }
        }

        result.push(PlatformSummary {
            code_name: snapshot.code_name,
            name: snapshot.display_name,
            enabled: snapshot.enabled,
            games,
            error: snapshot.error,
        });
    }

    Ok(result)
}

#[tauri::command]
async fn run_full_sync() -> Result<SyncOutcome, String> {
    tauri::async_runtime::spawn_blocking(perform_full_sync)
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
fn plan_sync() -> Result<SyncPlan, String> {
    let settings = Settings::new().map_err(|err| err.to_string())?;
    let rename_map = load_rename_map();
    let snapshots = gather_platform_snapshots();

    let (additions, addition_ids) = prepare_additions(&snapshots, &settings, &rename_map);
    let removals = collect_removals(&settings, &addition_ids);

    Ok(SyncPlan {
        additions,
        removals,
    })
}

#[tauri::command]
fn update_settings(update: SettingsUpdate) -> Result<Settings, String> {
    let mut settings = Settings::new().map_err(|err| err.to_string())?;

    if let Some(steam) = update.steam {
        if let Some(value) = steam.stop_steam {
            settings.steam.stop_steam = value;
        }
        if let Some(value) = steam.start_steam {
            settings.steam.start_steam = value;
        }
        if let Some(value) = steam.create_collections {
            settings.steam.create_collections = value;
        }
        if let Some(value) = steam.optimize_for_big_picture {
            settings.steam.optimize_for_big_picture = value;
        }
        if let Some(location) = steam.location {
            settings.steam.location = location;
        }
    }

    if let Some(grid) = update.steamgrid_db {
        if let Some(value) = grid.enabled {
            settings.steamgrid_db.enabled = value;
        }
        if let Some(value) = grid.prefer_animated {
            settings.steamgrid_db.prefer_animated = value;
        }
        if let Some(value) = grid.allow_nsfw {
            settings.steamgrid_db.allow_nsfw = value;
        }
        if let Some(value) = grid.only_download_boilr_images {
            settings.steamgrid_db.only_download_boilr_images = value;
        }
        if let Some(value) = grid.auth_key {
            settings.steamgrid_db.auth_key = value;
        }
    }

    if let Some(blacklisted) = update.blacklisted_games {
        settings.blacklisted_games = blacklisted;
    }

    let platforms = get_platforms();
    let sections = collect_platform_sections(&platforms);
    save_settings_with_sections(&settings, &sections).map_err(|err| err.to_string())?;

    Ok(settings)
}

#[tauri::command]
fn ping() -> &'static str {
    "pong"
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            load_settings,
            discover_games,
            plan_sync,
            update_settings,
            run_full_sync,
            ping
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn perform_full_sync() -> Result<SyncOutcome, String> {
    let settings = Settings::new().map_err(|err| err.to_string())?;
    let rename_map = load_rename_map();

    let snapshots = gather_platform_snapshots();
    let mut platform_errors = Vec::new();
    let mut platform_shortcuts: Vec<(String, Vec<ShortcutToImport>)> = Vec::new();

    for snapshot in snapshots {
        if !snapshot.enabled {
            continue;
        }

        match (snapshot.shortcuts, snapshot.error) {
            (Some(shortcuts), _) => {
                platform_shortcuts.push((snapshot.display_name, shortcuts));
            }
            (None, Some(message)) => platform_errors.push(PlatformError {
                code_name: snapshot.code_name,
                name: snapshot.display_name,
                message,
            }),
            (None, None) => {
                platform_errors.push(PlatformError {
                    code_name: snapshot.code_name,
                    name: snapshot.display_name,
                    message: "Platform returned no data".to_string(),
                });
            }
        }
    }

    let imported_platforms = platform_shortcuts.len();
    let shortcuts_considered: usize = platform_shortcuts
        .iter()
        .map(|(_, entries)| entries.len())
        .sum();

    if imported_platforms == 0 {
        return Ok(SyncOutcome {
            imported_platforms,
            shortcuts_considered,
            steam_users_updated: 0,
            images_requested: false,
            platform_errors,
        });
    }

    if settings.steam.stop_steam {
        ensure_steam_stopped();
    }

    backup_shortcuts(&settings.steam);

    #[cfg(target_family = "unix")]
    prepare_proton(&platform_shortcuts);

    let import_games = to_shortcut_owned(platform_shortcuts);
    let mut sender = None;
    let users = sync::sync_shortcuts(&settings, &import_games, &mut sender, &rename_map)
        .map_err(|err| err.to_string())?;

    let images_requested = settings.steamgrid_db.enabled;
    if images_requested {
        match Runtime::new() {
            Ok(runtime) => runtime.block_on(async {
                download_images(&settings, &users, &mut sender).await;
            }),
            Err(err) => eprintln!("Failed to initialise async runtime: {err:?}"),
        }
    }

    if let Err(err) = sync::fix_all_shortcut_icons(&settings) {
        eprintln!("Could not fix shortcuts with error {err}");
    }

    if settings.steam.start_steam {
        ensure_steam_started(&settings.steam);
    }

    Ok(SyncOutcome {
        imported_platforms,
        shortcuts_considered,
        steam_users_updated: users.len(),
        images_requested,
        platform_errors,
    })
}

fn gather_platform_snapshots() -> Vec<PlatformSnapshot> {
    let mut snapshots = Vec::new();
    for platform in get_platforms() {
        let display_name = platform.name().to_string();
        let code_name = platform.code_name().to_string();
        let enabled = platform.enabled();

        if !enabled {
            snapshots.push(PlatformSnapshot {
                display_name,
                code_name,
                enabled,
                shortcuts: None,
                error: None,
            });
            continue;
        }

        match platform.get_shortcut_info() {
            Ok(shortcuts) => snapshots.push(PlatformSnapshot {
                display_name,
                code_name,
                enabled,
                shortcuts: Some(shortcuts),
                error: None,
            }),
            Err(err) => snapshots.push(PlatformSnapshot {
                display_name,
                code_name,
                enabled,
                shortcuts: None,
                error: Some(err.to_string()),
            }),
        }
    }

    snapshots
}

fn to_shortcut_owned(
    shortcuts_to_import: Vec<(String, Vec<ShortcutToImport>)>,
) -> Vec<(String, Vec<ShortcutOwned>)> {
    let mut import_games = Vec::new();
    for (name, entries) in shortcuts_to_import {
        let shortcuts = entries.into_iter().map(|entry| entry.shortcut).collect();
        import_games.push((name, shortcuts));
    }
    import_games
}

#[cfg(target_family = "unix")]
fn prepare_proton(shortcut_infos: &[(String, Vec<ShortcutToImport>)]) {
    let mut shortcuts_to_proton = Vec::new();

    for (name, shortcuts) in shortcut_infos {
        for shortcut_info in shortcuts {
            if shortcut_info.needs_proton {
                ensure_links_folder_created(name);
                shortcuts_to_proton.push(format!("{}", shortcut_info.shortcut.app_id));
            }

            if shortcut_info.needs_symlinks {
                create_sym_links(&shortcut_info.shortcut);
            }
        }

        if let Err(err) = setup_proton_games(&shortcuts_to_proton) {
            eprintln!("failed to save proton settings: {err:?}");
        }
    }
}

fn prepare_additions(
    snapshots: &[PlatformSnapshot],
    settings: &Settings,
    rename_map: &HashMap<u32, String>,
) -> (Vec<AdditionPlan>, HashSet<u32>) {
    let mut additions = Vec::new();
    let mut addition_ids = HashSet::new();

    for snapshot in snapshots {
        if !snapshot.enabled {
            continue;
        }

        let Some(shortcuts) = snapshot.shortcuts.as_ref() else {
            continue;
        };

        for entry in shortcuts {
            if settings.blacklisted_games.contains(&entry.shortcut.app_id) {
                continue;
            }

            let mut shortcut = entry.shortcut.clone();
            if let Some(rename) = rename_map.get(&entry.shortcut.app_id) {
                shortcut.app_name = rename.clone();
                let template = Shortcut::new(
                    "0",
                    shortcut.app_name.as_str(),
                    &shortcut.exe,
                    "",
                    "",
                    "",
                    "",
                );
                shortcut.app_id = calculate_app_id_for_shortcut(&template);
            }

            let display_name = shortcut.app_name.clone();
            let app_name = shortcut.app_name.clone();
            let exe = shortcut.exe.clone();
            let start_dir = shortcut.start_dir.clone();
            let icon = icon_to_option(&shortcut.icon);

            addition_ids.insert(shortcut.app_id);
            additions.push(AdditionPlan {
                platform: snapshot.display_name.clone(),
                platform_code: snapshot.code_name.clone(),
                needs_proton: entry.needs_proton,
                needs_symlinks: entry.needs_symlinks,
                shortcut: PlannedShortcut {
                    app_id: shortcut.app_id,
                    app_name,
                    display_name,
                    exe,
                    start_dir,
                    icon,
                },
            });
        }
    }

    (additions, addition_ids)
}

fn collect_removals(settings: &Settings, addition_ids: &HashSet<u32>) -> Vec<RemovalPlan> {
    let mut removals = Vec::new();
    let mut seen = HashSet::new();

    let users = match get_shortcuts_paths(&settings.steam) {
        Ok(users) => users,
        Err(err) => {
            eprintln!("Failed to inspect Steam shortcuts: {err:?}");
            Vec::new()
        }
    };

    for user in users {
        let shortcut_info = match get_shortcuts_for_user(&user) {
            Ok(info) => info,
            Err(err) => {
                eprintln!(
                    "Failed to load shortcuts for user {}: {err:?}",
                    user.user_id
                );
                continue;
            }
        };

        for shortcut in shortcut_info.shortcuts {
            let reason = if shortcut.is_boilr_shortcut() {
                Some(RemovalReason::LegacyBoilr)
            } else if addition_ids.contains(&shortcut.app_id) {
                Some(RemovalReason::DuplicateAppId)
            } else {
                None
            };

            let Some(reason) = reason else {
                continue;
            };

            let key = (user.user_id.clone(), shortcut.app_id, reason);
            if !seen.insert(key) {
                continue;
            }

            let app_id = shortcut.app_id;
            let app_name = shortcut.app_name.clone();
            let display_name = shortcut.app_name.clone();
            let exe = shortcut.exe.clone();
            let start_dir = shortcut.start_dir.clone();
            let icon = icon_to_option(&shortcut.icon);

            removals.push(RemovalPlan {
                user_id: user.user_id.clone(),
                steam_user_data_folder: user.steam_user_data_folder.clone(),
                reason,
                shortcut: RemovalShortcut {
                    app_id,
                    app_name,
                    display_name,
                    exe,
                    start_dir,
                    icon,
                },
            });
        }
    }

    removals
}

fn icon_to_option(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn collect_platform_sections(platforms: &[Box<dyn GamesPlatform>]) -> Vec<(String, String)> {
    platforms
        .iter()
        .map(|platform| {
            (
                platform.code_name().to_string(),
                platform.get_settings_serializable(),
            )
        })
        .collect()
}

struct PlatformSnapshot {
    display_name: String,
    code_name: String,
    enabled: bool,
    shortcuts: Option<Vec<ShortcutToImport>>,
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PlatformSummary {
    code_name: String,
    name: String,
    enabled: bool,
    games: Vec<ShortcutSummary>,
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ShortcutSummary {
    app_id: u32,
    app_name: String,
    display_name: String,
    exe: String,
    start_dir: String,
    icon: Option<String>,
    needs_proton: bool,
    needs_symlinks: bool,
    blacklisted: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct SyncPlan {
    additions: Vec<AdditionPlan>,
    removals: Vec<RemovalPlan>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AdditionPlan {
    platform: String,
    platform_code: String,
    needs_proton: bool,
    needs_symlinks: bool,
    shortcut: PlannedShortcut,
}

#[derive(Debug, Serialize, Deserialize)]
struct PlannedShortcut {
    app_id: u32,
    app_name: String,
    display_name: String,
    exe: String,
    start_dir: String,
    icon: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
enum RemovalReason {
    LegacyBoilr,
    DuplicateAppId,
}

#[derive(Debug, Serialize, Deserialize)]
struct RemovalPlan {
    user_id: String,
    steam_user_data_folder: String,
    reason: RemovalReason,
    shortcut: RemovalShortcut,
}

#[derive(Debug, Serialize, Deserialize)]
struct RemovalShortcut {
    app_id: u32,
    app_name: String,
    display_name: String,
    exe: String,
    start_dir: String,
    icon: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SettingsUpdate {
    steam: Option<SteamSettingsUpdate>,
    #[serde(rename = "steamgrid_db")]
    steamgrid_db: Option<SteamGridDbUpdate>,
    blacklisted_games: Option<Vec<u32>>,
}

#[derive(Debug, Deserialize)]
struct SteamSettingsUpdate {
    stop_steam: Option<bool>,
    start_steam: Option<bool>,
    create_collections: Option<bool>,
    optimize_for_big_picture: Option<bool>,
    location: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
struct SteamGridDbUpdate {
    enabled: Option<bool>,
    prefer_animated: Option<bool>,
    allow_nsfw: Option<bool>,
    only_download_boilr_images: Option<bool>,
    auth_key: Option<Option<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PlatformError {
    code_name: String,
    name: String,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SyncOutcome {
    imported_platforms: usize,
    shortcuts_considered: usize,
    steam_users_updated: usize,
    images_requested: bool,
    platform_errors: Vec<PlatformError>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::{
        ipc::CallbackFn,
        test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY},
        webview::InvokeRequest,
        WebviewWindowBuilder,
    };

    fn build_mock_webview() -> tauri::WebviewWindow<tauri::test::MockRuntime> {
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![
                load_settings,
                discover_games,
                plan_sync,
                update_settings,
                run_full_sync,
                ping
            ])
            .build(mock_context(noop_assets()))
            .expect("failed to build tauri app for tests");

        WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("failed to build mock webview")
    }

    fn invoke_request(cmd: &str) -> InvokeRequest {
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: Default::default(),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        }
    }

    #[test]
    fn ping_command_returns_pong() {
        let webview = build_mock_webview();
        let response = get_ipc_response(&webview, invoke_request("ping"))
            .expect("ping command should succeed");
        let value = response
            .deserialize::<String>()
            .expect("invalid ping payload");
        assert_eq!(value, "pong");
    }

    #[test]
    fn load_settings_command_round_trips() {
        let webview = build_mock_webview();
        let response = get_ipc_response(&webview, invoke_request("load_settings"))
            .expect("load_settings command should succeed");
        let _settings = response
            .deserialize::<Settings>()
            .expect("settings payload is well formed");
    }

    #[test]
    fn discover_games_command_returns_payload() {
        let webview = build_mock_webview();
        let response = get_ipc_response(&webview, invoke_request("discover_games"))
            .expect("discover_games command should succeed");
        let _payload = response
            .deserialize::<Vec<PlatformSummary>>()
            .expect("discover_games payload is valid JSON");
    }

    #[test]
    fn plan_sync_command_returns_payload() {
        let webview = build_mock_webview();
        let response = get_ipc_response(&webview, invoke_request("plan_sync"))
            .expect("plan_sync command should succeed");
        let _payload = response
            .deserialize::<SyncPlan>()
            .expect("plan_sync payload is valid JSON");
    }
}
