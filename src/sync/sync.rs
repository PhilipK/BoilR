use steam_shortcuts_util::{shortcut::ShortcutOwned, shortcuts_to_bytes, Shortcut};

use crate::{
    egs::EpicPlatform,
    legendary::LegendaryPlatform,
    platform::Platform,
    settings::Settings,
    steam::{get_shortcuts_for_user, get_shortcuts_paths, get_users_images},
};
use std::error::Error;

use crate::{
    gog::GogPlatform,
    itch::ItchPlatform,
    origin::OriginPlatform,
    steamgriddb::{download_images, CachedSearch},
};
use std::{fs::File, io::Write, path::Path};

pub async fn run_sync(settings: &Settings) -> Result<(), Box<dyn Error>> {
    let userinfo_shortcuts = get_shortcuts_paths(&settings.steam)?;
    println!("Found {} user(s)", userinfo_shortcuts.len());
    for user in userinfo_shortcuts.iter() {
        let start_time = std::time::Instant::now();

        let mut shortcut_info = get_shortcuts_for_user(user);
        update_platforms(settings, &mut shortcut_info.shortcuts);
        save_shortcuts(&shortcut_info.shortcuts, Path::new(&shortcut_info.path));

        let duration = start_time.elapsed();
        println!("Finished synchronizing games in: {:?}", duration);
        if settings.steamgrid_db.enabled {
            let auth_key = &settings.steamgrid_db.auth_key;
            if let Some(auth_key) = auth_key {
                let start_time = std::time::Instant::now();
                println!("Checking for game images");
                let client = steamgriddb_api::Client::new(auth_key);
                let mut search = CachedSearch::new(&client);
                let known_images = get_users_images(user).unwrap();
                download_images(
                    known_images,
                    user.steam_user_data_folder.as_str(),
                    &shortcut_info.shortcuts,
                    &mut search,
                    &client,
                )
                .await?;
                search.save();
                let duration = start_time.elapsed();
                println!("Finished getting images in: {:?}", duration);
            } else {
                println!("Steamgrid DB Auth Key not found, please add one as described here:  https://github.com/PhilipK/steam_shortcuts_sync#configuration");
            }
        }
    }
    Ok(())
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
}

fn save_shortcuts(shortcuts: &Vec<ShortcutOwned>, path: &Path) {
    let mut shortcuts_refs = vec![];
    for shortcut in shortcuts {
        shortcuts_refs.push(shortcut.borrow());
    }
    let new_content = shortcuts_to_bytes(&shortcuts_refs);
    match File::create(path) {
        Ok(mut file) => match file.write_all(new_content.as_slice()) {
            Ok(_) => println!("Saved {} shortcuts", shortcuts.len()),
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
        let name = platform.name();

        #[cfg(target_os = "linux")]
        if platform.create_symlinks() {
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
                for shortcut in shortcuts_to_add {
                    let shortcut_owned: ShortcutOwned = shortcut.into();
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
