use crate::steamgriddb::{CachedSearch, ImageType};
use std::{
    collections::HashMap,
    env::{self},
    fmt,
    fs::File,
    io::Write,
    ops::Deref,
    path::Path,
};
mod egs;
mod legendary;
mod platform;
mod settings;
mod steam;
mod steamgriddb;

use crate::{
    egs::EpicPlatform,
    legendary::LegendaryPlatform,
    platform::Platform,
    settings::Settings,
    steam::{get_shortcuts_for_user, get_shortcuts_paths, get_users_images},
};
use std::error::Error;
use steam_shortcuts_util::{
    parse_shortcuts, shortcut::ShortcutOwned, shortcuts_to_bytes, Shortcut,
};
use steamgriddb_api::{search::SearchResult, Client};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let settings = Settings::new()?;

    let auth_key = settings.steamgrid_db.auth_key;
    if settings.steamgrid_db.enabled && auth_key.is_none() {
        println!("auth_key not found, please add it to the steamgrid_db settings ");
        return Ok(());
    }

    let auth_key = auth_key.unwrap();

    let client = steamgriddb_api::Client::new(auth_key);
    let mut search = CachedSearch::new(&client);

    let userinfo_shortcuts = get_shortcuts_paths()?;
    println!("Found {} user(s)", userinfo_shortcuts.len());

    for user in userinfo_shortcuts.iter() {
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

        let shortcuts = new_user_shortcuts.iter().map(|f| f.borrow()).collect();

        save_shortcuts(&shortcuts, Path::new(&shortcut_info.path));

        let known_images = get_users_images(user).unwrap();

        let shortcuts_to_search_for = shortcuts.iter().filter(|s| {
            let images = vec![
                format!("{}_hero.png", s.app_id),
                format!("{}p.png", s.app_id),
                format!("{}_logo.png", s.app_id),
            ];
            // if we are missing any of the images we need to search for them
            images.iter().any(|image| !known_images.contains(&image))
        });

        let mut search_results = HashMap::new();
        for s in shortcuts_to_search_for {
            println!("Searching for {}", s.app_name);
            let search = search.search(s.app_id, s.app_name).await?;
            if let Some(search) = search {
                search_results.insert(s.app_id, search);
            }
        }

        let types = vec![ImageType::Logo, ImageType::Hero, ImageType::Grid];
        for image_type in types {
            let mut images_needed = shortcuts
                .iter()
                .filter(|s| search_results.contains_key(&s.app_id))
                .filter(|s| !known_images.contains(&image_type.file_name(s.app_id)));
            let image_ids: Vec<usize> = images_needed
                .clone()
                .filter_map(|s| search_results.get(&s.app_id))
                .map(|search| *search)
                .collect();

            let query_type = match image_type {
                ImageType::Hero => steamgriddb_api::query_parameters::QueryType::Hero(None),
                ImageType::Grid => steamgriddb_api::query_parameters::QueryType::Grid(None),
                ImageType::Logo => steamgriddb_api::query_parameters::QueryType::Logo(None),
            };

            match client
                .get_images_for_ids(image_ids.as_slice(), &query_type)
                .await
            {
                Ok(images) => {
                    for image in images {
                        if let Some(shortcut) = images_needed.next() {
                            if let Ok(image) = image {
                                let grid_folder = Path::new(user.steam_user_data_folder.as_str())
                                    .join("config/grid");
                                let path = grid_folder.join(image_type.file_name(shortcut.app_id));
                                println!(
                                    "Downloading {} to {}",
                                    image.url,
                                    path.as_path().to_str().unwrap()
                                );
                                let mut file = File::create(path).unwrap();
                                let response = reqwest::get(image.url).await?;
                                let content = response.bytes().await?;
                                file.write(&content).unwrap();
                            }
                        }
                    }
                }
                Err(err) => println!("Error getting images: {}", err),
            }
        }
    }

    search.save();

    Ok(())
}

fn save_shortcuts(shortcuts: &Vec<Shortcut>, path: &Path) {
    let new_content = shortcuts_to_bytes(shortcuts);
    let mut file = File::create(path).unwrap();
    file.write(new_content.as_slice()).unwrap();
}

fn update_platform_shortcuts<P, T, E>(platform: &P, current_shortcuts: &mut Vec<ShortcutOwned>)
where
    P: Platform<T, E>,
    E: std::fmt::Debug + std::fmt::Display,
    T: Into<ShortcutOwned>,
{
    if platform.enabled() {
        let shortcuts_to_add_result = platform.get_shortcuts();
        match shortcuts_to_add_result {
            Ok(shortcuts_to_add) => {
                current_shortcuts.retain(|f| !f.tags.contains(&platform.name().to_owned()));
                for shortcut in shortcuts_to_add {
                    let shortcut_owned: ShortcutOwned = shortcut.into();
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
