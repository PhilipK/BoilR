use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct ManifestItem {
    #[serde(alias = "LaunchExecutable")]
    pub launch_executable: String,

    #[serde(alias = "ManifestLocation")]
    pub manifest_location: String,

    #[serde(alias = "DisplayName")]
    pub display_name: String,

    #[serde(alias = "InstallLocation")]
    pub install_location: String,

    #[serde(alias = "AppName")]
    pub app_name: String,

    #[serde(alias = "CatalogNamespace")]
    pub catalog_namespace: String,

    #[serde(alias = "CatalogItemId")]
    pub catalog_item_id: String,

    #[serde(alias = "bIsManaged")]
    pub is_managed: bool,

    #[serde(alias = "ExpectingDLCInstalled")]
    pub expected_dlc: Option<HashMap<String, bool>>,

    #[serde(default)]
    pub safe_launch: bool,

    //This is not acutally in the manifest, but it will get added by get_manifests.rs
    pub launcher_path: Option<PathBuf>,

    //This is not acutally in the manifest, but it will get added by get_manifests.rs if on linux
    pub compat_folder: Option<PathBuf>,
}

fn exe_shortcut(manifest: ManifestItem) -> ShortcutOwned {
    let exe = manifest.exe();
    let start_dir = manifest.install_location.clone();

    let exe = exe.trim_matches('\"');
    let start_dir = start_dir.trim_matches('\"');

    #[cfg(target_family = "unix")]
    let start_dir_string = format!("\"{start_dir}\"");
    #[cfg(target_family = "unix")]
    let start_dir = start_dir_string.as_str();

    #[cfg(target_family = "unix")]
    let exe_string = format!("\"{exe}\"");
    #[cfg(target_family = "unix")]
    let exe = exe_string.as_str();

    let parameters = match manifest.compat_folder.as_ref() {
        Some(compat_folder) => format!(
            "STEAM_COMPAT_DATA_PATH=\"{}\" %command%",
            compat_folder.to_string_lossy(),
        ),
        None => String::default(),
    };

    Shortcut::new(
        "0",
        manifest.display_name.as_str(),
        exe,
        start_dir,
        exe,
        "",
        parameters.as_str(),
    )
    .to_owned()
}

fn launcher_shortcut(manifest: ManifestItem) -> ShortcutOwned {
    let icon = manifest.exe();
    let url = match manifest.compat_folder.as_ref() {
        Some(compat_folder) => format!(
            "STEAM_COMPAT_DATA_PATH=\"{}\" %command% -'{}'",
            compat_folder.to_string_lossy(),
            manifest.get_launch_url()
        ),
        None => manifest.get_launch_url(),
    };

    let parent_folder = manifest
        .launcher_path
        .as_ref()
        .map(|p| {
            p.parent()
                .unwrap_or_else(|| Path::new(""))
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_default();

    #[cfg(target_family = "unix")]
    let parent_folder = format!("\"{parent_folder}\"");

    let launcher_path = manifest
        .launcher_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    #[cfg(target_family = "unix")]
    let launcher_path = format!("\"{launcher_path}\"");

    Shortcut::new(
        "0",
        manifest.display_name.as_str(),
        launcher_path.as_str(),
        parent_folder.as_str(),
        icon.as_str(),
        "",
        url.as_str(),
    )
    .to_owned()
}

impl From<ManifestItem> for ShortcutOwned {
    fn from(manifest: ManifestItem) -> Self {
        let mut owned_shortcut = if manifest.needs_launcher() {
            launcher_shortcut(manifest)
        } else {
            exe_shortcut(manifest)
        };
        owned_shortcut.tags.push("EGS".to_owned());
        owned_shortcut.tags.push("Ready TO Play".to_owned());
        owned_shortcut.tags.push("Installed".to_owned());
        owned_shortcut
    }
}

impl ManifestItem {
    fn exe(&self) -> String {
        let manifest = self;
        let exe_path = Path::new(&manifest.install_location)
            .join(&manifest.launch_executable)
            .to_string_lossy()
            .to_string();
        let exe = format!("\"{exe_path}\"");
        exe
    }

    fn get_launch_url(&self) -> String {
        format!(
            "com.epicgames.launcher://apps/{}%3A{}%3A{}?action=launch&silent=true",
            self.catalog_namespace, self.catalog_item_id, self.app_name
        )
    }

    pub fn get_key(&self) -> String {
        format!(
            "{}-{}-{}",
            self.catalog_namespace, self.catalog_item_id, self.app_name
        )
    }

    pub fn dedupe_key(&self) -> String {
        format!(
            "{}-{}-{}",
            self.install_location, self.launch_executable, self.is_managed
        )
    }

    fn needs_launcher(&self) -> bool {
        if self.safe_launch {
            return true;
        }
        match (&self.is_managed, &self.expected_dlc) {
            (true, _) => true,
            (false, Some(map)) => !map.is_empty(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {

    //Okay to unwrap in tests
    #![allow(clippy::unwrap_in_result)]
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn can_parse_item() {
        let json = include_str!("example_item.json");
        let _: ManifestItem = serde_json::from_str(json).unwrap();
    }

    #[test]
    fn generates_launch_string() {
        let json = include_str!("example_item.json");

        let manifest: ManifestItem = serde_json::from_str(json).unwrap();

        let expected ="com.epicgames.launcher://apps/2a09fb19b47f46dfb11ebd382f132a8f%3A88f4bb0bb06e4962a2042d5e20fb6ace%3A63a665088eb1480298f1e57943b225d8?action=launch&silent=true";
        let actual = manifest.get_launch_url();
        assert_eq!(expected, actual);
    }

    #[test]
    fn generates_shortcut_managed() {
        let json = include_str!("example_item.json");
        let mut manifest: ManifestItem = serde_json::from_str(json).unwrap();
        manifest.is_managed = true;
        let shortcut: ShortcutOwned = manifest.clone().into();

        assert_eq!(shortcut.launch_options, manifest.get_launch_url());
    }
    #[test]
    fn generates_shortcut_not_managed() {
        let json = include_str!("example_item.json");
        let mut manifest: ManifestItem = serde_json::from_str(json).unwrap();
        manifest.is_managed = false;
        manifest.expected_dlc = None;
        let shortcut: ShortcutOwned = manifest.into();

        #[cfg(target_os = "windows")]
        assert_eq!(shortcut.exe, "C:\\Games\\MarvelGOTG\\retail/gotg.exe");
        #[cfg(target_family = "unix")]
        assert_eq!(shortcut.exe, "\"C:\\Games\\MarvelGOTG/retail/gotg.exe\"");

        assert_eq!(shortcut.launch_options, "");
    }

    #[test]
    fn generates_shortcut_with_dlc() {
        let json = include_str!("example_item.json");
        let mut manifest: ManifestItem = serde_json::from_str(json).unwrap();
        manifest.is_managed = false;
        let shortcut: ShortcutOwned = manifest.into();

        let expected ="com.epicgames.launcher://apps/2a09fb19b47f46dfb11ebd382f132a8f%3A88f4bb0bb06e4962a2042d5e20fb6ace%3A63a665088eb1480298f1e57943b225d8?action=launch&silent=true";
        let actual = shortcut.launch_options;
        assert_eq!(expected, actual);
    }
}
