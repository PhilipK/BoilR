use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::{collections::HashMap, path::Path};

use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::error::Error;
use steamgriddb_api::query_parameters::GridDimentions; // 0.3.1

use steam_shortcuts_util::shortcut::ShortcutOwned;
use steamgriddb_api::Client;

use crate::settings::Settings;
use crate::steam::{get_shortcuts_for_user, get_users_images, SteamUsersInfo};
use crate::steamgriddb::ImageType;

use super::CachedSearch;

const CONCURRENT_REQUESTS: usize = 10;

pub async fn download_images_for_users<'b>(
    settings: &Settings,
    users: &[SteamUsersInfo],
    download_animated: bool,
) {
    let auth_key = &settings.steamgrid_db.auth_key;
    if let Some(auth_key) = auth_key {
        println!("Checking for game images");
        let start_time = std::time::Instant::now();
        let client = steamgriddb_api::Client::new(auth_key);
        let search = CachedSearch::new(&client);
        let search = &search;
        let client = &client;
        let to_downloads = stream::iter(users)
            .map(|user| {
                let shortcut_info = get_shortcuts_for_user(user);
                async move {
                    let known_images = get_users_images(user).unwrap_or_default();
                    let res = search_fo_to_download(
                        known_images,
                        user.steam_user_data_folder.as_str(),
                        &shortcut_info.shortcuts,
                        search,
                        client,
                        download_animated,
                    )
                    .await;
                    res.unwrap_or_default()
                }
            })
            .buffer_unordered(CONCURRENT_REQUESTS)
            .collect::<Vec<Vec<ToDownload>>>()
            .await;
        let to_downloads = to_downloads.iter().flatten().collect::<Vec<&ToDownload>>();
        if !to_downloads.is_empty() {
            search.save();

            stream::iter(to_downloads)
                .map(|to_download| async move {
                    if let Err(e) = download_to_download(to_download).await {
                        println!("Error downloading {:?}: {}", &to_download.path, e);
                    }
                })
                .buffer_unordered(CONCURRENT_REQUESTS)
                .collect::<Vec<()>>()
                .await;
            let duration = start_time.elapsed();
            println!("Finished getting images in: {:?}", duration);
        } else {
            println!("No images needed");
        }
    } else {
        println!("Steamgrid DB Auth Key not found, please add one as described here:  https://github.com/PhilipK/steam_shortcuts_sync#configuration");
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PublicGameResponseMetadata {
    store_asset_mtime: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PublicGameResponseSteam {
    id: String,
    metadata: Option<PublicGameResponseMetadata>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PublicGameResponsePlatforms {
    steam: Option<PublicGameResponseSteam>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PublicGameResponseData {
    platforms: Option<PublicGameResponsePlatforms>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PublicGameResponse {
    success: bool,
    data: Option<PublicGameResponseData>,
}

async fn search_fo_to_download(
    known_images: Vec<String>,
    user_data_folder: &str,
    shortcuts: &[ShortcutOwned],
    search: &CachedSearch<'_>,
    client: &Client,
    download_animated: bool,
) -> Result<Vec<ToDownload>, Box<dyn Error>> {
    let shortcuts_to_search_for = shortcuts.iter().filter(|s| {
        let images = vec![
            format!("{}_hero.png", s.app_id),
            format!("{}p.png", s.app_id),
            format!("{}.png", s.app_id),
            format!("{}_logo.png", s.app_id),
            format!("{}_bigpicture.png", s.app_id),
        ];
        // if we are missing any of the images we need to search for them
        images.iter().any(|image| !known_images.contains(image)) && !s.app_name.is_empty()
    });
    let shortcuts_to_search_for: Vec<&ShortcutOwned> = shortcuts_to_search_for.collect();
    if shortcuts_to_search_for.is_empty() {
        return Ok(vec![]);
    }
    let mut search_results = HashMap::new();
    let search_results_a = stream::iter(shortcuts_to_search_for)
        .map(|s| async move {
            let search_result = search.search(s.app_id, &s.app_name).await;
            if search_result.is_err() {
                return None;
            }
            let search_result = search_result.unwrap();
            search_result?;
            let search_result = search_result.unwrap();
            Some((s.app_id, search_result))
        })
        .buffer_unordered(CONCURRENT_REQUESTS)
        .collect::<Vec<Option<(u32, usize)>>>()
        .await;
    for (app_id, search) in search_results_a.into_iter().flatten() {
        search_results.insert(app_id, search);
    }
    let types = vec![
        ImageType::Logo,
        ImageType::Hero,
        ImageType::Grid,
        ImageType::WideGrid,
        ImageType::BigPicture,
    ];
    let mut to_download = vec![];
    for image_type in types {
        let images_needed = shortcuts
            .iter()
            .filter(|s| search_results.contains_key(&s.app_id))
            .filter(|s| !known_images.contains(&image_type.file_name(s.app_id)));
        let image_ids: Vec<usize> = images_needed
            .clone()
            .filter_map(|s| search_results.get(&s.app_id))
            .copied()
            .collect();

        let shortcuts: Vec<&ShortcutOwned> = images_needed.collect();
        let image_search_result =
            get_images_for_ids(client, &image_ids, &image_type, download_animated).await;
        match image_search_result {
            Ok(images) => {
                let grid_folder = Path::new(user_data_folder).join("config").join("grid");
                let images = images
                    .iter()
                    .enumerate()
                    .map(|(index, image)| (image, shortcuts[index], image_ids[index]));
                let download_for_this_type = stream::iter(images)
                    .filter_map(|(image, shortcut, game_id)| {
                        let path = grid_folder.join(image_type.file_name(shortcut.app_id));
                        async move {
                            let image_url = match image {
                                Ok(img) => Some(img.url.clone()),
                                Err(_) => get_steam_image_url(game_id, &image_type).await,
                            };
                            if let Some(url) = image_url {
                                Some(ToDownload {
                                    path,
                                    url,
                                    app_name: shortcut.app_name.clone(),
                                    image_type: image_type.clone(),
                                })
                            } else {
                                None
                            }
                        }
                    })
                    .collect::<Vec<ToDownload>>()
                    .await;

                to_download.extend(download_for_this_type);
            }
            Err(err) => println!("Error getting images: {}", err),
        }
    }
    Ok(to_download)
}

async fn get_images_for_ids(
    client: &Client,
    image_ids: &[usize],
    image_type: &ImageType,
    download_animated: bool,
) -> Result<
    Vec<steamgriddb_api::response::SteamGridDbResult<steamgriddb_api::images::Image>>,
    Box<dyn std::error::Error>,
> {
    use steamgriddb_api::query_parameters::AnimtionType;
    use steamgriddb_api::query_parameters::QueryType::*;
    let anymation_type = if download_animated {
        Some(&[AnimtionType::Animated][..])
    } else {
        None
    };
    let big_picture_dims = [GridDimentions::D920x430, GridDimentions::D460x215];
    use steamgriddb_api::query_parameters::GridQueryParameters;
    let big_picture_parameters = GridQueryParameters {
        dimentions: Some(&big_picture_dims),
        types: anymation_type,
        ..Default::default()
    };
    use steamgriddb_api::query_parameters::HeroQueryParameters;
    let hero_parameters = HeroQueryParameters {
        types: anymation_type,
        ..Default::default()
    };
    let grid_parameters = GridQueryParameters {
        types: anymation_type,
        ..Default::default()
    };
    use steamgriddb_api::query_parameters::LogoQueryParameters;
    let logo_parameters = LogoQueryParameters {
        types: anymation_type,
        ..Default::default()
    };
    let query_type = match image_type {
        ImageType::Hero => Hero(Some(hero_parameters)),
        ImageType::BigPicture => Grid(Some(big_picture_parameters)),
        ImageType::Grid => Grid(Some(grid_parameters)),
        ImageType::WideGrid => Grid(Some(big_picture_parameters)),
        ImageType::Logo => Logo(Some(logo_parameters)),
    };

    let image_search_result = client.get_images_for_ids(image_ids, &query_type).await;
    image_search_result
}

async fn get_steam_image_url(game_id: usize, image_type: &ImageType) -> Option<String> {
    let steamgriddb_page_url = format!("https://www.steamgriddb.com/api/public/game/{}/", game_id);
    let response = reqwest::get(steamgriddb_page_url).await;
    match response {
        Ok(response) => {
            let text_response = response.json::<PublicGameResponse>().await;
            match text_response {
                Ok(response) => {
                    let game_id = response
                        .data
                        .clone()
                        .map(|d| d.platforms.map(|p| p.steam.map(|s| s.id)));
                    let mtime = response.data.map(|d| {
                        d.platforms
                            .map(|p| p.steam.map(|s| s.metadata.map(|m| m.store_asset_mtime)))
                    });
                    if let (Some(Some(Some(steam_app_id))), Some(Some(Some(Some(Some(mtime)))))) =
                        (game_id, mtime)
                    {
                        return Some(image_type.steam_url(steam_app_id, mtime));
                    }
                }
                Err(_) => (),
            }
        }
        Err(_) => (),
    }
    return None;
}

async fn download_to_download(to_download: &ToDownload) -> Result<(), Box<dyn Error>> {
    println!(
        "Downloading {:?} for {} to {:?}",
        to_download.image_type, to_download.app_name, to_download.path
    );
    let path = &to_download.path;
    let url = &to_download.url;
    let mut file = File::create(path).unwrap();
    let response = reqwest::get(url).await?;
    let content = response.bytes().await?;
    file.write_all(&content).unwrap();
    Ok(())
}

pub struct ToDownload {
    path: PathBuf,
    url: String,
    app_name: String,
    image_type: ImageType,
}
