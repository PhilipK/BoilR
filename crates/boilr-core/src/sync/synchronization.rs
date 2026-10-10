use steam_shortcuts_util::{
    calculate_app_id_for_shortcut, shortcut::ShortcutOwned, shortcuts_to_bytes, Shortcut,
};
use tokio::sync::watch::Sender;

use crate::{
    settings::Settings,
    steam::{
        get_shortcuts_for_user, get_shortcuts_paths, is_flatpak_steam, run_flatpak_on_host,
        write_collections, Collection, ShortcutInfo, SteamUsersInfo,
    },
    steamgriddb::{download_images_for_users, ImageType},
};

use std::{
    collections::{HashMap, HashSet},
    error::Error,
};

use std::{fs::File, io::Write, path::Path};

pub const BOILR_TAG: &str = "boilr";

#[derive(Clone, Debug)]
pub enum SyncProgress {
    NotStarted,
    Starting,
    FoundGames {
        games_found: usize,
    },
    FindingImages,
    DownloadingImages {
        to_download: usize,
    },
    Done,
    /// Error occurred during sync - contains user-friendly error message
    Error {
        message: String,
    },
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
            save_shortcuts(&shortcut_info.shortcuts, Path::new(&shortcut_info.path))?
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
    let userinfo_shortcuts = get_shortcuts_paths(&settings.steam)?;
    let flatpak_steam = is_flatpak_steam(&settings.steam);
    if flatpak_steam {
        println!(
            "Steam is a Flatpak; launching Flatpak apps through flatpak-spawn --host. \
             Steam needs: flatpak override --user --talk-name=org.freedesktop.Flatpak com.valvesoftware.Steam"
        );
    }
    let mut all_shortcuts: Vec<ShortcutOwned> = platform_shortcuts
        .iter()
        .flat_map(|s| s.1.clone())
        .filter(|s| !settings.blacklisted_games.contains(&s.app_id))
        .collect();
    for shortcut in &mut all_shortcuts {
        shortcut.dev_kit_game_id = BOILR_TAG.to_string();
    }
    if let Some(sender) = &sender {
        let _ = sender.send(SyncProgress::FoundGames {
            games_found: all_shortcuts.len(),
        });
    }
    let mut previous_ids = HashMap::new();
    for shortcut in &mut all_shortcuts {
        // Renames are keyed by the platform's app id, so look up before the Flatpak rewrite.
        let rename = renames.get(&shortcut.app_id);
        if flatpak_steam {
            let id_before = shortcut.app_id;
            run_flatpak_on_host(shortcut);
            if shortcut.app_id != id_before {
                previous_ids.insert(shortcut.app_id, id_before);
            }
        }
        if let Some(rename) = rename {
            let original_id = shortcut.app_id;
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
            previous_ids.insert(shortcut.app_id, original_id);
        }
        println!("Appid: {} name: {}", shortcut.app_id, shortcut.app_name);
    }
    // Apply the same Flatpak rewrite to the collections, so their app ids match the new shortcuts.
    let platform_shortcuts: Vec<(String, Vec<ShortcutOwned>)> = platform_shortcuts
        .iter()
        .map(|(name, shortcuts)| {
            let mut shortcuts = shortcuts.clone();
            if flatpak_steam {
                shortcuts.iter_mut().for_each(run_flatpak_on_host);
            }
            (name.clone(), shortcuts)
        })
        .collect();
    println!("Found {} user(s)", userinfo_shortcuts.len());
    // Read every user's shortcuts before writing any. A file that can't be read is not
    // skipped: the user would see an import that "worked" but changed nothing (#558).
    let mut users_shortcuts = vec![];
    for user in userinfo_shortcuts.iter() {
        let shortcut_info = get_shortcuts_for_user(user).map_err(|err| {
            eyre::format_err!("{err}. Nothing was imported and the file was left as it is.")
        })?;
        users_shortcuts.push((user, shortcut_info));
    }
    for (user, mut shortcut_info) in users_shortcuts {
        let start_time = std::time::Instant::now();
        println!(
            "Found {} shortcuts for user: {}",
            shortcut_info.shortcuts.len(),
            user.user_id
        );

        let mut user_shortcuts = all_shortcuts.clone();
        keep_steam_fields(&shortcut_info.shortcuts, &mut user_shortcuts, &previous_ids);

        remove_old_shortcuts(&mut shortcut_info);
        remove_shortcuts_with_same_appid(&mut shortcut_info, &user_shortcuts);

        shortcut_info.shortcuts.extend(user_shortcuts);

        if let Err(e) = save_shortcuts(&shortcut_info.shortcuts, Path::new(&shortcut_info.path)) {
            eprintln!("Failed to save shortcuts for user {}: {}", user.user_id, e);
            if let Some(sender) = sender {
                let _ = sender.send(SyncProgress::Error { message: e });
            }
            // Continue with other users even if one fails
        }

        if settings.steam.create_collections {
            match write_shortcut_collections(&user.user_id, &platform_shortcuts) {
                Ok(_) => (),
                Err(_e) => eprintln!("Could not write collections, make sure steam is shut down"),
            }
        }

        let duration = start_time.elapsed();
        println!("Finished synchronizing games in: {duration:?}");
    }
    Ok(userinfo_shortcuts)
}

pub async fn download_images(
    settings: &Settings,
    userinfo_shortcuts: &[SteamUsersInfo],
    sender: &mut Option<Sender<SyncProgress>>,
) {
    if settings.steamgrid_db.enabled {
        download_images_for_users(settings, userinfo_shortcuts, sender).await;
        if settings.steamgrid_db.prefer_animated {
            let mut set = settings.clone();
            set.steamgrid_db.prefer_animated = false;
            download_images_for_users(&set, userinfo_shortcuts, sender).await;
        }
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

/// Steam stores some per-game state in `shortcuts.vdf` itself: the "Include in VR Library"
/// flag (#379), the last time the game was played (#389), and the overlay, desktop
/// configuration and hidden switches from the game's properties. A sync replaces each
/// shortcut with a freshly built one, so copy that state over from the shortcut Steam
/// already has under the same app id. `previous_ids` maps a renamed game's new app id to
/// its old one, so the first sync after a rename still finds Steam's entry.
fn keep_steam_fields(
    existing: &[ShortcutOwned],
    new_shortcuts: &mut [ShortcutOwned],
    previous_ids: &HashMap<u32, u32>,
) {
    let mut existing_by_id: HashMap<u32, &ShortcutOwned> = HashMap::new();
    for old in existing {
        // With duplicates, the one Steam last updated is the one the user sees.
        let entry = existing_by_id.entry(old.app_id).or_insert(old);
        if old.last_play_time > entry.last_play_time {
            *entry = old;
        }
    }
    for shortcut in new_shortcuts {
        let old = existing_by_id.get(&shortcut.app_id).or_else(|| {
            previous_ids
                .get(&shortcut.app_id)
                .and_then(|id| existing_by_id.get(id))
        });
        if let Some(old) = old {
            shortcut.open_vr = old.open_vr;
            shortcut.last_play_time = old.last_play_time;
            shortcut.allow_overlay = old.allow_overlay;
            shortcut.allow_desktop_config = old.allow_desktop_config;
            shortcut.is_hidden = old.is_hidden;
        }
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
                if let Err(e) =
                    save_shortcuts(&shortcut_info.shortcuts, Path::new(&shortcut_info.path))
                {
                    eprintln!(
                        "Failed to save shortcut icons for user {}: {}",
                        user.user_id, e
                    );
                    // Continue with other users
                }
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
        if shortcuts.is_empty() {
            continue;
        }
        let game_ids = shortcuts.iter().map(|s| s.app_id as usize).collect();
        collections.push(Collection {
            name: name.clone(),
            game_ids,
        });
    }
    println!("Writing {} collections ", collections.len());
    write_collections(steam_id.as_ref(), &collections)?;
    Ok(())
}

fn save_shortcuts(shortcuts: &[ShortcutOwned], path: &Path) -> Result<(), String> {
    let mut shortcuts_refs = vec![];
    for shortcut in shortcuts {
        shortcuts_refs.push(shortcut.borrow());
    }
    let new_content = shortcuts_to_bytes(&shortcuts_refs);
    match File::create(path) {
        Ok(mut file) => match file.write_all(new_content.as_slice()) {
            Ok(_) => {
                println!("Saved {} shortcuts", shortcuts.len());
                Ok(())
            }
            Err(e) => {
                Err(format!(
                    "Failed to write shortcuts to {}: {}. Check that Steam is not running and you have write permissions to the Steam folder.",
                    path.display(),
                    e
                ))
            }
        },
        Err(e) => {
            Err(format!(
                "Failed to create shortcuts file at {}: {}. Check that Steam is not running and you have write permissions to the Steam folder.",
                path.display(),
                e
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shortcut(name: &str, exe: &str) -> ShortcutOwned {
        Shortcut::new("0", name, exe, "", "", "", "").to_owned()
    }

    #[test]
    fn keeps_vr_flag_and_last_play_time_of_existing_shortcut() {
        let mut old = shortcut("Game", "/usr/bin/game");
        old.open_vr = 1;
        old.last_play_time = 1_700_000_000;
        let mut new_shortcuts = vec![shortcut("Game", "/usr/bin/game")];

        keep_steam_fields(&[old], &mut new_shortcuts, &HashMap::new());

        let kept = new_shortcuts.first();
        assert_eq!(kept.map(|s| s.open_vr), Some(1));
        assert_eq!(kept.map(|s| s.last_play_time), Some(1_700_000_000));
    }

    #[test]
    fn keeps_properties_set_in_steam() {
        let mut old = shortcut("Game", "/usr/bin/game");
        old.allow_overlay = false;
        old.allow_desktop_config = false;
        old.is_hidden = true;
        let mut new_shortcuts = vec![shortcut("Game", "/usr/bin/game")];

        keep_steam_fields(&[old], &mut new_shortcuts, &HashMap::new());

        let kept = new_shortcuts.first();
        assert_eq!(kept.map(|s| s.allow_overlay), Some(false));
        assert_eq!(kept.map(|s| s.allow_desktop_config), Some(false));
        assert_eq!(kept.map(|s| s.is_hidden), Some(true));
    }

    #[test]
    fn renamed_game_keeps_state_from_its_old_app_id() {
        let mut old = shortcut("Old Name", "/usr/bin/game");
        old.last_play_time = 1_700_000_000;
        let old_id = old.app_id;
        let mut new_shortcuts = vec![shortcut("New Name", "/usr/bin/game")];
        let new_id = new_shortcuts.first().map(|s| s.app_id).unwrap_or_default();
        let previous_ids = HashMap::from([(new_id, old_id)]);

        keep_steam_fields(&[old], &mut new_shortcuts, &previous_ids);

        assert_eq!(
            new_shortcuts.first().map(|s| s.last_play_time),
            Some(1_700_000_000)
        );
    }

    #[test]
    fn duplicate_entries_use_the_most_recently_played() {
        let mut played = shortcut("Game", "/usr/bin/game");
        played.last_play_time = 1_700_000_000;
        let stale = shortcut("Game", "/usr/bin/game");
        let mut new_shortcuts = vec![shortcut("Game", "/usr/bin/game")];

        keep_steam_fields(&[played, stale], &mut new_shortcuts, &HashMap::new());

        assert_eq!(
            new_shortcuts.first().map(|s| s.last_play_time),
            Some(1_700_000_000)
        );
    }

    #[test]
    fn new_games_start_without_steam_state() {
        let mut old = shortcut("Other", "/usr/bin/other");
        old.open_vr = 1;
        old.last_play_time = 1_700_000_000;
        let mut new_shortcuts = vec![shortcut("Game", "/usr/bin/game")];

        keep_steam_fields(&[old], &mut new_shortcuts, &HashMap::new());

        let fresh = new_shortcuts.first();
        assert_eq!(fresh.map(|s| s.open_vr), Some(0));
        assert_eq!(fresh.map(|s| s.last_play_time), Some(0));
    }
}
