use std::path::PathBuf;

#[derive(Default, Clone, Debug)]
pub struct EpicPaths {
    pub(crate) launcher_path: PathBuf,
    pub(crate) compat_folder_path: Option<PathBuf>,
    pub(crate) manifest_folder_path: PathBuf,
}

pub fn get_locations() -> Option<EpicPaths> {
    #[cfg(target_family = "unix")]
    {
        unix::get_locations()
    }
    #[cfg(target_os = "windows")]
    {
        windows::get_locations()
    }
}

#[cfg(target_family = "unix")]
mod unix {
    use super::EpicPaths;
    use std::path::Path;

    pub fn get_locations() -> Option<EpicPaths> {
        if let Ok(home) = std::env::var("HOME") {
            let compat_folder_path = Path::new(&home)
                .join(".steam")
                .join("steam")
                .join("steamapps")
                .join("compatdata");

            if let Ok(compat_folder) = std::fs::read_dir(compat_folder_path) {
                for dir in compat_folder.flatten() {
                    let binary_path = dir
                        .path()
                        .join("pfx")
                        .join("drive_c")
                        .join("Program Files (x86)")
                        .join("Epic Games")
                        .join("Launcher")
                        .join("Portal")
                        .join("Binaries");
                    if binary_path.exists() {
                        let launcher_path = if binary_path
                            .join("Win32")
                            .join("EpicGamesLauncher.exe")
                            .exists()
                        {
                            binary_path.join("Win32").join("EpicGamesLauncher.exe")
                        } else {
                            binary_path.join("Win64").join("EpicGamesLauncher.exe")
                        };
                        if launcher_path.exists() {
                            //We found a launcher, lets find the manifests

                            let manifest_folder_path = dir
                                .path()
                                .join("pfx")
                                .join("drive_c")
                                .join("ProgramData")
                                .join("Epic")
                                .join("EpicGamesLauncher")
                                .join("Data")
                                .join("Manifests");
                            if manifest_folder_path.exists() {
                                //We found all we need
                                return Some(EpicPaths {
                                    launcher_path,
                                    compat_folder_path: Some(dir.path()),
                                    manifest_folder_path,
                                });
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::EpicPaths;
    use std::{
        env,
        path::{Path, PathBuf},
    };

    fn manifest_location_from_registry() -> Option<PathBuf> {
        use winreg::enums::*;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(launcher) =
            hklm.open_subkey("SOFTWARE\\WOW6432Node\\Epic Games\\EpicGamesLauncher")
        {
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

    fn guess_default_launcher_location() -> PathBuf {
        let key = "SYSTEMDRIVE";
        let system_drive = env::var(key).unwrap_or_else(|_| String::from("c:"));
        Path::new(format!("{}\\", system_drive).as_str())
            .join("Program Files (x86)")
            .join("Epic Games")
            .join("Launcher")
            .join("Portal")
            .join("Binaries")
            .join("Win64")
            .join("EpicGamesLauncher.exe")
    }

    fn launcher_location_from_registry() -> Option<PathBuf> {
        use winreg::enums::*;
        use winreg::RegKey;

        RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey("SOFTWARE\\Classes\\com.epicgames.launcher\\shell\\open\\command")
            .ok()
            .and_then(|launcher| launcher.get_value("").ok())
            .and_then(|value: String| {
                value
                    .get(1..value.len() - 4)
                    .map(|path| Path::new(path).to_path_buf())
            })
            .filter(|path| path.exists())
    }

    fn guess_default_manifest_location() -> PathBuf {
        let key = "SYSTEMDRIVE";
        let system_drive = env::var(key).unwrap_or_else(|_| String::from("c:"));
        Path::new(format!("{}\\", system_drive).as_str())
            .join("ProgramData")
            .join("Epic")
            .join("EpicGamesLauncher")
            .join("Data")
            .join("Manifests")
    }

    pub fn get_locations() -> Option<EpicPaths> {
        {
            let manifest_folder_path =
                manifest_location_from_registry().unwrap_or_else(guess_default_manifest_location);
            let launcer_path =
                launcher_location_from_registry().unwrap_or_else(guess_default_launcher_location);
            if launcer_path.exists() && manifest_folder_path.exists() {
                Some(EpicPaths {
                    compat_folder_path: None,
                    manifest_folder_path,
                    launcher_path: launcer_path,
                })
            } else {
                None
            }
        }
    }
}
