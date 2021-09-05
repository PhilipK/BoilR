
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManifestItem {
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
}
