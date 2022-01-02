use steam_shortcuts_util::{shortcut::ShortcutOwned, shortcuts_to_bytes};

use crate::{
    egs::EpicPlatform,
    legendary::LegendaryPlatform,
    platform::Platform,
    settings::Settings,
    steam::{get_shortcuts_for_user, get_shortcuts_paths, ShortcutInfo, SteamUsersInfo},
    steamgriddb::download_images_for_users,
    uplay::Uplay,
};
use std::error::Error;

use crate::{gog::GogPlatform, itch::ItchPlatform, origin::OriginPlatform};
use std::{fs::File, io::Write, path::Path};

const BOILR_TAG: &str = "boilr";

pub async fn run_sync(settings: &Settings) -> Result<(), Box<dyn Error>> {
    let mut userinfo_shortcuts = get_shortcuts_paths(&settings.steam)?;
    println!("Found {} user(s)", userinfo_shortcuts.len());
    for user in userinfo_shortcuts.iter_mut() {
        let start_time = std::time::Instant::now();

        let mut shortcut_info = get_shortcuts_for_user(user);
        println!(
            "Found {} shortcuts for user: {}",
            shortcut_info.shortcuts.len(),
            user.steam_user_data_folder
        );

        remove_old_shortcuts(&mut shortcut_info);
        update_platforms(settings, &mut shortcut_info.shortcuts);
        fix_shortcut_icons(user, &mut shortcut_info.shortcuts);
        save_shortcuts(&shortcut_info.shortcuts, Path::new(&shortcut_info.path));
        user.shortcut_path = Some(shortcut_info.path.to_string_lossy().to_string());

        let duration = start_time.elapsed();
        println!("Finished synchronizing games in: {:?}", duration);
    }

    if settings.steamgrid_db.enabled {
        if settings.steamgrid_db.prefer_animated {
            println!("downloading animated images");
            download_images_for_users(settings, &userinfo_shortcuts, true).await;
        }
        download_images_for_users(settings, &userinfo_shortcuts, false).await;
    }

    Ok(())
}

fn remove_old_shortcuts(shortcut_info: &mut ShortcutInfo) {
    let boilr_tag = BOILR_TAG.to_string();
    shortcut_info
        .shortcuts
        .retain(|shortcut| !shortcut.tags.contains(&boilr_tag));
}

fn fix_shortcut_icons(user: &SteamUsersInfo, shortcuts: &mut Vec<ShortcutOwned>) {
    let image_folder = Path::new(&user.steam_user_data_folder)
        .join("config")
        .join("grid");
    for shortcut in shortcuts {
        #[cfg(not(target_os = "linux"))]
        let replace_icon = shortcut.icon.trim().eq("");
        #[cfg(target_os = "linux")]
        let replace_icon = shortcut.icon.trim().eq("") || shortcut.icon.eq(&shortcut.exe);
        if replace_icon {
            let app_id = steam_shortcuts_util::app_id_generator::calculate_app_id(
                &shortcut.exe,
                &shortcut.app_name,
            );
            let new_icon = image_folder
                .join(format!("{}_bigpicture.png", app_id))
                .to_string_lossy()
                .to_string();
            shortcut.icon = new_icon;
        }
    }
}

fn update_platforms(settings: &Settings, new_user_shortcuts: &mut Vec<ShortcutOwned>) {
    update_platform_shortcuts(
        &EpicPlatform::new(settings.epic_games.clone()),
        new_user_shortcuts,
    );
    update_platform_shortcuts(
        &LegendaryPlatform::new(settings.legendary.clone()),
        new_user_shortcuts,
    );
    update_platform_shortcuts(
        &ItchPlatform::new(settings.itch.clone()),
        new_user_shortcuts,
    );
    update_platform_shortcuts(
        &OriginPlatform {
            settings: settings.origin.clone(),
        },
        new_user_shortcuts,
    );
    update_platform_shortcuts(
        &GogPlatform {
            settings: settings.gog.clone(),
        },
        new_user_shortcuts,
    );
    update_platform_shortcuts(
        &Uplay {
            settings: settings.uplay.clone(),
        },
        new_user_shortcuts,
    );
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

fn update_platform_shortcuts<P, T, E>(platform: &P, current_shortcuts: &mut Vec<ShortcutOwned>)
where
    P: Platform<T, E>,
    E: std::fmt::Debug + std::fmt::Display,
    T: Into<ShortcutOwned>,
{
    if platform.enabled() {
        if let crate::platform::SettingsValidity::Invalid { reason } = platform.settings_valid() {
            eprintln!(
                "Setting for platform {} are invalid, reason: {}",
                platform.name(),
                reason
            );
            return;
        }

        #[cfg(target_os = "linux")]
        if platform.create_symlinks() {
            let name = platform.name();
            super::symlinks::ensure_links_folder_created(name);
        }

        let shortcuts_to_add_result = platform.get_shortcuts();

        match shortcuts_to_add_result {
            Ok(shortcuts_to_add) => {
                println!(
                    "Found {} game(s) for platform {}",
                    shortcuts_to_add.len(),
                    platform.name()
                );

                current_shortcuts.retain(|f| !f.tags.contains(&platform.name().to_owned()));
                let boilr_tag = BOILR_TAG.to_string();
                for shortcut in shortcuts_to_add {
                    let mut shortcut_owned: ShortcutOwned = shortcut.into();
                    if !shortcut_owned.tags.contains(&boilr_tag) {
                        shortcut_owned.tags.push(boilr_tag.clone());
                    }
                    #[cfg(target_os = "linux")]
                    let shortcut_owned = if platform.create_symlinks() {
                        crate::sync::symlinks::create_sym_links(&shortcut_owned)
                    } else {
                        shortcut_owned
                    };
                    current_shortcuts.push(shortcut_owned);
                }
            }
            Err(err) => {
                eprintln!("Error getting shortcuts from platform: {}", platform.name());
                eprintln!("{}", err);
            }
        }
    }
}
