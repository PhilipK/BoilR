use super::settings::EpicGamesLauncherSettings;
use super::ManifestItem;

use std::fs::{DirEntry, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};

pub(crate) fn get_egs_manifests(
    settings: &EpicGamesLauncherSettings,
) -> eyre::Result<Vec<ManifestItem>> {
    let locations = super::get_locations();
    match locations {
        Some(locations) => {
            let manifest_dir_path = locations.manifest_folder_path;
            let manifest_dir_result = std::fs::read_dir(&manifest_dir_path);

            match manifest_dir_result {
                Ok(manifest_dir) => {
                    let mut manifests: Vec<ManifestItem> = manifest_dir
                        .filter_map(|dir| dir.ok())
                        .filter_map(|dir| {
                            get_manifest_item(dir, locations.compat_folder_path.clone())
                        })
                        .filter(is_game_installed)
                        .filter(is_game_launchable)
                        .collect();

                    manifests.sort_by_key(|m| m.dedupe_key());
                    manifests.dedup_by_key(|m| m.dedupe_key());
                    for  manifest in &mut manifests {
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
                Err(err) => Err(eyre::format_err!(
                    "Could not read dir at: {:?} error: {:?}",
                    manifest_dir_path,
                    err
                )),
            }
        }
        None => Err(eyre::format_err!("Manifests not found")),
    }
}

fn is_game_installed(manifest: &ManifestItem) -> bool {
    Path::new(manifest.manifest_location.as_str()).exists()
}

fn is_game_launchable(manifest: &ManifestItem) -> bool {
    (!manifest.launch_executable.is_empty()) || (manifest.is_managed)
}

fn get_manifest_item(dir_entry: DirEntry, _path: Option<PathBuf>) -> Option<ManifestItem> {
    if let Some(extension) = dir_entry.path().extension() {
        if extension.eq("item") {
            if let Ok(file) = File::open(dir_entry.path()) {
                let reader = BufReader::new(file);

                #[cfg(target_family = "unix")]
                {
                    if let Ok(mut item) = serde_json::from_reader::<_, ManifestItem>(reader) {
                        if let Some(compat_folder) = _path {
                            item.manifest_location =
                                replace_with_dosdevices(&compat_folder, &item.manifest_location);
                            item.install_location =
                                replace_with_dosdevices(&compat_folder, &item.install_location);
                            return Some(item);
                        }
                    }
                }

                #[cfg(not(target_family = "unix"))]
                return serde_json::from_reader::<_, ManifestItem>(reader).ok();
            }
        }
    }
    None
}

#[cfg(target_family = "unix")]
fn replace_with_dosdevices(compat_folder: &Path, location: &str) -> String {
    let drive = location.get(0..2).map(|drive| drive.to_lowercase());
    let rest_path = location.get(3..).map(|rest| rest.replace('\\', "/"));
    if let (Some(drive), Some(rest_path)) = (drive, rest_path) {
        let path_buf = compat_folder
            .join("pfx")
            .join("dosdevices")
            .join(drive)
            .join(rest_path);
        path_buf.to_string_lossy().to_string()
    } else {
        location.to_string()
    }
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
