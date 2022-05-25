use super::{EpicGamesLauncherSettings, ManifestItem};
#[cfg(target_os = "windows")]
use std::env::{self};

use std::fs::{DirEntry, File};
use std::io::BufReader;
use std::path::Path;

use failure::*;

#[derive(Debug, Fail)]
pub enum EpicGamesManifestsError {
    #[fail(display = "EpicGamesLauncher not found")]
    NotFound,

    #[fail(
        display = "Could not read EpicGamesLauncher manifest directory at {} error: {}",
        path, error
    )]
    ReadDirError { path: String, error: std::io::Error },
}

pub(crate) fn get_egs_manifests(
    settings: &EpicGamesLauncherSettings,
) -> Result<Vec<ManifestItem>, EpicGamesManifestsError> {
    let locations = crate::egs::get_locations();
    match locations {
        Some(locations) => {
            let manifest_dir_path = locations.manifest_folder_path;
            let manifest_dir_result = std::fs::read_dir(&manifest_dir_path);

            match manifest_dir_result {
                Ok(manifest_dir) => {
                    let all_manifests = manifest_dir
                        .filter_map(|dir| dir.ok())
                        .filter_map(get_manifest_item);
                    let mut manifests = vec![];
                    for mut manifest in all_manifests {
                        #[cfg(target_family = "unix")]
                        {
                            if let Some(compat_folder) = locations.compat_folder_path.as_ref() {
                                //Strip off the c:\\
                                manifest.manifest_location = compat_folder
                                    .join("pfx")
                                    .join("drive_c")
                                    .join(&manifest.manifest_location[3..].replace("\\", "/"))
                                    .to_path_buf()
                                    .to_string_lossy()
                                    .to_string();

                                manifest.install_location = compat_folder
                                    .join("pfx")
                                    .join("drive_c")
                                    .join(&manifest.install_location[3..].replace("\\", "/"))
                                    .to_path_buf()
                                    .to_string_lossy()
                                    .to_string();
                                dbg!(&manifest.manifest_location);
                            }
                        }
                        if is_game_installed(&manifest) && is_game_launchable(&manifest) {
                            manifests.push(manifest);
                        }
                    }

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
                        manifest.launcher_path = Some(locations.launcher_path.clone());
                        manifest.compat_folder = locations.compat_folder_path.clone();
                        if settings.safe_launch.contains(&manifest.display_name)
                            || settings.safe_launch.contains(&manifest.get_key())
                        {
                            manifest.safe_launch = true;
                        }
                    }
                    Ok(manifests)
                }
                Err(err) => Err(EpicGamesManifestsError::ReadDirError {
                    error: err,
                    path: manifest_dir_path.to_string_lossy().to_string(),
                }),
            }
        }
        None => Err(EpicGamesManifestsError::NotFound),
    }
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
