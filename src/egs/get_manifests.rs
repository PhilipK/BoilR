use super::{EpicGamesLauncherSettings, ManifestItem};
#[cfg(target_os = "windows")]
use std::env::{self};

use std::fs::{DirEntry, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};

use failure::*;

#[derive(Debug, Fail)]
pub enum EpicGamesManifestsError {
    #[fail(display = "Path to EpicGamesLauncher not defined, it must be defined on linux")]
    PathNotDefined,

    #[fail(
        display = "EpicGamesLauncher path: {} could not be found. Try to specify a different path for the EpicGamesLauncher.",
        path
    )]
    PathNotFound { path: String },

    #[fail(
        display = "Could not read EpicGamesLauncher manifest directory at {} error: {}",
        path, error
    )]
    ReadDirError { path: String, error: std::io::Error },
}

pub(crate) fn get_egs_manifests(
    settings: &EpicGamesLauncherSettings,
) -> Result<Vec<ManifestItem>, EpicGamesManifestsError> {
    use EpicGamesManifestsError::*;

    let manifest_dir_path = get_manifest_dir_path(settings)?;
    let manifest_dir_result = std::fs::read_dir(&manifest_dir_path);

    #[cfg(target_os = "windows")]
    let launcher_path = launcher_location_from_registry().unwrap_or_else(guess_default_launcher_location);
    match manifest_dir_result {
        Ok(manifest_dir) => {
            let manifests = manifest_dir
                .filter_map(|dir| dir.ok())
                .filter_map(get_manifest_item)
                .filter(is_game_installed)
                .filter(is_game_launchable);
            let mut manifests: Vec<ManifestItem> = manifests.collect();
            manifests.sort_by_key(|m| {
                format!(
                    "{}-{}-{}",
                    m.install_location, m.launch_executable, m.is_managed
                )
            });
            manifests.dedup_by_key(|m| {
                format!(
                    "{}-{}-{}",
                    m.install_location, m.launch_executable, m.is_managed
                )
            });
            for mut manifest in &mut manifests {
                if settings.safe_launch.contains(&manifest.display_name)
                    || settings.safe_launch.contains(&manifest.get_key())
                {
                    manifest.safe_launch = true;
                    #[cfg(target_os = "windows")]
                    {
                        manifest.launcher_path = Some(launcher_path.clone());
                    }
                }
            }
            Ok(manifests)
        }
        Err(err) => Err(ReadDirError {
            error: err,
            path: manifest_dir_path,
        }),
    }
}

fn get_manifest_dir_path(
    settings: &EpicGamesLauncherSettings,
) -> Result<String, EpicGamesManifestsError> {
    use EpicGamesManifestsError::*;
    if let Some(location) = &settings.location {
        let path = Path::new(location);
        if path.exists() {
            return Ok(path.to_str().unwrap().to_string());
        } else {
            return Err(PathNotFound {
                path: path.to_str().unwrap().to_string(),
            });
        }
    } else {
        let path = get_default_manifests_location();

        match path {
            Some(path) => Ok(path.to_str().unwrap().to_string()),
            None => Err(PathNotDefined),
        }
    }
}

pub fn get_default_manifests_location() -> Option<PathBuf> {
    #[cfg(target_family = "unix")]
    {
        //No path defined for epic gamestore, and we cannot guess on linux
        None
    }

    #[cfg(target_os = "windows")]
    {
        let path = match manifest_location_from_registry() {
            Some(path) => path,
            None => guess_default_manifest_location(),
        };
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }
}

#[cfg(target_os = "windows")]
fn manifest_location_from_registry() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(launcher) = hklm.open_subkey("SOFTWARE\\WOW6432Node\\Epic Games\\EpicGamesLauncher") {
        let path_string: Result<String, _> = launcher.get_value("AppDataPath");
        if let Ok(path_string) = path_string {
            let path = Path::new(&path_string).join("Manifests");
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn guess_default_manifest_location() -> PathBuf {
    let key = "SYSTEMDRIVE";
    let system_drive =
        env::var(key).expect("We are on windows, we must know what the SYSTEMDRIVE is");

    let path = Path::new(format!("{}\\", system_drive).as_str())
        .join("ProgramData")
        .join("Epic")
        .join("EpicGamesLauncher")
        .join("Data")
        .join("Manifests");
    path
}

#[cfg(target_os = "windows")]
fn launcher_location_from_registry() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    
    if let Ok(launcher) = hklm.open_subkey("SOFTWARE\\Classes\\com.epicgames.launcher\\shell\\open\\command") {
        let launch_string: Result<String, _> = launcher.get_value("");
        if let Ok(launch_string) = launch_string {
            let path = Path::new(&launch_string[1..launch_string.len()-4]);
            if path.exists() {
                return Some(path.to_path_buf());
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn guess_default_launcher_location() -> PathBuf {
    let key = "SYSTEMDRIVE";
    let system_drive =
        env::var(key).expect("We are on windows, we must know what the SYSTEMDRIVE is");

    let path = Path::new(format!("{}\\", system_drive).as_str())
        .join("Program Files (x86)")
        .join("Epic Games")
        .join("Launcher")
        .join("Portal")
        .join("Binaries")
        .join("Win64")
        .join("EpicGamesLauncher.exe");
    path
}

fn is_game_installed(manifest: &ManifestItem) -> bool {
    Path::new(manifest.manifest_location.as_str()).exists()
}

fn is_game_launchable(manifest: &ManifestItem) -> bool {
    (!manifest.launch_executable.is_empty()) || (manifest.is_managed)
}

fn get_manifest_item(dir_entry: DirEntry) -> Option<ManifestItem> {
    if let Some(extension) = dir_entry.path().extension() {
        if extension.eq("item") {
            if let Ok(file) = File::open(dir_entry.path()) {
                let reader = BufReader::new(file);
                return serde_json::from_reader(reader).ok();
            }
        }
    }
    None
}

//Commented out because it will change from machine to machine
// #[cfg(test)]
// pub mod test{
//     use super::guess_default_launcher_location;
//     use super::launcher_location_from_registry;

//     #[test]
//     pub fn test_launcher_registry(){
//         let launcher = launcher_location_from_registry();
//         assert_eq!(Some(std::path::Path::new("C:\\Program Files (x86)\\Epic Games\\Launcher\\Portal\\Binaries\\Win64\\EpicGamesLauncher.exe").to_path_buf()),launcher);
//     }


//     #[test]
//     pub fn test_launcher_guess(){
//         let launcher = guess_default_launcher_location();
//         assert_eq!(std::path::Path::new("C:\\Program Files (x86)\\Epic Games\\Launcher\\Portal\\Binaries\\Win64\\EpicGamesLauncher.exe").to_path_buf(),launcher);
//     }
// }