use std::path::Path;

use serde::{Deserialize, Serialize};
use steam_shortcuts_util::{Shortcut, shortcut::ShortcutOwned};

#[derive(Serialize, Deserialize, Debug, Clone)]
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

    #[serde(alias = "LaunchCommand")]
    pub launch_command: String,
}



impl From<ManifestItem> for ShortcutOwned {
    fn from(manifest: ManifestItem) -> Self {
        
        let exe_path = Path::new(&manifest.install_location).join(manifest.launch_executable).to_string_lossy().to_string();
        let exe = format!("\"{}\"",exe_path);                    
        let mut start_dir = manifest.install_location.clone();
        if !manifest.install_location.starts_with('"') {
            start_dir = format!("\"{}\"", manifest.install_location);
        }
        let shortcut = Shortcut::new(
            0,
            manifest.display_name.as_str(),
            exe.as_str(),
            start_dir.as_str(),
            "",
            "",
            manifest.launch_command.as_str(),
        );
        let mut owned_shortcut = shortcut.to_owned();
        owned_shortcut.tags.push("EGS".to_owned());
        owned_shortcut.tags.push("Ready TO Play".to_owned());
        owned_shortcut.tags.push("Installed".to_owned());

        owned_shortcut
    }
}
