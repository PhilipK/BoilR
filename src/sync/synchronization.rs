use eframe::epaint::ahash::HashSet;
use steam_shortcuts_util::{
    calculate_app_id_for_shortcut, shortcut::ShortcutOwned, shortcuts_to_bytes, Shortcut,
};
use tokio::sync::watch::Sender;
use tracing::{debug, error, info, warn};

use crate::{
    platforms::{GamesPlatform, ShortcutToImport},
    settings::Settings,
    steam::{
        get_shortcuts_for_user, get_shortcuts_paths, write_collections, Collection, ShortcutInfo,
        SteamUsersInfo,
    },
    steamgriddb::{download_images_for_users, ImageType},
};

use std::{collections::HashMap, error::Error};

use std::{fs::File, io::Write, path::Path};

pub const BOILR_TAG: &str = "boilr";

pub enum SyncProgress {
    NotStarted,
    Starting,
    FoundGames { games_found: usize },
    FindingImages,
    DownloadingImages { to_download: usize },
    Done,
}

pub fn disconnect_shortcut(settings: &Settings, app_id: u32) -> Result<(), String> {
    let mut userinfo_shortcuts = get_shortcuts_paths(&settings.steam)
        .map_err(|e| format!("Getting shortcut paths failed: {e}"))?;

    for user in userinfo_shortcuts.iter_mut() {
        let shortcut_info = get_shortcuts_for_user(user);
        if let Ok(mut shortcut_info) = shortcut_info {
            for shortcut in shortcut_info.shortcuts.iter_mut() {
                if shortcut.app_id == app_id {
                    shortcut.dev_kit_game_id = "".to_string();
                    shortcut.tags.retain(|s| s != BOILR_TAG);
                }
            }
            save_shortcuts(&shortcut_info.shortcuts, Path::new(&shortcut_info.path));
        }
    }

    Ok(())
}

pub fn sync_shortcuts(
    settings: &Settings,
    platform_shortcuts: &[(String, Vec<ShortcutOwned>)],
    sender: &mut Option<Sender<SyncProgress>>,
    renames: &HashMap<u32, String>,
) -> eyre::Result<Vec<SteamUsersInfo>> {
    info!("Starting shortcut synchronization");

    let mut userinfo_shortcuts = get_shortcuts_paths(&settings.steam)?;
    info!(user_count = userinfo_shortcuts.len(), "Found Steam users");

    let mut all_shortcuts: Vec<ShortcutOwned> = platform_shortcuts
        .iter()
        .flat_map(|s| s.1.clone())
        .filter(|s| !settings.blacklisted_games.contains(&s.app_id))
        .collect();

    info!(shortcut_count = all_shortcuts.len(), "Total shortcuts to import (after blacklist filter)");

    for shortcut in &mut all_shortcuts {
        shortcut.dev_kit_game_id = BOILR_TAG.to_string();
    }

    if let Some(sender) = &sender {
        let _ = sender.send(SyncProgress::FoundGames {
            games_found: all_shortcuts.len(),
        });
    }

    for shortcut in &mut all_shortcuts {
        if let Some(rename) = renames.get(&shortcut.app_id) {
            debug!(app_id = shortcut.app_id, old_name = %shortcut.app_name, new_name = %rename, "Renaming shortcut");
            shortcut.app_name = rename.clone();
            let new_shortcut = Shortcut::new(
                "0",
                shortcut.app_name.as_str(),
                &shortcut.exe,
                "",
                "",
                "",
                "",
            );
            shortcut.app_id = calculate_app_id_for_shortcut(&new_shortcut);
        }
        debug!(app_id = shortcut.app_id, app_name = %shortcut.app_name, "Processing shortcut");
    }

    let ok_shorcuts = userinfo_shortcuts.iter_mut().filter_map(|user|{
        let shortcut_info = get_shortcuts_for_user(user).ok();
        shortcut_info.map(|shortcut_info| {
            (user,shortcut_info)
        })
    });

    for (user, mut shortcut_info) in ok_shorcuts {
        let start_time = std::time::Instant::now();
        info!(
            user_id = %user.user_id,
            existing_shortcuts = shortcut_info.shortcuts.len(),
            "Processing user"
        );

        let before_count = shortcut_info.shortcuts.len();
        remove_old_shortcuts(&mut shortcut_info);
        let after_remove_old = shortcut_info.shortcuts.len();
        debug!(removed = before_count - after_remove_old, "Removed old BoilR shortcuts");

        remove_shortcuts_with_same_appid(&mut shortcut_info, &all_shortcuts);
        let after_remove_dupes = shortcut_info.shortcuts.len();
        debug!(removed = after_remove_old - after_remove_dupes, "Removed duplicate app IDs");

        shortcut_info.shortcuts.extend(all_shortcuts.clone());
        info!(
            user_id = %user.user_id,
            total_shortcuts = shortcut_info.shortcuts.len(),
            "Saving shortcuts"
        );

        save_shortcuts(&shortcut_info.shortcuts, Path::new(&shortcut_info.path));

        if settings.steam.create_collections {
            match write_shortcut_collections(&user.user_id, platform_shortcuts) {
                Ok(_) => info!(user_id = %user.user_id, "Collections written successfully"),
                Err(e) => warn!(user_id = %user.user_id, error = %e, "Could not write collections - make sure Steam is shut down"),
            }
        }

        let duration = start_time.elapsed();
        info!(user_id = %user.user_id, duration_ms = duration.as_millis(), "Finished synchronizing user");
    }

    info!("Shortcut synchronization complete");
    Ok(userinfo_shortcuts)
}

pub async fn download_images(
    settings: &Settings,
    userinfo_shortcuts: &[SteamUsersInfo],
    sender: &mut Option<Sender<SyncProgress>>,
) {
    if settings.steamgrid_db.enabled {
        info!("Starting image download from SteamGridDB");
        download_images_for_users(settings, userinfo_shortcuts, sender).await;
        if settings.steamgrid_db.prefer_animated {
            debug!("Running second pass for non-animated images");
            let mut set = settings.clone();
            set.steamgrid_db.prefer_animated = false;
            download_images_for_users(&set, userinfo_shortcuts, sender).await;
        }
        info!("Image download complete");
    } else {
        debug!("SteamGridDB disabled, skipping image download");
    }
}

pub trait IsBoilRShortcut {
    fn is_boilr_shortcut(&self) -> bool;
}

impl IsBoilRShortcut for ShortcutOwned {
    fn is_boilr_shortcut(&self) -> bool {
        let boilr_tag = BOILR_TAG.to_string();
        self.tags.contains(&boilr_tag) || self.dev_kit_game_id.starts_with(&boilr_tag)
    }
}

fn remove_shortcuts_with_same_appid(
    shortcut_info: &mut ShortcutInfo,
    new_shortcuts: &[ShortcutOwned],
) {
    let app_ids: HashSet<u32> = new_shortcuts.iter().map(|s| s.app_id).collect();
    shortcut_info
        .shortcuts
        .retain(|shortcut| !app_ids.contains(&shortcut.app_id));
}

fn remove_old_shortcuts(shortcut_info: &mut ShortcutInfo) {
    shortcut_info
        .shortcuts
        .retain(|shortcut| !shortcut.is_boilr_shortcut());
}

pub fn fix_all_shortcut_icons(settings: &Settings) -> eyre::Result<()> {
    let mut userinfo_shortcuts = get_shortcuts_paths(&settings.steam)
        .map_err(|e| eyre::format_err!("Could not find steam shortcuts; {e}"))?;
    for user in userinfo_shortcuts.iter_mut() {
        let shortcut_info = get_shortcuts_for_user(user);
        if let Ok(mut shortcut_info) = shortcut_info {
            let changes = fix_shortcut_icons(
                user,
                &mut shortcut_info.shortcuts,
                settings.steam.optimize_for_big_picture,
            );
            if changes {
                save_shortcuts(&shortcut_info.shortcuts, Path::new(&shortcut_info.path));
            }
        }
    }
    Ok(())
}

fn fix_shortcut_icons(
    user: &SteamUsersInfo,
    shortcuts: &mut Vec<ShortcutOwned>,
    big_picture_mode: bool,
) -> bool {
    let image_folder = Path::new(&user.steam_user_data_folder)
        .join("config")
        .join("grid");
    let image_type = if big_picture_mode {
        ImageType::BigPicture
    } else {
        ImageType::Icon
    };

    let mut has_changes = false;
    for shortcut in shortcuts {
        let app_id = shortcut.app_id;
        let icon_exsists = Path::new(&shortcut.icon).exists() && !shortcut.icon.is_empty();
        for ext in ["ico", "png", "jpg", "webp"] {
            let path = image_folder.join(image_type.file_name(app_id, ext));
            if !icon_exsists && path.exists() {
                shortcut.icon = path.to_string_lossy().to_string();
                has_changes = true;
                break;
            }
        }
    }
    has_changes
}

fn write_shortcut_collections<S: AsRef<str>>(
    steam_id: S,
    platform_results: &[(String, Vec<ShortcutOwned>)],
) -> Result<(), Box<dyn Error>> {
    let mut collections = vec![];

    for (name, shortcuts) in platform_results {
        let game_ids = shortcuts.iter().map(|s| (s.app_id as usize)).collect();
        collections.push(Collection {
            name: name.clone(),
            game_ids,
        });
    }
    println!("Writing {} collections ", collections.len());
    write_collections(steam_id.as_ref(), &collections)?;
    Ok(())
}

pub fn get_platform_shortcuts(
    platform: Box<dyn GamesPlatform>,
) -> eyre::Result<Vec<ShortcutToImport>> {
    if platform.enabled() {
        platform.get_shortcut_info()
    } else {
        Ok(vec![])
    }
}

fn save_shortcuts(shortcuts: &[ShortcutOwned], path: &Path) {
    let mut shortcuts_refs = vec![];
    for shortcut in shortcuts {
        shortcuts_refs.push(shortcut.borrow());
    }
    let new_content = shortcuts_to_bytes(&shortcuts_refs);
    match File::create(path) {
        Ok(mut file) => match file.write_all(new_content.as_slice()) {
            Ok(_) => {
                info!(path = %path.display(), count = shortcuts.len(), "Saved shortcuts to file");
            }
            Err(e) => {
                error!(path = %path.display(), error = %e, "Failed to write shortcuts to file");
            }
        },
        Err(e) => {
            error!(path = %path.display(), error = %e, "Failed to create shortcuts file");
        }
    }
}
