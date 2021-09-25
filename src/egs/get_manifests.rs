use super::{EpicGamesLauncherSettings, ManifestItem};
#[cfg(target_os = "windows")]
use std::env::{self};

use std::fs::{DirEntry, File};
use std::io::BufReader;
use std::path::Path;

use failure::*;

#[derive(Debug, Fail)]
pub enum EpicGamesManifestsError {
    #[cfg(target_os = "linux")]
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

pub fn get_egs_manifests(
    settings: &EpicGamesLauncherSettings,
) -> Result<Vec<ManifestItem>, EpicGamesManifestsError> {
    use EpicGamesManifestsError::*;

    let manifest_dir_path = get_manifest_dir_path(settings)?;
    let manifest_dir_result = std::fs::read_dir(&manifest_dir_path);

    match manifest_dir_result {
        Ok(manifest_dir) => {
            let manifests = manifest_dir
                .filter_map(|dir| dir.ok())
                .filter_map(get_manifest_item)
                .filter(is_game_installed);
            Ok(manifests.collect())
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
        let path = Path::new(location).join("Data").join("Manifests");
        if path.exists() {
            return Ok(path.to_str().unwrap().to_string());
        } else {
            return Err(PathNotFound {
                path: path.to_str().unwrap().to_string(),
            });
        }
    } else {
        #[cfg(target_os = "linux")]
        {
            //No path defined for epic gamestore, and we cannot guess on linux
            return Err(PathNotDefined);
        }

        #[cfg(target_os = "windows")]
        {
            let key = "SYSTEMDRIVE";
            let system_drive =
                env::var(key).expect("We are on windows, we must know what the SYSTEMDRIVE is");

            let path = Path::new(system_drive.as_str())
                .join("ProgramData")
                .join("Epic")
                .join("EpicGamesLauncher")
                .join("Data")
                .join("Manifests");
            if path.exists() {
                return Ok(path.to_str().unwrap().to_string());
            } else {
                return Err(PathNotFound {
                    path: path.to_str().unwrap().to_string(),
                });
            }
        }
    }
}

fn is_game_installed(manifest: &ManifestItem) -> bool {
    Path::new(manifest.manifest_location.as_str()).exists()
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
