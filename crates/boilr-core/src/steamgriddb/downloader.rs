use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::error::Error;
use steamgriddb_api::query_parameters::{
    GridDimentions, MimeType, MimeTypeIcon, MimeTypeLogo, Nsfw, Platform,
};
use tokio::sync::watch::Sender; // 0.3.1

use steam_shortcuts_util::shortcut::ShortcutOwned;
use steamgriddb_api::Client;

use super::CachedSearch;
use crate::settings::Settings;
use crate::steam::{get_shortcuts_for_user, get_users_images, SteamUsersInfo};
use crate::steamgriddb::ImageType;
use crate::sync::IsBoilRShortcut;
use crate::sync::SyncProgress;
use crate::sync::{identity_key, identity_of};

const CONCURRENT_REQUESTS: usize = 10;

impl SearchSettings for Settings {
    fn download_animated(&self) -> bool {
        self.steamgrid_db.prefer_animated
    }

    fn download_big_picture(&self) -> bool {
        self.steam.optimize_for_big_picture
    }

    fn allow_nsfw(&self) -> bool {
        self.steamgrid_db.allow_nsfw
    }

    fn only_download_boilr_images(&self) -> bool {
        self.steamgrid_db.only_download_boilr_images
    }

    fn is_image_banned(&self, image_type: &ImageType, app_id: u32) -> bool {
        self.steamgrid_db.is_image_banned(image_type, app_id)
    }

    fn match_by_store_id(&self) -> bool {
        self.steamgrid_db.match_by_store_id
    }
}

/// The SteamGridDB platform and id a shortcut's store identity names, for the stores
/// SteamGridDB knows games by: Epic (`appName`), EA (offer id) and Battle.net (product code).
fn store_platform(shortcut: &ShortcutOwned) -> Option<(Platform, String)> {
    let key = identity_key(identity_of(shortcut)?);
    let (store, id) = key.split_once(':')?;
    let platform = match store {
        "epic" => Platform::EpicGameStore,
        "ea" => Platform::Origin,
        "battlenet" => Platform::BattleNet,
        _ => return None,
    };
    (!id.is_empty()).then(|| (platform, id.to_string()))
}

/// The title to search SteamGridDB for: the shortcut's name without the label BoilR may add
/// to games that are not installed ("Frostpunk (GOG, not installed)").
fn search_title(shortcut: &ShortcutOwned) -> &str {
    let store =
        identity_of(shortcut).and_then(|identity| identity.split_once(':').map(|(store, _)| store));
    match store {
        Some(store) => crate::owned::strip_label(&shortcut.app_name, store),
        None => &shortcut.app_name,
    }
}

fn platform_key(platform: &Platform) -> &'static str {
    match platform {
        Platform::Steam => "steam",
        Platform::Origin => "origin",
        Platform::EpicGameStore => "egs",
        Platform::BattleNet => "bnet",
        Platform::Uplay => "uplay",
        Platform::Flashpoint => "flashpoint",
    }
}

pub async fn download_images_for_users(
    settings: &Settings,
    users: &[SteamUsersInfo],
    sender: &mut Option<Sender<SyncProgress>>,
) {
    let auth_key = &settings.steamgrid_db.auth_key;
    if let Some(auth_key) = auth_key {
        println!("Checking for game images");
        let start_time = std::time::Instant::now();
        let client = steamgriddb_api::Client::new(auth_key);
        let search = CachedSearch::new(&client);
        let search = &search;
        let client = &client;
        if let Some(sender) = sender {
            let _ = sender.send(SyncProgress::FindingImages);
        }

        let users_info = users.iter().filter_map(|user| {
            let shortcut_info = get_shortcuts_for_user(user);
            shortcut_info
                .map(|shortcut_info| {
                    let data_folder = &user.steam_user_data_folder;
                    (shortcut_info, data_folder)
                })
                .ok()
        });
        let to_downloads = stream::iter(users_info)
            .map(|(shortcut_info, data_folder)| async move {
                let known_images = get_users_images(data_folder).unwrap_or_default();
                let res = search_for_images_to_download(
                    known_images,
                    data_folder.as_str(),
                    &shortcut_info.shortcuts,
                    search,
                    client,
                    settings,
                )
                .await;
                res.unwrap_or_default()
            })
            .buffer_unordered(CONCURRENT_REQUESTS)
            .collect::<Vec<Vec<ToDownload>>>()
            .await;
        let to_downloads = to_downloads.iter().flatten().collect::<Vec<&ToDownload>>();
        let total = to_downloads.len();
        if !to_downloads.is_empty() {
            if let Some(sender) = sender {
                let _ = sender.send(SyncProgress::DownloadingImages { to_download: total });
            }
            search.save();
            stream::iter(&to_downloads)
                .map(|to_download| async move {
                    if let Err(e) = download_to_download(to_download).await {
                        println!("Error downloading {:?}: {}", to_download.path, e);
                        dbg!(&e);
                    }
                })
                .buffer_unordered(CONCURRENT_REQUESTS)
                .collect::<Vec<()>>()
                .await;
            let duration = start_time.elapsed();
            println!("Finished getting images in: {duration:?}");

            //Validate that the downloads where ok
            for to_download in to_downloads {
                let file_length = std::fs::metadata(&to_download.path)
                    .map(|m| m.len())
                    .unwrap_or_default();
                if file_length < 2 {
                    // Image is too small, something went wrong
                    //Try to delete file again, don't care if it fails
                    let _ = std::fs::remove_file(&to_download.path);
                }
            }
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
    clienticon: Option<String>,
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

pub trait SearchSettings {
    fn download_animated(&self) -> bool;
    fn download_big_picture(&self) -> bool;
    fn allow_nsfw(&self) -> bool;
    fn only_download_boilr_images(&self) -> bool;
    fn is_image_banned(&self, image_type: &ImageType, app_id: u32) -> bool;
    /// Whether to find artwork by store id before searching by name.
    fn match_by_store_id(&self) -> bool;
}

async fn search_for_images_to_download<T: SearchSettings>(
    known_images: Vec<String>,
    user_data_folder: &str,
    shortcuts: &[ShortcutOwned],
    search: &CachedSearch<'_>,
    client: &Client,
    search_settins: &T,
) -> Result<Vec<ToDownload>, Box<dyn Error>> {
    let types = {
        let mut types = vec![
            ImageType::Logo,
            ImageType::Hero,
            ImageType::Grid,
            ImageType::WideGrid,
            ImageType::Icon,
        ];
        if search_settins.download_big_picture() {
            types.push(ImageType::BigPicture);
        }
        types
    };

    let shortcuts_to_search_for = shortcuts
        .iter()
        .filter(|s| !search_settins.only_download_boilr_images() || s.is_boilr_shortcut())
        .filter(|s| {
            // if we are missing any of the images we need to search for them
            types
                .iter()
                .map(|t| t.file_name_no_extension(s.app_id))
                .any(|image| !known_images.contains(&image))
                && !s.app_name.is_empty()
        });
    let shortcuts_to_search_for: Vec<&ShortcutOwned> = shortcuts_to_search_for.collect();
    if shortcuts_to_search_for.is_empty() {
        return Ok(vec![]);
    }
    let grid_folder = Path::new(user_data_folder).join("config").join("grid");
    let needs = |s: &ShortcutOwned, image_type: &ImageType| {
        !search_settins.is_image_banned(image_type, s.app_id)
            && !known_images.contains(&image_type.file_name_no_extension(s.app_id))
    };

    // Artwork found by store id first; what it does not find is searched for by name.
    let mut to_download = vec![];
    let mut resolved: HashSet<(u32, ImageType)> = HashSet::new();
    if search_settins.match_by_store_id() {
        let (found, found_for) = images_by_store_id(
            &shortcuts_to_search_for,
            &types,
            &needs,
            client,
            search_settins,
            &grid_folder,
        )
        .await;
        to_download.extend(found);
        resolved.extend(found_for);
    }
    let shortcuts_to_search_for: Vec<&ShortcutOwned> = shortcuts_to_search_for
        .into_iter()
        .filter(|s| {
            types
                .iter()
                .any(|t| needs(s, t) && !resolved.contains(&(s.app_id, *t)))
        })
        .collect();
    let mut search_results = HashMap::new();
    let search_results_a = stream::iter(shortcuts_to_search_for)
        .map(|s| async move {
            let search_result = search.search(s.app_id, search_title(s)).await;
            match search_result {
                Ok(Some(search_result)) => Some((s.app_id, search_result)),
                _ => None,
            }
        })
        .buffer_unordered(CONCURRENT_REQUESTS)
        .collect::<Vec<Option<(u32, usize)>>>()
        .await;
    for (app_id, search) in search_results_a.into_iter().flatten() {
        search_results.insert(app_id, search);
    }

    for image_type in types {
        // Each shortcut with the SteamGridDB game id its name search found.
        let wanted: Vec<(&ShortcutOwned, usize)> = shortcuts
            .iter()
            .filter(|s| needs(s, &image_type))
            .filter(|s| !resolved.contains(&(s.app_id, image_type)))
            .filter_map(|s| search_results.get(&s.app_id).map(|game_id| (s, *game_id)))
            .collect();

        // SteamGridDB answers at most 99 ids per request. Pair every answer with the shortcut
        // of its own batch: indexing into the full list put batches 2 and up on the first
        // 99 games, which then showed other games' artwork while the rest got none.
        for batch in wanted.chunks(99) {
            let image_ids: Vec<usize> = batch.iter().map(|(_, game_id)| *game_id).collect();
            let image_search_result = get_images_for_ids(
                client,
                &image_ids,
                &image_type,
                search_settins.download_animated(),
                search_settins.allow_nsfw(),
            )
            .await;
            match image_search_result {
                Ok(images) => {
                    let images = images
                        .iter()
                        .zip(batch)
                        .map(|(image, (shortcut, game_id))| (image, *shortcut, game_id));
                    let download_for_this_type = stream::iter(images)
                        .filter_map(|(image, shortcut, game_id)| {
                            let extension = image
                                .as_ref()
                                .map(|image| get_image_extension(&image.mime))
                                .unwrap_or("png");
                            let path =
                                grid_folder.join(image_type.file_name(shortcut.app_id, extension));
                            async move {
                                let image_url = match image {
                                    Ok(img) => Some(img.url.clone()),
                                    Err(_) => get_steam_image_url(*game_id, &image_type).await,
                                };
                                image_url.map(|url| ToDownload {
                                    path,
                                    url,
                                    app_name: shortcut.app_name.clone(),
                                    image_type,
                                })
                            }
                        })
                        .collect::<Vec<ToDownload>>()
                        .await;

                    to_download.extend(download_for_this_type);
                }
                Err(err) => eprintln!("Error getting images: {err}"),
            }
        }
    }
    Ok(to_download)
}

/// The shortcuts of one SteamGridDB platform, with their id there.
type StoreGames<'a> = (Platform, Vec<(&'a ShortcutOwned, String)>);

/// Artwork for the shortcuts SteamGridDB knows by store id ([`store_platform`]), and which
/// `(app id, image type)` it covers. A game SteamGridDB does not know by that id is left out,
/// for the name search.
async fn images_by_store_id<T: SearchSettings>(
    shortcuts: &[&ShortcutOwned],
    types: &[ImageType],
    needs: &impl Fn(&ShortcutOwned, &ImageType) -> bool,
    client: &Client,
    search_settins: &T,
    grid_folder: &Path,
) -> (Vec<ToDownload>, Vec<(u32, ImageType)>) {
    let mut to_download = vec![];
    let mut found_for = vec![];
    let mut by_platform: HashMap<&'static str, StoreGames> = HashMap::new();
    for shortcut in shortcuts {
        if let Some((platform, id)) = store_platform(shortcut) {
            by_platform
                .entry(platform_key(&platform))
                .or_insert_with(|| (platform, vec![]))
                .1
                .push((shortcut, id));
        }
    }
    for (platform, games) in by_platform.values() {
        for image_type in types {
            let wanted: Vec<&(&ShortcutOwned, String)> =
                games.iter().filter(|(s, _)| needs(s, image_type)).collect();
            for chunk in wanted.chunks(99) {
                let ids: Vec<&str> = chunk.iter().map(|(_, id)| id.as_str()).collect();
                let query = get_query_type(
                    search_settins.download_animated(),
                    image_type,
                    search_settins.allow_nsfw(),
                );
                // A single id gets SteamGridDB's single-game answer, not a list of answers.
                let images: Vec<Option<steamgriddb_api::images::Image>> = match ids.as_slice() {
                    [id] => match client
                        .get_images_for_platform_id(platform, id, &query)
                        .await
                    {
                        Ok(images) => vec![images.into_iter().next()],
                        Err(_) => vec![None],
                    },
                    _ => match client
                        .get_images_for_platform_ids(platform, &ids, &query)
                        .await
                    {
                        Ok(results) => results.into_iter().map(Result::ok).collect(),
                        Err(err) => {
                            eprintln!("Could not get artwork by store id: {err}");
                            continue;
                        }
                    },
                };
                for ((shortcut, _), image) in chunk.iter().zip(images) {
                    let Some(image) = image else {
                        continue;
                    };
                    let extension = get_image_extension(&image.mime);
                    to_download.push(ToDownload {
                        path: grid_folder.join(image_type.file_name(shortcut.app_id, extension)),
                        url: image.url.clone(),
                        app_name: shortcut.app_name.clone(),
                        image_type: *image_type,
                    });
                    found_for.push((shortcut.app_id, *image_type));
                }
            }
        }
    }
    (to_download, found_for)
}

pub fn get_image_extension(mime_type: &steamgriddb_api::images::MimeTypes) -> &'static str {
    match mime_type {
        steamgriddb_api::images::MimeTypes::Default(MimeType::Jpeg) => "jpg",
        steamgriddb_api::images::MimeTypes::Default(MimeType::Png) => "png",
        steamgriddb_api::images::MimeTypes::Default(MimeType::Webp) => "webp",
        steamgriddb_api::images::MimeTypes::Logo(MimeTypeLogo::Png) => "png",
        steamgriddb_api::images::MimeTypes::Logo(MimeTypeLogo::Webp) => "webp",
        steamgriddb_api::images::MimeTypes::Icon(MimeTypeIcon::Icon) => "ico",
        steamgriddb_api::images::MimeTypes::Icon(MimeTypeIcon::Png) => "png",
    }
}

async fn get_images_for_ids(
    client: &Client,
    image_ids: &[usize],
    image_type: &ImageType,
    download_animated: bool,
    allow_nsfw: bool,
) -> Result<Vec<steamgriddb_api::response::SteamGridDbResult<steamgriddb_api::images::Image>>, String>
{
    let query_type = get_query_type(download_animated, image_type, allow_nsfw);

    let image_search_result = client.get_images_for_ids(image_ids, &query_type).await;

    image_search_result.map_err(|e| format!("Image search failed {e:?}"))
}

const BIG_PICTURE_DIMS: [GridDimentions; 2] = [GridDimentions::D920x430, GridDimentions::D460x215];

pub fn get_query_type(
    download_animated: bool,
    image_type: &ImageType,
    allow_nsfw: bool,
) -> steamgriddb_api::QueryType<'_> {
    let anymation_type = if download_animated {
        Some(&[steamgriddb_api::query_parameters::AnimtionType::Animated][..])
    } else {
        None
    };
    use steamgriddb_api::query_parameters::GridQueryParameters;
    let allow_nsfw_enum = match allow_nsfw {
        true => Some(&Nsfw::Any),
        false => Some(&Nsfw::False),
    };
    let big_picture_parameters = GridQueryParameters {
        dimentions: Some(&BIG_PICTURE_DIMS),
        types: anymation_type,
        nsfw: allow_nsfw_enum,
        ..Default::default()
    };
    use steamgriddb_api::query_parameters::HeroQueryParameters;
    let hero_parameters = HeroQueryParameters {
        types: anymation_type,
        nsfw: allow_nsfw_enum,
        ..Default::default()
    };
    let grid_parameters = GridQueryParameters {
        types: anymation_type,
        nsfw: allow_nsfw_enum,
        ..Default::default()
    };
    use steamgriddb_api::query_parameters::LogoQueryParameters;
    let logo_parameters = LogoQueryParameters {
        types: anymation_type,
        nsfw: allow_nsfw_enum,
        ..Default::default()
    };

    use steamgriddb_api::query_parameters::IconQueryParameters;
    let icon_parameters = IconQueryParameters {
        nsfw: allow_nsfw_enum,
        ..Default::default()
    };
    let query_type = match image_type {
        ImageType::Hero => steamgriddb_api::QueryType::Hero(Some(hero_parameters)),
        ImageType::BigPicture => steamgriddb_api::QueryType::Grid(Some(big_picture_parameters)),
        ImageType::Grid => steamgriddb_api::QueryType::Grid(Some(grid_parameters)),
        ImageType::WideGrid => steamgriddb_api::QueryType::Grid(Some(big_picture_parameters)),
        ImageType::Logo => steamgriddb_api::QueryType::Logo(Some(logo_parameters)),
        ImageType::Icon => steamgriddb_api::QueryType::Icon(Some(icon_parameters)),
    };
    query_type
}

async fn get_steam_image_url(game_id: usize, image_type: &ImageType) -> Option<String> {
    if let ImageType::Icon = image_type {
        if let Some(url) = get_steam_icon_url(game_id).await {
            return Some(url);
        }
    }
    let steamgriddb_page_url = format!("https://www.steamgriddb.com/api/public/game/{game_id}/");
    let response = reqwest::get(steamgriddb_page_url).await;
    if let Ok(response) = response {
        let text_response = response.json::<PublicGameResponse>().await;
        if let Ok(response) = text_response {
            let game_id = response
                .data
                .clone()
                .map(|d| d.platforms.map(|p| p.steam.map(|s| s.id)));
            let mtime = response.data.map(|d| {
                d.platforms
                    .map(|p| p.steam.map(|s| s.metadata.map(|m| m.store_asset_mtime)))
            });
            if let (Some(Some(Some(steam_app_id))), Some(Some(Some(Some(mtime))))) =
                (game_id, mtime)
            {
                return Some(image_type.steam_url(&steam_app_id, mtime?));
            }
        }
    }
    None
}

async fn get_steam_icon_url(game_id: usize) -> Option<String> {
    let steamgriddb_page_url = format!("https://www.steamgriddb.com/api/public/game/{game_id}/");
    let response = reqwest::get(steamgriddb_page_url).await;
    if let Ok(response) = response {
        let text_response = response.json::<PublicGameResponse>().await;
        if let Ok(response) = text_response {
            let game_id = response
                .data
                .clone()
                .map(|d| d.platforms.map(|p| p.steam.map(|s| s.id)));
            let mtime = response.data.map(|d| {
                d.platforms
                    .map(|p| p.steam.map(|s| s.metadata.map(|m| m.clienticon)))
            });
            if let (Some(Some(Some(steam_app_id))), Some(Some(Some(Some(mtime))))) =
                (game_id, mtime)
            {
                return Some(icon_url(&steam_app_id.to_string(), &mtime?.to_string()));
            }
        }
    }
    None
}

fn icon_url(steam_app_id: &str, icon_id: &str) -> String {
    format!(
        "https://cdn.cloudflare.steamstatic.com/steamcommunity/public/images/apps/{steam_app_id}/{icon_id}.ico"
    )
}

pub async fn download_to_download(to_download: &ToDownload) -> eyre::Result<()> {
    println!(
        "Downloading {:?} for {} to {:?}",
        to_download.image_type, to_download.app_name, to_download.path
    );
    let path = &to_download.path;
    let url = &to_download.url;
    let mut file = File::create(path)?;
    let response = reqwest::get(url).await?;
    let content = response.bytes().await?;
    file.write_all(&content)?;
    Ok(())
}

pub struct ToDownload {
    pub path: PathBuf,
    pub url: String,
    pub app_name: String,
    pub image_type: ImageType,
}

#[cfg(test)]
mod store_id_tests {
    use super::*;
    use steam_shortcuts_util::Shortcut;

    fn shortcut(name: &str, identity: &str) -> ShortcutOwned {
        let mut shortcut = Shortcut::new("0", name, "exe", "", "", "", "").to_owned();
        shortcut.dev_kit_game_id = identity.to_string();
        shortcut
    }

    #[test]
    fn stores_steamgriddb_knows_are_looked_up_by_id() {
        let epic = shortcut("Fortnite", "boilr:epic:Fortnite");
        assert!(matches!(
            store_platform(&epic),
            Some((Platform::EpicGameStore, id)) if id == "Fortnite"
        ));
        let ea = shortcut("Zuma", "boilr:ea:OFB-EAST:52735,1002139");
        assert!(matches!(
            store_platform(&ea),
            Some((Platform::Origin, id)) if id == "OFB-EAST:52735"
        ));
        let bnet = shortcut("Overwatch", "boilr:battlenet:Pro");
        assert!(matches!(
            store_platform(&bnet),
            Some((Platform::BattleNet, _))
        ));
        // GOG and Amazon, and shortcuts without an identity, are searched by name.
        assert!(store_platform(&shortcut("Unreal", "boilr:gog:1")).is_none());
        assert!(store_platform(&shortcut("Game", "boilr")).is_none());
    }

    #[test]
    fn the_search_uses_the_title_without_a_label() {
        let labelled = shortcut("Frostpunk (GOG, not installed)", "boilr:gog:1");
        assert_eq!(search_title(&labelled), "Frostpunk");
        let plain = shortcut("Frostpunk (2018)", "boilr:gog:1");
        assert_eq!(search_title(&plain), "Frostpunk (2018)");
        let other = shortcut("Game (Not installed)", "boilr");
        assert_eq!(search_title(&other), "Game (Not installed)");
    }
}
