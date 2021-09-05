use std::{
    env::{self},
    fmt,
    path::Path,
};
mod egs;
use egs::get_egs_manifests;
use std::error::Error;
use steam_shortcuts_util::parse_shortcuts;

fn main() -> Result<(), Box<dyn Error>> {
    let egs_manifests = get_egs_manifests()?;
    println!("Found {} installed EGS Games", egs_manifests.len());

    let shortcut_content = get_shortcuts_content()?;
    let shortcuts = parse_shortcuts(shortcut_content.as_slice())?;

    println!("Shortcuts found {}", shortcuts.len());
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

fn get_shortcuts_content() -> Result<Vec<u8>, Box<dyn Error>> {
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
    let mut user_folders = std::fs::read_dir(&user_data_path)?;
    if let Some(Ok(folder)) = user_folders.next() {
        let path = folder.path();
        let shortcuts_folder = format!(
            "{}//config//shortcuts.vdf",
            path.to_str().expect("We just checked that this was there")
        );
        let content = std::fs::read(shortcuts_folder)?;
        Ok(content)
    } else {
        Err(Box::new(SteamUsersDataEmpty {
            location_tried: path_string,
        }))
    }
}
