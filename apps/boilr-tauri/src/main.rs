#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use boilr::{
    backups::backup_shortcuts,
    platforms::{get_platforms, GamesPlatform, ShortcutToImport},
    renames::load_rename_map,
};
use boilr_core::{
    settings::{load_setting_sections, save_settings_with_sections, Settings},
    steam::{
        ensure_steam_started, ensure_steam_stopped, get_shortcuts_for_user, get_shortcuts_paths,
    },
    sync::{self, download_images, IsBoilRShortcut, SyncProgress},
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use steam_shortcuts_util::{
    calculate_app_id_for_shortcut,
    shortcut::{Shortcut, ShortcutOwned},
};
use tauri::{Emitter, Manager};
use tokio::runtime::Runtime;
use toml::Value;

#[cfg(target_family = "unix")]
use boilr_core::{
    steam::setup_proton_games,
    sync::symlinks::{create_sym_links, ensure_links_folder_created},
};

#[derive(Clone)]
struct ProgressEmitter(Arc<dyn Fn(&str, ProgressPayload) + Send + Sync>);

impl ProgressEmitter {
    fn emit(&self, event: &str, payload: ProgressPayload) {
        (self.0)(event, payload);
    }
}

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
async fn run_full_sync(
    emitter_state: tauri::State<'_, ProgressEmitter>,
) -> Result<SyncOutcome, String> {
    use tokio::sync::watch;

    let emitter = emitter_state.inner().clone();
    let (progress_tx, mut progress_rx) = watch::channel(SyncProgress::NotStarted);

    emitter.emit(
        "sync-progress",
        progress_to_payload(&SyncProgress::NotStarted),
    );

    let emitter_for_events = emitter.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            match progress_rx.changed().await {
                Ok(_) => {
                    let progress = progress_rx.borrow().clone();
                    emitter_for_events.emit("sync-progress", progress_to_payload(&progress));

                    if matches!(progress, SyncProgress::Done) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    tauri::async_runtime::spawn_blocking(move || perform_full_sync(Some(progress_tx)))
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
fn update_platform_enabled(
    code_name: String,
    enabled: bool,
) -> Result<PlatformToggleResponse, String> {
    persist_platform_enabled(&code_name, enabled)?;

    let platforms = discover_games()?;
    let plan = plan_sync()?;

    Ok(PlatformToggleResponse { platforms, plan })
}

#[tauri::command]
fn ping() -> &'static str {
    "pong"
}

#[tauri::command]
fn list_platform_settings() -> Result<Vec<PlatformSettingsPayload>, String> {
    let platforms = get_platforms();
    platforms
        .into_iter()
        .map(|platform| platform_settings_payload(platform.as_ref()))
        .collect()
}

#[tauri::command]
fn update_platform_settings(
    code_name: String,
    settings: JsonValue,
) -> Result<PlatformSettingsPayload, String> {
    let table = json_to_toml_table(settings)?;
    persist_platform_table(&code_name, table)?;

    let platforms = get_platforms();
    let platform = platforms
        .into_iter()
        .find(|platform| platform.code_name() == code_name)
        .ok_or_else(|| format!("Platform {code_name} not found after updating settings"))?;
    platform_settings_payload(platform.as_ref())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            app.manage(ProgressEmitter(Arc::new(move |event, payload| {
                if let Err(err) = handle.emit(event, payload.clone()) {
                    eprintln!("Failed to emit {event} event: {err}");
                }
            })));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_settings,
            discover_games,
            plan_sync,
            update_settings,
            list_platform_settings,
            update_platform_settings,
            update_platform_enabled,
            run_full_sync,
            ping
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn perform_full_sync(
    progress: Option<tokio::sync::watch::Sender<SyncProgress>>,
) -> Result<SyncOutcome, String> {
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

    if let Some(sender) = progress.as_ref() {
        let _ = sender.send(SyncProgress::Starting);
    }

    if imported_platforms == 0 {
        if let Some(sender) = progress.as_ref() {
            let _ = sender.send(SyncProgress::Done);
        }
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
    let mut progress_for_sync = progress.clone();
    let users = sync::sync_shortcuts(
        &settings,
        &import_games,
        &mut progress_for_sync,
        &rename_map,
    )
    .map_err(|err| err.to_string())?;

    let images_requested = settings.steamgrid_db.enabled;
    if images_requested {
        match Runtime::new() {
            Ok(runtime) => {
                let mut progress_for_images = progress.clone();
                runtime.block_on(async {
                    download_images(&settings, &users, &mut progress_for_images).await;
                })
            }
            Err(err) => eprintln!("Failed to initialise async runtime: {err:?}"),
        }
    }

    if let Err(err) = sync::fix_all_shortcut_icons(&settings) {
        eprintln!("Could not fix shortcuts with error {err}");
    }

    if settings.steam.start_steam {
        ensure_steam_started(&settings.steam);
    }

    if let Some(sender) = progress.as_ref() {
        let _ = sender.send(SyncProgress::Done);
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

fn load_platform_table(code_name: &str) -> Result<toml::value::Table, String> {
    let sections = match load_setting_sections() {
        Ok(data) => data,
        Err(err) => {
            eprintln!("Falling back to defaults while loading {code_name} settings: {err:?}");
            HashMap::new()
        }
    };

    let default_table = default_settings_table(code_name);

    let mut table: toml::value::Table = match sections.get(code_name) {
        Some(serialized) if !serialized.trim().is_empty() => match toml::from_str(serialized) {
            Ok(parsed) => parsed,
            Err(err) => {
                eprintln!(
                    "Failed to parse existing settings for {code_name}, reinitialising from defaults: {err}"
                );
                default_table
                    .clone()
                    .unwrap_or_else(toml::value::Table::new)
            }
        },
        _ => default_table
            .clone()
            .unwrap_or_else(toml::value::Table::new),
    };

    if table.is_empty() {
        if let Some(defaults) = default_table {
            table = defaults;
        }
    }

    Ok(table)
}

fn persist_platform_table(code_name: &str, table: toml::value::Table) -> Result<(), String> {
    let settings = Settings::new().map_err(|err| err.to_string())?;
    let mut sections = match load_setting_sections() {
        Ok(data) => data,
        Err(err) => {
            eprintln!("Falling back to defaults while updating {code_name} settings: {err:?}");
            HashMap::new()
        }
    };

    let serialized = toml::to_string(&table)
        .map_err(|err| format!("Could not serialise settings for {code_name}: {err}"))?;
    sections.insert(code_name.to_string(), serialized);

    save_platform_sections(&settings, sections)
}

fn save_platform_sections(
    settings: &Settings,
    sections: HashMap<String, String>,
) -> Result<(), String> {
    let mut platform_sections: Vec<(String, String)> = sections.into_iter().collect();
    platform_sections.sort_by(|a, b| a.0.cmp(&b.0));
    save_settings_with_sections(settings, &platform_sections).map_err(|err| err.to_string())
}

fn json_to_toml_table(settings: JsonValue) -> Result<toml::value::Table, String> {
    let value = toml::Value::try_from(settings)
        .map_err(|err| format!("Could not convert settings payload to TOML: {err}"))?;
    match value {
        toml::Value::Table(table) => Ok(table),
        other => Err(format!(
            "Platform settings payload must be a table, found {other:?}"
        )),
    }
}

fn platform_settings_payload(
    platform: &dyn GamesPlatform,
) -> Result<PlatformSettingsPayload, String> {
    let serialized = platform.get_settings_serializable();
    let table = match toml::from_str::<toml::Value>(&serialized) {
        Ok(toml::Value::Table(table)) => table,
        Ok(_) => toml::value::Table::new(),
        Err(err) => {
            eprintln!(
                "Failed to parse settings for {}: {err}",
                platform.code_name()
            );
            toml::value::Table::new()
        }
    };

    let json = serde_json::to_value(&table).map_err(|err| {
        format!(
            "Could not serialise settings for {}: {err}",
            platform.code_name()
        )
    })?;

    Ok(PlatformSettingsPayload {
        code_name: platform.code_name().to_string(),
        name: platform.name().to_string(),
        settings: json,
    })
}

fn persist_platform_enabled(code_name: &str, enabled: bool) -> Result<(), String> {
    let mut table = load_platform_table(code_name)?;
    table.insert("enabled".to_string(), Value::Boolean(enabled));
    persist_platform_table(code_name, table)
}

fn default_settings_table(code_name: &str) -> Option<toml::value::Table> {
    let defaults = get_platforms()
        .into_iter()
        .find(|platform| platform.code_name() == code_name)?
        .get_settings_serializable();
    toml::from_str(&defaults).ok()
}

#[derive(Clone, Serialize)]
struct ProgressPayload {
    state: &'static str,
    games_found: Option<usize>,
    to_download: Option<usize>,
}

fn progress_to_payload(progress: &SyncProgress) -> ProgressPayload {
    match progress {
        SyncProgress::NotStarted => ProgressPayload {
            state: "not_started",
            games_found: None,
            to_download: None,
        },
        SyncProgress::Starting => ProgressPayload {
            state: "starting",
            games_found: None,
            to_download: None,
        },
        SyncProgress::FoundGames { games_found } => ProgressPayload {
            state: "found_games",
            games_found: Some(*games_found),
            to_download: None,
        },
        SyncProgress::FindingImages => ProgressPayload {
            state: "finding_images",
            games_found: None,
            to_download: None,
        },
        SyncProgress::DownloadingImages { to_download } => ProgressPayload {
            state: "downloading_images",
            games_found: None,
            to_download: Some(*to_download),
        },
        SyncProgress::Done => ProgressPayload {
            state: "done",
            games_found: None,
            to_download: None,
        },
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
struct PlatformToggleResponse {
    platforms: Vec<PlatformSummary>,
    plan: SyncPlan,
}

#[derive(Debug, Serialize, Deserialize)]
struct PlatformSettingsPayload {
    code_name: String,
    name: String,
    settings: JsonValue,
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
    use std::sync::{Mutex, OnceLock};
    use tauri::{
        ipc::CallbackFn,
        test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY},
        webview::InvokeRequest,
        WebviewWindowBuilder,
    };

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("test lock poisoned")
    }

    fn build_mock_webview() -> tauri::WebviewWindow<tauri::test::MockRuntime> {
        let app = mock_builder()
            .setup(|app| {
                app.manage(ProgressEmitter(Arc::new(|_, _| {})));
                Ok(())
            })
            .invoke_handler(tauri::generate_handler![
                load_settings,
                discover_games,
                plan_sync,
                update_settings,
                list_platform_settings,
                update_platform_settings,
                update_platform_enabled,
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

    #[test]
    fn list_platform_settings_command_returns_payload() {
        let webview = build_mock_webview();
        let response = get_ipc_response(&webview, invoke_request("list_platform_settings"))
            .expect("list_platform_settings command should succeed");
        let payload = response
            .deserialize::<Vec<PlatformSettingsPayload>>()
            .expect("list_platform_settings payload is valid JSON");
        assert!(!payload.is_empty());
    }

    #[test]
    fn update_platform_enabled_persists_config_flag() {
        let _guard = test_lock();
        use std::fs;

        let workspace_config = std::env::current_dir()
            .expect("cwd")
            .join("target/test-config");
        let _ = fs::remove_dir_all(&workspace_config);
        fs::create_dir_all(&workspace_config).expect("temp config dir");

        let previous_override = std::env::var_os("BOILR_CONFIG_HOME");
        let previous_xdg = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("BOILR_CONFIG_HOME", &workspace_config);
        std::env::set_var("XDG_CONFIG_HOME", &workspace_config);

        // Ensure a config file exists on disk.
        let settings = Settings::new().expect("settings should load");
        save_settings_with_sections(&settings, &[]).expect("write initial config");

        let config_path = boilr_core::config::get_config_file();
        let original = fs::read_to_string(&config_path).unwrap_or_default();

        let target_code = "epic_games";
        let sections_before =
            load_setting_sections().expect("platform sections should load before toggle");
        let was_enabled = sections_before
            .get(target_code)
            .map(|serialized| serialized.contains("enabled = true"))
            .unwrap_or(true);

        let should_enable = !was_enabled;
        let response = update_platform_enabled(target_code.to_string(), should_enable)
            .expect("toggle command should succeed");

        // Snapshot returned to the frontend should reflect the new state.
        let epic_snapshot = response
            .platforms
            .into_iter()
            .find(|platform| platform.code_name == target_code)
            .expect("toggled platform appears in response");
        assert_eq!(
            epic_snapshot.enabled, should_enable,
            "frontend payload should mirror persisted state"
        );

        let sections_after =
            load_setting_sections().expect("platform sections should load after toggle");
        let serialized = sections_after
            .get(target_code)
            .expect("toggled platform section should exist");
        assert!(
            serialized.contains(if should_enable {
                "enabled = true"
            } else {
                "enabled = false"
            }),
            "config should contain updated enabled flag"
        );

        // Restore original config so other tests/developers keep their preferences.
        fs::write(&config_path, original).expect("restoring config failed");
        if previous_override
            .as_ref()
            .map(|s| s.is_empty())
            .unwrap_or(false)
        {
            std::env::remove_var("BOILR_CONFIG_HOME");
        } else if let Some(value) = previous_override {
            std::env::set_var("BOILR_CONFIG_HOME", value);
        } else {
            std::env::remove_var("BOILR_CONFIG_HOME");
        }

        if previous_xdg.as_ref().map(|s| s.is_empty()).unwrap_or(false) {
            std::env::remove_var("XDG_CONFIG_HOME");
        } else if let Some(value) = previous_xdg {
            std::env::set_var("XDG_CONFIG_HOME", value);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        let _ = fs::remove_dir_all(&workspace_config);
    }

    #[test]
    fn update_platform_enabled_creates_config_if_missing() {
        let _guard = test_lock();
        use std::fs;

        let workspace_config = std::env::current_dir()
            .expect("cwd")
            .join("target/test-config-missing");
        let _ = fs::remove_dir_all(&workspace_config);
        fs::create_dir_all(&workspace_config).expect("temp config dir");

        let previous_override = std::env::var_os("BOILR_CONFIG_HOME");
        let previous_xdg = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("BOILR_CONFIG_HOME", &workspace_config);
        std::env::set_var("XDG_CONFIG_HOME", &workspace_config);

        // Ensure config file truly missing
        let config_path = boilr_core::config::get_config_file();
        if config_path.exists() {
            fs::remove_file(&config_path).expect("remove existing config");
        }

        let response =
            update_platform_enabled("epic_games".to_string(), false).expect("toggle should work");

        let epic_snapshot = response
            .platforms
            .into_iter()
            .find(|platform| platform.code_name == "epic_games")
            .expect("platform exists in response");
        assert!(!epic_snapshot.enabled);

        let sections = load_setting_sections().expect("platform sections should load");
        let serialized = sections
            .get("epic_games")
            .expect("epic section should exist");
        assert!(
            serialized.contains("enabled = false"),
            "config should persist disabled flag"
        );
        assert!(
            serialized.contains("safe_launch"),
            "default fields should be preserved"
        );

        if previous_override
            .as_ref()
            .map(|s| s.is_empty())
            .unwrap_or(false)
        {
            std::env::remove_var("BOILR_CONFIG_HOME");
        } else if let Some(value) = previous_override {
            std::env::set_var("BOILR_CONFIG_HOME", value);
        } else {
            std::env::remove_var("BOILR_CONFIG_HOME");
        }

        if previous_xdg.as_ref().map(|s| s.is_empty()).unwrap_or(false) {
            std::env::remove_var("XDG_CONFIG_HOME");
        } else if let Some(value) = previous_xdg {
            std::env::set_var("XDG_CONFIG_HOME", value);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        let _ = fs::remove_dir_all(&workspace_config);
    }

    #[test]
    fn update_platform_settings_updates_section() {
        let _guard = test_lock();
        use serde_json::json;
        use std::fs;

        let workspace_config = std::env::current_dir()
            .expect("cwd")
            .join("target/test-config-platform-settings");
        let _ = fs::remove_dir_all(&workspace_config);
        fs::create_dir_all(&workspace_config).expect("temp config dir");

        let previous_override = std::env::var_os("BOILR_CONFIG_HOME");
        let previous_xdg = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("BOILR_CONFIG_HOME", &workspace_config);
        std::env::set_var("XDG_CONFIG_HOME", &workspace_config);

        let settings = Settings::new().expect("settings should load");
        save_settings_with_sections(&settings, &[]).expect("write initial config");

        let response = update_platform_settings(
            "epic_games".to_string(),
            json!({
                "enabled": false,
                "safe_launch": ["ExampleGame"]
            }),
        )
        .expect("platform settings should update");

        assert_eq!(response.code_name, "epic_games");

        let sections = load_setting_sections().expect("sections load after update");
        let epic_section = sections
            .get("epic_games")
            .expect("epic settings should exist after update");
        assert!(epic_section.contains("enabled = false"));
        assert!(epic_section.contains("safe_launch = [\"ExampleGame\"]"));

        if previous_override
            .as_ref()
            .map(|s| s.is_empty())
            .unwrap_or(false)
        {
            std::env::remove_var("BOILR_CONFIG_HOME");
        } else if let Some(value) = previous_override {
            std::env::set_var("BOILR_CONFIG_HOME", value);
        } else {
            std::env::remove_var("BOILR_CONFIG_HOME");
        }

        if previous_xdg.as_ref().map(|s| s.is_empty()).unwrap_or(false) {
            std::env::remove_var("XDG_CONFIG_HOME");
        } else if let Some(value) = previous_xdg {
            std::env::set_var("XDG_CONFIG_HOME", value);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        let _ = fs::remove_dir_all(&workspace_config);
    }
}
