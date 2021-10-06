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

        let shortcut_info = get_shortcuts_for_user(user);

        let mut new_user_shortcuts: Vec<ShortcutOwned> = shortcut_info.shortcuts;

        update_platform_shortcuts(
            &EpicPlatform::new(settings.epic_games.clone()),
            &mut new_user_shortcuts,
        );

        update_platform_shortcuts(
            &LegendaryPlatform::new(settings.legendary.clone()),
            &mut new_user_shortcuts,
        );

        update_platform_shortcuts(
            &ItchPlatform::new(settings.itch.clone()),
            &mut new_user_shortcuts,
        );

        update_platform_shortcuts(
            &OriginPlatform {
                settings: settings.origin.clone(),
            },
            &mut new_user_shortcuts,
        );

        update_platform_shortcuts(
            &GogPlatform {
                settings: settings.gog.clone(),
            },
            &mut new_user_shortcuts,
        );

        let shortcuts = new_user_shortcuts.iter().map(|f| f.borrow()).collect();

        save_shortcuts(&shortcuts, Path::new(&shortcut_info.path));

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
                    shortcuts,
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

fn save_shortcuts(shortcuts: &Vec<Shortcut>, path: &Path) {
    let new_content = shortcuts_to_bytes(shortcuts);
    let mut file = File::create(path).unwrap();
    file.write_all(new_content.as_slice()).unwrap();
}

fn update_platform_shortcuts<P, T, E>(platform: &P, current_shortcuts: &mut Vec<ShortcutOwned>)
where
    P: Platform<T, E>,
    E: std::fmt::Debug + std::fmt::Display,
    T: Into<ShortcutOwned>,
{
    if platform.enabled() {
        let shortcuts_to_add_result = platform.get_shortcuts();

        #[cfg(target_os = "linux")]
        if platform.create_symlinks() {
            let boilr_links_path = get_boilr_links_path();
            if !boilr_links_path.exists() {
                if let Err(e) = std::fs::create_dir_all(&boilr_links_path) {
                    println!(
                        "Could not create links folder for symlinks at path: {:?} , error: {:?} , you can try to disable creating symlinks for platform {}",
                        boilr_links_path, e, platform.name()
                    );
                    return;
                }
            }
        }

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
                        create_sym_links(&shortcut_owned)
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
#[cfg(target_os = "linux")]
fn get_boilr_links_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").expect("Expected a home variable to be defined");
    let boilr_links_path = Path::new(&home).join(".boilr").join("links");
    boilr_links_path
}
#[cfg(target_os = "linux")]
fn create_sym_links(shortcut: &ShortcutOwned) -> ShortcutOwned {
    let links_folder = get_boilr_links_path();

    let target_link = links_folder.join(format!("t{}", shortcut.app_id));
    let workdir_link = links_folder.join(format!("w{}", shortcut.app_id));

    let target_original = Path::new(&shortcut.exe);
    let workdir_original = Path::new(&shortcut.start_dir);

    use std::os::unix::fs::symlink;

    match (
        symlink(&target_original, &target_link),
        symlink(&workdir_original, &workdir_link),
    ) {
        (Ok(_), Ok(_)) => {
            let exe = target_link.to_string_lossy().to_string();
            let start_dir = workdir_link.to_string_lossy().to_string();
            let new_shortcut = Shortcut::new(
                0,
                shortcut.app_name.as_str(),
                exe.as_str(),
                &start_dir.as_str(),
                shortcut.icon.as_str(),
                shortcut.shortcut_path.as_str(),
                shortcut.launch_options.as_str(),
            );
            let mut new_shortcut = new_shortcut.to_owned();
            new_shortcut.tags = shortcut.tags.clone();
            new_shortcut
        }
        _ => {
            println!("Could not create symlinks for game: {}", shortcut.app_name);
            shortcut.clone()
        }
    }
}
