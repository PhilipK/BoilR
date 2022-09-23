use steam_shortcuts_util::{
    calculate_app_id_for_shortcut, shortcut::ShortcutOwned, shortcuts_to_bytes, Shortcut,
};
use tokio::sync::watch::Sender;

use crate::{
    legendary::LegendaryPlatform,
    lutris::lutris_platform::LutrisPlatform,
    platforms::{Platform, PlatformEnum},
    settings::Settings,
    steam::{
        get_shortcuts_for_user, get_shortcuts_paths, setup_proton_games, write_collections,
        Collection, ShortcutInfo, SteamUsersInfo,
    },
    steamgriddb::{download_images_for_users, ImageType},
    uplay::Uplay,
};

#[cfg(target_family = "unix")]
use crate::heroic::HeroicPlatform;

#[cfg(target_family = "unix")]
use crate::flatpak::FlatpakPlatform;

use std::{collections::HashMap, error::Error};

use crate::{gog::GogPlatform, itch::ItchPlatform, origin::OriginPlatform};
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
        let mut shortcut_info = get_shortcuts_for_user(user);

        for shortcut in shortcut_info.shortcuts.iter_mut() {
            if shortcut.app_id == app_id {
                shortcut.dev_kit_game_id = "".to_string();
                shortcut.tags.retain(|s| s != BOILR_TAG);
            }
        }
        save_shortcuts(&shortcut_info.shortcuts, Path::new(&shortcut_info.path));
    }

    Ok(())
}

pub fn run_sync(
    settings: &Settings,
    sender: &mut Option<Sender<SyncProgress>>,
    renames: &HashMap<u32, String>,
    platforms: &[PlatformEnum],
) -> Result<Vec<SteamUsersInfo>, String> {
    if let Some(sender) = &sender {
        let _ = sender.send(SyncProgress::Starting);
    }

    let platform_shortcuts = get_platform_shortcuts(settings);
    let mut platform_enum_shortcuts = get_enum_platform_shortcuts(platforms);
    platform_enum_shortcuts.extend(platform_shortcuts);
    sync_shortcuts(settings, &platform_enum_shortcuts, sender, renames)
}

fn sync_shortcuts(
    settings: &Settings,
    platform_shortcuts: &Vec<(String, Vec<ShortcutOwned>)>,
    sender: &mut Option<Sender<SyncProgress>>,
    renames: &HashMap<u32, String>,
) -> Result<Vec<SteamUsersInfo>, String> {
    let mut userinfo_shortcuts = get_shortcuts_paths(&settings.steam)
        .map_err(|e| format!("Getting shortcut paths failed: {e}"))?;
    let mut all_shortcuts: Vec<ShortcutOwned> = platform_shortcuts
        .iter()
        .flat_map(|s| s.1.clone())
        .filter(|s| !settings.blacklisted_games.contains(&s.app_id))
        .collect();
    if let Some(sender) = &sender {
        let _ = sender.send(SyncProgress::FoundGames {
            games_found: all_shortcuts.len(),
        });
    }
    for shortcut in &mut all_shortcuts {
        if let Some(rename) = renames.get(&shortcut.app_id) {
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
        println!("Appid: {} name: {}", shortcut.app_id, shortcut.app_name);
    }
    println!("Found {} user(s)", userinfo_shortcuts.len());
    for user in userinfo_shortcuts.iter_mut() {
        let start_time = std::time::Instant::now();

        let mut shortcut_info = get_shortcuts_for_user(user);
        println!(
            "Found {} shortcuts for user: {}",
            shortcut_info.shortcuts.len(),
            user.user_id
        );

        remove_old_shortcuts(&mut shortcut_info);

        shortcut_info.shortcuts.extend(all_shortcuts.clone());

        fix_shortcut_icons(
            user,
            &mut shortcut_info.shortcuts,
            settings.steam.optimize_for_big_picture,
        );

        save_shortcuts(&shortcut_info.shortcuts, Path::new(&shortcut_info.path));

        if settings.steam.create_collections {
            match write_shortcut_collections(&user.user_id, &platform_shortcuts) {
                Ok(_) => (),
                Err(_e) => eprintln!("Could not write collections, make sure steam is shut down"),
            }
        }

        let duration = start_time.elapsed();
        println!("Finished synchronizing games in: {:?}", duration);
    }
    Ok(userinfo_shortcuts)
}

pub async fn download_images(
    settings: &Settings,
    userinfo_shortcuts: &[SteamUsersInfo],
    sender: &mut Option<Sender<SyncProgress>>,
) {
    if settings.steamgrid_db.enabled {
        if settings.steamgrid_db.prefer_animated {
            println!("downloading animated images");
            download_images_for_users(settings, userinfo_shortcuts, true, sender).await;
        }
        download_images_for_users(settings, userinfo_shortcuts, false, sender).await;
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

fn remove_old_shortcuts(shortcut_info: &mut ShortcutInfo) {
    shortcut_info
        .shortcuts
        .retain(|shortcut| !shortcut.is_boilr_shortcut());
}

fn fix_shortcut_icons(
    user: &SteamUsersInfo,
    shortcuts: &mut Vec<ShortcutOwned>,
    big_picture_mode: bool,
) {
    let image_folder = Path::new(&user.steam_user_data_folder)
        .join("config")
        .join("grid");
    let image_type = if big_picture_mode {
        ImageType::BigPicture
    } else {
        ImageType::Icon
    };

    for shortcut in shortcuts {
        if shortcut.is_boilr_shortcut() {
            let app_id = steam_shortcuts_util::app_id_generator::calculate_app_id(
                &shortcut.exe,
                &shortcut.app_name,
            );
            for ext in ["ico", "png", "jpg", "webp"] {
                let path = image_folder.join(image_type.file_name(app_id, ext));
                if path.exists() {
                    shortcut.icon = path.to_string_lossy().to_string();
                    break;
                }
            }
        }
    }
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

pub fn get_enum_platform_shortcuts(
    platforms: &[PlatformEnum],
) -> Vec<(String, Vec<ShortcutOwned>)> {
    platforms
        .iter()
        .filter(|p| p.enabled())
        .map(|p| (p.name().to_owned(), p.get_shortcuts()))
        .filter_map(|(name, shortcuts)| match shortcuts {
            Ok(shortcuts) => Some((name, shortcuts)),
            Err(_error) => None,
        })
        .collect()
}

pub fn get_platform_shortcuts(settings: &Settings) -> Vec<(String, Vec<ShortcutOwned>)> {
    let mut platform_results = vec![
        update_platform_shortcuts(&LegendaryPlatform::new(settings.legendary.clone())),
        update_platform_shortcuts(&ItchPlatform::new(settings.itch.clone())),
        update_platform_shortcuts(&OriginPlatform {
            settings: settings.origin.clone(),
        }),
        update_platform_shortcuts(&GogPlatform {
            settings: settings.gog.clone(),
        }),
        update_platform_shortcuts(&Uplay {
            settings: settings.uplay.clone(),
        }),
        update_platform_shortcuts(&LutrisPlatform {
            settings: settings.lutris.clone(),
        }),
    ];
    #[cfg(target_family = "unix")]
    {
        platform_results.push(update_platform_shortcuts(&HeroicPlatform {
            settings: settings.heroic.clone(),
        }));

        platform_results.push(update_platform_shortcuts(&FlatpakPlatform {
            settings: settings.flatpak.clone(),
        }));
        platform_results.push(update_platform_shortcuts(
            &crate::bottles::BottlesPlatform {
                settings: settings.bottles.clone(),
            },
        ));
    }
    platform_results.iter().filter_map(|p| p.clone()).collect()
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
                println!("Saved {} shortcuts", shortcuts.len())
            }
            Err(e) => println!(
                "Failed to save shortcuts to {} error: {}",
                path.to_string_lossy(),
                e
            ),
        },
        Err(e) => {
            println!(
                "Failed to save shortcuts to {} error: {}",
                path.to_string_lossy(),
                e
            );
        }
    }
}

fn update_platform_shortcuts<P, T, E>(platform: &P) -> Option<(String, Vec<ShortcutOwned>)>
where
    P: Platform<T, E>,
    E: std::fmt::Debug + std::fmt::Display,
    T: Into<ShortcutOwned>,
    T: Clone,
{
    if platform.enabled() {
        if let crate::platforms::SettingsValidity::Invalid { reason } = platform.settings_valid() {
            eprintln!(
                "Setting for platform {} are invalid, reason: {}",
                platform.name(),
                reason
            );
            return None;
        }

        let mut current_shortcuts = vec![];

        #[cfg(target_family = "unix")]
        if platform.create_symlinks() {
            let name = platform.name();
            super::symlinks::ensure_links_folder_created(name);
        }

        let shortcuts_to_add_result = platform.get_shortcuts();

        match shortcuts_to_add_result {
            Ok(shortcuts_to_add) => {
                let mut shortcuts_to_proton = vec![];
                let mut shortcuts_to_add_transformed = vec![];
                for shortcut in shortcuts_to_add {
                    let mut shortcut_owned: ShortcutOwned = shortcut.clone().into();
                    shortcut_owned.dev_kit_game_id =
                        format!("{}-{}", BOILR_TAG, shortcut_owned.app_id);
                    shortcuts_to_add_transformed.push((shortcut, shortcut_owned));
                }

                let shortcuts_to_add = shortcuts_to_add_transformed;

                println!(
                    "Found {} game(s) for platform {}",
                    shortcuts_to_add.len(),
                    platform.name()
                );

                for (orign_shortcut, shortcut_owned) in shortcuts_to_add {
                    #[cfg(target_family = "unix")]
                    let shortcut_owned = if platform.create_symlinks() {
                        crate::sync::symlinks::create_sym_links(&shortcut_owned)
                    } else {
                        shortcut_owned
                    };
                    if platform.needs_proton(&orign_shortcut) {
                        shortcuts_to_proton.push(format!("{}", shortcut_owned.app_id));
                    }
                    current_shortcuts.push(shortcut_owned.clone());
                }
                if !shortcuts_to_proton.is_empty() {
                    setup_proton_games(shortcuts_to_proton.as_slice());
                }

                let name = platform.name();
                return Some((name.to_string(), current_shortcuts));
            }
            Err(err) => {
                eprintln!("Error getting shortcuts from platform: {}", platform.name());
                eprintln!("{}", err);
            }
        }
    }
    None
}
