//! Artwork for Steam shortcuts: what each shortcut has now, choices from SteamGridDB, and
//! setting, removing or banning images. Mirrors the egui Images screen.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use boilr::platforms::{get_platforms, platform_sections};
use boilr_core::{
    settings::{save_settings, Settings},
    steam::{get_shortcuts_for_user, get_shortcuts_paths, SteamUsersInfo},
    steamgriddb::{get_image_extension, get_query_type, CachedSearch, ImageType, ToDownload},
    sync::{self, download_images, IsBoilRShortcut},
};
use serde::Serialize;
use tauri::Manager;

/// File extensions Steam artwork is stored with, as in the egui app.
const EXTENSIONS: [&str; 4] = ["png", "jpg", "ico", "webp"];

/// The image kinds, keyed by the names the frontend uses.
const KINDS: [(&str, ImageType); 6] = [
    ("grid", ImageType::Grid),
    ("wide_grid", ImageType::WideGrid),
    ("hero", ImageType::Hero),
    ("logo", ImageType::Logo),
    ("icon", ImageType::Icon),
    ("big_picture", ImageType::BigPicture),
];

fn kind(name: &str) -> Result<ImageType, String> {
    KINDS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, t)| *t)
        .ok_or_else(|| format!("Unknown image kind: {name}"))
}

fn settings() -> Result<Settings, String> {
    Settings::new().map_err(|err| err.to_string())
}

/// Runs a future to completion from a sync command. Tauri may run these on one of its runtime's
/// worker threads, where starting a second runtime panics, so reuse the running one there.
fn block_on<F: std::future::Future>(future: F) -> Result<F::Output, String> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => Ok(tokio::task::block_in_place(|| handle.block_on(future))),
        Err(_) => tokio::runtime::Runtime::new()
            .map(|rt| rt.block_on(future))
            .map_err(|err| err.to_string()),
    }
}

fn client(settings: &Settings) -> Result<steamgriddb_api::Client, String> {
    settings
        .steamgrid_db
        .auth_key
        .as_ref()
        .filter(|key| !key.trim().is_empty())
        .map(steamgriddb_api::Client::new)
        .ok_or_else(|| "Add a SteamGridDB key in Settings first.".to_string())
}

fn users(settings: &Settings) -> Result<Vec<SteamUsersInfo>, String> {
    get_shortcuts_paths(&settings.steam).map_err(|err| err.to_string())
}

fn user(settings: &Settings, user_id: &str) -> Result<SteamUsersInfo, String> {
    users(settings)?
        .into_iter()
        .find(|u| u.user_id == user_id)
        .ok_or_else(|| format!("Steam account {user_id} not found."))
}

fn grid_folder(user: &SteamUsersInfo) -> PathBuf {
    Path::new(&user.steam_user_data_folder)
        .join("config")
        .join("grid")
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LocalImage {
    pub path: String,
    /// Last modified time in seconds, so the UI can refresh a changed image.
    pub version: u64,
}

/// The current image of a kind, whichever extension it was saved with.
fn current_image(folder: &Path, app_id: u32, image_type: &ImageType) -> Option<LocalImage> {
    EXTENSIONS.iter().find_map(|ext| {
        let path = folder.join(image_type.file_name(app_id, ext));
        let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
        Some(LocalImage {
            path: path.to_string_lossy().to_string(),
            version: modified
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        })
    })
}

fn remove_images(folder: &Path, app_id: u32, image_type: &ImageType) {
    for ext in EXTENSIONS {
        let _ = std::fs::remove_file(folder.join(image_type.file_name(app_id, ext)));
    }
}

#[derive(Debug, Serialize)]
pub struct SteamAccount {
    pub user_id: String,
    pub shortcut_count: usize,
}

/// Steam accounts on this computer that have non-Steam shortcuts.
#[tauri::command(async)]
pub fn list_steam_accounts() -> Result<Vec<SteamAccount>, String> {
    let settings = settings()?;
    let mut accounts: Vec<SteamAccount> = users(&settings)?
        .iter()
        .filter_map(|u| {
            let info = get_shortcuts_for_user(u).ok()?;
            Some(SteamAccount {
                user_id: u.user_id.clone(),
                shortcut_count: info.shortcuts.len(),
            })
        })
        .filter(|a| a.shortcut_count > 0)
        .collect();
    accounts.sort_by_key(|a| std::cmp::Reverse(a.shortcut_count));
    Ok(accounts)
}

#[derive(Debug, Serialize)]
pub struct ArtworkGame {
    pub app_id: u32,
    pub name: String,
    pub from_boilr: bool,
    pub images: HashMap<&'static str, LocalImage>,
    /// Kinds BoilR has been told never to download for this game.
    pub never_download: Vec<&'static str>,
}

/// Every shortcut of a Steam account with the artwork it has now.
#[tauri::command(async)]
pub fn list_artwork<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    user_id: String,
) -> Result<Vec<ArtworkGame>, String> {
    let settings = settings()?;
    let user = user(&settings, &user_id)?;
    let folder = grid_folder(&user);
    // Let the web view load images from this account's artwork folder, and nothing else.
    let _ = std::fs::create_dir_all(&folder);
    app.asset_protocol_scope()
        .allow_directory(&folder, false)
        .map_err(|err| err.to_string())?;

    let info = get_shortcuts_for_user(&user).map_err(|err| err.to_string())?;
    let mut games: Vec<ArtworkGame> = info
        .shortcuts
        .iter()
        .map(|s| ArtworkGame {
            app_id: s.app_id,
            name: s.app_name.clone(),
            from_boilr: s.is_boilr_shortcut(),
            images: KINDS
                .iter()
                .filter_map(|(name, t)| current_image(&folder, s.app_id, t).map(|img| (*name, img)))
                .collect(),
            never_download: KINDS
                .iter()
                .filter(|(_, t)| settings.steamgrid_db.is_image_banned(t, s.app_id))
                .map(|(name, _)| *name)
                .collect(),
        })
        .collect();
    games.sort_by_key(|g| g.name.to_lowercase());
    Ok(games)
}

#[derive(Debug, Serialize)]
pub struct ArtworkOption {
    pub id: u32,
    pub thumb: String,
    pub url: String,
    pub extension: &'static str,
    pub width: u32,
    pub height: u32,
    pub author: String,
}

/// SteamGridDB's images of one kind for a game, honouring the animation and NSFW settings.
#[tauri::command(async)]
pub fn artwork_options(
    app_id: u32,
    name: String,
    kind_name: String,
) -> Result<Vec<ArtworkOption>, String> {
    let settings = settings()?;
    let image_type = kind(&kind_name)?;
    let client = client(&settings)?;
    block_on(async {
        let search = CachedSearch::new(&client);
        let grid_id = search
            .search(app_id, name.as_str())
            .await
            .map_err(|e| e.to_string())?;
        search.save();
        let Some(grid_id) = grid_id else {
            return Ok(vec![]);
        };
        let query = get_query_type(
            settings.steamgrid_db.prefer_animated,
            &image_type,
            settings.steamgrid_db.allow_nsfw,
        );
        let images = client
            .get_images_for_id(grid_id, &query)
            .await
            .map_err(|e| e.to_string())?;
        Ok(images
            .into_iter()
            .map(|i| ArtworkOption {
                id: i.id,
                extension: get_image_extension(&i.mime),
                thumb: i.thumb,
                url: i.url,
                width: i.width,
                height: i.height,
                author: i.author.name,
            })
            .collect())
    })?
}

/// Downloads a chosen image, replacing any image of that kind the game had.
#[tauri::command(async)]
pub fn set_artwork(
    user_id: String,
    app_id: u32,
    name: String,
    kind_name: String,
    url: String,
    extension: String,
) -> Result<LocalImage, String> {
    let settings = settings()?;
    let image_type = kind(&kind_name)?;
    if !EXTENSIONS.contains(&extension.as_str()) {
        return Err(format!("Unsupported image type: {extension}"));
    }
    if !url.starts_with("https://") {
        return Err("Images must come from an https address.".to_string());
    }
    let folder = grid_folder(&user(&settings, &user_id)?);
    std::fs::create_dir_all(&folder).map_err(|err| err.to_string())?;
    remove_images(&folder, app_id, &image_type);
    let to_download = ToDownload {
        path: folder.join(image_type.file_name(app_id, &extension)),
        url,
        app_name: name,
        image_type,
    };
    block_on(boilr_core::steamgriddb::download_to_download(&to_download))?
        .map_err(|e| e.to_string())?;
    if matches!(image_type, ImageType::Icon | ImageType::BigPicture) {
        point_shortcuts_at_icons(&settings);
    }
    current_image(&folder, app_id, &image_type)
        .ok_or_else(|| "The download produced no file.".to_string())
}

/// Removes a game's image of one kind; with `never_download`, BoilR also stops fetching it.
#[tauri::command(async)]
pub fn clear_artwork(
    user_id: String,
    app_id: u32,
    kind_name: String,
    never_download: bool,
) -> Result<Settings, String> {
    let mut settings = settings()?;
    let image_type = kind(&kind_name)?;
    remove_images(
        &grid_folder(&user(&settings, &user_id)?),
        app_id,
        &image_type,
    );
    if settings.steamgrid_db.is_image_banned(&image_type, app_id) != never_download {
        settings
            .steamgrid_db
            .set_image_banned(&image_type, app_id, never_download);
        save_settings(&settings, &platform_sections(&get_platforms()))
            .map_err(|e| e.to_string())?;
    }
    Ok(settings)
}

#[derive(Debug, Serialize)]
pub struct GameCandidate {
    pub id: usize,
    pub name: String,
    pub year: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct GameMatch {
    /// The SteamGridDB game BoilR uses for this shortcut, if any.
    pub current_id: Option<usize>,
    pub candidates: Vec<GameCandidate>,
}

fn year_of(timestamp: Option<usize>) -> Option<i32> {
    let seconds = i64::try_from(timestamp?).ok()?;
    time::OffsetDateTime::from_unix_timestamp(seconds)
        .ok()
        .map(|d| d.year())
}

/// Which SteamGridDB game a shortcut is matched to, and other games with a similar name.
#[tauri::command(async)]
pub fn artwork_game_match(
    app_id: u32,
    name: String,
    query: Option<String>,
) -> Result<GameMatch, String> {
    let settings = settings()?;
    let client = client(&settings)?;
    block_on(async {
        let search = CachedSearch::new(&client);
        let current_id = search
            .search(app_id, name.as_str())
            .await
            .map_err(|e| e.to_string())?;
        search.save();
        let results = client
            .search(query.as_deref().unwrap_or(&name))
            .await
            .map_err(|e| e.to_string())?;
        Ok(GameMatch {
            current_id,
            candidates: results
                .into_iter()
                .take(8)
                .map(|r| GameCandidate {
                    id: r.id,
                    name: r.name,
                    year: year_of(r.release_date),
                })
                .collect(),
        })
    })?
}

/// Points a shortcut at a different SteamGridDB game.
#[tauri::command(async)]
pub fn set_artwork_game(app_id: u32, name: String, grid_id: usize) -> Result<(), String> {
    let settings = settings()?;
    let client = client(&settings)?;
    let mut search = CachedSearch::new(&client);
    search.set_cache(app_id, name, grid_id);
    Ok(())
}

/// Downloads artwork for every shortcut that is missing some, like the end of an import.
#[tauri::command(async)]
pub fn find_missing_artwork() -> Result<(), String> {
    let settings = settings()?;
    client(&settings)?;
    let users = users(&settings)?;
    block_on(download_images(&settings, &users, &mut None))?;
    point_shortcuts_at_icons(&settings);
    Ok(())
}

/// Steam takes a shortcut's small icon from its `icon` field, not from the grid folder.
fn point_shortcuts_at_icons(settings: &Settings) {
    if let Err(err) = sync::fix_all_shortcut_icons(settings) {
        eprintln!("Could not point shortcuts at their icons: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_on_works_on_a_runtime_worker() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .build()
            .expect("runtime");
        let result =
            runtime.block_on(async { tokio::spawn(async { block_on(async { 2 }) }).await });
        assert_eq!(result.expect("task"), Ok(2));
    }

    #[test]
    fn finds_current_image_with_any_extension() {
        let dir = std::env::temp_dir().join(format!("boilr-art-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create folder");
        assert_eq!(current_image(&dir, 42, &ImageType::Hero), None);

        std::fs::write(dir.join("42_hero.webp"), b"x").expect("write");
        let found = current_image(&dir, 42, &ImageType::Hero).expect("hero found");
        assert!(found.path.ends_with("42_hero.webp"));
        assert_eq!(current_image(&dir, 42, &ImageType::Grid), None);

        std::fs::write(dir.join("42_hero.png"), b"x").expect("write");
        remove_images(&dir, 42, &ImageType::Hero);
        assert_eq!(current_image(&dir, 42, &ImageType::Hero), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn knows_every_kind() {
        for (name, _) in KINDS {
            assert!(kind(name).is_ok());
        }
        assert!(kind("poster").is_err());
    }

    #[test]
    fn release_year_from_timestamp() {
        assert_eq!(year_of(Some(1_600_000_000)), Some(2020));
        assert_eq!(year_of(None), None);
    }

    /// Talks to the real SteamGridDB. Run with a key file and a copy of a Steam userdata folder:
    /// BOILR_SGDB_KEY_FILE=... BOILR_TEST_SHORTCUTS=.../shortcuts.vdf \
    ///   cargo test live_steamgriddb -- --ignored --exact artwork::tests::live_steamgriddb
    #[test]
    #[ignore = "needs network and a SteamGridDB key"]
    fn live_steamgriddb() {
        let key = std::fs::read_to_string(std::env::var("BOILR_SGDB_KEY_FILE").expect("key file"))
            .expect("read key");
        let shortcuts = std::env::var("BOILR_TEST_SHORTCUTS").expect("shortcuts.vdf to copy");
        let root = std::env::temp_dir().join(format!("boilr-art-live-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let config_dir = root.join("config").join("boilr");
        let user_dir = root
            .join("steam")
            .join("userdata")
            .join("10342635")
            .join("config");
        std::fs::create_dir_all(&config_dir).expect("config dir");
        std::fs::create_dir_all(&user_dir).expect("user dir");
        std::fs::copy(&shortcuts, user_dir.join("shortcuts.vdf")).expect("copy shortcuts");
        std::fs::write(
            config_dir.join("config.toml"),
            format!(
                "[steam]\nlocation = \"{}\"\n[steamgrid_db]\nenabled = true\nauth_key = \"{}\"\n",
                root.join("steam").display(),
                key.trim()
            ),
        )
        .expect("write config");
        std::env::set_var("XDG_CONFIG_HOME", root.join("config"));

        let accounts = list_steam_accounts().expect("accounts");
        assert_eq!(
            accounts.first().map(|a| a.user_id.as_str()),
            Some("10342635")
        );

        let app = tauri::test::mock_app();
        let games = list_artwork(app.handle().clone(), "10342635".to_string()).expect("artwork");
        let game = games
            .iter()
            .find(|g| g.name == "Outer Wilds")
            .expect("Outer Wilds shortcut");
        assert!(game.images.is_empty(), "fresh copy has no artwork");

        let options =
            artwork_options(game.app_id, game.name.clone(), "grid".to_string()).expect("options");
        assert!(
            !options.is_empty(),
            "SteamGridDB has covers for Outer Wilds"
        );
        let pick = &options[0];
        let saved = set_artwork(
            "10342635".to_string(),
            game.app_id,
            game.name.clone(),
            "grid".to_string(),
            pick.url.clone(),
            pick.extension.to_string(),
        )
        .expect("download cover");
        assert!(std::fs::metadata(&saved.path).expect("saved file").len() > 1000);

        let matched = artwork_game_match(game.app_id, game.name.clone(), None).expect("match");
        assert!(matched.current_id.is_some());
        assert!(matched
            .candidates
            .iter()
            .any(|c| c.name.contains("Outer Wilds")));

        let settings = clear_artwork(
            "10342635".to_string(),
            game.app_id,
            "grid".to_string(),
            true,
        )
        .expect("clear");
        assert!(settings
            .steamgrid_db
            .is_image_banned(&ImageType::Grid, game.app_id));
        assert!(!std::path::Path::new(&saved.path).exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
