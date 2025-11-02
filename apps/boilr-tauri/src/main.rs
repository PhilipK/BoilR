#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use boilr::{
    backups::backup_shortcuts,
    platforms::{get_platforms, ShortcutToImport},
    renames::load_rename_map,
};
use boilr_core::{
    settings::Settings,
    steam::{ensure_steam_started, ensure_steam_stopped},
    sync::{self, download_images},
};
use serde::{Deserialize, Serialize};
use steam_shortcuts_util::shortcut::ShortcutOwned;
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
fn ping() -> &'static str {
    "pong"
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            load_settings,
            discover_games,
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
    needs_proton: bool,
    needs_symlinks: bool,
    blacklisted: bool,
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
}
