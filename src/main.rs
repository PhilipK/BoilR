use std::{env::{self}, fmt, fs::File, io::Write, path::Path};
mod egs;
use egs::{get_egs_manifests, ManifestItem};
use std::error::Error;
use steam_shortcuts_util::{Shortcut, parse_shortcuts, shortcuts_to_bytes};

fn main() -> Result<(), Box<dyn Error>> {
    let egs_manifests = get_egs_manifests()?;
    println!("Found {} installed EGS Games", egs_manifests.len());

    let userinfo_shortcuts = get_shortcuts_paths()?;
    println!("Found {} user(s)", userinfo_shortcuts.len());

    userinfo_shortcuts.iter().for_each(|user| {
        let mut shortcuts = vec![];
        let mut new_path = user.shortcut_path.clone();
        if let Some(shortcut_path) = &user.shortcut_path {
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
            new_path = Some(format!("{}/{}", user.steam_user_data_folder, "config/shortcuts.vdf"));
        }
        let cur_number_of_shortcuts = shortcuts.len();
        egs_manifests.iter().enumerate().for_each(|(i, manifest)| {
            let exe = format!(
                "{}/{}",
                manifest.install_location, manifest.launch_executable
            );
            let shortcut = Shortcut::new(
                cur_number_of_shortcuts + i,
                manifest.display_name.as_str(),
                exe.as_str(),
                manifest.install_location.as_str(),
                exe.as_str(),
                exe.as_str(),
                "",
            );
            shortcuts.push(shortcut.to_owned());
        });

        let shortcut_refs = shortcuts.iter().map(|f| f.borrow()).collect();
        let new_content = shortcuts_to_bytes(&shortcut_refs);
        
        let mut file = File::create(new_path.unwrap()).unwrap();
        file.write(new_content.as_slice()).unwrap();
    });

    Ok(())
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
