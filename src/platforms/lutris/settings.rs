use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LutrisSettings {
    pub enabled: bool,
    pub executable: String,
    pub flatpak: bool,
    pub flatpak_image: String,
    pub installed: bool,
}

impl Default for LutrisSettings {
    fn default() -> Self {
        #[cfg(target_family = "unix")]
        let enabled = true;

        #[cfg(not(target_family = "unix"))]
        let enabled = false;

        Self {
            enabled,
            executable: "lutris".to_string(),
            flatpak: true,
            flatpak_image: "net.lutris.Lutris".to_string(),
            installed: true,
        }
    }
}
