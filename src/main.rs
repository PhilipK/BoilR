use std::{
    collections::HashMap,
    env::{self},
    fmt,
    fs::File,
    io::Write,
    path::Path,
};
mod cached_search;
mod egs;
use egs::{get_egs_manifests, ManifestItem};
use std::error::Error;
use steam_shortcuts_util::{
    parse_shortcuts, shortcut::ShortcutOwned, shortcuts_to_bytes, Shortcut,
};
use steamgriddb_api::{search::SearchResult, Client};

use crate::cached_search::CachedSearch;

pub struct ShortcutInfo {
    pub path: String,
    pub shortcuts: Vec<ShortcutOwned>,
}



fn get_shortcuts_for_user(user: &SteamUsersInfo) -> ShortcutInfo {
    let mut shortcuts = vec![];
    let mut new_path = user.shortcut_path.clone();
    if let Some(shortcut_path) = &user.shortcut_path {
        // std::fs::remove_file(path)
        //TODO remove unwrap
        let content = std::fs::read(shortcut_path).unwrap();
        shortcuts = parse_shortcuts(content.as_slice())
            .unwrap()
            .iter()
            .map(|s| s.to_owned())
            .collect();
        println!(
            "Found {} shortcuts , for user: {}",
            shortcuts.len(),
            user.steam_user_data_folder
        );
    } else {
        println!(
            "Did not find a shortcut file for user {}, createing a new",
            user.steam_user_data_folder
        );
        std::fs::create_dir_all(format!("{}/{}", user.steam_user_data_folder, "config")).unwrap();
        new_path = Some(format!(
            "{}/{}",
            user.steam_user_data_folder, "config/shortcuts.vdf"
        ));
    }
    ShortcutInfo {
        shortcuts,
        path: new_path.unwrap(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    
    let auth_key = std::fs::read_to_string("auth_key.txt")?;
    let client = steamgriddb_api::Client::new(auth_key);
    let mut search = CachedSearch::new(&client);

    let egs_manifests = get_egs_manifests()?;
    let egs_shortcuts: Vec<ShortcutOwned> =
        egs_manifests.iter().map(manifest_to_shortcut).collect();
    println!("Found {} installed EGS Games", egs_manifests.len());

    let userinfo_shortcuts = get_shortcuts_paths()?;
    println!("Found {} user(s)", userinfo_shortcuts.len());

    for user in userinfo_shortcuts.iter() {
        let shortcut_info = get_shortcuts_for_user(user);
        let new_user_shortcuts: Vec<&ShortcutOwned> = shortcut_info
            .shortcuts
            .iter()
            .filter(|user_shortcut| !user_shortcut.tags.contains(&"EGS".to_owned()))
            .chain(egs_shortcuts.iter())
            .collect();

        let shortcuts = new_user_shortcuts.iter().map(|f| f.borrow()).collect();

        let new_content = shortcuts_to_bytes(&shortcuts);
        let mut file = File::create(shortcut_info.path).unwrap();
        file.write(new_content.as_slice()).unwrap();

        let known_images = get_users_images(user).unwrap();
        // let mut hash_map = HashMap::new();

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
            let search = search.search(s.app_id,s.app_name).await?;
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

pub enum ImageType {
    Hero,
    Grid,
    Logo,
}

impl ImageType {
    pub fn file_name(&self, app_id: u32) -> String {
        match self {
            ImageType::Hero => format!("{}_hero.png", app_id),
            ImageType::Grid => format!("{}p.png", app_id),
            ImageType::Logo => format!("{}_logo.png", app_id),
        }
    }
}


fn get_users_images(user: &SteamUsersInfo) -> Result<Vec<String>, Box<dyn Error>> {
    let grid_folder = Path::new(user.steam_user_data_folder.as_str()).join("config/grid");
    std::fs::create_dir_all(&grid_folder)?;
    let user_folders = std::fs::read_dir(&grid_folder)?;
    let file_names = user_folders
        .filter_map(|image| image.ok())
        .map(|image| image.file_name().into_string().unwrap())
        .collect();
    Ok(file_names)
}

fn manifest_to_shortcut(manifest: &ManifestItem) -> ShortcutOwned {
    let exe = format!(
        "\"{}\\{}\"",
        manifest.install_location, manifest.launch_executable
    );
    let mut start_dir = manifest.install_location.clone();
    if !manifest.install_location.starts_with("\"") {
        start_dir = format!("\"{}\"", manifest.install_location);
    }
    let shortcut = Shortcut::new(
        0,
        manifest.display_name.as_str(),
        exe.as_str(),
        start_dir.as_str(),
        "",
        "",
        "",
    );
    let mut owned_shortcut = shortcut.to_owned();
    owned_shortcut.tags.push("EGS".to_owned());
    owned_shortcut.tags.push("Ready TO Play".to_owned());
    owned_shortcut.tags.push("Installed".to_owned());

    owned_shortcut
}

#[derive(Debug)]
struct SteamFolderNotFound {
    location_tried: String,
}

impl fmt::Display for SteamFolderNotFound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Could not find steam user data at location: {}  Please specify it in the configuration",
            self.location_tried
        )
    }
}

impl Error for SteamFolderNotFound {
    fn description(&self) -> &str {
        self.location_tried.as_str()
    }
}

#[derive(Debug)]
struct SteamUsersDataEmpty {
    location_tried: String,
}

impl fmt::Display for SteamUsersDataEmpty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Steam users data folder is empty: {}  Please specify it in the configuration",
            self.location_tried
        )
    }
}

impl Error for SteamUsersDataEmpty {
    fn description(&self) -> &str {
        self.location_tried.as_str()
    }
}

struct SteamUsersInfo {
    pub steam_user_data_folder: String,
    pub shortcut_path: Option<String>,
}

/// Get the paths to the steam users shortcuts (one for each user)
fn get_shortcuts_paths() -> Result<Vec<SteamUsersInfo>, Box<dyn Error>> {
    let key = "PROGRAMFILES(X86)";
    let program_files = env::var(key)?;
    let path_string = format!(
        "{program_files}//Steam//userdata//",
        program_files = program_files
    );
    let user_data_path = Path::new(path_string.as_str());
    if !user_data_path.exists() {
        return Result::Err(Box::new(SteamFolderNotFound {
            location_tried: path_string,
        }));
    }
    let user_folders = std::fs::read_dir(&user_data_path)?;
    let users_info = user_folders
        .filter_map(|f| f.ok())
        .map(|folder| {
            let folder_path = folder.path();
            let folder_str = folder_path
                .to_str()
                .expect("We just checked that this was there");
            let path = format!("{}//config//shortcuts.vdf", folder_str);
            let shortcuts_path = Path::new(path.as_str());
            let mut shortcuts_path_op = None;
            if shortcuts_path.exists() {
                shortcuts_path_op = Some(shortcuts_path.to_str().unwrap().to_string());
            }
            SteamUsersInfo {
                steam_user_data_folder: folder_str.to_string(),
                shortcut_path: shortcuts_path_op,
            }
        })
        .collect();
    Ok(users_info)
}
